//! Seeding a new worktree at start (T-0298; WRM-04, SES-01, SES-02, d0082) against temporary repositories, with a
//! temporary data folder: from the warm build, from the checkout (env files left out, its uncommitted changes marked
//! changed), nothing to seed, a returning ticket and a re-run start not seeded, the timestamp trick, a failed clone,
//! and Clear waiting for a running seed. Builds are fake: files the test writes. Where the test disk can't clone, a
//! copying stand-in does the cloning (it lives here only; Otaq itself never copies).

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, SystemTime};

use common::{Fixture, me};
use otaq_core::board::settings::ProjectSettings;
use otaq_core::board::tickets::Ticket;
use otaq_core::data_dir::DataDir;
use otaq_core::domain::{StartStep, Status};
use otaq_core::git::testing::{commit, git};
use otaq_core::ports::PortPool;
use otaq_core::service::EventBus;
use otaq_core::session::Sessions;
use otaq_core::start::{StartOutcome, Starter};
use otaq_core::store::projects::Project;
use otaq_core::warm::clone::{CloneError, CloneProbe, CloneSupport, Cloner, FsCloner, probe};
use otaq_core::warm::seed::Seeder;
use otaq_core::warm::store::{ClearOutcome, WarmBuilds, WarmRecord, record_file_name};

/// 2000-01-01 00:00 UTC, the time unchanged tracked files get.
const OLD: Duration = Duration::from_secs(946_684_800);

/// Copies instead of cloning, for a test disk that can't clone. Never used outside the tests.
#[derive(Debug)]
struct CopyCloner;

impl Cloner for CopyCloner {
    fn clone_tree(
        &self,
        from: &Path,
        to: &Path,
        only: Option<&[PathBuf]>,
    ) -> Result<(), CloneError> {
        let io = |path: &Path| {
            let path = path.to_owned();
            move |error| CloneError::Io { path, error }
        };
        let whole = [PathBuf::new()];
        for rel in only.unwrap_or(&whole) {
            let dst = to.join(rel);
            if dst.symlink_metadata().is_ok() {
                return Err(CloneError::Io {
                    path: dst,
                    error: std::io::ErrorKind::AlreadyExists.into(),
                });
            }
            copy(&from.join(rel), &dst).map_err(io(&dst))?;
        }
        Ok(())
    }
}

fn copy(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(from, to).map(|_| ())
    }
}

/// The file system's cloning where the test disk can, else the copying stand-in; and every path list it was given.
#[derive(Debug)]
struct Recording {
    inner: Box<dyn Cloner>,
    calls: Mutex<Vec<Vec<PathBuf>>>,
    /// When set: tells the test the copy began, then waits to be let go (for "Clear waits for a copy").
    gate: Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>>,
    /// Fails after cloning the first path (leaving it behind, as a careless cloner would).
    fail: bool,
}

impl Recording {
    fn new(data: &DataDir) -> Self {
        let inner: Box<dyn Cloner> = if probe(data).available {
            Box::new(FsCloner)
        } else {
            println!("note: the test disk can't clone; a copying stand-in clones instead");
            Box::new(CopyCloner)
        };
        Self {
            inner,
            calls: Mutex::new(Vec::new()),
            gate: Mutex::new(None),
            fail: false,
        }
    }

    fn calls(&self) -> Vec<Vec<PathBuf>> {
        self.calls.lock().unwrap().clone()
    }
}

impl Cloner for Recording {
    fn clone_tree(
        &self,
        from: &Path,
        to: &Path,
        only: Option<&[PathBuf]>,
    ) -> Result<(), CloneError> {
        self.calls
            .lock()
            .unwrap()
            .push(only.map(<[PathBuf]>::to_vec).unwrap_or_default());
        if let Some((began, release)) = self.gate.lock().unwrap().take() {
            began.send(()).unwrap();
            release.recv_timeout(Duration::from_secs(60)).unwrap();
        }
        if self.fail {
            let first = &only.unwrap()[..1];
            self.inner.clone_tree(from, to, Some(first))?;
            return Err(CloneError::Io {
                path: to.to_owned(),
                error: std::io::Error::other("disk full"),
            });
        }
        self.inner.clone_tree(from, to, only)
    }
}

struct World {
    f: Fixture,
    data: DataDir,
    warm: WarmBuilds,
    starter: Starter,
    cloner: Arc<Recording>,
}

async fn world_with(change: impl FnOnce(&mut Recording)) -> World {
    let f = Fixture::shop().await;
    let data = f.data();
    let events = EventBus::new(64);
    let available = CloneSupport {
        available: true,
        file_system: "apfs".into(),
        volume: "/".into(),
    };
    let warm = WarmBuilds::new(
        data.clone(),
        Arc::new(CloneProbe::forced(available)),
        f.board.clone(),
        events.clone(),
    );
    let mut cloner = Recording::new(&data);
    change(&mut cloner);
    let cloner = Arc::new(cloner);
    // No launch context: every start stops at the launch step, after seeding, env files and setup.
    let starter = Starter::new(
        f.board.clone(),
        data.clone(),
        PortPool::new(f.board.store().clone()),
        Sessions::new(f.board.store().clone(), events.clone()),
        None,
        events,
    );
    starter.set_seeder(Seeder::new(warm.clone(), cloner.clone()));
    // The fake build writes `out/`; `.env.local` is the env file. Both are gitignored.
    let repo = f.repo();
    std::fs::create_dir_all(repo.join("src")).unwrap();
    commit(&repo, ".gitignore", "out/\n.env.local\n");
    commit(&repo, "src/a.txt", "a\n");
    commit(&repo, "src/b.txt", "b\n");
    World {
        f,
        data,
        warm,
        starter,
        cloner,
    }
}

async fn world() -> World {
    world_with(|_| {}).await
}

impl World {
    fn repo(&self) -> PathBuf {
        self.f.repo()
    }

    async fn project(&self) -> Project {
        self.f.board.project("SHOP").await.unwrap()
    }

    async fn settings(&self, change: impl FnOnce(&mut ProjectSettings)) {
        let mut settings = self.f.board.project_settings("SHOP").await.unwrap();
        change(&mut settings);
        self.f
            .board
            .set_project_settings("SHOP", &settings)
            .await
            .unwrap();
    }

    /// Keeps a fake build (`out/app.bin`) as SHOP-9's, built at `built_at`.
    async fn keep(&self, built_at: &str) {
        let project = self.project().await;
        let incoming = self.warm.incoming(&project).await.unwrap();
        std::fs::create_dir_all(incoming.path().join("out")).unwrap();
        std::fs::write(incoming.path().join("out/app.bin"), "warm build").unwrap();
        let record = WarmRecord {
            built_at: built_at.to_owned(),
            base: built_at.to_owned(),
            from: "SHOP-9".into(),
            kept_at: otaq_core::store::now_ms() - 2 * 3_600_000,
            project: String::new(),
        };
        self.warm.replace(&project, incoming, record).await.unwrap();
    }

    async fn queued(&self, title: &str) -> Ticket {
        let t = self.f.create(title).await;
        self.f.force_status(&t, Status::Queued).await;
        t
    }

    async fn start(&self, key: &str) -> StartOutcome {
        self.starter.start(key, &me()).await.unwrap()
    }

    fn worktree(&self, key: &str) -> PathBuf {
        self.data.worktree_path("shop-web", key).unwrap()
    }

    fn log(&self, key: &str) -> String {
        std::fs::read_to_string(self.data.setup_log_path(key).unwrap())
            .unwrap_or_default()
            .replace("\r\n", "\n")
    }
}

fn head(repo: &Path) -> String {
    git(repo, &["rev-parse", "HEAD"])
}

fn modified(path: &Path) -> SystemTime {
    std::fs::symlink_metadata(path).unwrap().modified().unwrap()
}

fn is_old(path: &Path) -> bool {
    modified(path) == SystemTime::UNIX_EPOCH + OLD
}

fn is_recent(path: &Path) -> bool {
    modified(path) > SystemTime::now() - Duration::from_secs(3_600)
}

/// The start went past seeding, env files and setup, to the launch (no launch context in these tests).
fn went_on(outcome: &StartOutcome) {
    assert!(
        matches!(
            outcome,
            StartOutcome::Interrupted {
                step: StartStep::Launch,
                ..
            }
        ),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn a_new_worktree_is_seeded_from_the_warm_build_with_the_timestamp_trick() {
    let w = world().await;
    let built_at = head(&w.repo());
    w.keep(&built_at).await;
    // main moves on after the build was kept: b.txt changes.
    commit(&w.repo(), "src/b.txt", "b changed\n");
    w.settings(|s| s.setup_command = "echo setup ran".into())
        .await;
    w.queued("SHOP-1").await;

    went_on(&w.start("SHOP-1").await);

    let wt = w.worktree("SHOP-1");
    assert_eq!(
        std::fs::read_to_string(wt.join("out/app.bin")).unwrap(),
        "warm build"
    );
    assert!(!wt.join(record_file_name()).exists(), "the record stays");
    assert_eq!(w.cloner.calls(), vec![vec![PathBuf::from("out")]]);
    // Unchanged since the seed's commit: old. Changed: now.
    assert!(is_old(&wt.join("src/a.txt")));
    assert!(is_old(&wt.join(".gitignore")));
    assert!(is_recent(&wt.join("src/b.txt")));
    let log = w.log("SHOP-1");
    let first = format!(
        "Started from shop-web's warm build of {} (SHOP-9, 2 h ago) in ",
        &built_at[..7]
    );
    assert!(log.starts_with(&first), "{log}");
    assert!(log.contains(" s.\n$ echo setup ran\nsetup ran"), "{log}");
    assert!(
        w.warm.status("shop-web").copying.is_empty(),
        "slot released"
    );
    // Git sees the clone as ignored files only.
    assert_eq!(git(&wt, &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn with_no_warm_build_the_checkout_seeds_without_its_env_files() {
    let w = world().await;
    let repo = w.repo();
    // The user's own build, an env file, and an uncommitted change to a tracked file.
    std::fs::create_dir_all(repo.join("out")).unwrap();
    std::fs::write(repo.join("out/app.bin"), "checkout build").unwrap();
    std::fs::write(repo.join(".env.local"), "SECRET=1\n").unwrap();
    std::fs::write(repo.join("src/a.txt"), "a, being edited\n").unwrap();
    w.settings(|s| s.env_files = vec![".env.local".into()])
        .await;
    w.queued("SHOP-1").await;

    went_on(&w.start("SHOP-1").await);

    let wt = w.worktree("SHOP-1");
    assert_eq!(
        std::fs::read_to_string(wt.join("out/app.bin")).unwrap(),
        "checkout build"
    );
    // The env file came from step 4, not from the clone.
    assert_eq!(w.cloner.calls(), vec![vec![PathBuf::from("out")]]);
    assert!(wt.join(".env.local").exists());
    // The checkout's build reflects its uncommitted a.txt: marked changed.
    assert!(is_recent(&wt.join("src/a.txt")));
    assert!(is_old(&wt.join("src/b.txt")));
    assert_eq!(
        w.log("SHOP-1"),
        format!(
            "Started from your checkout of shop-web ({}).\n",
            &head(&repo)[..7]
        )
    );
}

#[tokio::test]
async fn nothing_is_seeded_without_a_seed_or_with_the_switch_off() {
    let w = world().await;
    w.settings(|s| s.setup_command = "echo setup ran".into())
        .await;
    w.queued("SHOP-1").await;
    went_on(&w.start("SHOP-1").await);
    assert!(w.cloner.calls().is_empty());
    assert!(w.log("SHOP-1").starts_with("$ echo setup ran\n"));

    // The switch off: a warm build is there, but nothing is seeded.
    w.keep(&head(&w.repo())).await;
    w.settings(|s| s.warm_builds = false).await;
    w.queued("SHOP-2").await;
    went_on(&w.start("SHOP-2").await);
    assert!(w.cloner.calls().is_empty());
    assert!(!w.worktree("SHOP-2").join("out").exists());
}

#[tokio::test]
async fn a_rerun_start_and_a_returning_ticket_are_not_seeded_again() {
    let w = world().await;
    w.keep(&head(&w.repo())).await;
    w.settings(|s| s.setup_command = "exit 1".into()).await;
    let t = w.queued("SHOP-1").await;
    assert!(matches!(
        w.start("SHOP-1").await,
        StartOutcome::Interrupted {
            step: StartStep::Setup,
            ..
        }
    ));
    assert_eq!(w.cloner.calls().len(), 1);
    let wt = w.worktree("SHOP-1");
    std::fs::remove_dir_all(wt.join("out")).unwrap();

    // Retry start reuses the worktree: not seeded twice (SES-02), and its log begins anew.
    w.starter.retry("SHOP-1", &me()).await.unwrap();
    w.start("SHOP-1").await;
    assert_eq!(w.cloner.calls().len(), 1);
    assert!(!wt.join("out").exists());
    assert!(
        w.log("SHOP-1").starts_with("$ exit 1"),
        "{}",
        w.log("SHOP-1")
    );

    // A returning (parked) ticket keeps its worktree as it is.
    w.settings(|s| s.setup_command.clear()).await;
    sqlx::query("UPDATE tickets SET parked = TRUE WHERE id = ?")
        .bind(&t.id)
        .execute(w.f.board.store().pool())
        .await
        .unwrap();
    w.f.force_status(&t, Status::Queued).await;
    went_on(&w.start("SHOP-1").await);
    assert_eq!(w.cloner.calls().len(), 1);
    assert!(!wt.join("out").exists());
}

#[tokio::test]
async fn a_seed_commit_git_cant_resolve_skips_the_trick() {
    let w = world().await;
    w.keep("0123456789abcdef0123456789abcdef01234567").await;
    w.queued("SHOP-1").await;
    went_on(&w.start("SHOP-1").await);
    let wt = w.worktree("SHOP-1");
    assert!(wt.join("out/app.bin").exists(), "still seeded");
    // The files keep their checkout times.
    assert!(is_recent(&wt.join("src/a.txt")));
    assert!(is_recent(&wt.join("src/b.txt")));
}

#[tokio::test]
async fn a_failed_clone_leaves_the_worktree_clean_and_the_start_goes_on() {
    let w = world_with(|c| c.fail = true).await;
    let project = w.project().await;
    // Two paths, so the failing cloner leaves the first behind.
    let incoming = w.warm.incoming(&project).await.unwrap();
    for dir in ["build", "out"] {
        std::fs::create_dir_all(incoming.path().join(dir)).unwrap();
        std::fs::write(incoming.path().join(dir).join("f"), "x").unwrap();
    }
    let record = WarmRecord {
        built_at: head(&w.repo()),
        base: head(&w.repo()),
        from: "SHOP-9".into(),
        kept_at: otaq_core::store::now_ms(),
        project: String::new(),
    };
    w.warm.replace(&project, incoming, record).await.unwrap();
    w.settings(|s| s.setup_command = "echo setup ran".into())
        .await;
    w.queued("SHOP-1").await;

    went_on(&w.start("SHOP-1").await);

    let wt = w.worktree("SHOP-1");
    assert_eq!(w.cloner.calls().len(), 1);
    assert!(!wt.join("build").exists() && !wt.join("out").exists());
    assert_eq!(git(&wt, &["status", "--porcelain", "--ignored"]), "");
    assert!(w.log("SHOP-1").starts_with("$ echo setup ran\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn clear_waits_for_a_running_seed() {
    let (began_tx, began) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let w = world_with(|c| *c.gate.get_mut().unwrap() = Some((began_tx, release_rx))).await;
    w.keep(&head(&w.repo())).await;
    w.f.skip_to(21).await;
    assert_eq!(w.queued("SHOP-21").await.key.to_string(), "SHOP-21");
    let starter = w.starter.clone();
    let start = tokio::spawn(async move { starter.start("SHOP-21", &me()).await.unwrap() });
    tokio::task::spawn_blocking(move || {
        began
            .recv_timeout(Duration::from_secs(60))
            .expect("the copy began")
    })
    .await
    .unwrap();
    assert_eq!(w.warm.status("shop-web").copying, vec!["SHOP-21"]);

    let warm = w.warm.clone();
    let project = w.project().await;
    let clear = tokio::spawn(async move { warm.clear(&project).await.unwrap() });
    for _ in 0..200 {
        if w.warm.status("shop-web").clearing {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(w.warm.status("shop-web").clearing, "Clear waits");
    assert!(!clear.is_finished());

    release.send(()).unwrap();
    assert_eq!(clear.await.unwrap(), ClearOutcome::Cleared);
    went_on(&start.await.unwrap());
    assert!(w.worktree("SHOP-21").join("out/app.bin").exists());
    assert!(!w.warm.path(&w.project().await).unwrap().exists());
}
