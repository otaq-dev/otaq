//! Env files and the setup command (T-0049), in a temporary repo and its ticket worktree. The setup commands
//! are ordinary shell commands. (Not named after the setup command: Windows asks for administrator rights to run
//! any program whose name contains "setup", this test's binary included.)

mod common;

use std::path::PathBuf;

use common::Fixture;
use otaq_core::data_dir::DataDir;
use otaq_core::start::{Setup, copy_env_files, prepare_worktree, run_setup};

struct Env {
    f: Fixture,
    repo: PathBuf,
    worktree: PathBuf,
    log: PathBuf,
}

async fn setup() -> Env {
    let f = Fixture::shop().await;
    let t = f.create("SHOP-1").await;
    let data = DataDir::new(f.home.path().join("home"));
    let wt = prepare_worktree(&f.board, &data, &t.key.to_string())
        .await
        .unwrap();
    Env {
        repo: f.repos.root().join("shop-web"),
        log: data.setup_log_path("SHOP-1").unwrap(),
        worktree: wt.path,
        f,
    }
}

#[tokio::test]
async fn env_files_are_copied_and_overwritten() {
    let e = setup().await;
    std::fs::write(e.repo.join(".env.local"), "A=1\n").unwrap();
    std::fs::create_dir_all(e.repo.join("config")).unwrap();
    std::fs::write(e.repo.join("config/.env"), "B=2\n").unwrap();
    let files = vec![".env.local".to_owned(), "config/.env".to_owned()];

    copy_env_files(&e.repo, &e.worktree, &files).unwrap();
    assert_eq!(
        std::fs::read_to_string(e.worktree.join(".env.local")).unwrap(),
        "A=1\n"
    );
    assert_eq!(
        std::fs::read_to_string(e.worktree.join("config/.env")).unwrap(),
        "B=2\n"
    );

    // Re-running replaces the copies with the repo's current files.
    std::fs::write(e.repo.join(".env.local"), "A=changed\n").unwrap();
    copy_env_files(&e.repo, &e.worktree, &files).unwrap();
    assert_eq!(
        std::fs::read_to_string(e.worktree.join(".env.local")).unwrap(),
        "A=changed\n"
    );
}

#[tokio::test]
async fn a_missing_env_file_fails_the_step() {
    let e = setup().await;
    let err = copy_env_files(&e.repo, &e.worktree, &[".env.local".to_owned()]).unwrap_err();
    let message = err.to_string();
    assert!(
        message.starts_with("`.env.local` not found in "),
        "{message}"
    );
    assert!(message.ends_with("shop-web"), "{message}");

    let err = copy_env_files(&e.repo, &e.worktree, &["../secret".to_owned()]).unwrap_err();
    assert!(err.to_string().contains("inside the repository"), "{err}");
}

#[tokio::test]
async fn zero_config_passes_straight_through() {
    let e = setup().await;
    copy_env_files(&e.repo, &e.worktree, &[]).unwrap();
    assert_eq!(
        run_setup("  ", &e.worktree, &e.log, &[]).await.unwrap(),
        Setup::Skipped
    );
    assert!(!e.log.exists(), "no log without a command");
}

#[tokio::test]
async fn setup_runs_in_the_worktree_with_the_users_environment() {
    let e = setup().await;
    let marker = "OTAQ_SETUP_TEST_MARKER";
    // The service's environment is the user's; a variable set in this process reaches the command.
    // SAFETY: tests in this binary don't read this variable concurrently.
    unsafe { std::env::set_var(marker, "from-the-user") };
    // `cmd` syntax on Windows, where setup commands run with `cmd` (Q33).
    let command = if cfg!(windows) {
        format!("echo installing& cd> where.txt& (echo %{marker}%)> env.txt")
    } else {
        format!("echo installing; pwd > where.txt; echo ${marker} > env.txt; true")
    };
    assert_eq!(
        run_setup(&command, &e.worktree, &e.log, &[]).await.unwrap(),
        Setup::Succeeded
    );
    let pwd = std::fs::read_to_string(e.worktree.join("where.txt")).unwrap();
    assert_eq!(
        otaq_core::paths::canonical(PathBuf::from(pwd.trim())).unwrap(),
        otaq_core::paths::canonical(&e.worktree).unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(e.worktree.join("env.txt"))
            .unwrap()
            .replace("\r\n", "\n"),
        "from-the-user\n"
    );
    let log = std::fs::read_to_string(&e.log)
        .unwrap()
        .replace("\r\n", "\n");
    assert!(log.starts_with("$ echo installing"), "{log}");
    assert!(log.contains("\ninstalling\n"), "{log}");
    drop(e.f);
}

#[tokio::test]
async fn a_failing_setup_reports_exit_code_and_last_lines() {
    let e = setup().await;
    let command = if cfg!(windows) {
        "(for /l %i in (1,1,12) do @echo line %i)& 1>&2 echo oops& exit 1"
    } else {
        "for i in 1 2 3 4 5 6 7 8 9 10 11 12; do echo line $i; done; echo oops >&2; false"
    };
    let err = run_setup(command, &e.worktree, &e.log, &[])
        .await
        .unwrap_err();
    let message = err.to_string();
    assert!(
        message.starts_with("setup command failed (exit 1):\n"),
        "{message}"
    );
    assert!(message.ends_with("line 12\noops"), "{message}");
    assert!(
        !message.contains("line 2\n"),
        "only the last lines: {message}"
    );
    let log = std::fs::read_to_string(&e.log)
        .unwrap()
        .replace("\r\n", "\n");
    assert!(
        log.contains("line 1\n") && log.contains("oops"),
        "full log kept"
    );

    // Re-running replaces the log.
    run_setup("echo second", &e.worktree, &e.log, &[])
        .await
        .unwrap();
    let log = std::fs::read_to_string(&e.log).unwrap();
    assert!(!log.contains("oops"), "{log}");
}
