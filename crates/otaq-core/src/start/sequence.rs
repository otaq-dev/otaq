//! The start sequence (SES-01, SES-02; d0018, d0007, d0059), in order:
//!
//! 1. resolve the base (the epic's branch for an epic child);
//! 2. sync the base: fetch, fast-forward if only behind, **hold** in Queued if diverged (skipped for an epic
//!    branch that exists; the first child of an epic syncs the epic's base before creating the epic branch);
//! 3. branch `otaq/<KEY>`, worktree and `base_sha`; a new worktree is then seeded from the project's warm build
//!    or the user's checkout ([`crate::warm::seed`], WRM-04), as part of the same step;
//! 4. env files; then the ticket's resources (the project's Create command, d0087); 5. setup command; 6. two ports;
//! 7. launch the agent through its tool's adapter, store the tool and tool id, and set the ticket to Agent
//!    Working.
//!
//! Every step reuses what exists, so the whole sequence can run again (SES-02). A step that fails leaves the
//! ticket in Queued · **Start interrupted** with the step and its message; nothing is rolled back or deleted.
//! Only **Retry start** (a user's click, [`Starter::retry`]) runs it again; no code path re-runs a failed start
//! by itself (P2, d0018). The step a running start is at is stored too, so a start that the service stopping
//! cut short becomes Start interrupted at the next service start ([`Starter::interrupt_cut_short`]) rather
//! than being picked up again by the scheduler.
//!
//! **Returning to Dev** (Send back, Send to agent to fix, a parked ticket dragged back) goes through the same
//! sequence, but the ticket already has its worktree: base sync, env files and setup are skipped while the
//! worktree is there, and step 7 continues the stored conversation (its tool's resume) instead of starting
//! a new one, with the skill still to run (`resume_skill`, or an inbox answer in `resume_prompt`) as its first
//! prompt; both stay stored until the agent's first hook or command shows the conversation got them. If the agent's session is still
//! live, it is reused and the skill waits as an automatic delivery for an empty input line (d0048). A returning
//! ticket whose build was freed at Review gets it back before its ports and agent ([`TicketEnv::fetch_build`],
//! M19-c); a setup that fails there is Start interrupted at the setup step, like a new worktree's.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{Value, json};

use super::{StartError, begin_setup_log, copy_env_files, prepare_worktree, run_setup};
use crate::agent_tools::{Adapters, FirstPrompt, LaunchRequest, LaunchRole, WORK_TICKET_SKILL};
use crate::board::tickets::{Base, EventKind, Ticket, record_event};
use crate::board::{Board, BoardError, Refusal};
use crate::data_dir::DataDir;
use crate::domain::{Actor, BuildState, StartStep, Status};
use crate::git::BaseSync;
use crate::naming;
use crate::ports::PortPool;
use crate::protocol::Event;
use crate::recovery::Recovery;
use crate::service::EventBus;
use crate::session::deliveries::{Deliveries, DeliveryKind};
use crate::session::env::applied;
use crate::session::{
    SessionInfo, SessionKind, SessionTicket, Sessions, SpawnSpec, StoredConversation, TerminalSize,
};
use crate::store::now_ms;
use crate::store::tickets::set_state_since;
use crate::ticket_env::TicketEnv;
use crate::ticket_env::fetch::FetchWhy;
use crate::ticket_env::resources::{Resources, command_vars};
use crate::tools::Os;
use crate::warm::seed::Seeder;

/// The agent's terminal size until a client attaches and resizes it.
const INITIAL_SIZE: TerminalSize = TerminalSize {
    cols: 120,
    rows: 36,
};

/// The statuses a start can begin from: Queued, and Queued while waiting for a slot (d0016).
pub const WAITING_TO_START: &[Status] = &[Status::Queued, Status::WaitingForSlot];

/// The message of a start that the service stopping cut short.
const CUT_SHORT: &str = "the background service stopped during this step";

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
    /// The agent tools. Without a launch context (the plugin can't be found, and in tests that never launch)
    /// the launch step fails.
    adapters: Adapters,
    events: EventBus,
    /// Tickets whose start is running, so one ticket never runs two at once.
    running: Mutex<HashSet<String>>,
    /// Types a returning ticket's skill into its live session (set once the service has its deliveries).
    deliveries: OnceLock<Deliveries>,
    /// Interrupts a ticket whose agent exited before its start finished (set once the service has it).
    recovery: OnceLock<Recovery>,
    /// Seeds new worktrees (set once the service has its warm builds; unset, nothing is seeded).
    seeder: OnceLock<Seeder>,
    /// Brings a returning ticket's freed build back (set once the service has it; unset, nothing is brought back).
    ticket_env: OnceLock<TicketEnv>,
    /// Runs Create where no ticket environment is set (standalone starters).
    resources: Resources,
}

impl Starter {
    pub fn new(
        board: Board,
        data: DataDir,
        ports: PortPool,
        sessions: Sessions,
        adapters: Adapters,
        events: EventBus,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                data,
                ports,
                sessions,
                adapters,
                events,
                running: Mutex::new(HashSet::new()),
                deliveries: OnceLock::new(),
                recovery: OnceLock::new(),
                seeder: OnceLock::new(),
                ticket_env: OnceLock::new(),
                resources: Resources::default(),
            }),
        }
    }

    /// Gives the starter the service's deliveries, for skills a returning ticket still has to run in a live
    /// session.
    pub fn set_deliveries(&self, deliveries: Deliveries) {
        let _ = self.inner.deliveries.set(deliveries);
    }

    /// Gives the starter the service's seeder, for new worktrees (WRM-04).
    pub fn set_seeder(&self, seeder: Seeder) {
        let _ = self.inner.seeder.set(seeder);
    }

    /// Gives the starter the service's ticket environment, for a returning ticket whose build was freed (M19-c).
    pub fn set_ticket_env(&self, ticket_env: TicketEnv) {
        let _ = self.inner.ticket_env.set(ticket_env);
    }

    /// The ticket resource commands: the service's (shared with Template refresh), else the starter's own.
    fn resources(&self) -> &Resources {
        self.inner
            .ticket_env
            .get()
            .map_or(&self.inner.resources, TicketEnv::resources)
    }

    /// Gives the starter the service's recovery, for an agent that exits before its start is recorded.
    pub fn set_recovery(&self, recovery: Recovery) {
        let _ = self.inner.recovery.set(recovery);
    }

    /// Runs the whole start sequence for a ticket waiting to start (Queued, or waiting for a slot). The
    /// scheduler decides when (T-0079); this never checks dependencies or limits.
    ///
    /// The ticket is claimed before its status is read, so two callers can't both start it: the second one
    /// either finds the claim or, once the first has finished, a ticket that isn't waiting any more.
    pub async fn start(&self, key: &str, actor: &Actor) -> Result<StartOutcome, BoardError> {
        let id = self.inner.board.ticket(key).await?.id;
        if !self
            .inner
            .running
            .lock()
            .expect("running starts")
            .insert(id.clone())
        {
            return Ok(StartOutcome::Skipped);
        }
        let outcome = match self.inner.board.ticket_by_id(&id).await {
            Ok(ticket) if WAITING_TO_START.contains(&ticket.status) => {
                self.run(&ticket, actor).await
            }
            Ok(_) => Ok(StartOutcome::Skipped),
            Err(e) => Err(e),
        };
        self.inner
            .running
            .lock()
            .expect("running starts")
            .remove(&id);
        outcome
    }

    /// The ids of the tickets whose start sequence is running now.
    pub fn starting(&self) -> HashSet<String> {
        self.inner.running.lock().expect("running starts").clone()
    }

    /// Marks the step a start is at, so a start cut short by the service stopping (or crashing) is found at the
    /// next service start and shown as Start interrupted instead of being run again silently (SES-02).
    async fn mark_step(&self, ticket: &Ticket, step: StartStep) {
        let result = sqlx::query(
            "UPDATE tickets SET start_problem_step = ?, start_problem_message = NULL WHERE id = ?",
        )
        .bind(step.as_str())
        .bind(&ticket.id)
        .execute(self.inner.board.store().pool())
        .await;
        if let Err(e) = result {
            tracing::warn!(ticket = %ticket.key, error = %e, "can't record the start step");
        }
    }

    /// At service start: tickets still waiting to start with a step marked had their start cut short when the
    /// previous service stopped. They become Start interrupted at that step; only Retry start runs them again.
    pub async fn interrupt_cut_short(board: &Board) -> Result<u64, BoardError> {
        let now = now_ms();
        let result = sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, start_problem_message = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE state IN (?, ?) AND start_problem_step IS NOT NULL AND start_problem_message IS NULL"
        ))
        .bind(Status::StartInterrupted.column().as_str())
        .bind(Status::StartInterrupted.as_str())
        .bind(CUT_SHORT)
        .bind(now)
        .bind(Status::StartInterrupted.as_str())
        .bind(now)
        .bind(Status::Queued.as_str())
        .bind(Status::WaitingForSlot.as_str())
        .execute(board.store().pool())
        .await?;
        Ok(result.rows_affected())
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
                        "tool": session.tool,
                        "tool_session_id": session.tool_session_id,
                    }),
                )
                .await?;
                // The start is the agent's first sign of life (the card's "active … ago", ATT-05). A returning
                // ticket is no longer parked. Its first prompt (a Send back's skill, an inbox answer) stays until
                // the agent shows it got it ([`Board::first_prompt_delivered`]): a resume that can't find its
                // conversation never ran it, and Retry or Resume must send it again (T-0086, T-0112). A child
                // that joined its epic's queue isn't "new" any more once it started (EPC-13).
                sqlx::query(
                    "UPDATE tickets SET last_signal_at = ?, parked = FALSE, reopened = FALSE, return_mode = NULL,
                                        joined_queue_by = NULL
                     WHERE id = ?",
                )
                    .bind(now_ms())
                    .bind(&ticket.id)
                    .execute(self.inner.board.store().pool())
                    .await?;
                tracing::info!(ticket = %key, session = %session.id, "agent started");
                // An agent that exited before the ticket was Working wasn't seen as an exit (d0018): check now.
                if let (Ok(now), Some(recovery)) = (
                    self.inner.sessions.info(&session.id),
                    self.inner.recovery.get(),
                ) && !now.is_running()
                    && let Err(e) = recovery.agent_session_ended(&now).await
                {
                    tracing::warn!(ticket = %key, error = %e, "can't record the agent's exit");
                }
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
        // With the project's variables, for the user's hooks (ENV-02).
        let git = &board
            .project_git(&project.key)
            .await
            .map_err(|e| (StartStep::Base, e.into()))?;
        let returning = Returning::of(board, ticket)
            .await
            .map_err(|e| (StartStep::Base, e.into()))?;
        let keep_worktree = returning.is_some()
            && ticket
                .work
                .as_ref()
                .and_then(|w| w.worktree.as_deref())
                .is_some_and(|w| Path::new(w).is_dir());

        // 1. The base, and which base branch to bring up to date.
        self.mark_step(ticket, StartStep::Base).await;
        let to_sync = match &ticket.base {
            _ if keep_worktree => None,
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
        self.mark_step(ticket, StartStep::SyncBase).await;
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
        self.mark_step(ticket, StartStep::Worktree).await;
        // A folder an earlier attempt made is reused, never seeded twice (SES-02).
        let new_worktree = !keep_worktree
            && !inner
                .data
                .worktree_path(&crate::warm::store::repo_name(&project), &key)
                .is_ok_and(|p| p.join(".git").exists());
        let worktree = prepare_worktree(board, &inner.data, &key)
            .await
            .map_err(at(StartStep::Worktree))?;
        if board
            .store()
            .ticket(&ticket.id)
            .await
            .map_err(|e| (StartStep::Worktree, e.into()))?
            .is_some_and(|row| row.reopened)
        {
            self.rebase_reopened(ticket, &worktree, repo, actor)
                .await
                .map_err(at(StartStep::Worktree))?;
        }
        self.publish_changed(ticket).await;
        // Seeding never fails a start (WRM-04).
        let seeded = match inner.seeder.get() {
            Some(seeder) if new_worktree => {
                seeder
                    .seed(board, git, &project, &key, &worktree.path, false)
                    .await
            }
            _ => None,
        };

        // 4 and 5. Env files and the setup command, from the app's settings (d0023).
        self.mark_step(ticket, StartStep::EnvFiles).await;
        let settings = board
            .project_settings(&project.key)
            .await
            .map_err(|e| (StartStep::EnvFiles, e.into()))?;
        if !keep_worktree {
            // The log begins with where the worktree was seeded from, even if the env files fail.
            let log = inner
                .data
                .setup_log_path(&key)
                .map_err(|e| (StartStep::Setup, e.into()))?;
            let first_line = seeded.as_ref().map(|s| s.log_line());
            if let Err(e) = begin_setup_log(&log, first_line.as_deref()) {
                tracing::warn!(ticket = %key, error = %e, "can't begin the setup log");
            }
            copy_env_files(repo, &worktree.path, &settings.env_files)
                .map_err(at(StartStep::EnvFiles))?;
            // The ticket's own resources, before setup so setup can use them (M19-d, d0087).
            self.mark_step(ticket, StartStep::Resources).await;
            self.resources()
                .create(board, &inner.data, &project, ticket, &worktree.path, &log)
                .await
                .map_err(at(StartStep::Resources))?;
            self.mark_step(ticket, StartStep::Setup).await;
            let recorded = board
                .store()
                .ticket(&ticket.id)
                .await
                .map_err(|e| (StartStep::Setup, e.into()))?
                .and_then(|row| row.resource_name);
            let own = command_vars(
                &inner.data,
                &project,
                &ticket.key,
                recorded.as_deref(),
                &settings,
            );
            let project_env = applied(&settings.env_vars, Os::current());
            run_setup(
                &settings.setup_command,
                &worktree.path,
                &log,
                &project_env,
                &own,
            )
            .await
            .map_err(at(StartStep::Setup))?;
            // The worktree was set up afresh: whatever build it has is in it.
            if build_state(ticket) != BuildState::Present {
                board
                    .store()
                    .set_build_state(&ticket.id, BuildState::Present, None)
                    .await
                    .map_err(|e| (StartStep::Setup, e.into()))?;
            }
        } else if let Some(env) = inner.ticket_env.get() {
            // A returning ticket whose build was freed at Review (a full limit or a gone session): it comes back
            // before the agent does (M19-c). Always asked, never judged from `ticket` (read without the ticket's
            // lock): a Free still moving the build out when the ticket was queued again finishes first, and the
            // fetch reads the ticket again under the lock (a build still there costs one read).
            if build_state(ticket) != BuildState::Present {
                self.mark_step(ticket, StartStep::Setup).await;
            }
            env.fetch_build(ticket, FetchWhy::Start)
                .await
                .map_err(at(StartStep::Setup))?;
        }

        // 6. Two ports.
        self.mark_step(ticket, StartStep::Ports).await;
        let ports = inner
            .ports
            .allocate(&ticket.id)
            .await
            .map_err(|e| (StartStep::Ports, StartError::Other(e.to_string())))?;

        // 7. The agent. A session still running from an earlier attempt is reused.
        self.mark_step(ticket, StartStep::Launch).await;
        if let Some(running) = inner.sessions.running_of(&ticket.id, SessionKind::Agent) {
            if let (Some(skill), Some(deliveries)) = (
                returning.as_ref().and_then(|r| r.skill.as_deref()),
                inner.deliveries.get(),
            ) {
                let text = inner
                    .adapters
                    .of_session(running.tool)
                    .skill_reference(skill);
                deliveries
                    .queue_delivery(&running.id, DeliveryKind::Skill, &text)
                    .await
                    .map_err(|e| (StartStep::Launch, StartError::Other(e.to_string())))?;
            }
            return Ok(Done::Started(running));
        }
        let conversation = returning.and_then(|r| {
            let prompt = r.first_prompt();
            r.conversation.map(|c| (c, prompt))
        });
        // A new conversation runs on the new-session tool; a resumed one on the tool and model it ran on (ADP-04).
        let (tool, resume, model, first_prompt) = match conversation {
            Some((c, prompt)) => (Some(c.tool), Some(c.id), c.model, prompt),
            None => (
                None,
                None,
                None,
                Some(FirstPrompt::OtaqSkill {
                    name: WORK_TICKET_SKILL.to_owned(),
                    message: None,
                }),
            ),
        };
        let launched = inner
            .adapters
            .launch(
                tool,
                &LaunchRequest {
                    role: LaunchRole::TicketAgent,
                    cwd: worktree.path.clone(),
                    project: project.key.clone(),
                    ticket: Some(ticket.key.clone()),
                    epic: None,
                    port: ports.agent,
                    first_prompt,
                    resume,
                    model,
                },
            )
            .map_err(|e| (StartStep::Launch, StartError::Other(e.to_string())))?;
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
                program: launched.plan.program,
                args: launched.plan.args,
                cwd: launched.plan.cwd,
                env: launched.plan.env,
                size: INITIAL_SIZE,
                tool: Some(launched.tool),
                tool_session_id: launched.tool_session_id,
                model: launched.model,
            })
            .await
            .map_err(|e| (StartStep::Launch, StartError::Other(e.to_string())))?;
        Ok(Done::Started(session))
    }

    /// A reopened ticket's old branch is first rebased onto the current base with no tokens (GIT-06; the
    /// checkpoint rule keeps uncommitted files). Clean: the new `base_sha` is recorded. A conflict leaves the
    /// branch as it was and records "conflicts with `main`" for `otaq prime`; the agent still starts. A
    /// hook that refuses the checkpoint stops the start (GIT-02).
    async fn rebase_reopened(
        &self,
        ticket: &Ticket,
        worktree: &super::Worktree,
        repo: &Path,
        actor: &Actor,
    ) -> Result<(), StartError> {
        use crate::board::conflicts::{Conflict, ConflictReason};
        use crate::git::checkpoint::{RebaseError, RebaseOutcome, checkpoint_rejected_message};
        let board = &self.inner.board;
        let git = &board.project_git(ticket.key.project.as_str()).await?;
        let onto = super::worktree::branch_commit(git, repo, &worktree.base_branch).await?;
        let outcome = match git.rebase_with_checkpoint(&worktree.path, &onto).await {
            Ok(outcome) => outcome,
            Err(RebaseError::CheckpointRejected { .. }) => {
                return Err(StartError::Other(checkpoint_rejected_message(
                    &ticket.key.to_string(),
                )));
            }
            Err(RebaseError::Git(e)) => return Err(e.into()),
        };
        let payload = match outcome {
            RebaseOutcome::Clean => {
                sqlx::query("UPDATE tickets SET base_sha = ? WHERE id = ?")
                    .bind(&onto)
                    .bind(&ticket.id)
                    .execute(board.store().pool())
                    .await
                    .map_err(crate::store::StoreError::from)?;
                board.clear_conflict(&ticket.id).await?;
                json!({ "onto": worktree.base_branch, "sha": onto })
            }
            RebaseOutcome::RebaseConflict { files } | RebaseOutcome::UndoConflict { files } => {
                board
                    .set_conflict(
                        &ticket.id,
                        &Conflict {
                            target: worktree.base_branch.clone(),
                            files: files.clone(),
                            reason: ConflictReason::Reopen,
                        },
                    )
                    .await?;
                json!({ "onto": worktree.base_branch, "conflict": files })
            }
        };
        let mut tx = board
            .store()
            .begin_write()
            .await
            .map_err(crate::store::StoreError::from)?;
        record_event(&mut tx, &ticket.id, EventKind::Rebased, actor, payload).await?;
        tx.commit().await.map_err(crate::store::StoreError::from)?;
        Ok(())
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
        let mut tx = board.store().begin_write().await?;
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
            let mut tx = self.inner.board.store().begin_write().await?;
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

/// Where the ticket's build was when it was read.
fn build_state(ticket: &Ticket) -> BuildState {
    ticket
        .work
        .as_ref()
        .map(|w| w.build_state)
        .unwrap_or_default()
}

enum Done {
    Started(SessionInfo),
    Held(String),
}

/// A ticket coming back to Dev with its work: parked, sent back to its agent with a skill to run, or resumed
/// after it was interrupted (or started from scratch in the same worktree).
struct Returning {
    /// The skill to run once the agent is live (`address-review`, `restack`); `None` for a parked ticket.
    skill: Option<String>,
    /// The user's answer from the inbox to an Interrupted question (ATT-08), sent as the resumed
    /// conversation's first prompt.
    prompt: Option<String>,
    /// The conversation of its latest agent session, continued with its tool's resume.
    conversation: Option<StoredConversation>,
}

impl Returning {
    /// What the resumed conversation gets first: the inbox answer, else the skill.
    fn first_prompt(&self) -> Option<FirstPrompt> {
        self.prompt.clone().map(FirstPrompt::Text).or_else(|| {
            self.skill.clone().map(|name| FirstPrompt::OtaqSkill {
                name,
                message: None,
            })
        })
    }
}

impl Returning {
    async fn of(board: &Board, ticket: &Ticket) -> Result<Option<Self>, BoardError> {
        let Some(row) = board.store().ticket(&ticket.id).await? else {
            return Ok(None);
        };
        if row.resume_skill.is_none() && !row.parked && row.return_mode.is_none() {
            return Ok(None);
        }
        // Start from scratch: the same worktree, a new conversation.
        if row.return_mode.as_deref() == Some(crate::recovery::RETURN_SCRATCH) {
            return Ok(Some(Self {
                skill: None,
                prompt: None,
                conversation: None,
            }));
        }
        let conversation =
            StoredConversation::latest_agent(board.store().pool(), &ticket.id).await?;
        Ok(Some(Self {
            skill: row.resume_skill,
            prompt: row.resume_prompt,
            conversation,
        }))
    }
}
