//! Caller identity by process ancestry (T-0064, CLI-01, CLI-04): ordinary nested shells inside PTY sessions stand
//! in for `claude` and the commands it runs.

use std::path::Path;
use std::time::Duration;

use crewboard_core::board::projects::AddProject;
use crewboard_core::board::tickets::{NewTicketInput, Ticket};
use crewboard_core::caller::{self, Caller};
use crewboard_core::data_dir::DataDir;
use crewboard_core::domain::Actor;
use crewboard_core::git::testing::TempRepos;
use crewboard_core::naming;
use crewboard_core::service::{Service, ServiceConfig, Stopper};
use crewboard_core::session::{SessionKind, SessionTicket, Sessions, SpawnSpec, TerminalSize};

struct World {
    home: tempfile::TempDir,
    _repos: TempRepos,
    stopper: Stopper,
    task: tokio::task::JoinHandle<()>,
    sessions: Sessions,
    board: crewboard_core::board::Board,
    project_id: String,
    repo: std::path::PathBuf,
    tickets: Vec<Ticket>,
}

impl World {
    async fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let data = DataDir::new(home.path().join("home"));
        let service = Service::bind(ServiceConfig::new(data)).await.unwrap();
        let (stopper, board, sessions) = (service.stopper(), service.board(), service.sessions());
        let task = tokio::spawn(service.serve());
        let repos = TempRepos::new();
        let repo = repos.repo("shop-web", 1);
        let project = board
            .add_project(&AddProject {
                path: repo.display().to_string(),
                ..Default::default()
            })
            .await
            .unwrap();
        let me = Actor::User("me".into());
        let mut tickets = Vec::new();
        for title in ["Split user full name", "Add wishlist"] {
            let input = NewTicketInput {
                project: "SHOP".into(),
                title: title.into(),
                ..Default::default()
            };
            tickets.push(board.create_ticket(&input, &me).await.unwrap());
        }
        Self {
            home,
            _repos: repos,
            stopper,
            task,
            sessions,
            board,
            project_id: project.id,
            repo,
            tickets,
        }
    }

    fn socket(&self) -> std::path::PathBuf {
        DataDir::new(self.home.path().join("home")).socket_path()
    }

    /// Runs `script` with `sh -c` as a session of `kind` for SHOP-1, with `$CREWBOARD_TICKET` set like an agent's.
    async fn run(&self, kind: SessionKind, script: &str) -> String {
        let ticket = &self.tickets[0];
        self.sessions
            .spawn(SpawnSpec {
                kind,
                project_id: self.project_id.clone(),
                project_key: "SHOP".into(),
                ticket: Some(SessionTicket {
                    id: ticket.id.clone(),
                    key: ticket.key.clone(),
                }),
                program: "sh".into(),
                args: vec!["-c".into(), script.into()],
                cwd: self.repo.clone(),
                env: vec![(naming::ticket_env_var(), ticket.key.to_string())],
                size: TerminalSize::default(),
                claude_session_id: None,
            })
            .await
            .unwrap()
            .id
    }

    async fn stop(self) {
        self.stopper.stop();
        self.task.await.unwrap();
    }
}

/// Waits for a file to be written and returns its trimmed contents.
async fn read_when_written(path: &Path) -> String {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(text) = std::fs::read_to_string(path)
                && text.ends_with('\n')
            {
                return text.trim().to_owned();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{} never written", path.display()))
}

#[tokio::test(flavor = "multi_thread")]
async fn nested_shells_inside_an_agent_session_are_that_agent() {
    let w = World::new().await;
    let out = w.home.path().join("pid");
    // A shell inside a shell inside the session, with the ticket variable removed on the way.
    let script = format!(
        "env -u {var} sh -c 'sh -c \"echo \\$\\$ > {out}; exec sleep 30\"'",
        var = naming::ticket_env_var(),
        out = out.display()
    );
    let session = w.run(SessionKind::Agent, &script).await;
    let pid: u32 = read_when_written(&out).await.parse().unwrap();
    match caller::resolve(Some(pid), &w.sessions) {
        Caller::Agent {
            ticket,
            ticket_id,
            session: s,
        } => {
            assert_eq!(ticket.to_string(), "SHOP-1");
            assert_eq!(ticket_id, w.tickets[0].id);
            assert_eq!(s, session);
        }
        Caller::User => panic!("the nested shell must resolve to SHOP-1's agent"),
    }
    // This test process is outside every session: the user.
    assert_eq!(
        caller::resolve(Some(std::process::id()), &w.sessions),
        Caller::User
    );
    assert_eq!(caller::resolve(None, &w.sessions), Caller::User);
    // Once the session ends, its processes are no longer an agent.
    w.sessions.stop(&session).await.unwrap();
    assert_eq!(caller::resolve(Some(pid), &w.sessions), Caller::User);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn shell_project_and_run_sessions_are_the_user() {
    let w = World::new().await;
    for kind in [SessionKind::Shell, SessionKind::Project, SessionKind::Run] {
        let out = w.home.path().join(format!("pid-{kind}"));
        let script = format!("sh -c 'echo $$ > {}; exec sleep 30'", out.display());
        let session = w.run(kind, &script).await;
        let pid: u32 = read_when_written(&out).await.parse().unwrap();
        assert_eq!(
            caller::resolve(Some(pid), &w.sessions),
            Caller::User,
            "{kind}"
        );
        w.sessions.stop(&session).await.unwrap();
    }
    w.stop().await;
}

/// A request sent from inside the agent session (through `nc`, an ordinary command) is recorded as the agent,
/// even when it claims to be the user and the ticket variable is unset.
#[tokio::test(flavor = "multi_thread")]
async fn requests_from_an_agent_session_act_as_the_agent() {
    let w = World::new().await;
    let out = w.home.path().join("reply");
    let request = r#"{"protocol_version":1,"id":1,"caller":"nevzat","request":{"method":"ticket.update","params":{"key":"SHOP-1","title":"Split the full name"}}}"#;
    let script = format!(
        "printf '%s\\n' '{request}' | env -u {var} nc -U -w 2 '{sock}' > '{out}.tmp'; mv '{out}.tmp' '{out}'; echo >> '{out}'",
        var = naming::ticket_env_var(),
        sock = w.socket().display(),
        out = out.display()
    );
    w.run(SessionKind::Agent, &script).await;
    let reply = read_when_written(&out).await;
    assert!(reply.contains("\"reply\""), "{reply}");
    let ticket = w.board.ticket("SHOP-1").await.unwrap();
    assert_eq!(ticket.title, "Split the full name");
    assert_eq!(
        ticket.updated_by,
        Some(Actor::Agent("SHOP-1".parse().unwrap()))
    );
    w.stop().await;
}

/// A client can't claim another process's PID: the service compares it with the socket's peer PID.
#[tokio::test(flavor = "multi_thread")]
async fn a_claimed_pid_must_be_the_peer() {
    use crewboard_core::protocol::{
        Body, ErrorKind, FrameReader, Message, Request, decode, write_message,
    };
    use tokio::io::BufReader;

    let w = World::new().await;
    let stream = tokio::net::UnixStream::connect(w.socket()).await.unwrap();
    let (read, mut write) = stream.into_split();
    let mut reader = FrameReader::new(BufReader::new(read));

    let honest = Message::request(1, Request::Ping);
    write_message(&mut write, &honest).await.unwrap();
    let frame = reader.next_frame().await.unwrap().unwrap();
    assert!(matches!(decode(frame).unwrap().body, Body::Reply(_)));

    let lying = Message {
        pid: Some(1),
        ..Message::request(2, Request::Ping)
    };
    write_message(&mut write, &lying).await.unwrap();
    let frame = reader.next_frame().await.unwrap().unwrap();
    let Body::Error(error) = decode(frame).unwrap().body else {
        panic!("a foreign PID must be refused")
    };
    assert_eq!(error.kind, ErrorKind::BadRequest);
    drop(write);
    w.stop().await;
}
