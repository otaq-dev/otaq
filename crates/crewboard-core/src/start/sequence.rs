//! The start sequence (SES-01, SES-02; d0018, d0007, d0059), in order:
//!
//! 1. resolve the base (the epic's branch for an epic child);
//! 2. sync the base: fetch, fast-forward if only behind, **hold** in Queued if diverged (skipped for an epic
//!    branch that exists; the first child of an epic syncs the epic's base before creating the epic branch);
//! 3. branch `crewboard/<KEY>`, worktree and `base_sha`;
//! 4. env files; 5. setup command; 6. two ports;
//! 7. launch the agent with its Claude session ID and set the ticket to Agent Working.
//!
//! Every step reuses what exists, so the whole sequence can run again (SES-02). A step that fails leaves the
//! ticket in Queued · **Start interrupted** with the step and its message; nothing is rolled back or deleted.
//! Only **Retry start** (a user's click, [`Starter::retry`]) runs it again; no code path re-runs a failed start
//! by itself (P2, d0018).

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::{StartError, copy_env_files, prepare_worktree, run_setup};
use crate::board::tickets::{Base, EventKind, Ticket, record_event};
use crate::board::{Board, BoardError, Refusal};
use crate::data_dir::DataDir;
use crate::domain::{Actor, StartStep, Status};
use crate::git::BaseSync;
use crate::naming;
use crate::ports::PortPool;
use crate::protocol::Event;
use crate::service::EventBus;
use crate::session::launch::{LaunchContext, launch_agent};
use crate::session::{SessionInfo, SessionKind, SessionTicket, Sessions, SpawnSpec, TerminalSize};
use crate::store::now_ms;
use crate::store::tickets::set_state_since;

/// The agent's terminal size until a client attaches and resizes it.
const INITIAL_SIZE: TerminalSize = TerminalSize {
    cols: 120,
    rows: 36,
};

/// How a start ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartOutcome {
    /// Agent Working, with this session.
    Started(SessionInfo),
    /// Held in Queued: the base diverged from its remote.
    Held { message: String },
    /// Start interrupted at `step`.
    Interrupted { step: StartStep, message: String },
    /// The ticket isn't Queued (any more), or a start for it is already running: nothing was done.
    Skipped,
}

/// Runs start sequences. Cheap to clone.
#[derive(Clone)]
pub struct Starter {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    data: DataDir,
    ports: PortPool,
    sessions: Sessions,
    /// `None` when the plugin can't be found (and in tests that never launch): the launch step fails then.
    launch: Option<LaunchContext>,
    events: EventBus,
    /// Tickets whose start is running, so one ticket never runs two at once.
    running: Mutex<HashSet<String>>,
}

impl Starter {
    pub fn new(
        board: Board,
        data: DataDir,
        ports: PortPool,
        sessions: Sessions,
        launch: Option<LaunchContext>,
        events: EventBus,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                data,
                ports,
                sessions,
                launch,
                events,
                running: Mutex::new(HashSet::new()),
            }),
        }
    }

    /// **TEMPORARY trigger** (questions.md Q11.3): until the scheduler exists, a ticket that enters Queued starts
    /// at once, with no readiness check and no limits. **T-0079 (M5) replaces this function and its one call**
    /// (in the service's `ticket.move` handler).
    pub fn start_on_enter_queued(&self, ticket: &Ticket, actor: &Actor) {
        if ticket.status != Status::Queued {
            return;
        }
        let starter = self.clone();
        let key = ticket.key.to_string();
        let actor = actor.clone();
        tokio::spawn(async move {
            if let Err(e) = starter.start(&key, &actor).await {
                tracing::error!(ticket = %key, error = %e, "start failed");
            }
        });
    }

    /// Runs the whole start sequence for a Queued ticket.
    pub async fn start(&self, key: &str, actor: &Actor) -> Result<StartOutcome, BoardError> {
        let ticket = self.inner.board.ticket(key).await?;
        if ticket.status != Status::Queued {
            return Ok(StartOutcome::Skipped);
        }
        if !self
            .inner
            .running
            .lock()
            .expect("running starts")
            .insert(ticket.id.clone())
        {
            return Ok(StartOutcome::Skipped);
        }
        let outcome = self.run(&ticket, actor).await;
        self.inner
            .running
            .lock()
            .expect("running starts")
            .remove(&ticket.id);
        outcome
    }

    /// **Retry start** (the user's click): puts a ticket whose start stopped (Start interrupted, or Held
    /// because its base diverged) back to Queued so the whole sequence runs again. The caller then runs
    /// [`start`](Self::start).
    pub async fn retry(&self, key: &str, actor: &Actor) -> Result<Ticket, BoardError> {
        let ticket = self.inner.board.ticket(key).await?;
        let retryable = ticket.status == Status::StartInterrupted
            || (ticket.status == Status::Held
                && ticket
                    .start_problem
                    .as_ref()
                    .is_some_and(|p| p.step == StartStep::SyncBase));
        if !retryable {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!("{} has no stopped start to retry", ticket.key),
            ));
        }
        self.set_state(
            &ticket,
            Status::Queued,
            None,
            actor,
            EventKind::StartRetried,
            json!({}),
        )
        .await
    }

    async fn run(&self, ticket: &Ticket, actor: &Actor) -> Result<StartOutcome, BoardError> {
        let key = ticket.key.to_string();
        match self.steps(ticket, actor).await {
            Ok(Done::Started(session)) => {
                self.set_state(
                    ticket,
                    Status::Working,
                    None,
                    actor,
                    EventKind::Started,
                    json!({
                        "session": session.id,
                        "claude_session_id": session.claude_session_id,
                    }),
                )
                .await?;
                // The start is the agent's first sign of life (the card's "active … ago", ATT-05).
                sqlx::query("UPDATE tickets SET last_signal_at = ? WHERE id = ?")
                    .bind(now_ms())
                    .bind(&ticket.id)
                    .execute(self.inner.board.store().pool())
                    .await?;
                tracing::info!(ticket = %key, session = %session.id, "agent started");
                Ok(StartOutcome::Started(session))
            }
            Ok(Done::Held(message)) => {
                self.set_state(
                    ticket,
                    Status::Held,
                    Some((StartStep::SyncBase, &message)),
                    actor,
                    EventKind::Held,
                    json!({ "step": StartStep::SyncBase, "message": message }),
                )
                .await?;
                tracing::info!(ticket = %key, %message, "start held");
                Ok(StartOutcome::Held { message })
            }
            Err((step, e)) => {
                let message = e.to_string();
                self.set_state(
                    ticket,
                    Status::StartInterrupted,
                    Some((step, &message)),
                    actor,
                    EventKind::StartInterrupted,
                    json!({ "step": step, "message": message }),
                )
                .await?;
                tracing::warn!(ticket = %key, %step, %message, "start interrupted");
                Ok(StartOutcome::Interrupted { step, message })
            }
        }
    }

    async fn steps(&self, ticket: &Ticket, actor: &Actor) -> Result<Done, (StartStep, StartError)> {
        let inner = &self.inner;
        let board = &inner.board;
        let key = ticket.key.to_string();
        let at = |step: StartStep| move |e: StartError| (step, e);
        let project = board
            .project(ticket.key.project.as_str())
            .await
            .map_err(|e| (StartStep::Base, e.into()))?;
        let repo = Path::new(&project.repo_path);
        let git = board.git().map_err(|e| (StartStep::Base, e.into()))?;

        // 1. The base, and which base branch to bring up to date.
        let to_sync = match &ticket.base {
            Base::Branch(branch) if branch.is_empty() => {
                return Err((
                    StartStep::Base,
                    StartError::Other(format!("{key} has no base branch")),
                ));
            }
            Base::Branch(branch) => Some(branch.clone()),
            Base::Epic(epic) => {
                let epic = board
                    .ticket(&epic.to_string())
                    .await
                    .map_err(|e| (StartStep::Base, e.into()))?;
                let epic_ref = format!("refs/heads/{}", naming::branch(&epic.key.to_string()));
                let exists = git
                    .resolve(repo, &epic_ref)
                    .await
                    .map_err(|e| (StartStep::Base, e.into()))?
                    .is_some();
                match (&epic.base, exists) {
                    // The epic branch is local only (d0059).
                    (_, true) => None,
                    // The first child: the epic's base is synced before the epic branch is made from it.
                    (Base::Branch(branch), false) => Some(branch.clone()),
                    (Base::Epic(_), false) => None,
                }
            }
        };

        // 2. Keep the base current.
        if let Some(branch) = to_sync {
            match git.sync_base(repo, &branch).await {
                Ok(BaseSync::Diverged { message }) => return Ok(Done::Held(message)),
                Ok(BaseSync::CheckoutDirty { message }) => {
                    self.note(
                        ticket,
                        actor,
                        "base_not_updated",
                        json!({ "message": message }),
                    )
                    .await;
                }
                Ok(_) => {}
                Err(e) => return Err((StartStep::SyncBase, e.into())),
            }
        }

        // 3. Branch, worktree, base_sha.
        let worktree = prepare_worktree(board, &inner.data, &key)
            .await
            .map_err(at(StartStep::Worktree))?;
        self.publish_changed(ticket).await;

        // 4 and 5. Env files and the setup command, from the app's settings (d0023).
        let settings = board
            .project_settings(&project.key)
            .await
            .map_err(|e| (StartStep::EnvFiles, e.into()))?;
        copy_env_files(repo, &worktree.path, &settings.env_files)
            .map_err(at(StartStep::EnvFiles))?;
        let log = inner
            .data
            .setup_log_path(&key)
            .map_err(|e| (StartStep::Setup, e.into()))?;
        run_setup(&settings.setup_command, &worktree.path, &log)
            .await
            .map_err(at(StartStep::Setup))?;

        // 6. Two ports.
        let ports = inner
            .ports
            .allocate(&ticket.id)
            .await
            .map_err(|e| (StartStep::Ports, StartError::Other(e.to_string())))?;

        // 7. The agent. A session still running from an earlier attempt is reused.
        if let Some(running) = inner.sessions.running_of(&ticket.id, SessionKind::Agent) {
            return Ok(Done::Started(running));
        }
        let Some(launch) = &inner.launch else {
            return Err((
                StartStep::Launch,
                StartError::Other(format!(
                    "{}'s Claude Code plugin wasn't found next to the {} command",
                    naming::PRODUCT_NAME,
                    naming::COMMAND_NAME
                )),
            ));
        };
        let command = launch_agent(launch, &ticket.key, &worktree.path, ports.agent);
        let session = inner
            .sessions
            .spawn(SpawnSpec {
                kind: SessionKind::Agent,
                project_id: project.id.clone(),
                project_key: project.key.clone(),
                ticket: Some(SessionTicket {
                    id: ticket.id.clone(),
                    key: ticket.key.clone(),
                }),
                program: command.program,
                args: command.args,
                cwd: command.cwd,
                env: command.env,
                size: INITIAL_SIZE,
                claude_session_id: Some(command.claude_session_id),
            })
            .await
            .map_err(|e| (StartStep::Launch, StartError::Other(e.to_string())))?;
        Ok(Done::Started(session))
    }

    /// Sets the ticket's status and start problem, records the event and tells clients.
    async fn set_state(
        &self,
        ticket: &Ticket,
        status: Status,
        problem: Option<(StartStep, &str)>,
        actor: &Actor,
        kind: EventKind,
        payload: Value,
    ) -> Result<Ticket, BoardError> {
        let board = &self.inner.board;
        let now = now_ms();
        let mut tx = board.store().pool().begin().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, start_problem_step = ?, start_problem_message = ?,
                                updated_by = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(status.column().as_str())
        .bind(status.as_str())
        .bind(problem.map(|(step, _)| step.as_str()))
        .bind(problem.map(|(_, message)| message))
        .bind(actor.to_string())
        .bind(now)
        .bind(status.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        record_event(&mut tx, &ticket.id, kind, actor, payload).await?;
        tx.commit().await?;
        let after = board.ticket_by_id(&ticket.id).await?;
        self.publish(&after, ticket.status != after.status);
        Ok(after)
    }

    /// Records a note in the ticket's history without changing its status.
    async fn note(&self, ticket: &Ticket, actor: &Actor, what: &str, payload: Value) {
        let result = async {
            let mut tx = self.inner.board.store().pool().begin().await?;
            let mut payload = payload;
            payload["note"] = json!(what);
            record_event(&mut tx, &ticket.id, EventKind::StartNote, actor, payload).await?;
            tx.commit().await?;
            Ok::<_, BoardError>(())
        }
        .await;
        if let Err(e) = result {
            tracing::warn!(ticket = %ticket.key, error = %e, "can't record a start note");
        }
    }

    async fn publish_changed(&self, ticket: &Ticket) {
        if let Ok(after) = self.inner.board.ticket_by_id(&ticket.id).await {
            self.publish(&after, false);
        }
    }

    fn publish(&self, ticket: &Ticket, status_changed: bool) {
        let project = ticket.key.project.to_string();
        self.inner.events.publish(Event::TicketChanged {
            project: project.clone(),
            id: ticket.id.clone(),
            key: ticket.key.clone(),
        });
        if status_changed {
            self.inner.events.publish(Event::CardStatusChanged {
                project,
                id: ticket.id.clone(),
                key: ticket.key.clone(),
                status: ticket.status,
            });
        }
    }
}

enum Done {
    Started(SessionInfo),
    Held(String),
}
