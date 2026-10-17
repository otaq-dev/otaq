//! A background service in a temporary data folder, for tests (d0056), and **agent shells**: an ordinary `sh`
//! running as a ticket's agent session, so commands typed into it reach the service as that agent (CLI-01)
//! without any real `claude`.
//!
//! ```ignore
//! let mut world = TestService::start().await;
//! world.add_project("shop-web").await;
//! let shop1 = world.create("SHOP", "Split user full name").await;
//! let agent = world.agent_shell(&shop1).await;
//! let ran = agent.run("otaq show").await;
//! ```

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use tempfile::TempDir;

use super::{Service, ServiceConfig, Stopper};
use crate::attention::Inbox;
use crate::board::Board;
use crate::board::projects::AddProject;
use crate::board::tickets::{NewTicketInput, Ticket};
use crate::data_dir::DataDir;
use crate::domain::{Actor, Status};
use crate::git::testing::TempRepos;
use crate::naming;
use crate::recovery::Recovery;
use crate::recovery::cleanup::Cleanup;
use crate::review::Review;
use crate::session::deliveries::Deliveries;
use crate::session::{SessionKind, SessionTicket, Sessions, SpawnSpec, TerminalSize};
use crate::signals::Signals;
use crate::start::Starter;
use crate::store::projects::Project;
use crate::store::tickets::set_state_since;
use crate::usage::Usage;

/// A running service with its own data folder and repositories. Stop it with [`TestService::stop`].
pub struct TestService {
    pub home: TempDir,
    pub data: DataDir,
    pub repos: TempRepos,
    pub board: Board,
    pub sessions: Sessions,
    pub deliveries: Deliveries,
    pub signals: Signals,
    pub review: Review,
    pub starter: Starter,
    pub recovery: Recovery,
    pub inbox: Inbox,
    pub cleanup: Cleanup,
    pub usage: Usage,
    pub plan_limit: crate::usage::limit::PlanLimit,
    pub runs: crate::terminals::run::Runs,
    pub suggester: crate::terminals::suggest::Suggester,
    pub shells: crate::terminals::shell::Shells,
    pub prs: crate::pr::service::Prs,
    pub messages: crate::messages::Messages,
    pub validation: crate::validation::Validation,
    pub project_agents: crate::project_agents::ProjectAgents,
    pub warm: crate::warm::store::WarmBuilds,
    pub adapters: crate::agent_tools::Adapters,
    pub events: super::EventBus,
    stopper: Stopper,
    task: tokio::task::JoinHandle<()>,
}

impl TestService {
    pub async fn start() -> Self {
        Self::start_with(|config| config).await
    }

    /// Starts with a changed configuration.
    pub async fn start_with(configure: impl FnOnce(ServiceConfig) -> ServiceConfig) -> Self {
        // Short: the socket path must fit a Unix socket address.
        let home = TempDir::new().expect("temporary folder");
        Self::start_in(home, TempRepos::new(), configure).await
    }

    /// Adds a repository with one commit on `main` as a project (`shop-web` gets the key SHOP).
    pub async fn add_project(&self, name: &str) -> Project {
        let repo = self.repos.repo(name, 1);
        self.board
            .add_project(&AddProject {
                path: repo.display().to_string(),
                ..Default::default()
            })
            .await
            .expect("project added")
    }

    /// Creates a ticket in Backlog as the user.
    pub async fn create(&self, project: &str, title: &str) -> Ticket {
        self.board
            .create_ticket(
                &NewTicketInput {
                    project: project.into(),
                    title: title.into(),
                    ..Default::default()
                },
                &Actor::User("me".into()),
            )
            .await
            .expect("ticket created")
    }

    /// The sample board of `design-briefs/00-README.md`, as far as the CLI tests need it: project SHOP
    /// (`shop-web`), fillers SHOP-1…11 and 29…39 (Backlog), SHOP-12…SHOP-28 with the brief's titles, statuses and
    /// links (SHOP-20 depends on SHOP-18 and SHOP-19, SHOP-21 on SHOP-20), and the epic SHOP-40 Wishlist with
    /// SHOP-40.1 (Merged), 40.2 (Working, depends on 40.1) and 40.3 (Backlog).
    pub async fn shop_sample(&self) -> Project {
        let project = self.add_project("shop-web").await;
        let me = Actor::User("me".into());
        for _ in 1..12 {
            self.create("SHOP", "filler").await;
        }
        let rows: [(&str, Status); 17] = [
            ("Add dark mode toggle", Status::AsksYou),
            ("Seed demo data", Status::Backlog),
            ("Fix cart rounding", Status::Backlog),
            ("Product images", Status::Backlog),
            ("Coupon codes", Status::Backlog),
            ("Add Stripe client", Status::Merged),
            ("Checkout API", Status::ReadyForReview),
            ("Price table", Status::Working),
            ("Checkout page", Status::Queued),
            ("Order confirmation email", Status::Queued),
            ("Fix flaky login redirect", Status::NeedsYou),
            ("Split user full name", Status::AsksYou),
            ("Update footer links", Status::Backlog),
            ("Upgrade to Next.js 15", Status::Interrupted),
            ("Add sitemap", Status::PrOpen),
            ("Search results paging", Status::Working),
            ("Cart badge count", Status::StartInterrupted),
        ];
        for (title, status) in rows {
            let ticket = self.create("SHOP", title).await;
            self.force_status(&ticket, status).await;
        }
        for (child, parent) in [
            ("SHOP-20", "SHOP-18"),
            ("SHOP-20", "SHOP-19"),
            ("SHOP-21", "SHOP-20"),
        ] {
            self.board
                .add_dependency(child, parent, &me)
                .await
                .expect("link added");
        }
        for _ in 29..40 {
            self.create("SHOP", "filler").await;
        }
        let epic = self
            .board
            .create_ticket(
                &NewTicketInput {
                    project: "SHOP".into(),
                    title: "Wishlist".into(),
                    kind: crate::domain::Kind::Epic,
                    external_id: Some("PAY-123".into()),
                    ..Default::default()
                },
                &me,
            )
            .await
            .expect("epic created");
        for (title, status) in [
            ("Wishlist API", Status::Merged),
            ("Wishlist button", Status::Working),
            ("Wishlist page", Status::Backlog),
        ] {
            let child = self
                .board
                .create_ticket(
                    &NewTicketInput {
                        project: "SHOP".into(),
                        title: title.into(),
                        epic: Some(epic.key.to_string()),
                        ..Default::default()
                    },
                    &me,
                )
                .await
                .expect("child created");
            self.force_status(&child, status).await;
        }
        self.board
            .add_dependency("SHOP-40.2", "SHOP-40.1", &me)
            .await
            .expect("link added");
        project
    }

    /// Puts a ticket straight into `status` (and its column), as if the board had got it there.
    pub async fn force_status(&self, ticket: &Ticket, status: Status) {
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(status.column().as_str())
        .bind(status.as_str())
        .bind(status.as_str())
        .bind(crate::store::now_ms())
        .bind(&ticket.id)
        .execute(self.board.store().pool())
        .await
        .expect("status set");
    }

    /// Gives `ticket` a worktree: a fresh repository with one commit on `main`, standing in for the one the start
    /// sequence makes. Returns its folder.
    pub async fn give_worktree(&self, ticket: &Ticket) -> PathBuf {
        let worktree = self.repos.repo(&format!("wt-{}", ticket.key), 1);
        sqlx::query("UPDATE tickets SET worktree_path = ? WHERE id = ?")
            .bind(worktree.display().to_string())
            .bind(&ticket.id)
            .execute(self.board.store().pool())
            .await
            .expect("worktree set");
        worktree
    }

    /// Puts the ticket in Validating with an open round 1 judged by `session` (tests report the verdict from it).
    pub async fn validating_with(&self, ticket: &Ticket, session: &str) {
        self.force_status(ticket, Status::Validating).await;
        sqlx::query(
            "INSERT INTO validations (id, ticket_id, round, trigger, session_id, started_at) VALUES (?, ?, 1, 'agent', ?, ?)",
        )
        .bind(crate::store::new_id())
        .bind(&ticket.id)
        .bind(session)
        .bind(crate::store::now_ms())
        .execute(self.board.store().pool())
        .await
        .expect("round opened");
    }

    /// The ticket's status now.
    pub async fn status(&self, key: &str) -> Status {
        self.board.ticket(key).await.expect("ticket").status
    }

    /// An `sh` running as `ticket`'s agent session, in the project's repository, with `$OTAQ_TICKET` and
    /// `$OTAQ_HOME` set like an agent's.
    pub async fn agent_shell(&self, ticket: &Ticket) -> AgentShell {
        self.shell(ticket, SessionKind::Agent).await
    }

    /// An `sh` running as a session of `kind` for `ticket`.
    pub async fn shell(&self, ticket: &Ticket, kind: SessionKind) -> AgentShell {
        let project = self
            .board
            .project(ticket.key.project.as_str())
            .await
            .expect("project");
        let dir = TempDir::new().expect("temporary folder");
        let info = self
            .sessions
            .spawn(SpawnSpec {
                kind,
                project_id: project.id.clone(),
                project_key: project.key.clone(),
                ticket: Some(SessionTicket {
                    id: ticket.id.clone(),
                    key: ticket.key.clone(),
                }),
                program: "sh".into(),
                args: vec![],
                cwd: PathBuf::from(&project.repo_path),
                env: vec![
                    (naming::ticket_env_var(), ticket.key.to_string()),
                    (DataDir::env_var(), self.data.root().display().to_string()),
                    ("PS1".into(), String::new()),
                    ("PATH".into(), path_without_build_folders()),
                ],
                size: TerminalSize {
                    cols: 400,
                    rows: 50,
                },
                tool: None,
                tool_session_id: None,
                model: None,
            })
            .await
            .expect("session starts");
        AgentShell {
            sessions: self.sessions.clone(),
            session: info.id,
            dir,
            next: AtomicU32::new(0),
        }
    }

    /// Types commands into an existing session whose program is `sh` (a Project agent's stand-in, say).
    pub fn shell_in(&self, session: &str) -> AgentShell {
        AgentShell {
            sessions: self.sessions.clone(),
            session: session.to_owned(),
            dir: TempDir::new().expect("temporary folder"),
            next: AtomicU32::new(0),
        }
    }

    /// Waits until the session's output contains `needle` (30 s at most) and returns the output so far.
    pub async fn wait_for_output(&self, session: &str, needle: &str) -> String {
        let sessions = self.sessions.clone();
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let attached = sessions.attach(session).expect("session in memory");
                let text = String::from_utf8_lossy(&attached.scrollback).into_owned();
                if text.contains(needle) {
                    return text;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("the session never printed {needle:?}"))
    }

    /// Gives `ticket` its branch and worktree as the start sequence does; returns the worktree.
    pub async fn start_worktree(&self, ticket: &Ticket) -> PathBuf {
        crate::start::prepare_worktree(&self.board, &self.data, &ticket.key.to_string())
            .await
            .expect("worktree prepared")
            .path
    }

    /// Records an ended Claude Code agent session of `ticket` with `tool_session_id`, as an earlier start would
    /// have.
    pub async fn record_ended_agent(&self, ticket: &Ticket, tool_session_id: &str) {
        let project = self
            .board
            .project(ticket.key.project.as_str())
            .await
            .expect("project");
        sqlx::query(
            "INSERT INTO sessions (id, project_id, ticket_id, kind, tool, tool_session_id, state, started_at,
                                   ended_at)
             VALUES (?, ?, ?, 'agent', ?, ?, 'ended', ?, ?)",
        )
        .bind(crate::store::new_id())
        .bind(&project.id)
        .bind(&ticket.id)
        .bind(crate::agent_tools::Tool::Claude.as_str())
        .bind(tool_session_id)
        .bind(crate::store::now_ms() - 1000)
        .bind(crate::store::now_ms())
        .execute(self.board.store().pool())
        .await
        .expect("session recorded");
    }

    pub async fn stop(self) {
        self.stopper.stop();
        self.task.await.expect("service task");
    }

    /// Stops the service cleanly and starts a new one on the same data folder and repositories, as a
    /// user-clicked restart does (the startup reconcile runs).
    pub async fn restart(self) -> Self {
        self.restart_with(|config| config).await
    }

    /// [`restart`](Self::restart) with a changed configuration.
    pub async fn restart_with(
        self,
        configure: impl FnOnce(ServiceConfig) -> ServiceConfig,
    ) -> Self {
        let Self {
            home,
            data,
            repos,
            stopper,
            task,
            ..
        } = self;
        stopper.stop();
        task.await.expect("service task");
        Self::bind_in(home, data, repos, configure).await
    }

    /// Starts a service on an existing data folder (after a crash, say).
    pub async fn start_in(
        home: TempDir,
        repos: TempRepos,
        configure: impl FnOnce(ServiceConfig) -> ServiceConfig,
    ) -> Self {
        let data = DataDir::new(home.path().join("h"));
        Self::bind_in(home, data, repos, configure).await
    }

    async fn bind_in(
        home: TempDir,
        data: DataDir,
        repos: TempRepos,
        configure: impl FnOnce(ServiceConfig) -> ServiceConfig,
    ) -> Self {
        let service = Service::bind(configure(ServiceConfig::new(data.clone())))
            .await
            .expect("service starts");
        let inbox = service.inbox();
        let cleanup = service.cleanup();
        let usage = service.usage();
        let plan_limit = service.plan_limit();
        let runs = service.runs();
        let suggester = service.suggester();
        let shells = service.shells();
        let prs = service.prs();
        let messages = service.messages();
        let validation = service.validation();
        let project_agents = service.project_agents();
        let warm = service.warm();
        let adapters = service.adapters();
        let events = service.events();
        let (board, sessions, deliveries, signals, review, starter, recovery, stopper) = (
            service.board(),
            service.sessions(),
            service.deliveries(),
            service.signals(),
            service.review(),
            service.starter(),
            service.recovery(),
            service.stopper(),
        );
        let task = tokio::spawn(service.serve());
        Self {
            home,
            data,
            repos,
            board,
            sessions,
            deliveries,
            signals,
            review,
            starter,
            recovery,
            inbox,
            cleanup,
            usage,
            plan_limit,
            runs,
            suggester,
            shells,
            prs,
            messages,
            validation,
            project_agents,
            warm,
            adapters,
            events,
            stopper,
            task,
        }
    }
}

/// The test's PATH without cargo's build folders, for every stand-in agent that is a plain `sh` (an agent shell, or
/// a `LaunchContext` whose `claude` is `sh`): a message typed into it runs as commands. On Windows, cargo puts the
/// build folders (`target/debug`, with the real `otaq`, and its `deps`) on PATH for the tests; without them, as on
/// macOS and Linux, a message's `otaq answer … --stdin` (or a lead's `otaq note … --stdin`) fails at once instead of
/// reading the next command typed into the shell as its input.
pub fn path_without_build_folders() -> String {
    let test = std::env::current_exe().expect("the test's path");
    let build: Vec<PathBuf> = test
        .ancestors()
        .skip(1)
        .take(2)
        .map(Path::to_path_buf)
        .collect();
    let path = std::env::var_os("PATH").unwrap_or_default();
    let kept = std::env::split_paths(&path).filter(|dir| !build.contains(dir));
    std::env::join_paths(kept)
        .expect("PATH entries")
        .to_string_lossy()
        .into_owned()
}

/// A shell inside a session. Each [`run`](AgentShell::run) types one command line into it.
pub struct AgentShell {
    sessions: Sessions,
    pub session: String,
    dir: TempDir,
    next: AtomicU32,
}

/// What a command printed, and how it exited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl AgentShell {
    /// Runs `command` (shell syntax) in the session and waits for it.
    pub async fn run(&self, command: &str) -> Ran {
        self.run_with_stdin(command, "").await
    }

    /// Sends one raw protocol line to the service at `socket` from inside the session, with an ordinary
    /// command (`nc -U`; on Windows a PowerShell pipe client), and returns what it printed: the answer line.
    pub async fn send_raw(&self, socket: &Path, line: &str) -> Ran {
        if cfg!(windows) {
            let n = self.next.load(Ordering::SeqCst);
            let script = self.dir.path().join(format!("request.{n}.ps1"));
            let name = crate::ipc::endpoint_name(socket);
            std::fs::write(
                &script,
                format!(
                    "$pipe = New-Object System.IO.Pipes.NamedPipeClientStream('.', '{name}', \
                     [System.IO.Pipes.PipeDirection]::InOut)\n\
                     $pipe.Connect(5000)\n\
                     $writer = New-Object System.IO.StreamWriter($pipe)\n\
                     $writer.AutoFlush = $true\n\
                     $reader = New-Object System.IO.StreamReader($pipe)\n\
                     $writer.WriteLine('{line}')\n\
                     [Console]::Out.WriteLine($reader.ReadLine())\n",
                    line = line.replace('\'', "''"),
                ),
            )
            .expect("request script");
            self.run(&format!(
                "powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File '{}'",
                script.display()
            ))
            .await
        } else {
            self.run(&format!(
                "printf '%s\\n' '{line}' | nc -U -w 1 '{}'",
                socket.display()
            ))
            .await
        }
    }

    /// Runs `command` with `stdin` as its standard input.
    pub async fn run_with_stdin(&self, command: &str, stdin: &str) -> Ran {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        let file = |name: &str| self.dir.path().join(format!("{name}.{n}"));
        std::fs::write(file("in"), stdin).expect("stdin file");
        // The command goes into a script and the shell is typed only the short line that sources it. Git for
        // Windows' `sh` loses typeahead when it falls behind (tests in parallel, each with several sessions): of a
        // long line only the first hundred-odd characters arrived, which left the shell inside an open quote.
        // `set +m`: without job control the shell doesn't move each command into a process group of its own.
        // On macOS the shell and its child both make that group, and under load the loser of that race prints
        // "child setpgid (…): Operation not permitted" into the command's error output.
        std::fs::write(
            file("sh"),
            format!(
                ": > '{started}'\nset +m; {{ {command} ; }} < '{input}' > '{out}' 2> '{err}'; echo $? > '{tmp}'; \
                 mv '{tmp}' '{status}'\n",
                started = file("started").display(),
                input = file("in").display(),
                out = file("out").display(),
                err = file("err").display(),
                tmp = file("tmp").display(),
                status = file("status").display(),
            ),
        )
        .expect("command script");
        let line = format!(
            ". '{script}'{enter}",
            script = file("sh").display(),
            // A Unix terminal turns Return into a newline; a Windows console passes Return on as it is.
            enter = if cfg!(windows) { "\r" } else { "\n" },
        );
        self.sessions
            .write(&self.session, line.as_bytes())
            .expect("typed into the shell");
        // A line that still didn't arrive whole never started: Ctrl+C drops what did arrive, and it is typed again.
        for _ in 0..4 {
            if wait_for_file(&file("started"), Duration::from_secs(10))
                .await
                .is_some()
            {
                break;
            }
            self.sessions
                .write(&self.session, b"\x03")
                .expect("typed into the shell");
            tokio::time::sleep(Duration::from_millis(500)).await;
            self.sessions
                .write(&self.session, line.as_bytes())
                .expect("typed into the shell");
        }
        let status = match wait_for_file(&file("status"), Duration::from_secs(30)).await {
            Some(status) => status,
            None => {
                let scrollback = self
                    .sessions
                    .attach(&self.session)
                    .map(|a| String::from_utf8_lossy(&a.scrollback).into_owned())
                    .unwrap_or_default();
                let tail: String = scrollback
                    .chars()
                    .rev()
                    .take(3000)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                panic!(
                    "{} never written (started: {}); the session shows: {tail:?}",
                    file("status").display(),
                    file("started").exists(),
                )
            }
        };
        Ran {
            status: status.trim().parse().expect("exit status"),
            stdout: std::fs::read_to_string(file("out")).unwrap_or_default(),
            stderr: std::fs::read_to_string(file("err")).unwrap_or_default(),
        }
    }
}

async fn wait_for_file(path: &Path, limit: Duration) -> Option<String> {
    tokio::time::timeout(limit, async {
        loop {
            if let Ok(text) = std::fs::read_to_string(path) {
                return text;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .ok()
}
