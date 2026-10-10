//! The Shell from the CLI's side (T-0137, RUN-05): a plain shell owned by the service, with `crewboard` on PATH and
//! the user's full access. `sh` stands in for the user's login shell.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crewboard_core::domain::Status;
use crewboard_core::service::ServiceConfig;
use crewboard_core::service::testing::TestService;

const CB: &str = env!("CARGO_BIN_EXE_crewboard");

fn with_crewboard(mut c: ServiceConfig) -> ServiceConfig {
    c.terminals.bin_dir = Path::new(CB).parent().map(PathBuf::from);
    c.terminals.shell = "/bin/sh".into();
    c.terminals.shell_args = Vec::new();
    c
}

/// Types `command` into the session and waits for its exit status (written to a file).
async fn run(w: &TestService, session: &str, command: &str) -> (i32, String) {
    let dir = w.home.path();
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let out = dir.join(format!("out.{n}"));
    let status = dir.join(format!("status.{n}"));
    let line = format!(
        "{command} > '{}' 2>&1; echo $? > '{}.tmp'; mv '{}.tmp' '{}'\n",
        out.display(),
        status.display(),
        status.display(),
        status.display()
    );
    w.sessions.write(session, line.as_bytes()).unwrap();
    let code = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(text) = std::fs::read_to_string(&status) {
                return text.trim().parse::<i32>().unwrap();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the command finished");
    (code, std::fs::read_to_string(&out).unwrap_or_default())
}

#[tokio::test(flavor = "multi_thread")]
async fn the_shell_has_the_users_full_access() {
    let w = TestService::start_with(with_crewboard).await;
    w.add_project("shop-web").await;
    let own = w.create("SHOP", "Invoice PDF export").await;
    let other = w.create("SHOP", "Another one").await;
    let parent = w.create("SHOP", "Its parent").await;
    w.start_worktree(&own).await;
    w.force_status(&own, Status::Working).await;
    let shell = w
        .shells
        .open_shell(&own.key.to_string(), false)
        .await
        .unwrap();
    // An agent may only link its own ticket; the Shell is the user's terminal.
    let (code, out) = run(
        &w,
        &shell.id,
        &format!("crewboard dep add {} {}", other.key, parent.key),
    )
    .await;
    assert_eq!(code, 0, "{out}");
    let other = w.board.ticket(&other.key.to_string()).await.unwrap();
    assert_eq!(other.depends_on, std::slice::from_ref(&parent.key));
    w.stop().await;
}
