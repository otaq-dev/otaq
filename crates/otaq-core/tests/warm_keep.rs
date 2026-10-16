//! Keeping the build at Done (T-0297; WRM-03, REC-05, d0082) against a running service in a temporary data folder and
//! temporary repositories. The "build" is fake: the test writes gitignored files and folders into the worktree. The
//! probe is forced, so nothing here depends on the test disk being able to clone (keeping only renames); the one
//! seed after a keep clones where the test disk can, else a copying stand-in does.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use otaq_core::domain::{Actor, Status};
use otaq_core::git::testing::{commit, git};
use otaq_core::review::LocalMergeOutcome;
use otaq_core::service::testing::TestService;
use otaq_core::store::projects::Project;
use otaq_core::warm::clone::{CloneProbe, CloneSupport};
use otaq_core::warm::seed::Seeder;
use otaq_core::warm::store::{TrackedTimes, WarmRecord, record_file_name, times_file_name};

fn me() -> Actor {
    Actor::User("me".into())
}

async fn world(available: bool) -> TestService {
    TestService::start_with(|config| otaq_core::service::ServiceConfig {
        clone_probe: Arc::new(CloneProbe::forced(CloneSupport {
            available,
            file_system: if available { "apfs" } else { "exfat" }.into(),
            volume: "/Volumes/Test".into(),
        })),
        ..config
    })
    .await
}

/// shop-web with a committed `.gitignore` and its env files in the checkout, listed in its settings.
async fn shop_web(w: &TestService) -> Project {
    let project = w.add_project("shop-web").await;
    let repo = Path::new(&project.repo_path);
    commit(
        repo,
        ".gitignore",
        "out/\n*.log\n.env.local\nconfig/\nlocked/\n",
    );
    std::fs::write(repo.join(".env.local"), "API=1\n").unwrap();
    std::fs::create_dir_all(repo.join("config")).unwrap();
    std::fs::write(repo.join("config").join(".env"), "DB=1\n").unwrap();
    let mut settings = w.board.project_settings("SHOP").await.unwrap();
    settings.env_files = vec![".env.local".into(), "config/.env".into()];
    w.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    project
}

/// A started ticket with one commit of its own.
async fn started(w: &TestService, title: &str, file: &str) -> (String, PathBuf) {
    let t = w.create("SHOP", title).await;
    let wt = w.start_worktree(&t).await;
    commit(&wt, file, "code\n");
    w.force_status(&t, Status::ReadyForReview).await;
    (t.key.to_string(), wt)
}

/// What a build leaves: gitignored files and folders, and the env files copied in at start.
fn build(wt: &Path, content: &str) {
    std::fs::create_dir_all(wt.join("out").join("deep")).unwrap();
    std::fs::write(wt.join("out").join("deep").join("app.bin"), content).unwrap();
    std::fs::write(wt.join("build.log"), "built\n").unwrap();
    std::fs::write(wt.join(".env.local"), "API=1\n").unwrap();
    std::fs::create_dir_all(wt.join("config")).unwrap();
    std::fs::write(wt.join("config").join(".env"), "DB=1\n").unwrap();
    std::fs::write(wt.join("config").join("cache.json"), "{}\n").unwrap();
}

async fn merge(w: &TestService, key: &str) {
    let outcome = w.review.local_merge(key, "m", false, &me()).await.unwrap();
    assert!(
        matches!(outcome, LocalMergeOutcome::Merged { .. }),
        "{outcome:?}"
    );
}

/// The project's single stock entry (on a disk that can clone the stock keeps one); a path that isn't there when
/// it has none.
fn warm(w: &TestService, project: &Project) -> PathBuf {
    let parent = w.warm.path(project).unwrap();
    let mut numbers: Vec<u64> = std::fs::read_dir(&parent)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_string_lossy().parse().ok())
        .collect();
    numbers.sort();
    match numbers.as_slice() {
        [] => parent.join("none"),
        [n] => parent.join(n.to_string()),
        more => panic!("one entry expected, found {more:?}"),
    }
}

fn built(w: &TestService, project: &Project) -> Option<String> {
    std::fs::read_to_string(warm(w, project).join("out/deep/app.bin")).ok()
}

fn rev(repo: &str, what: &str) -> String {
    git(Path::new(repo), &["rev-parse", what]).trim().to_owned()
}

fn modified(path: &Path) -> SystemTime {
    std::fs::symlink_metadata(path).unwrap().modified().unwrap()
}

fn set_modified(path: &Path, time: SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

fn ago(secs: u64) -> SystemTime {
    SystemTime::now() - Duration::from_secs(secs)
}

#[cfg(unix)]
async fn until(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("{what}");
}

#[tokio::test]
async fn done_keeps_the_gitignored_files_and_removes_the_worktree() {
    let w = world(true).await;
    let project = shop_web(&w).await;
    let (key, wt) = started(&w, "Checkout API", "api.ts").await;
    build(&wt, "v1");
    let api = modified(&wt.join("api.ts"));

    merge(&w, &key).await;

    assert!(!wt.exists(), "worktree removed");
    let folder = warm(&w, &project);
    assert_eq!(built(&w, &project).as_deref(), Some("v1"), "folders kept");
    assert!(folder.join("build.log").is_file(), "files kept");
    assert!(folder.join("config/cache.json").is_file());
    assert!(!folder.join(".env.local").exists(), "env files aren't kept");
    assert!(
        !folder.join("config/.env").exists(),
        "nor inside a kept folder"
    );
    assert!(!folder.join("api.ts").exists(), "tracked files aren't kept");
    let record = WarmRecord::read(&folder).expect("a record");
    let branch = otaq_core::naming::branch(&key);
    assert_eq!(record.built_at, rev(&project.repo_path, &branch));
    let base = git(
        Path::new(&project.repo_path),
        &["merge-base", &branch, "main"],
    );
    assert_eq!(record.base, base.trim());
    assert_eq!(record.from, key);
    assert_eq!(record.project, project.id);
    assert!(record.kept_at > 0);
    assert!(folder.join(record_file_name()).is_file());
    let times = TrackedTimes::read(&folder).expect("the tracked files' times");
    assert_eq!(times.get("api.ts"), Some(api));
    assert!(times.get(".gitignore").is_some());
    assert_eq!(times.get("build.log"), None, "only tracked files");
    w.stop().await;
}

/// Q71: SHOP-1 builds, then edits `cart.ts` and commits it without building again, and is merged. SHOP-2, seeded
/// from that build, must not take `cart.ts` as built: it gets the time it had in SHOP-1's worktree, newer than the
/// output, so a tool comparing times rebuilds it. Files unchanged since the build keep their older times.
#[tokio::test]
async fn a_file_committed_after_the_last_build_is_newer_than_the_output_in_the_next_seed() {
    let w = world(true).await;
    let project = shop_web(&w).await;
    let (key, wt) = started(&w, "Checkout API", "api.ts").await;
    let written = ago(7_200);
    set_modified(&wt.join("api.ts"), written);
    build(&wt, "v1");
    let built = ago(3_600);
    set_modified(&wt.join("out/deep/app.bin"), built);
    commit(&wt, "cart.ts", "new cart\n");
    let edited = ago(1_800);
    set_modified(&wt.join("cart.ts"), edited);
    merge(&w, &key).await;
    assert_eq!(
        TrackedTimes::read(&warm(&w, &project))
            .unwrap()
            .get("cart.ts"),
        Some(edited)
    );
    // main moves on after the merge.
    commit(Path::new(&project.repo_path), "hotfix.ts", "fix\n");

    let t = w.create("SHOP", "Receipts").await;
    let next = w.start_worktree(&t).await;
    let seeder = Seeder::new(w.warm.clone(), Arc::from(common::test_cloner(&w.data)));
    let git = w.board.project_git("SHOP").await.unwrap();
    seeder
        .seed(&w.board, &git, &project, &t.key.to_string(), &next)
        .await
        .expect("seeded");

    let output = modified(&next.join("out/deep/app.bin"));
    assert_eq!(output, built, "a clone keeps the output's time");
    assert_eq!(modified(&next.join("cart.ts")), edited);
    assert!(modified(&next.join("cart.ts")) > output, "rebuilt");
    assert_eq!(modified(&next.join("api.ts")), written);
    assert!(written < output, "taken as built");
    assert!(
        modified(&next.join("hotfix.ts")) > ago(600),
        "changed since: now"
    );
    assert!(!next.join(times_file_name()).exists());
    w.stop().await;
}

#[tokio::test]
async fn a_dirty_worktree_keeps_everything() {
    let w = world(true).await;
    let project = shop_web(&w).await;
    let (key, wt) = started(&w, "Price table", "price.ts").await;
    build(&wt, "v1");
    std::fs::write(wt.join("notes.md"), "not committed\n").unwrap();
    let t = w.board.ticket(&key).await.unwrap();

    let cleanup = w.review.finish(&t).await.unwrap();

    assert!(!cleanup.worktree_removed);
    assert!(wt.join("out/deep/app.bin").is_file(), "the build stays");
    assert!(wt.join("build.log").is_file());
    assert!(!warm(&w, &project).exists(), "nothing kept");
    w.stop().await;
}

#[tokio::test]
async fn switch_off_deletes_as_before() {
    let w = world(true).await;
    let project = shop_web(&w).await;
    let mut settings = w.board.project_settings("SHOP").await.unwrap();
    settings.warm_builds = false;
    w.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    let (key, wt) = started(&w, "Checkout API", "api.ts").await;
    build(&wt, "v1");

    merge(&w, &key).await;

    assert!(!wt.exists(), "worktree removed");
    assert!(!warm(&w, &project).exists(), "nothing kept");
    w.stop().await;
}

#[tokio::test]
async fn a_disk_that_cant_clone_deletes_as_before() {
    let w = world(false).await;
    let project = shop_web(&w).await;
    let (key, wt) = started(&w, "Checkout API", "api.ts").await;
    build(&wt, "v1");

    merge(&w, &key).await;

    assert!(!wt.exists(), "worktree removed");
    assert!(!warm(&w, &project).exists(), "nothing kept");
    w.stop().await;
}

#[tokio::test]
async fn a_newer_base_replaces_and_an_older_one_doesnt() {
    let w = world(true).await;
    let project = shop_web(&w).await;
    // SHOP-1 starts on an older main; SHOP-2 after a commit lands there.
    let (older, older_wt) = started(&w, "Old idea", "old.ts").await;
    commit(Path::new(&project.repo_path), "hotfix.ts", "fix\n");
    let (first, first_wt) = started(&w, "Checkout API", "api.ts").await;
    build(&first_wt, "first");
    merge(&w, &first).await;
    assert_eq!(built(&w, &project).as_deref(), Some("first"));

    // Closed without merging, built on the older base: dropped.
    build(&older_wt, "older");
    w.review.close(&older, true, &me()).await.unwrap();
    assert!(!older_wt.exists(), "worktree removed");
    assert_eq!(
        built(&w, &project).as_deref(),
        Some("first"),
        "not replaced"
    );
    let record = WarmRecord::read(&warm(&w, &project)).unwrap();
    assert_eq!(record.from, first);
    let times = TrackedTimes::read(&warm(&w, &project)).unwrap();
    assert!(times.get("api.ts").is_some() && times.get("old.ts").is_none());

    // Started after the merge: a newer base replaces it.
    let (newer, newer_wt) = started(&w, "Receipts", "receipt.ts").await;
    build(&newer_wt, "newer");
    merge(&w, &newer).await;
    assert_eq!(built(&w, &project).as_deref(), Some("newer"), "replaced");
    assert_eq!(WarmRecord::read(&warm(&w, &project)).unwrap().from, newer);
    // The times file moved in with the build.
    let times = TrackedTimes::read(&warm(&w, &project)).unwrap();
    assert!(times.get("receipt.ts").is_some(), "the newer build's times");
    w.stop().await;
}

/// A folder that can't be moved (an empty read-only folder: renaming a folder to another parent needs write
/// permission on it) stops the keep halfway: what was moved is deleted with `.incoming`, and Done goes on.
#[cfg(unix)]
#[tokio::test]
async fn a_failure_halfway_still_removes_the_worktree() {
    use std::os::unix::fs::PermissionsExt;

    let w = world(true).await;
    let project = shop_web(&w).await;
    let (key, wt) = started(&w, "Checkout API", "api.ts").await;
    build(&wt, "v1");
    let locked = wt.join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();

    merge(&w, &key).await;

    assert!(!wt.exists(), "worktree removed");
    assert!(!warm(&w, &project).exists(), "nothing kept");
    let incoming = warm(&w, &project).with_extension("incoming");
    until("the half-filled .incoming is deleted", || {
        !incoming.exists()
    })
    .await;
    w.stop().await;
}
