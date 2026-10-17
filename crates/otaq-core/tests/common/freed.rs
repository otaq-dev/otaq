//! Tickets whose build was freed at Review (M19-c), for the bring-back tests (T-0309): a running service whose disk is
//! forced to rename builds (as on NTFS or exFAT), so a freed build goes into the stock and is renamed back out. The
//! build is fake: files the test writes into the worktree. Included with `#[path]` by the tests that need it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use otaq_core::board::settings::ProjectSettings;
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{BuildState, Status};
use otaq_core::git::testing::{commit, git};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use otaq_core::session::launch::LaunchContext;
use otaq_core::store::projects::Project;
use otaq_core::ticket_env::free::FreeOutcome;
use otaq_core::ticket_env::handoff::HandoffMode;
use otaq_core::warm::clone::{CloneProbe, CloneSupport};

/// What the project's setup command prints, so a test can tell it ran.
pub const SETUP_RAN: &str = "setup ran";

/// A service whose disk can't clone (builds are renamed). `claude` stands in as `sh` printing its arguments, then
/// sleeping, so a test reads the command line a resumed agent got.
pub async fn world() -> TestService {
    world_with(|config| config).await
}

/// [`world`], with `change` applied to the service's settings after.
pub async fn world_with(change: impl FnOnce(ServiceConfig) -> ServiceConfig) -> TestService {
    let w = TestService::start_with(|config| {
        change(ServiceConfig {
            clone_probe: Arc::new(CloneProbe::forced(CloneSupport {
                available: false,
                file_system: "exfat".into(),
                volume: "/Volumes/Test".into(),
            })),
            launch: Some(LaunchContext {
                claude: vec![
                    "sh".into(),
                    "-c".into(),
                    "printf 'ARGS %s\\n' \"$*\"; exec sleep 30".into(),
                    "sh".into(),
                ],
                bin_dir: "/nonexistent".into(),
                plugin_dir: "/nonexistent/plugin".into(),
                path: std::env::var_os("PATH").unwrap_or_default(),
                claude_config_dir: None,
            }),
            ..config
        })
    })
    .await;
    assert_eq!(w.warm.mode(), HandoffMode::Move);
    w
}

/// shop-web with a committed `.gitignore`, `.env.local` as its env file and a setup command that prints
/// [`SETUP_RAN`]; `change` adjusts the settings.
pub async fn shop_web(w: &TestService, change: impl FnOnce(&mut ProjectSettings)) -> Project {
    let project = w.add_project("shop-web").await;
    commit(
        Path::new(&project.repo_path),
        ".gitignore",
        "out/\n*.log\n.env.local\n",
    );
    let mut settings = w.board.project_settings("SHOP").await.unwrap();
    settings.env_files = vec![".env.local".into()];
    settings.setup_command = format!("echo {SETUP_RAN}");
    change(&mut settings);
    w.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    project
}

/// What a build leaves in `wt`: gitignored files, and the env file copied in at start.
pub fn build(wt: &Path, content: &str) {
    std::fs::create_dir_all(wt.join("out").join("deep")).unwrap();
    std::fs::write(wt.join("out").join("deep").join("app.bin"), content).unwrap();
    std::fs::write(wt.join("build.log"), "built\n").unwrap();
    std::fs::write(wt.join(".env.local"), "API=1\n").unwrap();
}

/// The build's output in `wt`, if it is there.
pub fn app_bin(wt: &Path) -> Option<String> {
    std::fs::read_to_string(wt.join("out").join("deep").join("app.bin")).ok()
}

/// A started ticket with a commit on `file` and a build (holding `title`), in Ready for Review.
pub async fn in_review(w: &TestService, title: &str, file: &str) -> (Ticket, PathBuf) {
    let t = w.create("SHOP", title).await;
    let wt = w.start_worktree(&t).await;
    commit(&wt, file, "code\n");
    build(&wt, title);
    w.force_status(&t, Status::ReadyForReview).await;
    (w.board.ticket(&t.key.to_string()).await.unwrap(), wt)
}

/// [`in_review`], with its build freed into the stock.
pub async fn freed(w: &TestService, title: &str, file: &str) -> (Ticket, PathBuf) {
    let (t, wt) = in_review(w, title, file).await;
    let outcome = w.ticket_env.free(&t.key).await;
    // The service's own sweep may have freed it first.
    assert!(
        matches!(
            outcome,
            FreeOutcome::Freed { entry: Some(_) } | FreeOutcome::NotNeeded
        ),
        "{outcome:?}"
    );
    assert_eq!(app_bin(&wt), None, "the build left the worktree");
    assert_eq!(state(w, &t.key.to_string()).await, BuildState::Freed);
    (w.board.ticket(&t.key.to_string()).await.unwrap(), wt)
}

pub async fn state(w: &TestService, key: &str) -> BuildState {
    w.board
        .ticket(key)
        .await
        .unwrap()
        .work
        .unwrap_or_default()
        .build_state
}

/// Waits until `key`'s build is in `want` (10 s at most).
pub async fn until_state(w: &TestService, key: &str, want: BuildState) {
    for _ in 0..400 {
        if state(w, key).await == want {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!(
        "{key}'s build never became {want:?}: {:?}",
        state(w, key).await
    );
}

/// The project's stock entries, by number.
pub fn entries(w: &TestService, project: &Project) -> Vec<u64> {
    let parent = w.warm.path(project).unwrap();
    let mut numbers: Vec<u64> = std::fs::read_dir(&parent)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_string_lossy().parse().ok())
        .collect();
    numbers.sort();
    numbers
}

/// The ticket's setup log.
pub fn setup_log(w: &TestService, key: &str) -> String {
    std::fs::read_to_string(w.data.setup_log_path(key).unwrap())
        .unwrap_or_default()
        .replace("\r\n", "\n")
}

pub fn porcelain(wt: &Path) -> String {
    git(wt, &["status", "--porcelain"])
}
