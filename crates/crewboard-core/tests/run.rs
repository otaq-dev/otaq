//! Run (T-0133, RUN-01): the project's run command in the ticket's worktree with `$PORT` = the Run port, only on
//! click; starting → running once the port answers; Stop ends the whole process group; an early exit is "failed to
//! start"; a port held by another program is "port in use"; Done and Close stop it with a toast; a restart leaves
//! "stopped by restart". Ordinary shell commands stand in for a project's dev server.

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::ops::RangeInclusive;
use std::time::Duration;

use crewboard_core::board::Refusal;
use crewboard_core::board::settings::ProjectSettings;
use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Actor, Status};
use crewboard_core::git::testing::commit;
use crewboard_core::ports::PortPool;
use crewboard_core::protocol::Event;
use crewboard_core::service::testing::TestService;
use crewboard_core::terminals::run::{RunInfo, RunState, StopCause};

fn me() -> Actor {
    Actor::User("me".into())
}

async fn set_run_command(w: &TestService, command: &str) {
    let settings = ProjectSettings {
        run_command: command.into(),
        open_url: "http://localhost:$PORT/swagger".into(),
        ..Default::default()
    };
    w.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
}

/// A started ticket: a worktree and its two ports from `range`.
async fn started(w: &TestService, title: &str, range: RangeInclusive<u16>) -> (Ticket, u16) {
    let t = w.create("SHOP", title).await;
    w.start_worktree(&t).await;
    let ports = PortPool::with_range(w.board.store().clone(), range)
        .allocate(&t.id)
        .await
        .unwrap();
    w.force_status(&t, Status::Working).await;
    (w.board.ticket(&t.key.to_string()).await.unwrap(), ports.run)
}

/// Waits until the ticket's Run is in `state` (10 s at most).
async fn wait_state(w: &TestService, key: &str, state: RunState) -> RunInfo {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let info = w.runs.info(key).await.unwrap();
            if info.state == state {
                return info;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{key}'s Run never became {state:?}"))
}

fn output(w: &TestService, session: &str) -> String {
    String::from_utf8_lossy(&w.sessions.attach(session).unwrap().scrollback).into_owned()
}

fn alive(pid: i32) -> bool {
    // Signal 0: only checks whether the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

#[tokio::test(flavor = "multi_thread")]
async fn refused_without_a_run_command_or_a_worktree() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let (t, _) = started(&w, "Price table", 23_000..=23_099).await;
    let err = w.runs.start(&t.key.to_string()).await.unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NoRunCommand));
    assert_eq!(err.to_string(), "shop-web has no run command yet");

    set_run_command(&w, "sleep 30").await;
    let backlog = w.create("SHOP", "Not started").await;
    let err = w.runs.start(&backlog.key.to_string()).await.unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NoWorktree));
    let info = w.runs.info("SHOP-2").await.unwrap();
    assert_eq!(info.state, RunState::NotStarted);
    assert_eq!(info.command, "sleep 30");
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn runs_in_the_worktree_with_the_run_port_and_stop_ends_the_process_group() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let (t, port) = started(&w, "Price table", 23_100..=23_199).await;
    let worktree = t.work.as_ref().unwrap().worktree.clone().unwrap();
    set_run_command(
        &w,
        "echo ready; echo \"port=$PORT ticket=$CREWBOARD_TICKET\"; pwd; sleep 30 & echo $! > child.pid; wait",
    )
    .await;
    let key = t.key.to_string();

    let info = w.runs.start(&key).await.unwrap();
    assert_eq!(info.state, RunState::Starting, "nothing listens yet");
    assert_eq!(info.port, Some(port));
    assert_eq!(
        info.url().as_deref(),
        Some(format!("http://localhost:{port}/swagger").as_str())
    );
    let session = info.session.clone().unwrap();
    let text = w.wait_for_output(&session, "ticket=").await;
    assert!(
        text.starts_with("$ echo ready"),
        "first line shows the command: {text:?}"
    );
    assert!(text.contains("ready"));
    assert!(
        text.contains(&format!("port={port} ticket={key}")),
        "{text}"
    );
    let canonical = std::fs::canonicalize(&worktree).unwrap();
    w.wait_for_output(&session, canonical.to_str().unwrap())
        .await;
    // The card shows ▶ :port while it runs.
    assert_eq!(
        w.board.ticket(&key).await.unwrap().flags.run_port,
        Some(port)
    );
    // Starting it again leaves it alone.
    assert_eq!(
        w.runs.start(&key).await.unwrap().session,
        Some(session.clone())
    );
    assert_eq!(w.runs.running().await.len(), 1);

    let pid_file = std::path::Path::new(&worktree).join("child.pid");
    let child: i32 = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(text) = std::fs::read_to_string(&pid_file)
                && let Ok(pid) = text.trim().parse()
            {
                return pid;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(alive(child));

    let info = w.runs.stop(&key).await.unwrap();
    assert_eq!(info.state, RunState::Stopped);
    assert_eq!(info.stopped_by, Some(StopCause::You));
    assert!(output(&w, &session).contains("] stopped by you"));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!alive(child), "the whole process group stopped");
    assert_eq!(w.board.ticket(&key).await.unwrap().flags.run_port, None);
    assert!(w.runs.running().await.is_empty());
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn running_once_the_port_answers() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let (t, port) = started(&w, "Price table", 23_200..=23_299).await;
    set_run_command(&w, "echo listening soon; sleep 30").await;
    let key = t.key.to_string();
    let info = w.runs.start(&key).await.unwrap();
    assert_eq!(info.state, RunState::Starting);
    // Something now listens on the Run port (the dev server came up).
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
    let info = wait_state(&w, &key, RunState::Running).await;
    assert_eq!(info.port, Some(port));
    drop(listener);
    w.runs.stop(&key).await.unwrap();
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_early_exit_is_failed_to_start_with_the_log_kept() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let (t, _) = started(&w, "Price table", 23_300..=23_399).await;
    set_run_command(
        &w,
        "echo 'ERR_PNPM_NO_SCRIPT  Missing script: dev' >&2; exit 1",
    )
    .await;
    let key = t.key.to_string();
    let info = w.runs.start(&key).await.unwrap();
    let session = info.session.unwrap();
    let info = wait_state(&w, &key, RunState::FailedToStart).await;
    assert_eq!(info.exit_code, Some(1));
    assert_eq!(info.stopped_by, None);
    let text = output(&w, &session);
    assert!(text.contains("Missing script: dev"), "{text}");
    assert!(text.contains("] exited with code 1"), "{text}");
    assert_eq!(w.board.ticket(&key).await.unwrap().flags.run_port, None);

    // A program that ran and finished cleanly is just stopped.
    set_run_command(&w, "echo done").await;
    w.runs.start(&key).await.unwrap();
    let info = wait_state(&w, &key, RunState::Stopped).await;
    assert_eq!(info.exit_code, Some(0));
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_port_held_by_another_program_is_port_in_use() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let (t, port) = started(&w, "Price table", 23_400..=23_499).await;
    set_run_command(&w, "sleep 30").await;
    let held = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)).unwrap();
    let info = w.runs.start(&t.key.to_string()).await.unwrap();
    assert_eq!(info.state, RunState::PortInUse);
    assert_eq!(info.port, Some(port));
    assert_eq!(info.session, None, "nothing started");
    drop(held);
    let info = w.runs.start(&t.key.to_string()).await.unwrap();
    assert_eq!(info.state, RunState::Starting);
    w.runs.stop(&t.key.to_string()).await.unwrap();
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn close_stops_it_with_a_toast_and_a_restart_leaves_stopped_by_restart() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    set_run_command(&w, "sleep 30").await;
    let (closing, _) = started(&w, "Won't do", 23_500..=23_599).await;
    let (other, _) = started(&w, "Price table", 23_500..=23_599).await;
    let (closing_key, other_key) = (closing.key.to_string(), other.key.to_string());
    let session = w.runs.start(&closing_key).await.unwrap().session.unwrap();
    w.runs.start(&other_key).await.unwrap();
    let mut listener = w.events.listen();

    w.review.close(&closing_key, true, &me()).await.unwrap();
    let info = w.runs.info(&closing_key).await.unwrap();
    assert_eq!(info.state, RunState::Stopped);
    assert_eq!(
        info.stopped_by,
        Some(StopCause::Finished {
            status: Status::ClosedNotMerged
        })
    );
    assert!(!w.sessions.info(&session).unwrap().is_running());
    assert!(output(&w, &session).contains(&format!("] stopped · {closing_key} is closed")));
    let toast = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(Event::RunStopped { message, .. }) = listener.receiver.recv().await {
                return message;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(toast, format!("Run stopped · {closing_key} is closed"));

    // A restart ends the other one; it shows "stopped by restart" and nothing starts it again.
    let w = w.restart().await;
    let info = wait_state(&w, &other_key, RunState::StoppedByRestart).await;
    assert_eq!(info.stopped_by, Some(StopCause::Restart));
    assert!(w.runs.running().await.is_empty());
    assert_eq!(
        w.board.ticket(&other_key).await.unwrap().flags.run_port,
        None
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_local_merge_stops_it_as_done() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    set_run_command(&w, "sleep 30").await;
    let (t, _) = started(&w, "Checkout API", 23_600..=23_699).await;
    let worktree = t.work.as_ref().unwrap().worktree.clone().unwrap();
    commit(std::path::Path::new(&worktree), "api.ts", "api\n");
    w.force_status(&t, Status::ReadyForReview).await;
    let key = t.key.to_string();
    w.runs.start(&key).await.unwrap();
    w.review.local_merge(&key, "m", false, &me()).await.unwrap();
    let info = w.runs.info(&key).await.unwrap();
    assert_eq!(info.state, RunState::Stopped);
    assert_eq!(
        info.stopped_by,
        Some(StopCause::Finished {
            status: Status::Merged
        })
    );
    w.stop().await;
}

/// Q30: Park frees the ports (d0052), so the Run stops first: its process is gone and the Run port is free.
#[tokio::test(flavor = "multi_thread")]
async fn park_stops_the_run_and_frees_its_port() {
    let w = TestService::start_with(|mut c| {
        c.terminals.shell = "/bin/sh".into();
        c.terminals.shell_args = Vec::new();
        c
    })
    .await;
    w.add_project("shop-web").await;
    let (t, port) = started(&w, "Price table", 23_700..=23_799).await;
    let worktree = t.work.as_ref().unwrap().worktree.clone().unwrap();
    // A dev server that holds the Run port.
    set_run_command(&w, "echo $$ > run.pid; exec nc -lk $PORT").await;
    let key = t.key.to_string();
    let session = w.runs.start(&key).await.unwrap().session.unwrap();
    let pid_file = std::path::Path::new(&worktree).join("run.pid");
    let pid: i32 = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(text) = std::fs::read_to_string(&pid_file)
                && let Ok(pid) = text.trim().parse()
            {
                return pid;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(alive(pid));
    wait_state(&w, &key, RunState::Running).await;
    assert!(
        !crewboard_core::ports::is_free_on_machine(port),
        "the Run holds its port"
    );
    // The Shell holds no port: it stays open.
    let shell = w.shells.open_shell(&key, false).await.unwrap();

    w.review.park(&key, true, &me()).await.unwrap();
    let info = w.runs.info(&key).await.unwrap();
    assert_eq!(info.state, RunState::Stopped);
    assert_eq!(info.stopped_by, Some(StopCause::Parked));
    assert!(!w.sessions.info(&session).unwrap().is_running());
    assert!(output(&w, &session).contains(&format!("] stopped · {key} was parked")));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!alive(pid), "the Run's process is gone");
    let row = w.board.store().ticket(&t.id).await.unwrap().unwrap();
    assert_eq!((row.agent_port, row.run_port), (None, None), "ports freed");
    assert!(
        crewboard_core::ports::is_free_on_machine(port),
        "nothing listens on the freed Run port"
    );
    assert_eq!(w.board.ticket(&key).await.unwrap().flags.run_port, None);
    assert!(w.runs.running().await.is_empty());
    assert!(
        w.sessions.info(&shell.id).unwrap().is_running(),
        "the Shell stays"
    );
    w.stop().await;
}
