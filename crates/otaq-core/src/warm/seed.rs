//! Seeding a new worktree at start (WRM-04, SES-01 step 3, SES-02; d0082, d0010, d0023).
//!
//! Only a **new** worktree is seeded (not one a returning ticket keeps, not a folder a re-run start reuses), and only
//! while warm builds are on for the project. The seed is, in order:
//! 1. the project's warm build, under a reader slot named by the ticket, so a Clear waits for the copy (WRM-06);
//! 2. with none, the user's own checkout, if it has gitignored files. Its env files are left out. A checkout on
//!    another disk can't be cloned ([`CloneError::Unsupported`]) and is skipped: never a plain copy (WRM-01);
//! 3. else nothing.
//!
//! The seed's gitignored paths are cloned into the worktree, skipping every path git already checked out there
//! (a folder both have is gone into) and the warm build's record and times file. Then **the timestamp trick**: every
//! tracked file gets the modified time it had where the build was made (the warm build's times file, recorded at
//! Done; or the checkout's own times), and the files that differ from the seed's commit (`git diff --name-only
//! <seed> HEAD`, plus a checkout's own uncommitted changes to tracked files) get the current time, as does a tracked
//! file with no recorded time. Build tools compare source times with output times, so they redo only what differs,
//! and a file committed after the last build still looks newer than its output (Q71). A seed commit git can't
//! resolve, or a warm build without a times file (kept before they were recorded), skips the trick: the files keep
//! their checkout times and the first build redoes more, which is slower but correct.
//!
//! Seeding never fails a start: a failure removes what was cloned, is logged, and the ticket builds from nothing.
//! Nothing here knows any build tool: every gitignored file is treated the same.

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use super::clone::{CloneError, Cloner, remove_tree};
use super::store::{TrackedTimes, WarmBuilds, record_file_name, times_file_name};
use crate::board::Board;
use crate::git::Git;
use crate::store::now_ms;
use crate::store::projects::Project;

/// Seeds new worktrees. Cheap to clone.
#[derive(Clone)]
pub struct Seeder {
    warm: WarmBuilds,
    cloner: Arc<dyn Cloner>,
}

impl fmt::Debug for Seeder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Seeder")
            .field("cloner", &self.cloner)
            .finish_non_exhaustive()
    }
}

/// Where a worktree was seeded from, for the first line of its setup log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeededFrom {
    /// The project's warm build: its "built at" commit, the ticket it was kept from and when, when it has a record.
    WarmBuild {
        built_at: Option<String>,
        from: Option<String>,
        kept_at: Option<i64>,
    },
    /// The user's own checkout at this `HEAD` (`None` before its first commit).
    Checkout { head: Option<String> },
}

/// A seeded worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seeded {
    /// The project's name: `shop-web`.
    pub project: String,
    pub from: SeededFrom,
    /// How long the clone and the timestamp trick took.
    pub took: Duration,
    /// When it was seeded, in milliseconds since the epoch (the warm build's age is measured from it).
    pub at: i64,
}

impl Seeded {
    /// "Started from shop-web's warm build of 3e1f9a2 (SHOP-21, 2 h ago) in 4 s." or "Started from your checkout of
    /// shop-web (9c41e7b).".
    pub fn log_line(&self) -> String {
        let project = &self.project;
        match &self.from {
            SeededFrom::WarmBuild {
                built_at,
                from,
                kept_at,
            } => {
                let of = built_at
                    .as_deref()
                    .map(|sha| format!(" of {}", short_sha(sha)))
                    .unwrap_or_default();
                let origin = match (from, kept_at) {
                    (Some(from), Some(kept_at)) => {
                        format!(" ({from}, {})", age(self.at - kept_at))
                    }
                    (Some(from), None) => format!(" ({from})"),
                    (None, Some(kept_at)) => format!(" ({})", age(self.at - kept_at)),
                    (None, None) => String::new(),
                };
                format!(
                    "Started from {project}'s warm build{of}{origin} in {}.",
                    took(self.took)
                )
            }
            SeededFrom::Checkout { head } => match head {
                Some(head) => format!(
                    "Started from your checkout of {project} ({}).",
                    short_sha(head)
                ),
                None => format!("Started from your checkout of {project}."),
            },
        }
    }
}

impl Seeder {
    /// Seeds with `cloner`: [`super::clone::FsCloner`] in the service; a test stands in for it where its disk can't
    /// clone.
    pub fn new(warm: WarmBuilds, cloner: Arc<dyn Cloner>) -> Self {
        Self { warm, cloner }
    }

    /// Seeds the new `worktree` of `ticket` (`SHOP-18`) in `project`, if warm builds are on and there is a seed.
    /// `None` when nothing was seeded, for whatever reason; a failure is logged, never returned.
    pub async fn seed(
        &self,
        board: &Board,
        git: &Git,
        project: &Project,
        ticket: &str,
        worktree: &Path,
    ) -> Option<Seeded> {
        match self.warm.enabled(&project.key).await {
            Ok(true) => {}
            Ok(false) => return None,
            Err(e) => {
                tracing::warn!(ticket, error = %e, "can't tell whether warm builds are on; not seeding");
                return None;
            }
        }
        let env_files = match board.project_settings(&project.key).await {
            Ok(settings) => settings.env_files,
            Err(e) => {
                tracing::warn!(ticket, error = %e, "can't read the project's settings; not seeding");
                return None;
            }
        };
        let leave_out: Vec<PathBuf> = env_files
            .iter()
            .map(|f| f.trim())
            .filter(|f| !f.is_empty())
            .map(PathBuf::from)
            .collect();
        let started = Instant::now();
        let result = match self.warm.read(project, ticket).await {
            Some(slot) => {
                let record = slot.record().cloned();
                let times = {
                    let path = slot.path().to_owned();
                    tokio::task::spawn_blocking(move || TrackedTimes::read(&path))
                        .await
                        .ok()
                        .flatten()
                };
                let top = match list_top(slot.path()) {
                    Ok(top) => top,
                    Err(e) => {
                        tracing::warn!(ticket, error = %e, "can't read the warm build; not seeding");
                        return None;
                    }
                };
                let plan = self.clone_seed(slot.path(), worktree, top, leave_out).await;
                // The copy is done: a Clear waiting for it may go on.
                drop(slot);
                let plan = plan?;
                let commit = record.as_ref().map(|r| r.built_at.clone());
                self.trick(
                    git,
                    worktree,
                    commit.as_deref(),
                    times,
                    Vec::new(),
                    &plan,
                    ticket,
                )
                .await?;
                SeededFrom::WarmBuild {
                    built_at: commit,
                    from: record.as_ref().map(|r| r.from.clone()),
                    kept_at: record.as_ref().map(|r| r.kept_at),
                }
            }
            None => {
                let repo = PathBuf::from(&project.repo_path);
                let ignored = match ignored_paths(git, &repo).await {
                    Ok(ignored) if ignored.is_empty() => return None,
                    Ok(ignored) => ignored,
                    Err(e) => {
                        tracing::warn!(ticket, error = %e, "can't list the checkout's ignored files; not seeding");
                        return None;
                    }
                };
                // The checkout's commit and its own changes, read before cloning so they match what is cloned.
                let head = git.resolve(&repo, "HEAD").await.ok().flatten();
                let uncommitted = match &head {
                    Some(_) => match git.run(&repo, &["diff", "--name-only", "-z", "HEAD"]).await {
                        Ok(out) => split_z(&out),
                        Err(e) => {
                            tracing::warn!(ticket, error = %e, "can't list the checkout's changes; not seeding");
                            return None;
                        }
                    },
                    None => Vec::new(),
                };
                // Where the checkout's build was made: its tracked files' own times.
                let times = match git.run(&repo, &["ls-files", "-z"]).await {
                    Ok(out) => {
                        let (repo, tracked) = (repo.clone(), split_z(&out));
                        tokio::task::spawn_blocking(move || TrackedTimes::measure(&repo, &tracked))
                            .await
                            .ok()
                    }
                    Err(e) => {
                        tracing::warn!(ticket, error = %e, "can't list the checkout's files; not seeding");
                        return None;
                    }
                };
                let plan = self.clone_seed(&repo, worktree, ignored, leave_out).await?;
                self.trick(
                    git,
                    worktree,
                    head.as_deref(),
                    times,
                    uncommitted,
                    &plan,
                    ticket,
                )
                .await?;
                SeededFrom::Checkout { head }
            }
        };
        let seeded = Seeded {
            project: project.name.clone(),
            from: result,
            took: started.elapsed(),
            at: now_ms(),
        };
        tracing::info!(ticket, line = %seeded.log_line(), "worktree seeded");
        Some(seeded)
    }

    /// Clones the `candidates` of `seed` (relative paths) that git didn't check out into `worktree`, leaving out
    /// `leave_out`. `None` when there was nothing to clone or the clone failed (logged; what it made is removed).
    async fn clone_seed(
        &self,
        seed: &Path,
        worktree: &Path,
        candidates: Vec<PathBuf>,
        leave_out: Vec<PathBuf>,
    ) -> Option<Plan> {
        let cloner = self.cloner.clone();
        let (seed, worktree) = (seed.to_owned(), worktree.to_owned());
        let shown = seed.clone();
        let result = tokio::task::spawn_blocking(move || {
            let plan =
                plan(&seed, &worktree, candidates, &leave_out).map_err(|error| CloneError::Io {
                    path: seed.clone(),
                    error,
                })?;
            if plan.paths.is_empty() {
                return Ok(None);
            }
            match cloner.clone_tree(&seed, &worktree, Some(&plan.paths)) {
                Ok(()) => Ok(Some(plan)),
                Err(e) => {
                    plan.undo(&worktree);
                    Err(e)
                }
            }
        })
        .await;
        match result {
            Ok(Ok(plan)) => plan,
            Ok(Err(CloneError::Unsupported { path })) => {
                tracing::info!(
                    seed = %shown.display(),
                    path = %path.display(),
                    "the seed can't be cloned copy-on-write (another disk); not seeding"
                );
                None
            }
            Ok(Err(e)) => {
                tracing::warn!(seed = %shown.display(), error = %e, "seeding failed; the worktree builds from nothing");
                None
            }
            Err(e) => {
                tracing::warn!(seed = %shown.display(), error = %e, "seeding stopped");
                None
            }
        }
    }

    /// The timestamp trick after a clone, with the `times` the tracked files had where the build was made. `None`
    /// (with the clone removed) when it failed; a seed commit git can't resolve, or no recorded times, skips it.
    #[allow(clippy::too_many_arguments)]
    async fn trick(
        &self,
        git: &Git,
        worktree: &Path,
        commit: Option<&str>,
        times: Option<TrackedTimes>,
        mut changed: Vec<String>,
        plan: &Plan,
        ticket: &str,
    ) -> Option<()> {
        let resolved = match commit {
            Some(commit) => git.resolve(worktree, commit).await.ok().flatten(),
            None => None,
        };
        let Some(commit) = resolved else {
            tracing::info!(
                ticket,
                ?commit,
                "the seed's commit can't be resolved; files keep their checkout times"
            );
            return Some(());
        };
        let Some(times) = times else {
            tracing::info!(
                ticket,
                "the warm build has no recorded times; files keep their checkout times"
            );
            return Some(());
        };
        let lists = async {
            let tracked = split_z(&git.run(worktree, &["ls-files", "-z"]).await?);
            let diff = git
                .run(worktree, &["diff", "--name-only", "-z", &commit, "HEAD"])
                .await?;
            Ok::<_, crate::git::GitError>((tracked, split_z(&diff)))
        }
        .await;
        let result = match lists {
            Ok((tracked, diff)) => {
                changed.extend(diff);
                let root = worktree.to_owned();
                tokio::task::spawn_blocking(move || mark_times(&root, &tracked, &changed, &times))
                    .await
                    .unwrap_or_else(|e| Err(io::Error::other(e)))
                    .map_err(|e| e.to_string())
            }
            Err(e) => Err(e.to_string()),
        };
        match result {
            Ok(()) => Some(()),
            Err(error) => {
                tracing::warn!(ticket, %error, "the timestamp trick failed; the clone is removed");
                let (plan, root) = (plan.clone(), worktree.to_owned());
                let _ = tokio::task::spawn_blocking(move || plan.undo(&root)).await;
                None
            }
        }
    }
}

/// What a seed clones: `paths` (relative), and the folders that had to be made for them because an env file
/// inside was left out (removed with them on a failure).
#[derive(Debug, Clone, Default)]
struct Plan {
    paths: Vec<PathBuf>,
    made: Vec<PathBuf>,
}

impl Plan {
    /// Removes everything this plan put into `worktree`. None of it was there before (git didn't check it out).
    fn undo(&self, worktree: &Path) {
        for path in self.paths.iter().chain(self.made.iter().rev()) {
            remove_tree(&worktree.join(path));
        }
    }
}

/// The paths of `seed` to clone into `worktree`: each candidate that isn't there yet, and inside a folder both
/// have, its entries the same way. `leave_out` (env files) is never cloned; a folder holding one is gone into.
fn plan(
    seed: &Path,
    worktree: &Path,
    candidates: Vec<PathBuf>,
    leave_out: &[PathBuf],
) -> io::Result<Plan> {
    let mut plan = Plan::default();
    let mut todo = candidates;
    todo.reverse();
    while let Some(rel) = todo.pop() {
        if leave_out.contains(&rel) {
            continue;
        }
        let Ok(source) = fs::symlink_metadata(seed.join(&rel)) else {
            continue;
        };
        let holds_left_out = source.is_dir() && leave_out.iter().any(|l| l.starts_with(&rel));
        let into = match fs::symlink_metadata(worktree.join(&rel)) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if !holds_left_out {
                    plan.paths.push(rel);
                    continue;
                }
                plan.made.push(rel.clone());
                fs::create_dir_all(worktree.join(&rel))?;
                rel
            }
            Err(e) => return Err(e),
            Ok(there) if there.is_dir() && source.is_dir() => rel,
            // Git checked it out: the worktree's own file stays.
            Ok(_) => continue,
        };
        let mut children: Vec<PathBuf> = fs::read_dir(seed.join(&into))?
            .map(|entry| entry.map(|e| into.join(e.file_name())))
            .collect::<io::Result<_>>()?;
        children.sort();
        children.reverse();
        todo.extend(children);
    }
    Ok(plan)
}

/// The entries at the top of a warm build, without its record and times file.
fn list_top(folder: &Path) -> io::Result<Vec<PathBuf>> {
    let own = [record_file_name(), times_file_name()];
    let mut top: Vec<PathBuf> = fs::read_dir(folder)?
        .filter_map(|entry| entry.ok().map(|e| e.file_name()))
        .filter(|name| !own.iter().any(|own| name.to_string_lossy() == own.as_str()))
        .map(PathBuf::from)
        .collect();
    top.sort();
    Ok(top)
}

/// The checkout's gitignored paths, a wholly ignored folder as one path.
async fn ignored_paths(git: &Git, repo: &Path) -> Result<Vec<PathBuf>, crate::git::GitError> {
    let out = git
        .run(
            repo,
            &[
                "ls-files",
                "-z",
                "--others",
                "--ignored",
                "--exclude-standard",
                "--directory",
            ],
        )
        .await?;
    Ok(split_z(&out)
        .into_iter()
        .map(|p| PathBuf::from(p.trim_end_matches('/')))
        .filter(|p| !p.as_os_str().is_empty())
        .collect())
}

fn split_z(out: &str) -> Vec<String> {
    out.split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Every tracked file gets its time in `times`, except the changed ones and those without a recorded time, which get
/// the current time. A file that isn't there is passed over.
fn mark_times(
    root: &Path,
    tracked: &[String],
    changed: &[String],
    times: &TrackedTimes,
) -> io::Result<()> {
    let now = SystemTime::now();
    let changed: HashSet<&str> = changed.iter().map(String::as_str).collect();
    for file in tracked {
        let time = match times.get(file) {
            Some(time) if !changed.contains(file.as_str()) => time,
            _ => now,
        };
        match set_time(&root.join(file), time) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            other => other?,
        }
    }
    Ok(())
}

/// Sets the access and modified times of `path` itself (a symlink is not followed).
#[cfg(unix)]
#[allow(clippy::unnecessary_cast)]
fn set_time(path: &Path, time: SystemTime) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    let since = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    // SAFETY: `timespec` is plain data; every field the call reads is set below.
    let mut times: [libc::timespec; 2] = unsafe { std::mem::zeroed() };
    for t in &mut times {
        t.tv_sec = since.as_secs() as libc::time_t;
        t.tv_nsec = since.subsec_nanos() as _;
    }
    // SAFETY: `c` is a valid null-terminated path and `times` holds two entries.
    let done = unsafe {
        libc::utimensat(
            libc::AT_FDCWD,
            c.as_ptr(),
            times.as_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if done == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Sets the access and modified times of `path`. Symlinks are passed over: std can't set a link's own times
/// here, and following one would touch a file outside the trick.
#[cfg(windows)]
fn set_time(path: &Path, time: SystemTime) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_WRITE_ATTRIBUTES,
    };
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Ok(());
    }
    let file = fs::OpenOptions::new()
        .access_mode(FILE_WRITE_ATTRIBUTES)
        // Lets a folder (a submodule) be opened too.
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    file.set_times(fs::FileTimes::new().set_accessed(time).set_modified(time))
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// "just now", "12 min ago", "2 h ago", "3 d ago".
fn age(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    match minutes {
        0 => "just now".to_owned(),
        1..60 => format!("{minutes} min ago"),
        60..1440 => format!("{} h ago", minutes / 60),
        _ => format!("{} d ago", minutes / 1440),
    }
}

/// "under 1 s", "4 s", "2 min".
fn took(d: Duration) -> String {
    match d.as_secs() {
        0 => "under 1 s".to_owned(),
        s @ 1..60 => format!("{s} s"),
        s => format!("{} min", s / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded(from: SeededFrom, took: Duration) -> Seeded {
        Seeded {
            project: "shop-web".into(),
            from,
            took,
            at: 10 * 3_600_000,
        }
    }

    #[test]
    fn the_log_line_says_where_the_seed_came_from() {
        let warm = SeededFrom::WarmBuild {
            built_at: Some("3e1f9a2c0ffee".into()),
            from: Some("SHOP-21".into()),
            kept_at: Some(8 * 3_600_000),
        };
        assert_eq!(
            seeded(warm, Duration::from_millis(4_300)).log_line(),
            "Started from shop-web's warm build of 3e1f9a2 (SHOP-21, 2 h ago) in 4 s."
        );
        let bare = SeededFrom::WarmBuild {
            built_at: None,
            from: None,
            kept_at: None,
        };
        assert_eq!(
            seeded(bare, Duration::from_millis(200)).log_line(),
            "Started from shop-web's warm build in under 1 s."
        );
        let checkout = SeededFrom::Checkout {
            head: Some("9c41e7bdeadbeef".into()),
        };
        assert_eq!(
            seeded(checkout, Duration::from_secs(3)).log_line(),
            "Started from your checkout of shop-web (9c41e7b)."
        );
    }

    #[test]
    fn ages_and_durations_read_short() {
        assert_eq!(age(10_000), "just now");
        assert_eq!(age(12 * 60_000), "12 min ago");
        assert_eq!(age(3 * 86_400_000), "3 d ago");
        assert_eq!(took(Duration::from_secs(150)), "2 min");
    }

    #[test]
    fn the_plan_skips_what_git_checked_out_and_the_env_files() {
        let dir = tempfile::tempdir().unwrap();
        let (seed, wt) = (dir.path().join("seed"), dir.path().join("wt"));
        for f in [
            "out/app.bin",
            "src/gen.o",
            "src/main.c",
            "config/.env.local",
            "config/cache",
        ] {
            let path = seed.join(f);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }
        fs::create_dir_all(wt.join("src")).unwrap();
        fs::write(wt.join("src/main.c"), "tracked").unwrap();
        let candidates = ["config", "out", "src"].map(PathBuf::from).to_vec();
        let leave_out = [PathBuf::from("config/.env.local")];
        let plan = plan(&seed, &wt, candidates, &leave_out).unwrap();
        assert_eq!(
            plan.paths,
            ["config/cache", "out", "src/gen.o"]
                .map(PathBuf::from)
                .to_vec()
        );
        assert_eq!(plan.made, vec![PathBuf::from("config")]);
        plan.undo(&wt);
        assert!(!wt.join("config").exists());
        assert!(wt.join("src/main.c").exists());
    }
}
