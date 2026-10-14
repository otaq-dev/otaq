//! The Project terminal and the Shell from the CLI's side (T-0138 RUN-06, T-0137 RUN-05): plain shells owned by
//! the service, with `otaq` on PATH and the user's full access. `sh` stands in for the user's login shell.

use std::path::{Path, PathBuf};
use std::time::Duration;

use otaq_core::domain::{Actor, Status};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use otaq_core::session::SessionKind;

const CB: &str = env!("CARGO_BIN_EXE_otaq");

fn with_otaq(mut c: ServiceConfig) -> ServiceConfig {
    c.terminals.bin_dir = Path::new(CB).parent().map(PathBuf::from);
    c.terminals.shell = "sh".into();
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
        "{{ {command} ; }} > '{}' 2>&1; echo $? > '{}.tmp'; mv '{}.tmp' '{}'\n",
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
async fn the_project_terminal_creates_backlog_tickets_as_the_user() {
    let w = TestService::start_with(with_otaq).await;
    let project = w.add_project("shop-web").await;
    let terminal = w.shells.open_project("SHOP", false).await.unwrap();
    assert_eq!(terminal.session.kind, SessionKind::Project);
    assert_eq!(terminal.session.ticket, None);
    assert_eq!(terminal.name, "Terminal 1");

    // Git for Windows' sh prints the folder in its own form; `pwd -W` gives the Windows one (`C:/Users/…`).
    let pwd = if cfg!(windows) {
        "pwd -W | tr / '\\\\'"
    } else {
        "pwd"
    };
    let (code, out) = run(
        &w,
        &terminal.session.id,
        &format!("{pwd}; echo \"ticket=[$OTAQ_TICKET]\""),
    )
    .await;
    assert_eq!(code, 0);
    let repo = otaq_core::paths::canonical(std::path::Path::new(&project.repo_path)).unwrap();
    assert!(out.contains(repo.to_str().unwrap()), "{out}");
    assert!(out.contains("ticket=[]"), "{out}");

    let (code, out) = run(
        &w,
        &terminal.session.id,
        &format!("'{CB}' create \"Checkout API\""),
    )
    .await;
    assert_eq!(code, 0, "{out}");
    let ticket = w.board.ticket("SHOP-1").await.unwrap();
    assert_eq!(ticket.title, "Checkout API");
    assert_eq!(ticket.status, Status::Backlog);
    assert!(
        matches!(ticket.created_by, Actor::User(_)),
        "created as the user: {:?}",
        ticket.created_by
    );

    // `otaq` is on PATH there; a loop is refused with the CLI's error in the session.
    run(
        &w,
        &terminal.session.id,
        "otaq create \"Price table\" --deps SHOP-1",
    )
    .await;
    let (code, out) = run(&w, &terminal.session.id, "otaq dep add SHOP-1 SHOP-2").await;
    assert_ne!(code, 0);
    assert!(out.contains("SHOP-1"), "{out}");
    assert!(
        out.to_lowercase().contains("loop") || out.to_lowercase().contains("cycle"),
        "{out}"
    );

    // Opening it again shows the same one; exiting ends it ("Ended", reopen is a click).
    assert_eq!(
        w.shells
            .open_project("SHOP", false)
            .await
            .unwrap()
            .session
            .id,
        terminal.session.id
    );
    w.sessions.write(&terminal.session.id, b"exit\n").unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while w.sessions.info(&terminal.session.id).unwrap().is_running() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let ended = w.shells.open_project("SHOP", false).await.unwrap();
    assert_eq!(ended.session.id, terminal.session.id);
    assert!(!ended.session.is_running());
    let reopened = w.shells.open_project("SHOP", true).await.unwrap();
    assert!(reopened.session.is_running());
    // The ended one is still open (its output stays), so the new one is Terminal 2.
    assert_eq!(reopened.name, "Terminal 2");
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_shell_has_the_users_full_access() {
    let w = TestService::start_with(with_otaq).await;
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
        &format!("otaq dep add {} {}", other.key, parent.key),
    )
    .await;
    assert_eq!(code, 0, "{out}");
    let other = w.board.ticket(&other.key.to_string()).await.unwrap();
    assert_eq!(other.depends_on, std::slice::from_ref(&parent.key));
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn tickets_created_in_the_project_terminal_are_in_its_summary() {
    let w = TestService::start_with(with_otaq).await;
    w.add_project("shop-web").await;
    w.create("SHOP", "Made in the app").await;
    let terminal = w.shells.open_project("SHOP", false).await.unwrap();
    let (code, out) = run(&w, &terminal.session.id, "otaq create \"Checkout API\"").await;
    assert_eq!(code, 0, "{out}");
    let here = w.board.created_in_terminal("SHOP").await.unwrap();
    assert_eq!(here.session.as_deref(), Some(terminal.session.id.as_str()));
    let keys: Vec<String> = here.tickets.iter().map(|t| t.key.to_string()).collect();
    assert_eq!(keys, ["SHOP-2"], "only what came from the terminal");
    w.stop().await;
}
