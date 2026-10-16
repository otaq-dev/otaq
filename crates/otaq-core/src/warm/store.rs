//! The warm build store (WRM-02, WRM-05…WRM-07, d0082): one kept build per project at `<app-data>/warm/<repo-name>`,
//! its record, the lock that lets a Clear wait for a running copy, replace (forward only) and Clear.
//!
//! - **The folder** holds a worktree's gitignored files as laid out there, plus the record file
//!   ([`record_file_name`]): built at, base, from, kept at, and the project it was kept for; and the times file
//!   ([`times_file_name`], [`TrackedTimes`]): the tracked files' modified times in the kept worktree (Q71). Seeding
//!   skips both. They move with the folder on replace and Clear.
//! - **Warm builds on** for a project ([`WarmBuilds::enabled`]): its switch is on and the data folder's disk can clone.
//! - **The lock**, one per warm build: a seed holds a **reader** slot, named by its ticket, while it clones
//!   ([`WarmBuilds::read`]). Replace and Clear take the **writer** slot and wait for the readers; while a writer waits,
//!   no new reader starts, so a Clear never waits for more than the copies already running. [`WarmBuilds::status`]
//!   says who is copying, for "Clearing… (waiting for SHOP-21's copy)".
//! - **Replace** ([`WarmBuilds::replace`]): the caller moves the new files into `<repo-name>.incoming/`
//!   ([`WarmBuilds::incoming`], one keep at a time per warm build), then, under the writer slot, an older build is
//!   dropped and a newer one swapped in through `<repo-name>.old/`. Old folders are deleted in the background.
//! - **Clear** ([`WarmBuilds::clear`]) renames the folder to `<repo-name>.old/` under the writer slot and deletes it in
//!   the background. It never turns the switch off.
//! - At service start, `.incoming` and `.old` folders left by a crash are deleted ([`remove_interrupted`]).
//! - Each change (kept, replaced, cleared, a Clear starting to wait) publishes one [`Event::WarmBuildChanged`].
//!
//! Nothing here knows any build tool: every gitignored file is treated the same.

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex as AsyncMutex, Notify, OwnedMutexGuard, oneshot};

use super::clone::{CloneProbe, CloneSupport, remove_tree};
use crate::board::{Board, BoardError};
use crate::data_dir::{DataDir, DataDirError};
use crate::naming;
use crate::protocol::Event;
use crate::service::EventBus;
use crate::store::projects::Project;

/// The record file inside a warm build: `.otaq-warm.json`. Its leading dot and Otaq's name keep it clear of a
/// project's own files; seeding skips it.
pub fn record_file_name() -> String {
    format!(".{}-warm.json", naming::COMMAND_NAME)
}

/// What a warm build remembers about where it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarmRecord {
    /// The worktree's `HEAD` when it was kept (the seed's commit for the timestamp trick).
    pub built_at: String,
    /// `git merge-base HEAD <base branch>` at that moment, for "forward only".
    pub base: String,
    /// The ticket it was kept from: `SHOP-21`.
    pub from: String,
    /// When it was kept, in milliseconds since the epoch.
    pub kept_at: i64,
    /// The id of the project it was kept for (two projects whose folders share a name share `warm/<repo-name>`).
    #[serde(default)]
    pub project: String,
}

impl WarmRecord {
    /// Reads the record of the warm build at `folder`; `None` when there is none or it can't be read.
    pub fn read(folder: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(folder.join(record_file_name())).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Writes the record into `folder`.
    pub fn write(&self, folder: &Path) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self).expect("the record serializes");
        std::fs::write(folder.join(record_file_name()), json)
    }
}

/// The times file inside a warm build: `.otaq-warm-times`, next to the record. Seeding skips it too.
pub fn times_file_name() -> String {
    format!(".{}-warm-times", naming::COMMAND_NAME)
}

/// The first entry of a times file: its format.
const TIMES_FORMAT: &str = "times 1";

/// The modified times of a worktree's tracked files where a build was made (Q71): recorded at Done into the warm
/// build's times file, or read from the user's checkout. A seeded worktree gives its unchanged tracked files these
/// times, so a file committed after the last build still looks newer than the build's output.
///
/// The file is compact for tens of thousands of files: NUL-separated entries, the first [`TIMES_FORMAT`], then one
/// `<seconds> <nanoseconds> <path>` per file (times since the epoch; paths as `git ls-files -z` gives them).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackedTimes(HashMap<String, std::time::SystemTime>);

impl TrackedTimes {
    /// The modified times of `files` (relative to `root`) on disk. A symlink's own time, not followed; a file that
    /// isn't there is left out.
    pub fn measure(root: &Path, files: &[String]) -> Self {
        Self(
            files
                .iter()
                .filter_map(|file| {
                    let time = std::fs::symlink_metadata(root.join(file))
                        .and_then(|m| m.modified())
                        .ok()?;
                    Some((file.clone(), time))
                })
                .collect(),
        )
    }

    /// The recorded time of `file`.
    pub fn get(&self, file: &str) -> Option<std::time::SystemTime> {
        self.0.get(file).copied()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Reads the times file of the warm build at `folder`; `None` when there is none (a warm build kept before the
    /// times were recorded) or it can't be read.
    pub fn read(folder: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(folder.join(times_file_name())).ok()?;
        let mut entries = text.split('\0');
        if entries.next() != Some(TIMES_FORMAT) {
            return None;
        }
        let mut times = HashMap::new();
        for entry in entries.filter(|e| !e.is_empty()) {
            let mut parts = entry.splitn(3, ' ');
            let (Some(secs), Some(nanos), Some(path)) = (parts.next(), parts.next(), parts.next())
            else {
                return None;
            };
            let since = std::time::Duration::new(secs.parse().ok()?, nanos.parse().ok()?);
            times.insert(path.to_owned(), std::time::SystemTime::UNIX_EPOCH + since);
        }
        Some(Self(times))
    }

    /// Writes the times file into `folder`.
    pub fn write(&self, folder: &Path) -> io::Result<()> {
        use std::io::Write;
        let file = std::fs::File::create(folder.join(times_file_name()))?;
        let mut out = io::BufWriter::new(file);
        out.write_all(TIMES_FORMAT.as_bytes())?;
        for (path, time) in &self.0 {
            // A time before the epoch doesn't happen on a build machine; it reads as the epoch.
            let since = time
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            write!(out, "\0{} {} {path}", since.as_secs(), since.subsec_nanos())?;
        }
        out.flush()
    }
}

/// The folder name a project's worktrees and warm build go under: its repository folder's name, as in `wt/`.
pub fn repo_name(project: &Project) -> String {
    Path::new(&project.repo_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| project.name.clone())
}

/// `<repo-name>.incoming` or `<repo-name>.old`, next to the warm build.
fn sibling(data: &DataDir, repo: &str, suffix: &str) -> Result<PathBuf, DataDirError> {
    Ok(data
        .warm_path(repo)?
        .with_file_name(format!("{repo}.{suffix}")))
}

/// Whether a folder in `warm/` is one of the store's own temporary folders, or the probe's, never a warm build.
fn is_temporary(name: &str) -> bool {
    name.starts_with('.') || name.ends_with(".incoming") || name.ends_with(".old")
}

/// Deletes the `<repo-name>.incoming/` and `<repo-name>.old/` folders a crash left in `warm/` (and a probe's). They
/// are Otaq's own temporary folders, not leftovers to confirm. Called once at service start, before anything uses
/// the store.
pub fn remove_interrupted(data: &DataDir) -> usize {
    let Ok(entries) = std::fs::read_dir(data.warm_dir()) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_temporary(&name) {
            remove_tree(&entry.path());
            removed += 1;
        }
    }
    removed
}

#[derive(Debug)]
pub enum WarmError {
    /// The project's folder name can't be a folder name.
    Path(DataDirError),
    Io {
        path: PathBuf,
        error: io::Error,
    },
    Board(BoardError),
}

impl fmt::Display for WarmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(e) => write!(f, "{e}"),
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
            Self::Board(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for WarmError {}

impl From<DataDirError> for WarmError {
    fn from(e: DataDirError) -> Self {
        Self::Path(e)
    }
}

impl From<BoardError> for WarmError {
    fn from(e: BoardError) -> Self {
        Self::Board(e)
    }
}

fn io_at(path: &Path) -> impl FnOnce(io::Error) -> WarmError + '_ {
    move |error| WarmError::Io {
        path: path.to_owned(),
        error,
    }
}

/// Who is copying from a warm build, and whether a Clear waits for them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarmStatus {
    /// The tickets whose new worktrees are being seeded from it now: `["SHOP-21"]`.
    pub copying: Vec<String>,
    /// A Clear (or the switch turned off) waits for those copies, then clears.
    pub clearing: bool,
}

/// One warm build in `warm/`, for Settings and Cleanup (`warm.list`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarmBuildInfo {
    /// The folder's name: `shop-web`.
    pub repo: String,
    pub path: String,
    /// The key of the project it belongs to; `None` for a folder no project owns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The sum of its files' sizes (apparent size, as Cleanup shows for worktrees).
    pub size_bytes: u64,
    /// The short SHA it was built at; `None` without a record (as are `from` and `kept_at`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub built_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept_at: Option<i64>,
    #[serde(flatten)]
    pub status: WarmStatus,
}

/// The `warm.list` reply: whether warm builds are available at all (the switch's reason when not), and every one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarmList {
    pub support: CloneSupport,
    pub builds: Vec<WarmBuildInfo>,
}

/// What `warm.clear` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ClearOutcome {
    /// There was no warm build to clear.
    Nothing,
    /// Cleared (its folder is deleted in the background).
    Cleared,
    /// Waiting for these tickets' copies; it clears when they end, and publishes [`Event::WarmBuildChanged`].
    Waiting { copying: Vec<String> },
}

/// What [`WarmBuilds::replace`] did with the new build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Replaced {
    /// It is the project's warm build now (the first one, or newer than the old one).
    Kept,
    /// Dropped: the warm build there is newer, belongs to another project, or warm builds are off now.
    Dropped,
}

/// The lock and the cached size of one warm build.
#[derive(Default)]
struct Slots {
    /// The reader slots: an id and the ticket being seeded.
    readers: Vec<(u64, String)>,
    writer: bool,
    writers_waiting: u32,
    clearing: bool,
    /// Bumped on every change of the folder, so a cached size is known stale.
    generation: u64,
    /// `(generation, size)`.
    size: Option<(u64, u64)>,
    /// Held from moving the files into `.incoming` until replace is done with it: one keep at a time.
    incoming: Arc<AsyncMutex<()>>,
    /// Held while `.old` exists (from the rename until the background delete ends).
    old: Arc<AsyncMutex<()>>,
}

struct Inner {
    data: DataDir,
    probe: Arc<CloneProbe>,
    board: Board,
    events: EventBus,
    slots: Mutex<HashMap<String, Slots>>,
    /// Woken on every release of a slot.
    changed: Notify,
    next_reader: AtomicU64,
}

/// The warm builds of every project. Cheap to clone.
#[derive(Clone)]
pub struct WarmBuilds {
    inner: Arc<Inner>,
}

impl fmt::Debug for WarmBuilds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WarmBuilds")
            .field("dir", &self.inner.data.warm_dir())
            .finish_non_exhaustive()
    }
}

impl WarmBuilds {
    pub fn new(data: DataDir, probe: Arc<CloneProbe>, board: Board, events: EventBus) -> Self {
        Self {
            inner: Arc::new(Inner {
                data,
                probe,
                board,
                events,
                slots: Mutex::new(HashMap::new()),
                changed: Notify::new(),
                next_reader: AtomicU64::new(1),
            }),
        }
    }

    /// Whether the data folder's disk can clone (probed once, at service start).
    pub fn support(&self) -> &CloneSupport {
        self.inner.probe.support(&self.inner.data)
    }

    /// Warm builds on for the project (key): its switch is on and the disk can clone. Off for either reason means
    /// no seeding, no keeping, and Done deletes as before.
    pub async fn enabled(&self, project: &str) -> Result<bool, BoardError> {
        Ok(self.support().available
            && self
                .inner
                .board
                .project_settings(project)
                .await?
                .warm_builds)
    }

    /// The project's warm build folder (it may not exist).
    pub fn path(&self, project: &Project) -> Result<PathBuf, DataDirError> {
        self.inner.data.warm_path(&repo_name(project))
    }

    /// The record of the project's warm build, when it has one that is its own.
    pub async fn record(&self, project: &Project) -> Option<WarmRecord> {
        let folder = self.path(project).ok()?;
        let record = WarmRecord::read(&folder)?;
        let owner = self.owner(&repo_name(project), Some(&record)).await?;
        (owner.id == project.id).then_some(record)
    }

    /// Who is copying from the warm build in `warm/<repo>`, and whether a Clear waits.
    pub fn status(&self, repo: &str) -> WarmStatus {
        self.with(repo, |s| WarmStatus {
            copying: s.readers.iter().map(|(_, t)| t.clone()).collect(),
            clearing: s.clearing,
        })
    }

    /// A reader slot on the project's warm build, named by the ticket being seeded (T-0298), held while it clones.
    /// Waits while a replace or Clear holds or waits for the writer slot. `None` when, by then, the project has no
    /// warm build of its own; the slot is released then.
    pub async fn read(&self, project: &Project, ticket: &str) -> Option<ReadSlot> {
        let repo = repo_name(project);
        let folder = self.inner.data.warm_path(&repo).ok()?;
        let id = self.inner.next_reader.fetch_add(1, Ordering::Relaxed);
        self.wait_until(&repo, |s| {
            let free = !s.writer && s.writers_waiting == 0;
            if free {
                s.readers.push((id, ticket.to_owned()));
            }
            free
        })
        .await;
        let mut slot = ReadSlot {
            store: self.clone(),
            repo,
            id,
            path: folder,
            record: None,
        };
        if !slot.path.is_dir() {
            return None;
        }
        slot.record = WarmRecord::read(&slot.path);
        let owner = self.owner(&slot.repo, slot.record.as_ref()).await?;
        (owner.id == project.id).then_some(slot)
    }

    /// The project's `<repo-name>.incoming/` folder for a keep (T-0297), held until [`replace`](Self::replace) takes
    /// it or it is dropped (then deleted in the background). Waits for another keep of the same warm build. The
    /// folder is empty: anything a crash left there is removed first.
    pub async fn incoming(&self, project: &Project) -> Result<Incoming, WarmError> {
        let repo = repo_name(project);
        let path = sibling(&self.inner.data, &repo, "incoming")?;
        let lock = self.with(&repo, |s| s.incoming.clone());
        let guard = lock.lock_owned().await;
        let ready = {
            let path = path.clone();
            tokio::task::spawn_blocking(move || {
                remove_tree(&path);
                DataDir::ensure_dir(&path)
            })
            .await
            .expect("the incoming folder is made")
        };
        ready.map_err(io_at(&path))?;
        Ok(Incoming {
            path,
            guard: Some(guard),
        })
    }

    /// Makes the files in `incoming` the project's warm build, **forward only**: when a warm build exists and its
    /// base isn't an ancestor of (or equal to) `record.base`, the new one is dropped, since an older build never
    /// replaces a newer one. Also dropped when the folder belongs to another project, or warm builds were turned
    /// off meanwhile. Waits for the copies running from the old one. Dropped and replaced folders are deleted in
    /// the background.
    pub async fn replace(
        &self,
        project: &Project,
        mut incoming: Incoming,
        record: WarmRecord,
    ) -> Result<Replaced, WarmError> {
        let repo = repo_name(project);
        let folder = self.inner.data.warm_path(&repo)?;
        let record = WarmRecord {
            project: project.id.clone(),
            ..record
        };
        record
            .write(&incoming.path)
            .map_err(io_at(&incoming.path))?;
        let writer = self.write(&repo).await;
        let keep =
            self.enabled(&project.key).await? && self.is_newer(project, &folder, &record).await?;
        if !keep {
            drop(writer);
            tracing::info!(project = %project.key, from = %record.from, "an older warm build isn't kept");
            return Ok(Replaced::Dropped);
        }
        let old = self.set_aside(&repo, &folder).await?;
        if let Err(e) = std::fs::rename(&incoming.path, &folder) {
            // The old build goes back.
            if let Some(old) = &old {
                let _ = std::fs::rename(&old.path, &folder);
            }
            return Err(io_at(&folder)(e));
        }
        // Taken: nothing is left to delete.
        incoming.guard.take();
        self.changed_folder(&repo);
        drop(writer);
        if let Some(old) = old {
            old.delete();
        }
        self.measure_later(&repo);
        tracing::info!(project = %project.key, from = %record.from, "warm build kept");
        self.publish(&repo, Some(project));
        Ok(Replaced::Kept)
    }

    /// Clears the project's warm build (WRM-06): waits for the copies running from it, renames it to `.old` and
    /// deletes that in the background. Returns once it is cleared. The switch stays as it is.
    pub async fn clear(&self, project: &Project) -> Result<ClearOutcome, WarmError> {
        self.clear_repo(&repo_name(project), Some(project), None)
            .await
    }

    /// [`clear`](Self::clear) for a request: answers at once, either done or "waiting for SHOP-21's copy" (the Clear
    /// then goes on in the background and publishes [`Event::WarmBuildChanged`] when done), so the app's other
    /// requests on the same connection don't wait behind it.
    pub async fn start_clear(&self, project: Option<&Project>, repo: &str) -> ClearOutcome {
        let (tx, rx) = oneshot::channel();
        let store = self.clone();
        let (project, repo) = (project.cloned(), repo.to_owned());
        tokio::spawn(async move {
            let what = project
                .as_ref()
                .map(|p| p.key.clone())
                .unwrap_or(repo.clone());
            if let Err(e) = store.clear_repo(&repo, project.as_ref(), Some(tx)).await {
                tracing::warn!(warm_build = %what, error = %e, "can't clear the warm build");
            }
        });
        rx.await.unwrap_or(ClearOutcome::Nothing)
    }

    async fn clear_repo(
        &self,
        repo: &str,
        project: Option<&Project>,
        mut progress: Option<oneshot::Sender<ClearOutcome>>,
    ) -> Result<ClearOutcome, WarmError> {
        let mut answer = |outcome: &ClearOutcome| {
            if let Some(tx) = progress.take() {
                let _ = tx.send(outcome.clone());
            }
        };
        let folder = self.inner.data.warm_path(repo)?;
        let owned = match project {
            Some(project) => {
                let record = WarmRecord::read(&folder);
                self.owner(repo, record.as_ref())
                    .await
                    .is_some_and(|owner| owner.id == project.id)
            }
            None => true,
        };
        // A second Clear while one waits joins it.
        let already = self.with(repo, |s| std::mem::replace(&mut s.clearing, true));
        if !folder.is_dir() || !owned || already {
            if !already {
                self.with(repo, |s| s.clearing = false);
            }
            let outcome = if already {
                ClearOutcome::Waiting {
                    copying: self.status(repo).copying,
                }
            } else {
                ClearOutcome::Nothing
            };
            answer(&outcome);
            return Ok(outcome);
        }
        let copying = self.status(repo).copying;
        if !copying.is_empty() {
            answer(&ClearOutcome::Waiting { copying });
            self.publish(repo, project);
        }
        let writer = self.write(repo).await;
        let result = self.set_aside(repo, &folder).await;
        self.with(repo, |s| s.clearing = false);
        let old = match result {
            Ok(old) => old,
            Err(e) => {
                drop(writer);
                self.publish(repo, project);
                return Err(e);
            }
        };
        self.changed_folder(repo);
        drop(writer);
        if let Some(old) = old {
            old.delete();
        }
        tracing::info!(warm_build = %repo, "warm build cleared");
        self.publish(repo, project);
        answer(&ClearOutcome::Cleared);
        Ok(ClearOutcome::Cleared)
    }

    /// Every warm build in `warm/` (WRM-07), by folder name. The store's temporary folders aren't listed; a folder
    /// without a record lists with its size only.
    pub async fn list(&self) -> Result<WarmList, BoardError> {
        let dir = self.inner.data.warm_dir();
        let mut repos: Vec<String> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| !is_temporary(name))
            .collect();
        repos.sort();
        let mut builds = Vec::with_capacity(repos.len());
        for repo in repos {
            let path = dir.join(&repo);
            let record = WarmRecord::read(&path);
            let project = self.owner(&repo, record.as_ref()).await.map(|p| p.key);
            builds.push(WarmBuildInfo {
                size_bytes: self.size(&repo, &path).await,
                built_at: record.as_ref().map(|r| short_sha(&r.built_at)),
                from: record.as_ref().map(|r| r.from.clone()),
                kept_at: record.as_ref().map(|r| r.kept_at),
                status: self.status(&repo),
                path: path.display().to_string(),
                project,
                repo,
            });
        }
        Ok(WarmList {
            support: self.support().clone(),
            builds,
        })
    }

    /// The project a warm build belongs to: the one its record names, else (no record, or that project is gone)
    /// the first project, by key, whose folder has that name.
    async fn owner(&self, repo: &str, record: Option<&WarmRecord>) -> Option<Project> {
        let mut projects = self.inner.board.projects().await.ok()?;
        projects.sort_by(|a, b| a.key.cmp(&b.key));
        if let Some(named) = record.and_then(|r| projects.iter().find(|p| p.id == r.project)) {
            return Some(named.clone());
        }
        projects.into_iter().find(|p| repo_name(p) == repo)
    }

    /// Whether `record` may replace what is at `folder`: nothing is there, or the project's own build there has a
    /// base that is an ancestor of (or equal to) the new base. A base git can't resolve (history rewritten) can't
    /// be compared, so the new build wins.
    async fn is_newer(
        &self,
        project: &Project,
        folder: &Path,
        record: &WarmRecord,
    ) -> Result<bool, WarmError> {
        if !folder.is_dir() {
            return Ok(true);
        }
        let old = WarmRecord::read(folder);
        let owner = self.owner(&repo_name(project), old.as_ref()).await;
        if owner.is_some_and(|owner| owner.id != project.id) {
            return Ok(false);
        }
        let Some(old) = old else {
            return Ok(true);
        };
        let git = self.inner.board.project_git(&project.key).await?;
        let (code, _) = git
            .run_with_code(
                Path::new(&project.repo_path),
                &["merge-base", "--is-ancestor", &old.base, &record.base],
            )
            .await
            .map_err(BoardError::Git)?;
        Ok(code != 1)
    }

    /// Renames the warm build at `folder` to `<repo>.old` (waiting for an earlier `.old` to be deleted first).
    /// `None` when there was nothing to rename.
    async fn set_aside(&self, repo: &str, folder: &Path) -> Result<Option<Old>, WarmError> {
        if !folder.is_dir() {
            return Ok(None);
        }
        let path = sibling(&self.inner.data, repo, "old")?;
        let lock = self.with(repo, |s| s.old.clone());
        let guard = lock.lock_owned().await;
        std::fs::rename(folder, &path).map_err(io_at(folder))?;
        Ok(Some(Old { path, guard }))
    }

    /// The apparent size of the warm build at `path`, measured off the async threads and cached until it changes.
    async fn size(&self, repo: &str, path: &Path) -> u64 {
        let (generation, cached) = self.with(repo, |s| (s.generation, s.size));
        if let Some((at, size)) = cached
            && at == generation
        {
            return size;
        }
        let measured = {
            let path = path.to_owned();
            tokio::task::spawn_blocking(move || apparent_size(&path))
                .await
                .unwrap_or(0)
        };
        self.with(repo, |s| {
            if s.generation == generation {
                s.size = Some((generation, measured));
            }
        });
        measured
    }

    /// Measures a new warm build in the background, so the next `warm.list` finds it cached.
    fn measure_later(&self, repo: &str) {
        let (store, repo) = (self.clone(), repo.to_owned());
        let Ok(path) = self.inner.data.warm_path(&repo) else {
            return;
        };
        tokio::spawn(async move {
            store.size(&repo, &path).await;
        });
    }

    fn changed_folder(&self, repo: &str) {
        self.with(repo, |s| {
            s.generation += 1;
            s.size = None;
        });
    }

    fn publish(&self, repo: &str, project: Option<&Project>) {
        self.inner.events.publish(Event::WarmBuildChanged {
            repo: repo.to_owned(),
            project: project.map(|p| p.key.clone()),
        });
    }

    /// Takes the writer slot, waiting for every reader. While it waits, new readers wait too.
    async fn write(&self, repo: &str) -> WriteSlot {
        let _waiting = WaitingWriter::new(self, repo);
        self.wait_until(repo, |s| {
            let free = !s.writer && s.readers.is_empty();
            if free {
                s.writer = true;
            }
            free
        })
        .await;
        WriteSlot {
            store: self.clone(),
            repo: repo.to_owned(),
        }
    }

    /// Waits until `take` (run under the lock) succeeds.
    async fn wait_until(&self, repo: &str, mut take: impl FnMut(&mut Slots) -> bool) {
        loop {
            let notified = self.inner.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.with(repo, &mut take) {
                return;
            }
            notified.await;
        }
    }

    fn with<T>(&self, repo: &str, f: impl FnOnce(&mut Slots) -> T) -> T {
        let mut slots = self.inner.slots.lock().expect("warm build slots");
        f(slots.entry(repo.to_owned()).or_default())
    }

    fn release(&self, repo: &str, f: impl FnOnce(&mut Slots)) {
        self.with(repo, f);
        self.inner.changed.notify_waiters();
    }
}

/// A reader slot on a warm build (T-0298 clones from [`path`](Self::path) while holding it). Released when dropped.
pub struct ReadSlot {
    store: WarmBuilds,
    repo: String,
    id: u64,
    path: PathBuf,
    record: Option<WarmRecord>,
}

impl ReadSlot {
    /// The warm build's folder.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Its record; `None` for a folder without one (no seed commit, so no timestamp trick).
    pub fn record(&self) -> Option<&WarmRecord> {
        self.record.as_ref()
    }
}

impl Drop for ReadSlot {
    fn drop(&mut self) {
        let id = self.id;
        self.store
            .release(&self.repo, |s| s.readers.retain(|(r, _)| *r != id));
    }
}

impl fmt::Debug for ReadSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadSlot")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

struct WriteSlot {
    store: WarmBuilds,
    repo: String,
}

impl Drop for WriteSlot {
    fn drop(&mut self) {
        self.store.release(&self.repo, |s| s.writer = false);
    }
}

/// Counts a writer waiting for its slot (so new readers wait), also when the wait is given up.
struct WaitingWriter {
    store: WarmBuilds,
    repo: String,
}

impl WaitingWriter {
    fn new(store: &WarmBuilds, repo: &str) -> Self {
        store.with(repo, |s| s.writers_waiting += 1);
        Self {
            store: store.clone(),
            repo: repo.to_owned(),
        }
    }
}

impl Drop for WaitingWriter {
    fn drop(&mut self) {
        self.store.release(&self.repo, |s| s.writers_waiting -= 1);
    }
}

/// `<repo-name>.incoming/`, filled by a keep (T-0297 moves the gitignored files into [`path`](Self::path)) and handed
/// to [`WarmBuilds::replace`]. Dropped without that, it is deleted in the background.
pub struct Incoming {
    path: PathBuf,
    guard: Option<OwnedMutexGuard<()>>,
}

impl Incoming {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Incoming {
    fn drop(&mut self) {
        if let Some(guard) = self.guard.take() {
            delete_in_background(self.path.clone(), guard);
        }
    }
}

impl fmt::Debug for Incoming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Incoming")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// A warm build renamed to `<repo-name>.old/`, to delete.
struct Old {
    path: PathBuf,
    guard: OwnedMutexGuard<()>,
}

impl Old {
    fn delete(self) {
        delete_in_background(self.path, self.guard);
    }
}

/// Deletes `path` off the async threads, holding `guard` until it is gone.
fn delete_in_background(path: PathBuf, guard: OwnedMutexGuard<()>) {
    let delete = move || {
        remove_tree(&path);
        drop(guard);
    };
    match tokio::runtime::Handle::try_current() {
        Ok(runtime) => {
            runtime.spawn_blocking(delete);
        }
        Err(_) => delete(),
    }
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// The sum of the sizes of everything in `folder` (symlinks not followed).
fn apparent_size(folder: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![folder.to_owned()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total += meta.len();
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_round_trips_and_keeps_clear_of_project_files() {
        let tmp = tempfile::tempdir().unwrap();
        let record = WarmRecord {
            built_at: "3e1f9a2c0d".into(),
            base: "9c41e7b".into(),
            from: "SHOP-21".into(),
            kept_at: 1_760_000_000_000,
            project: "p1".into(),
        };
        assert_eq!(WarmRecord::read(tmp.path()), None);
        record.write(tmp.path()).unwrap();
        assert_eq!(WarmRecord::read(tmp.path()), Some(record));
        let name = record_file_name();
        assert!(name.starts_with('.') && name.ends_with("-warm.json"));
        assert!(name.contains(naming::COMMAND_NAME));
        // An unreadable record is no record.
        std::fs::write(tmp.path().join(&name), "{").unwrap();
        assert_eq!(WarmRecord::read(tmp.path()), None);
    }

    #[test]
    fn the_times_round_trip_to_the_nanosecond() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("wt");
        std::fs::create_dir_all(root.join("src dir")).unwrap();
        std::fs::write(root.join("src dir/a b.txt"), "a").unwrap();
        std::fs::write(root.join("c.txt"), "c").unwrap();
        let files = ["src dir/a b.txt", "c.txt", "gone.txt"].map(String::from);
        let times = TrackedTimes::measure(&root, &files);
        assert_eq!(times.len(), 2, "a file that isn't there is left out");
        assert_eq!(TrackedTimes::read(tmp.path()), None);
        times.write(tmp.path()).unwrap();
        let read = TrackedTimes::read(tmp.path()).unwrap();
        assert_eq!(read, times);
        let a = std::fs::metadata(root.join("src dir/a b.txt"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(read.get("src dir/a b.txt"), Some(a));
        let name = times_file_name();
        assert!(name.starts_with('.') && name.contains(naming::COMMAND_NAME));
        // Another format, or a broken entry, is no times file.
        std::fs::write(tmp.path().join(&name), "times 9\0").unwrap();
        assert_eq!(TrackedTimes::read(tmp.path()), None);
        std::fs::write(tmp.path().join(&name), "times 1\0x 1 a").unwrap();
        assert_eq!(TrackedTimes::read(tmp.path()), None);
    }

    #[test]
    fn temporary_folders_are_told_apart_from_warm_builds() {
        let data = DataDir::new("/data");
        assert_eq!(
            sibling(&data, "shop-web", "incoming").unwrap(),
            Path::new("/data/warm/shop-web.incoming")
        );
        assert!(sibling(&data, "../x", "old").is_err());
        for temporary in ["shop-web.incoming", "shop-web.old", ".probe"] {
            assert!(is_temporary(temporary), "{temporary}");
        }
        for build in ["shop-web", "shop.web", "old-app"] {
            assert!(!is_temporary(build), "{build}");
        }
    }

    #[test]
    fn interrupted_folders_are_removed_and_builds_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let data = DataDir::new(tmp.path());
        assert_eq!(remove_interrupted(&data), 0);
        for folder in ["shop-web", "shop-web.incoming", "shop-web.old", ".probe"] {
            std::fs::create_dir_all(data.warm_dir().join(folder).join("deep")).unwrap();
        }
        assert_eq!(remove_interrupted(&data), 3);
        let left: Vec<String> = std::fs::read_dir(data.warm_dir())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, ["shop-web"]);
    }

    #[test]
    fn sizes_add_up_the_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("a").join("b")).unwrap();
        std::fs::write(tmp.path().join("a").join("one"), "12").unwrap();
        std::fs::write(tmp.path().join("a").join("b").join("two"), "345").unwrap();
        assert_eq!(apparent_size(tmp.path()), 5);
        assert_eq!(short_sha("3e1f9a2c0d"), "3e1f9a2");
    }
}
