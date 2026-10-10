//! The PTY session manager (T-0050) with ordinary commands in place of `claude`: output and scrollback,
//! reconnecting, resize, exit detection, stopping, one agent session per ticket, and the protocol requests.
//! Every test stops what it started (the manager also kills its sessions when dropped).

mod common;

use std::time::Duration;

use common::Fixture;
use crewboard_core::client::{Client, ClientConfig};
use crewboard_core::data_dir::DataDir;
use crewboard_core::protocol::{
    Event, KeyParams, Reply, Request, ResizeParams, SessionIdParams, SessionListParams,
    SessionTarget, decode_bytes,
};
use crewboard_core::service::{EventBus, Service, ServiceConfig};
use crewboard_core::session::{
    Attached, END_STOPPED, SessionError, SessionKind, SessionState, SessionTicket, Sessions,
    SpawnSpec, TerminalSize,
};
use tokio::sync::broadcast;

const WAIT: Duration = Duration::from_secs(10);

fn spec(f: &Fixture, ticket: Option<SessionTicket>, script: &str) -> SpawnSpec {
    SpawnSpec {
        kind: SessionKind::Agent,
        project_id: String::new(),
        project_key: "SHOP".into(),
        ticket,
        program: "sh".into(),
        args: vec!["-c".into(), script.into()],
        cwd: f.repos.root(),
        env: vec![],
        size: TerminalSize::default(),
        claude_session_id: None,
    }
}

async fn project_id(f: &Fixture) -> String {
    f.board.project("SHOP").await.unwrap().id
}

async fn ticket(f: &Fixture, title: &str) -> SessionTicket {
    let t = f.create(title).await;
    SessionTicket {
        id: t.id,
        key: t.key,
    }
}

fn manager(f: &Fixture) -> Sessions {
    Sessions::new(f.board.store().clone(), EventBus::new(64))
}

/// Reads live output until `needle` shows up in everything seen so far (scrollback included).
async fn read_until(attached: &mut Attached, seen: &mut Vec<u8>, needle: &str) {
    let found = |seen: &[u8]| String::from_utf8_lossy(seen).contains(needle);
    tokio::time::timeout(WAIT, async {
        while !found(seen) {
            match attached.output.recv().await {
                Ok(chunk) => seen.extend_from_slice(&chunk.data),
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{needle:?} not seen in {:?}", String::from_utf8_lossy(seen)));
    assert!(
        found(seen),
        "{needle:?} not in {:?}",
        String::from_utf8_lossy(seen)
    );
}

#[tokio::test]
async fn output_is_stored_and_exit_is_recorded() {
    let f = Fixture::shop().await;
    let sessions = manager(&f);
    let mut s = spec(&f, Some(ticket(&f, "t").await), "echo hi; sleep 1");
    s.project_id = project_id(&f).await;
    let info = sessions.spawn(s).await.unwrap();
    assert_eq!(info.state, SessionState::Running);

    let ended = tokio::time::timeout(WAIT, sessions.wait_ended(&info.id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ended.state, SessionState::Ended);
    assert_eq!(ended.end_reason.as_deref(), Some("exit 0"));

    let attached = sessions.attach(&info.id).unwrap();
    let text = String::from_utf8_lossy(&attached.scrollback);
    assert!(text.contains("hi"), "{text:?}");
    assert_eq!(attached.start, 0);
    assert_eq!(attached.end, attached.scrollback.len() as u64);

    let row: (String, Option<i64>, Option<String>, String) =
        sqlx::query_as("SELECT state, ended_at, end_reason, kind FROM sessions WHERE id = ?")
            .bind(&info.id)
            .fetch_one(f.board.store().pool())
            .await
            .unwrap();
    assert_eq!(row.0, "ended");
    assert!(row.1.is_some());
    assert_eq!(row.2.as_deref(), Some("exit 0"));
    assert_eq!(row.3, "agent");
}

#[tokio::test]
async fn exit_code_is_the_end_reason() {
    let f = Fixture::shop().await;
    let sessions = manager(&f);
    let mut s = spec(&f, None, "exit 3");
    s.project_id = project_id(&f).await;
    let info = sessions.spawn(s).await.unwrap();
    let ended = tokio::time::timeout(WAIT, sessions.wait_ended(&info.id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ended.end_reason.as_deref(), Some("exit 3"));
}

#[tokio::test]
async fn reconnecting_client_gets_scrollback_then_live_output() {
    let f = Fixture::shop().await;
    let sessions = manager(&f);
    let mut s = spec(&f, None, "");
    s.project_id = project_id(&f).await;
    s.program = "cat".into();
    s.args = vec![];
    let info = sessions.spawn(s).await.unwrap();

    let mut first = sessions.attach(&info.id).unwrap();
    let mut seen = first.scrollback.clone();
    sessions.write(&info.id, b"one\n").unwrap();
    // The terminal echoes the line, then `cat` prints it.
    read_until(&mut first, &mut seen, "one\r\none").await;

    // A client attaching now gets "one" from the scrollback and only new output live.
    let mut second = sessions.attach(&info.id).unwrap();
    assert!(String::from_utf8_lossy(&second.scrollback).contains("one"));
    let mut seen2 = Vec::new();
    sessions.write(&info.id, b"two\n").unwrap();
    read_until(&mut second, &mut seen2, "two").await;
    let live = String::from_utf8_lossy(&seen2);
    assert!(!live.contains("one"), "no duplicate output: {live:?}");

    sessions.stop(&info.id).await.unwrap();
    let ended = sessions.info(&info.id).unwrap();
    assert_eq!(ended.end_reason.as_deref(), Some(END_STOPPED));
}

#[tokio::test]
async fn scrollback_is_bounded() {
    let f = Fixture::shop().await;
    let sessions = Sessions::with_scrollback_limit(f.board.store().clone(), EventBus::new(8), 1000);
    let mut s = spec(
        &f,
        None,
        "i=0; while [ $i -lt 2000 ]; do i=$((i+1)); echo line $i; done",
    );
    s.project_id = project_id(&f).await;
    let info = sessions.spawn(s).await.unwrap();
    tokio::time::timeout(WAIT, sessions.wait_ended(&info.id))
        .await
        .unwrap();
    let attached = sessions.attach(&info.id).unwrap();
    assert!(attached.scrollback.len() <= 1000);
    assert!(attached.start > 0);
    let text = String::from_utf8_lossy(&attached.scrollback);
    assert!(text.trim_end().ends_with("line 2000"), "{text:?}");
    assert!(text.starts_with("line "), "starts at a line: {text:?}");
}

#[tokio::test]
async fn resize_reaches_the_program() {
    let f = Fixture::shop().await;
    let sessions = manager(&f);
    let mut s = spec(&f, None, "read go; stty size; sleep 5");
    s.project_id = project_id(&f).await;
    let info = sessions.spawn(s).await.unwrap();
    let mut attached = sessions.attach(&info.id).unwrap();
    sessions
        .resize(
            &info.id,
            TerminalSize {
                cols: 100,
                rows: 40,
            },
        )
        .unwrap();
    assert_eq!(
        sessions.info(&info.id).unwrap().size,
        Some(TerminalSize {
            cols: 100,
            rows: 40
        })
    );
    sessions.write(&info.id, b"\r").unwrap();
    let mut seen = attached.scrollback.clone();
    read_until(&mut attached, &mut seen, "40 100").await;
    sessions.stop(&info.id).await.unwrap();
}

#[tokio::test]
async fn one_agent_session_per_ticket() {
    let f = Fixture::shop().await;
    let sessions = manager(&f);
    let t = ticket(&f, "t").await;
    let mut s = spec(&f, Some(t.clone()), "sleep 30");
    s.project_id = project_id(&f).await;
    let first = sessions.spawn(s.clone()).await.unwrap();
    let err = sessions.spawn(s.clone()).await.unwrap_err();
    assert!(
        matches!(&err, SessionError::AlreadyRunning { session, .. } if *session == first.id),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "SHOP-1 already has a running agent session"
    );

    // Another kind for the same ticket is fine (a Shell later), and so is a new agent once the first ended.
    let mut shell = s.clone();
    shell.kind = SessionKind::Shell;
    let shell = sessions.spawn(shell).await.unwrap();
    sessions.stop(&first.id).await.unwrap();
    let second = sessions.spawn(s).await.unwrap();
    assert_eq!(
        sessions.running_of(&t.id, SessionKind::Agent).unwrap().id,
        second.id
    );

    let history = sessions.history(&t).await.unwrap();
    let ids: Vec<_> = history.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, [&second.id, &shell.id, &first.id], "newest first");
    assert_eq!(history[2].end_reason.as_deref(), Some(END_STOPPED));
    sessions.shutdown().await;
    assert!(sessions.in_memory().iter().all(|i| !i.is_running()));
}

#[tokio::test]
async fn shutdown_ends_every_session_and_leftovers_are_recorded() {
    let f = Fixture::shop().await;
    let sessions = manager(&f);
    let mut s = spec(&f, None, "trap '' HUP; sleep 30");
    s.project_id = project_id(&f).await;
    let stubborn = sessions.spawn(s.clone()).await.unwrap();
    s.args = vec!["-c".into(), "sleep 30".into()];
    let plain = sessions.spawn(s).await.unwrap();
    sessions.shutdown().await;
    for id in [&stubborn.id, &plain.id] {
        let info = sessions.info(id).unwrap();
        assert_eq!(info.state, SessionState::Ended);
        assert_eq!(info.end_reason.as_deref(), Some("service stopped"));
    }
    // A row left open by a crashed service is closed at the next start.
    sqlx::query("UPDATE sessions SET ended_at = NULL, state = 'running' WHERE id = ?")
        .bind(&plain.id)
        .execute(f.board.store().pool())
        .await
        .unwrap();
    assert_eq!(Sessions::end_leftovers(f.board.store()).await.unwrap(), 1);
}

#[tokio::test]
async fn protocol_list_attach_resize_detach() {
    let tmp = tempfile::tempdir().unwrap();
    let data = DataDir::new(tmp.path().join("home"));
    let service = Service::bind(ServiceConfig::new(data.clone()))
        .await
        .unwrap();
    let stopper = service.stopper();
    let sessions = service.sessions();
    let board = service.board();
    let task = tokio::spawn(service.serve());
    let repos = crewboard_core::git::testing::TempRepos::new();
    let repo = repos.repo("shop-web", 1);
    let project = board
        .add_project(&crewboard_core::board::projects::AddProject {
            path: repo.display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let t = board
        .create_ticket(
            &crewboard_core::board::tickets::NewTicketInput {
                project: "SHOP".into(),
                title: "t".into(),
                ..Default::default()
            },
            &common::me(),
        )
        .await
        .unwrap();

    let client = Client::connect(ClientConfig::new(&data)).await.unwrap();
    let mut events = client.subscribe().await.unwrap();
    // Sessions are started by the start sequence (T-0053); here one is spawned directly.
    let info = sessions
        .spawn(SpawnSpec {
            kind: SessionKind::Agent,
            project_id: project.id.clone(),
            project_key: "SHOP".into(),
            ticket: Some(SessionTicket {
                id: t.id.clone(),
                key: t.key.clone(),
            }),
            program: "cat".into(),
            args: vec![],
            cwd: repo.clone(),
            env: vec![],
            size: TerminalSize::default(),
            claude_session_id: None,
        })
        .await
        .unwrap();
    let started = next_event(&mut events, |e| matches!(e, Event::SessionState { .. })).await;
    assert!(
        matches!(started, Event::SessionState { session } if session.id == info.id && session.is_running())
    );

    sessions.write(&info.id, b"before\n").unwrap();
    // The terminal echoes the line, then cat prints it: wait for both before attaching, so no output of
    // "before" is still on its way.
    tokio::time::timeout(WAIT, async {
        while !String::from_utf8_lossy(&sessions.attach(&info.id).unwrap().scrollback)
            .contains("before\r\nbefore")
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    // Attach by ticket key: the stored output comes in the reply.
    let Reply::SessionAttached(attached) = client
        .request(Request::SessionAttach(SessionTarget {
            session: None,
            key: Some("SHOP-1".into()),
        }))
        .await
        .unwrap()
    else {
        panic!()
    };
    assert!(
        String::from_utf8_lossy(&decode_bytes(&attached.scrollback).unwrap()).contains("before")
    );
    assert_eq!(attached.session.id, info.id);

    // Live output follows as session_output events, starting where the scrollback ended.
    sessions.write(&info.id, b"after\n").unwrap();
    let mut live = Vec::new();
    let mut first_offset = None;
    while !String::from_utf8_lossy(&live).contains("after") {
        if let Event::SessionOutput {
            session,
            offset,
            data,
        } = next_event(&mut events, |e| matches!(e, Event::SessionOutput { .. })).await
        {
            assert_eq!(session, info.id);
            first_offset.get_or_insert(offset);
            live.extend(decode_bytes(&data).unwrap());
        }
    }
    assert!(first_offset.unwrap() >= attached.end);
    assert!(!String::from_utf8_lossy(&live).contains("before"));

    client
        .request(Request::SessionResize(ResizeParams {
            session: info.id.clone(),
            size: TerminalSize {
                cols: 120,
                rows: 30,
            },
        }))
        .await
        .unwrap();
    let Reply::Sessions(list) = client
        .request(Request::SessionList(SessionListParams {
            key: Some("SHOP-1".into()),
        }))
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0].size,
        Some(TerminalSize {
            cols: 120,
            rows: 30
        })
    );
    assert_eq!(list[0].ticket.as_ref().unwrap().to_string(), "SHOP-1");

    let Reply::Ok = client
        .request(Request::SessionDetach(SessionIdParams {
            session: info.id.clone(),
        }))
        .await
        .unwrap()
    else {
        panic!()
    };
    let err = client
        .request(Request::SessionAttach(SessionTarget {
            session: Some("nope".into()),
            key: None,
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("no session"), "{err}");

    // Stopping the service ends the session and records why.
    stopper.stop();
    task.await.unwrap();
    let reason: (Option<String>,) = sqlx::query_as("SELECT end_reason FROM sessions WHERE id = ?")
        .bind(&info.id)
        .fetch_one(
            crewboard_core::store::Store::open_in(&data)
                .await
                .unwrap()
                .pool(),
        )
        .await
        .unwrap();
    assert_eq!(reason.0.as_deref(), Some("service stopped"));
    let _ = KeyParams { key: String::new() };
}

async fn next_event(
    events: &mut broadcast::Receiver<Event>,
    wanted: impl Fn(&Event) -> bool,
) -> Event {
    tokio::time::timeout(WAIT, async {
        loop {
            let event = events.recv().await.unwrap();
            if wanted(&event) {
                return event;
            }
        }
    })
    .await
    .expect("an event in time")
}

#[tokio::test]
async fn keystrokes_from_several_clients_reach_cat_and_track_unsent_text() {
    let tmp = tempfile::tempdir().unwrap();
    let data = DataDir::new(tmp.path().join("home"));
    let service = Service::bind(ServiceConfig::new(data.clone()))
        .await
        .unwrap();
    let stopper = service.stopper();
    let sessions = service.sessions();
    let board = service.board();
    let task = tokio::spawn(service.serve());
    let repos = crewboard_core::git::testing::TempRepos::new();
    let repo = repos.repo("shop-web", 1);
    let project = board
        .add_project(&crewboard_core::board::projects::AddProject {
            path: repo.display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let info = sessions
        .spawn(SpawnSpec {
            kind: SessionKind::Shell,
            project_id: project.id,
            project_key: "SHOP".into(),
            ticket: None,
            program: "cat".into(),
            args: vec![],
            cwd: repo,
            env: vec![],
            size: TerminalSize::default(),
            claude_session_id: None,
        })
        .await
        .unwrap();

    let app = Client::connect(ClientConfig::new(&data)).await.unwrap();
    let attach = Client::connect(ClientConfig::new(&data)).await.unwrap();
    let mut events = app.subscribe().await.unwrap();
    let input = |text: &[u8]| {
        Request::SessionInput(crewboard_core::protocol::InputParams {
            session: info.id.clone(),
            data: crewboard_core::protocol::encode_bytes(text),
            reply: false,
        })
    };

    app.request(input(b"from the app ")).await.unwrap();
    attach.request(input(b"and attach")).await.unwrap();
    assert!(sessions.has_unsent_input(&info.id).unwrap());
    attach.request(input(b"\x7f\x7f")).await.unwrap();
    assert!(sessions.has_unsent_input(&info.id).unwrap(), "text is left");
    app.request(input(b"\r")).await.unwrap();
    assert!(
        !sessions.has_unsent_input(&info.id).unwrap(),
        "Enter sends it"
    );
    let enter = next_event(&mut events, |e| matches!(e, Event::SessionEnter { .. })).await;
    assert!(matches!(enter, Event::SessionEnter { session, ticket: None } if session == info.id));

    // cat echoes both clients' typing as one line.
    let mut attached = sessions.attach(&info.id).unwrap();
    let mut seen = attached.scrollback.clone();
    read_until(&mut attached, &mut seen, "from the app and atta\r\n").await;

    // Crewboard's own typing doesn't set the flag; its clear_line resets it.
    sessions.write(&info.id, b"note").unwrap();
    assert!(!sessions.has_unsent_input(&info.id).unwrap());
    app.request(input(b"draft")).await.unwrap();
    sessions.clear_line(&info.id).unwrap();
    assert!(!sessions.has_unsent_input(&info.id).unwrap());

    stopper.stop();
    task.await.unwrap();
}

#[tokio::test]
async fn agent_launch_through_a_pty_with_an_ordinary_command() {
    use crewboard_core::session::launch::{LaunchContext, launch_agent};
    let f = Fixture::shop().await;
    let sessions = manager(&f);
    let t = ticket(&f, "t").await;
    // An ordinary command stands in for claude: it prints its arguments and environment.
    let ctx = LaunchContext {
        claude: vec![
            "sh".into(),
            "-c".into(),
            "printf 'ARG=%s\\n' \"$@\"; env".into(),
            "sh".into(),
        ],
        bin_dir: "/opt/cb/bin".into(),
        plugin_dir: "/opt/cb/plugin".into(),
        path: std::env::var_os("PATH").unwrap(),
    };
    let cmd = launch_agent(&ctx, &t.key, &f.repos.root(), 4019);
    let info = sessions
        .spawn(SpawnSpec {
            kind: SessionKind::Agent,
            project_id: project_id(&f).await,
            project_key: "SHOP".into(),
            ticket: Some(t.clone()),
            program: cmd.program.clone(),
            args: cmd.args.clone(),
            cwd: cmd.cwd.clone(),
            env: cmd.env.clone(),
            size: TerminalSize::default(),
            claude_session_id: Some(cmd.claude_session_id.clone()),
        })
        .await
        .unwrap();
    assert_eq!(
        info.claude_session_id.as_deref(),
        Some(cmd.claude_session_id.as_str())
    );
    tokio::time::timeout(WAIT, sessions.wait_ended(&info.id))
        .await
        .unwrap();
    let out = String::from_utf8(sessions.attach(&info.id).unwrap().scrollback).unwrap();
    let name = crewboard_core::naming::COMMAND_NAME;
    for line in [
        "ARG=--plugin-dir".to_owned(),
        "ARG=/opt/cb/plugin".to_owned(),
        "ARG=--allowedTools".to_owned(),
        format!("ARG=Bash({name}:*)"),
        "ARG=--session-id".to_owned(),
        format!("ARG={}", cmd.claude_session_id),
        format!("ARG=/{name}:work-ticket"),
        format!("{}=SHOP-1", crewboard_core::naming::ticket_env_var()),
        "PORT=4019".to_owned(),
        "PATH=/opt/cb/bin:".to_owned(),
    ] {
        assert!(out.contains(&line), "{line:?} missing in {out:?}");
    }
    let stored: (Option<String>,) =
        sqlx::query_as("SELECT claude_session_id FROM sessions WHERE id = ?")
            .bind(&info.id)
            .fetch_one(f.board.store().pool())
            .await
            .unwrap();
    assert_eq!(stored.0, Some(cmd.claude_session_id));
}

#[tokio::test]
async fn focus_reports_are_kept_and_published() {
    let f = Fixture::shop().await;
    let bus = EventBus::new(16);
    let mut events = bus.subscribe();
    let sessions = Sessions::new(f.board.store().clone(), bus);
    let mut s = spec(&f, Some(ticket(&f, "t").await), "sleep 30");
    s.project_id = project_id(&f).await;
    let info = sessions.spawn(s).await.unwrap();
    assert!(!sessions.is_focused(&info.id).unwrap());
    sessions.set_focused(&info.id, 1, true).unwrap();
    sessions.set_focused(&info.id, 1, true).unwrap();
    assert!(sessions.is_focused(&info.id).unwrap());
    sessions.set_focused(&info.id, 1, false).unwrap();
    let mut seen = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let Event::SessionFocus {
            focused, ticket, ..
        } = event
        {
            assert_eq!(ticket.unwrap().to_string(), "SHOP-1");
            seen.push(focused);
        }
    }
    assert_eq!(seen, [true, false], "only changes are published");
    sessions.shutdown().await;
}
