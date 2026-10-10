//! The start sequence (T-0053) against temporary repositories, with an ordinary command in place of `claude`:
//! success, setup failure → Start interrupted → Retry start, diverged base → Held, no silent re-runs, and over
//! the socket: entering Queued queues, the scheduler starts it (T-0079).

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::{Fixture, me};
use crewboard_core::board::moves::DropTarget;
use crewboard_core::board::projects::AddProject;
use crewboard_core::board::settings::ProjectSettings;
use crewboard_core::board::tickets::{NewTicketInput, StartProblem, Ticket};
use crewboard_core::client::{Client, ClientConfig};
use crewboard_core::data_dir::DataDir;
use crewboard_core::domain::{StartStep, Status};
use crewboard_core::git::testing::{TempRepos, commit, git};
use crewboard_core::ports::{PortPool, is_free_on_machine};
use crewboard_core::protocol::{KeyParams, MoveParams, Reply, Request};
use crewboard_core::service::{EventBus, Service, ServiceConfig};
use crewboard_core::session::launch::LaunchContext;
use crewboard_core::session::{SessionKind, SessionState, Sessions};
use crewboard_core::start::{StartOutcome, Starter};

/// An ordinary command standing in for `claude`: prints a line, then waits like an interactive program.
fn stand_in() -> LaunchContext {
    LaunchContext {
        claude: vec![
            "sh".into(),
            "-c".into(),
            "echo agent started; exec cat".into(),
            "sh".into(),
        ],
        bin_dir: "/opt/cb/bin".into(),
        plugin_dir: "/opt/cb/plugin".into(),
        path: std::env::var_os("PATH").unwrap(),
        claude_config_dir: None,
    }
}

fn free_ports(store: crewboard_core::store::Store) -> PortPool {
    // Pick a range where the first ports are free right now.
    for _ in 0..50 {
        let start = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        if let Some(end) = start.checked_add(9)
            && (start..=start + 3).all(is_free_on_machine)
        {
            return PortPool::with_range(store, start..=end);
        }
    }
    panic!("no free ports");
}

struct Setup {
    f: Fixture,
    sessions: Sessions,
    starter: Starter,
    data: DataDir,
}

fn starter_for(f: &Fixture, launch: Option<LaunchContext>) -> (Sessions, Starter, DataDir) {
    let data = DataDir::new(f.home.path().join("home"));
    let events = EventBus::new(64);
    let sessions = Sessions::new(f.board.store().clone(), events.clone());
    let starter = Starter::new(
        f.board.clone(),
        data.clone(),
        free_ports(f.board.store().clone()),
        sessions.clone(),
        launch,
        events,
    );
    (sessions, starter, data)
}

async fn setup() -> Setup {
    let f = Fixture::shop().await;
    let (sessions, starter, data) = starter_for(&f, Some(stand_in()));
    Setup {
        f,
        sessions,
        starter,
        data,
    }
}

async fn queued(f: &Fixture, title: &str) -> Ticket {
    let t = f
        .create_with(NewTicketInput {
            title: title.into(),
            done_when: vec!["works".into()],
            ..common::shop_input()
        })
        .await;
    let t = f
        .board
        .move_ticket(&t.key.to_string(), DropTarget::DevQueued, false, &me())
        .await
        .unwrap();
    assert_eq!(t.status, Status::Queued);
    t
}

async fn set_project(f: &Fixture, settings: ProjectSettings) {
    f.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
}

async fn event_kinds(f: &Fixture, t: &Ticket) -> Vec<String> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT kind FROM events WHERE ticket_id = ? ORDER BY at, id")
            .bind(&t.id)
            .fetch_all(f.board.store().pool())
            .await
            .unwrap();
    rows.into_iter().map(|(k,)| k).collect()
}

fn repo(f: &Fixture) -> PathBuf {
    f.repos.root().join("shop-web")
}

#[tokio::test]
async fn a_successful_start_leaves_the_ticket_working() {
    let s = setup().await;
    std::fs::write(repo(&s.f).join(".env.local"), "SECRET=1\n").unwrap();
    set_project(
        &s.f,
        ProjectSettings {
            env_files: vec![".env.local".into()],
            setup_command: "echo installed > setup-ran.txt".into(),
            ..Default::default()
        },
    )
    .await;
    queued(&s.f, "Checkout API").await;

    let outcome = s.starter.start("SHOP-1", &me()).await.unwrap();
    let StartOutcome::Started(session) = outcome else {
        panic!("{outcome:?}")
    };
    let t = s.f.ticket("SHOP-1").await;
    assert_eq!(t.status, Status::Working);
    assert_eq!(t.start_problem, None);
    let work = t.work.clone().unwrap();
    let worktree = PathBuf::from(work.worktree.unwrap());
    assert_eq!(
        worktree,
        s.data.worktree_path("shop-web", "SHOP-1").unwrap()
    );
    assert!(worktree.join(".env.local").is_file());
    assert!(worktree.join("setup-ran.txt").is_file());
    assert_eq!(
        work.branch.as_deref(),
        Some(crewboard_core::naming::branch("SHOP-1").as_str())
    );
    assert_eq!(
        work.base_sha,
        Some(git(&repo(&s.f), &["rev-parse", "main"]))
    );
    let (agent, run) = (work.agent_port.unwrap(), work.run_port.unwrap());
    assert_ne!(agent, run);

    // Crewboard's status line (USG-04): a settings file in the data folder, never in the worktree, whose only
    // key is `statusLine`.
    let settings = crewboard_core::session::statusline::settings_path(&s.data, &t.key);
    assert!(settings.starts_with(s.data.root()));
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
    let keys: Vec<&String> = written.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["statusLine"]);
    assert_eq!(
        written["statusLine"]["command"],
        crewboard_core::session::statusline::command_line(std::path::Path::new("/opt/cb/bin"))
    );
    assert!(!worktree.join(".claude").exists());

    // A live agent session with its Claude session ID, running in the worktree with $PORT.
    assert_eq!(session.kind, SessionKind::Agent);
    assert!(session.claude_session_id.is_some());
    let running = s
        .sessions
        .running_of(&t.id, SessionKind::Agent)
        .expect("live session");
    assert_eq!(running.id, session.id);
    s.sessions.write(&session.id, b"echo-check\n").unwrap();
    let mut attached = s.sessions.attach(&session.id).unwrap();
    let mut seen = attached.scrollback.clone();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !String::from_utf8_lossy(&seen).contains("agent started") {
            seen.extend_from_slice(&attached.output.recv().await.unwrap().data);
        }
    })
    .await
    .unwrap();

    assert_eq!(event_kinds(&s.f, &t).await, ["created", "moved", "started"]);
    s.sessions.shutdown().await;
}

#[tokio::test]
async fn a_failed_setup_is_start_interrupted_and_retry_runs_it_all_again() {
    let s = setup().await;
    set_project(
        &s.f,
        ProjectSettings {
            setup_command: "echo resolving packages; echo 'ERR_PNPM_NO_LOCKFILE' >&2; exit 1"
                .into(),
            ..Default::default()
        },
    )
    .await;
    let t = queued(&s.f, "Fix flaky login redirect").await;

    let outcome = s.starter.start("SHOP-1", &me()).await.unwrap();
    let StartOutcome::Interrupted { step, message } = &outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(*step, StartStep::Setup);
    assert!(
        message.starts_with("setup command failed (exit 1):"),
        "{message}"
    );
    assert!(message.contains("ERR_PNPM_NO_LOCKFILE"), "{message}");
    let after = s.f.ticket("SHOP-1").await;
    assert_eq!(after.status, Status::StartInterrupted);
    assert_eq!(
        after.start_problem,
        Some(StartProblem {
            step: StartStep::Setup,
            message: message.clone()
        })
    );
    // Nothing rolled back: the worktree and branch stay; no session, no ports yet.
    let worktree = s.data.worktree_path("shop-web", "SHOP-1").unwrap();
    assert!(worktree.is_dir());
    assert!(s.sessions.running_of(&t.id, SessionKind::Agent).is_none());
    assert_eq!(after.work.as_ref().unwrap().agent_port, None);
    let log = std::fs::read_to_string(s.data.setup_log_path("SHOP-1").unwrap()).unwrap();
    assert!(log.contains("resolving packages"), "{log}");

    // Never re-run by itself: time passes, a start call finds it not Queued.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        s.starter.start("SHOP-1", &me()).await.unwrap(),
        StartOutcome::Skipped
    );
    assert_eq!(s.f.ticket("SHOP-1").await.status, Status::StartInterrupted);

    // The user fixes the command and clicks Retry start: the whole sequence runs and reuses the worktree.
    set_project(
        &s.f,
        ProjectSettings {
            setup_command: "true".into(),
            ..Default::default()
        },
    )
    .await;
    let retried = s.starter.retry("SHOP-1", &me()).await.unwrap();
    assert_eq!(retried.status, Status::Queued);
    assert_eq!(retried.start_problem, None);
    let outcome = s.starter.start("SHOP-1", &me()).await.unwrap();
    assert!(matches!(outcome, StartOutcome::Started(_)), "{outcome:?}");
    assert_eq!(s.f.ticket("SHOP-1").await.status, Status::Working);
    assert_eq!(
        event_kinds(&s.f, &t).await,
        [
            "created",
            "moved",
            "start_interrupted",
            "start_retried",
            "started"
        ]
    );

    // Retrying a ticket whose start didn't stop is refused.
    let err = s.starter.retry("SHOP-1", &me()).await.unwrap_err();
    assert!(err.to_string().contains("no stopped start"), "{err}");
    s.sessions.shutdown().await;
}

#[tokio::test]
async fn without_a_launch_context_the_launch_step_stops() {
    let f = Fixture::shop().await;
    let (sessions, starter, _) = starter_for(&f, None);
    queued(&f, "t").await;
    let outcome = starter.start("SHOP-1", &me()).await.unwrap();
    let StartOutcome::Interrupted { step, message } = outcome else {
        panic!()
    };
    assert_eq!(step, StartStep::Launch);
    assert!(message.contains("plugin wasn't found"), "{message}");
    assert!(sessions.in_memory().is_empty());
}

#[tokio::test]
async fn a_diverged_base_holds_the_ticket_without_a_worktree() {
    let f = Fixture::new().await;
    let clone = f.repos.clone_with_origin("billing-api", "develop", 1);
    f.board
        .add_project(&AddProject {
            path: clone.display().to_string(),
            key: Some("BILL".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    let other = f.repos.root().join("billing-api-seed");
    commit(&clone, "mine.txt", "mine\n");
    commit(&other, "theirs.txt", "theirs\n");
    git(&other, &["push", "--quiet", "origin", "develop"]);
    let t = f
        .create_with(NewTicketInput {
            project: "BILL".into(),
            title: "Invoices".into(),
            done_when: vec!["works".into()],
            ..Default::default()
        })
        .await;
    f.board
        .move_ticket("BILL-1", DropTarget::DevQueued, false, &me())
        .await
        .unwrap();
    let (_sessions, starter, data) = starter_for(&f, Some(stand_in()));

    let outcome = starter.start("BILL-1", &me()).await.unwrap();
    let message = "`develop` and `origin/develop` have diverged. Push or pull, then retry";
    assert_eq!(
        outcome,
        StartOutcome::Held {
            message: message.into()
        }
    );
    let held = f.ticket("BILL-1").await;
    assert_eq!(held.status, Status::Held);
    assert_eq!(
        held.start_problem.as_ref().unwrap().step,
        StartStep::SyncBase
    );
    assert_eq!(held.start_problem.as_ref().unwrap().message, message);
    assert!(
        !data
            .worktree_path("billing-api", "BILL-1")
            .unwrap()
            .exists()
    );
    assert_eq!(held.work, None);

    // After the user pulls (here: the local commit goes away), Retry start works.
    git(&clone, &["reset", "--quiet", "--hard", "origin/develop"]);
    starter.retry("BILL-1", &me()).await.unwrap();
    let outcome = starter.start("BILL-1", &me()).await.unwrap();
    assert!(matches!(outcome, StartOutcome::Started(_)), "{outcome:?}");
    drop(t);
}

#[tokio::test]
async fn the_first_epic_child_syncs_the_epic_base() {
    let s = setup().await;
    let epic = s.f.epic("Wishlist").await;
    let child =
        s.f.create_with(NewTicketInput {
            title: "Wishlist API".into(),
            epic: Some(epic.key.to_string()),
            done_when: vec!["works".into()],
            ..common::shop_input()
        })
        .await;
    s.f.board
        .move_ticket(&child.key.to_string(), DropTarget::DevQueued, false, &me())
        .await
        .unwrap();
    let outcome = s.starter.start("SHOP-1.1", &me()).await.unwrap();
    assert!(matches!(outcome, StartOutcome::Started(_)), "{outcome:?}");
    let epic_branch = crewboard_core::naming::branch("SHOP-1");
    assert_eq!(
        git(&repo(&s.f), &["rev-parse", &epic_branch]),
        git(&repo(&s.f), &["rev-parse", "main"])
    );
    s.sessions.shutdown().await;
}

#[tokio::test]
async fn entering_queued_over_the_socket_and_the_scheduler_starts_it() {
    let repos = TempRepos::new();
    let repo = repos.repo("shop-web", 1);
    let tmp = tempfile::tempdir().unwrap();
    let data = DataDir::new(tmp.path().join("home"));
    let mut config = ServiceConfig::new(data.clone());
    config.launch = Some(stand_in());
    let service = Service::bind(config).await.unwrap();
    let stopper = service.stopper();
    let board = service.board();
    let sessions = service.sessions();
    let task = tokio::spawn(service.serve());
    board
        .add_project(&AddProject {
            path: repo.display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    board
        .set_project_settings(
            "SHOP",
            &ProjectSettings {
                setup_command: "exit 7".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "t".into(),
                done_when: vec!["works".into()],
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    let client = Client::connect(ClientConfig::new(&data)).await.unwrap();

    let Reply::Ticket(moved) = client
        .request(Request::TicketMove(MoveParams {
            key: "SHOP-1".into(),
            target: DropTarget::DevQueued,
            confirmed: false,
        }))
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(moved.status, Status::Queued);
    let status = wait_for_status(&board, Status::StartInterrupted).await;
    assert_eq!(status.start_problem.unwrap().step, StartStep::Setup);

    board
        .set_project_settings("SHOP", &ProjectSettings::default())
        .await
        .unwrap();
    let Reply::Ticket(retried) = client
        .request(Request::TicketRetryStart(KeyParams {
            key: "SHOP-1".into(),
        }))
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(retried.status, Status::Queued);
    let working = wait_for_status(&board, Status::Working).await;
    let session = sessions
        .running_of(&working.id, SessionKind::Agent)
        .unwrap();
    assert_eq!(session.state, SessionState::Running);

    let Reply::ServiceStatus(status) = client.request(Request::ServiceStatus).await.unwrap() else {
        panic!()
    };
    assert_eq!(status.agents_working, 1);

    stopper.stop();
    task.await.unwrap();
}

async fn wait_for_status(board: &crewboard_core::board::Board, status: Status) -> Ticket {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let t = board.ticket("SHOP-1").await.unwrap();
            if t.status == status {
                return t;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("never reached {status}"))
}
