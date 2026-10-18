//! Failure and recovery (SDD §17; d0018, d0022, d0045): **detect and report, never self-heal**.
//!
//! - **Startup reconcile** (REC-01): when the service starts (after a reboot, a crash or a user-clicked restart),
//!   every ticket whose agent was at work is compared with its session by [`reconcile::reconcile`]. A fresh service
//!   has no sessions yet, so they are all gone: tickets at work become **Interrupted** (needs-you ones keep their label
//!   and question), Ready for Review and Approved tickets only get a History note. Run and Shell sessions end as
//!   "stopped by restart". Starts cut short are Start interrupted already ([`crate::start::Starter::interrupt_cut_short`]),
//!   so reconcile leaves them alone.
//! - **The agent tool exited** (d0018): an agent session that ends on its own while its ticket is at work interrupts the
//!   ticket the same way. Sessions Otaq stopped itself (Park, Close, Done) don't.
//! - **Nothing resumes automatically.** Resume is the user's click (T-0112).
//!
//! Each service start is recorded in `service_runs`; a previous run without a clean stop means this start followed
//! a crash or a reboot (REC-06's summary).

pub mod cleanup;
pub mod reconcile;

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{broadcast, watch};

use crate::agent_tools::{Adapters, Tool};
use crate::board::tickets::{EventKind, record_event};
use crate::board::{Board, BoardError};
use crate::domain::{Actor, StartStep, Status, TicketKey};
use crate::protocol::Event;
use crate::service::EventBus;
use crate::session::{
    END_MODEL_REFUSED, END_SERVICE_STOPPED, END_STOPPED, END_STOPPED_BY_RESTART, SessionInfo,
    SessionKind, Sessions,
};
use crate::store::tickets::set_state_since;
use crate::store::{Store, StoreError, new_id, now_ms};
pub use reconcile::{AT_WORK, Outcome, reconcile, reconcile_validation};

/// How many entries "Before the restart" shows by default.
pub const DEFAULT_ENTRIES: usize = 40;

/// "Before the restart" (REC-02; d0045, d0020): what the Terminal tab of an Interrupted ticket shows, read
/// **read-only** from the agent tool's own saved conversation ([`crate::agent_tools::AgentTool::last_entries`]). Otaq
/// never copies terminal output to disk. A change of the tool's format may break this view, never a workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BeforeRestart {
    /// The last messages and actions, oldest first.
    Available {
        /// The agent tool the conversation ran on.
        tool: Tool,
        tool_session_id: String,
        entries: Vec<TranscriptEntry>,
    },
    /// "Conversation no longer available": no stored tool session id, or the tool has removed (or can't give)
    /// the conversation.
    NotAvailable {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool: Option<Tool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_session_id: Option<String>,
    },
}

/// One line of "Before the restart".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TranscriptEntry {
    /// The user's (or Otaq's) message to the agent.
    User {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
    },
    /// The agent's message.
    Assistant {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
    },
    /// A tool the agent ran: its name and a short summary ("Bash · pnpm test", "Edit · lib/pricing.ts").
    Action {
        tool: String,
        summary: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
    },
}

/// Puts `warm/` in order at start (d0082, d0085): a warm build from before the stock becomes entry `0`, and the
/// `<repo-name>.incoming/` and `<repo-name>.old/` folders and half-made entries a crash left are deleted. They are
/// Otaq's own temporary folders, not leftovers for the user to confirm (REC-04 is about worktrees, branches and
/// sessions). Returns the repos whose trash still holds something, for [`WarmBuilds::empty_trash_later`]: start
/// never waits for the trash.
///
/// [`WarmBuilds::empty_trash_later`]: crate::warm::store::WarmBuilds::empty_trash_later
pub async fn remove_interrupted_warm_work(data: &crate::data_dir::DataDir) -> Vec<String> {
    let data = data.clone();
    match tokio::task::spawn_blocking(move || crate::warm::store::remove_interrupted(&data)).await {
        Ok(found) => {
            if found.removed > 0 || found.migrated > 0 {
                tracing::info!(
                    removed = found.removed,
                    migrated = found.migrated,
                    "interrupted warm-build folders removed"
                );
            }
            found.trash
        }
        Err(_) => Vec::new(),
    }
}

/// "Before the restart" for a ticket (REC-02): the last `limit` messages and actions of its latest agent
/// conversation, read by the adapter of the tool it ran on.
/// For an epic it is its lead's conversation (LEAD-12, d0083: the Lead tab after a restart), which runs in the
/// project's folder. Read-only; never an error that blocks the ticket.
pub async fn before_restart(
    board: &Board,
    adapters: &Adapters,
    key: &str,
    limit: usize,
) -> Result<BeforeRestart, BoardError> {
    let ticket = board.ticket(key).await?;
    let latest: Option<(Tool, Option<String>)> = if ticket.is_epic() {
        let row: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT tool, tool_session_id FROM project_agents WHERE epic_id = ? LIMIT 1",
        )
        .bind(&ticket.id)
        .fetch_optional(board.store().pool())
        .await?;
        row.and_then(|(tool, id)| Some((tool.parse().ok()?, id)))
    } else {
        crate::session::StoredConversation::latest_agent(board.store().pool(), &ticket.id)
            .await?
            .map(|c| (c.tool, Some(c.id)))
    };
    let (tool, latest) = latest.unzip();
    let latest = latest.flatten();
    let (Some(tool), Some(session_id)) = (tool, latest.clone()) else {
        return Ok(BeforeRestart::NotAvailable {
            tool,
            tool_session_id: latest,
        });
    };
    let cwd = match ticket
        .work
        .as_ref()
        .and_then(|w| w.worktree.clone())
        .filter(|_| !ticket.is_epic())
    {
        Some(worktree) => std::path::PathBuf::from(worktree),
        None => {
            std::path::PathBuf::from(board.project(ticket.key.project.as_str()).await?.repo_path)
        }
    };
    let (adapters, id) = (adapters.clone(), session_id.clone());
    let entries =
        tokio::task::spawn_blocking(move || adapters.get(tool).last_entries(&id, &cwd, limit))
            .await
            .ok()
            .flatten();
    Ok(match entries {
        Some(entries) => BeforeRestart::Available {
            tool,
            tool_session_id: session_id,
            entries,
        },
        None => BeforeRestart::NotAvailable {
            tool: Some(tool),
            tool_session_id: Some(session_id),
        },
    })
}

/// Why a ticket was interrupted, as written in its History.
pub const REASON_RESTART: &str = "restart";

/// `tickets.return_mode`: the next start continues the stored conversation (Resume, Retry).
pub const RETURN_RESUME: &str = "resume";
/// `tickets.return_mode`: the next start is a new conversation in the same worktree (Start from scratch).
pub const RETURN_SCRATCH: &str = "scratch";

/// The tickets Resume all resumes (d0045): Interrupted Dev tickets only, never Start interrupted, Resume failed,
/// Review or Approved.
pub fn resume_all_eligible<'a>(
    tickets: impl IntoIterator<Item = &'a crate::board::tickets::Ticket>,
) -> Vec<TicketKey> {
    let mut keys: Vec<TicketKey> = tickets
        .into_iter()
        .filter(|t| t.status == Status::Interrupted)
        .map(|t| t.key.clone())
        .collect();
    keys.sort();
    keys
}

/// The previous run of the service, as this start found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviousRun {
    pub started_at: i64,
    /// It stopped cleanly (`otaq daemon stop`, a user-clicked restart). Otherwise it crashed or the machine
    /// rebooted.
    pub clean: bool,
}

/// This run of the service, recorded in `service_runs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub id: String,
    pub previous: Option<PreviousRun>,
}

impl Run {
    /// Records the start of a run and reads the one before it.
    pub async fn begin(store: &Store, build_id: &str) -> Result<Self, StoreError> {
        let previous: Option<(i64, Option<i64>)> = sqlx::query_as(
            "SELECT started_at, stopped_at FROM service_runs ORDER BY started_at DESC, id DESC LIMIT 1",
        )
        .fetch_optional(store.pool())
        .await?;
        let id = new_id();
        sqlx::query("INSERT INTO service_runs (id, pid, build_id, started_at) VALUES (?, ?, ?, ?)")
            .bind(&id)
            .bind(i64::from(std::process::id()))
            .bind(build_id)
            .bind(now_ms())
            .execute(store.pool())
            .await?;
        Ok(Self {
            id,
            previous: previous.map(|(started_at, stopped_at)| PreviousRun {
                started_at,
                clean: stopped_at.is_some(),
            }),
        })
    }

    /// Records a clean stop.
    pub async fn end(&self, store: &Store) -> Result<(), StoreError> {
        sqlx::query("UPDATE service_runs SET stopped_at = ? WHERE id = ?")
            .bind(now_ms())
            .bind(&self.id)
            .execute(store.pool())
            .await?;
        Ok(())
    }
}

/// What this start found (REC-01) and the app summarizes (REC-06).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupReport {
    /// The previous run ended without a clean stop: a crash or a reboot.
    pub unclean: bool,
    /// Tickets that became Interrupted.
    pub interrupted: Vec<TicketKey>,
    /// Ready for Review / Approved tickets whose session ended with the restart (History note only).
    pub session_ended: Vec<TicketKey>,
    /// Run, Shell and Project terminal sessions the restart stopped.
    pub stopped_by_restart: Vec<String>,
    /// Interrupted tickets that were validating: Resume starts a fresh validation (REC-01's M12 row, d0063).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub was_validating: Vec<TicketKey>,
    /// Project agents the restart ended, by id: "Ended by restart · Resume" (d0068).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_agents_ended: Vec<String>,
    /// "New agent"s that never got a request: gone after the restart.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub new_agents_discarded: u32,
    /// Epic leads the restart ended, by id: "Ended by restart · Resume", counted on their own ("1 epic lead
    /// ended", d0083). Not part of Resume all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub leads_ended: Vec<String>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// Told about every Run / Shell / Project terminal session a restart stopped (M9 shows "stopped by restart").
pub type RestartHook = Arc<dyn Fn(&SessionInfo) + Send + Sync>;

/// Startup reconcile and agent exits. Cheap to clone.
#[derive(Clone)]
pub struct Recovery {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    sessions: Sessions,
    /// Reads a resumed session's output for the tool's "conversation not found" (REC-03).
    adapters: Adapters,
    events: EventBus,
    report: Mutex<StartupReport>,
    /// The Run / Shell / Project terminal sessions the last startup found stopped by the restart.
    stopped: Mutex<Vec<SessionInfo>>,
    restart_hooks: Mutex<Vec<RestartHook>>,
}

impl Recovery {
    pub fn new(board: Board, sessions: Sessions, adapters: Adapters, events: EventBus) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                sessions,
                adapters,
                events,
                report: Mutex::new(StartupReport::default()),
                stopped: Mutex::new(Vec::new()),
                restart_hooks: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Registers a hook for the Run / Shell / Project terminal sessions a restart stopped. A hook registered
    /// after [`startup`](Self::startup) is told about that startup's sessions at once.
    pub fn on_stopped_by_restart(&self, hook: RestartHook) {
        let stopped = self.inner.stopped.lock().expect("stopped sessions").clone();
        for info in &stopped {
            hook(info);
        }
        self.inner
            .restart_hooks
            .lock()
            .expect("restart hooks")
            .push(hook);
    }

    /// Adds what the Project agents' startup reconcile did to the report (REC-06, d0068).
    pub fn record_project_agents(&self, agents: &crate::project_agents::StartupAgents) {
        let mut report = self.inner.report.lock().expect("startup report");
        report.project_agents_ended = agents.ended.clone();
        report.new_agents_discarded = agents.discarded;
        report.leads_ended = agents.leads_ended.clone();
    }

    /// What the last [`startup`](Self::startup) found.
    pub fn startup_report(&self) -> StartupReport {
        self.inner.report.lock().expect("startup report").clone()
    }

    /// The startup reconcile (REC-01). Runs once, when the service starts, after the previous run's sessions were
    /// recorded as ended.
    pub async fn startup(
        &self,
        previous: Option<PreviousRun>,
    ) -> Result<StartupReport, BoardError> {
        let inner = &self.inner;
        let pool = inner.board.store().pool();
        let since = previous.map_or(0, |p| p.started_at);
        let mut report = StartupReport {
            unclean: previous.is_some_and(|p| !p.clean),
            ..Default::default()
        };

        let watched: Vec<&str> = AT_WORK
            .iter()
            .chain([&Status::ReadyForReview, &Status::PrOpen])
            .map(|s| s.as_str())
            .collect();
        let sql = format!(
            "SELECT id, state FROM tickets WHERE state IN ({})",
            vec!["?"; watched.len()].join(", ")
        );
        let mut query = sqlx::query_as::<_, (String, String)>(sqlx::AssertSqlSafe(sql));
        for state in &watched {
            query = query.bind(*state);
        }
        for (id, state) in query.fetch_all(pool).await? {
            let Ok(status) = state.parse::<Status>() else {
                continue;
            };
            let alive = inner.sessions.running_of(&id, SessionKind::Agent).is_some();
            match reconcile(status, alive) {
                Outcome::Unchanged => {}
                Outcome::Interrupt { keep_label } => {
                    if let Some(key) = self
                        .interrupt(&id, status, keep_label, json!({ "reason": REASON_RESTART }))
                        .await?
                    {
                        report.interrupted.push(key);
                    }
                }
                Outcome::NoteSessionEnded => {
                    if let Some(key) = self.note_session_ended(&id, since).await? {
                        report.session_ended.push(key);
                    }
                }
                Outcome::InterruptValidation => {}
            }
        }
        // Validating tickets whose validator is gone (REC-01's M12 row).
        let validating: Vec<(String, String)> =
            sqlx::query_as("SELECT id, state FROM tickets WHERE state IN (?, ?)")
                .bind(Status::Validating.as_str())
                .bind(Status::ValidatorNeedsYou.as_str())
                .fetch_all(pool)
                .await?;
        for (id, state) in validating {
            let Ok(status) = state.parse::<Status>() else {
                continue;
            };
            let alive = inner
                .sessions
                .running_of(&id, SessionKind::Validator)
                .is_some();
            if reconcile_validation(status, alive) == Outcome::InterruptValidation
                && let Some(key) = self
                    .interrupt_validation(&id, status, json!({ "reason": REASON_RESTART }))
                    .await?
            {
                report.was_validating.push(key.clone());
                report.interrupted.push(key);
            }
        }

        let stopped: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM sessions WHERE kind IN ('run', 'shell', 'project') AND end_reason = ?
             AND started_at >= ? ORDER BY started_at, id",
        )
        .bind(END_STOPPED_BY_RESTART)
        .bind(since)
        .fetch_all(pool)
        .await?;
        let hooks = inner.restart_hooks.lock().expect("restart hooks").clone();
        let mut infos = Vec::new();
        for (session,) in stopped {
            if let Ok(info) = inner.sessions.stored(&session).await {
                for hook in &hooks {
                    hook(&info);
                }
                infos.push(info);
            }
            report.stopped_by_restart.push(session);
        }
        *inner.stopped.lock().expect("stopped sessions") = infos;

        report.interrupted.sort();
        report.session_ended.sort();
        if !report.interrupted.is_empty() || !report.session_ended.is_empty() {
            tracing::warn!(
                unclean = report.unclean,
                interrupted = report.interrupted.len(),
                session_ended = report.session_ended.len(),
                "startup reconcile"
            );
        }
        *inner.report.lock().expect("startup report") = report.clone();
        Ok(report)
    }

    /// An agent session ended while the service runs. If the agent tool exited on its own (not stopped by Otaq)
    /// while its ticket was at work, the ticket is Interrupted (d0018). Returns the ticket's key when it was.
    pub async fn agent_session_ended(
        &self,
        info: &SessionInfo,
    ) -> Result<Option<TicketKey>, BoardError> {
        if info.kind != SessionKind::Agent || info.is_running() {
            return Ok(None);
        }
        let reason = info.end_reason.as_deref().unwrap_or_default();
        // Ended by Otaq on a refused model: its ticket's state is already set (AGT-05).
        if reason == END_STOPPED || reason == END_SERVICE_STOPPED || reason == END_MODEL_REFUSED {
            return Ok(None);
        }
        let Some(key) = &info.ticket else {
            return Ok(None);
        };
        let ticket = match self.inner.board.ticket(&key.to_string()).await {
            Ok(ticket) => ticket,
            Err(BoardError::NotFound(_)) => return Ok(None),
            Err(e) => return Err(e),
        };
        // A newer session of the ticket runs (a resume started meanwhile): this end changes nothing.
        if let Some(running) = self
            .inner
            .sessions
            .running_of(&ticket.id, SessionKind::Agent)
            && running.id != info.id
        {
            return Ok(None);
        }
        // A resume whose tool can't find the conversation: Resume failed, never a fallback (REC-03).
        let output = self
            .inner
            .sessions
            .attach(&info.id)
            .map(|a| a.scrollback)
            .unwrap_or_default();
        if AT_WORK.contains(&ticket.status)
            && self
                .inner
                .adapters
                .of_session(info.tool)
                .conversation_not_found(&output)
        {
            return self.resume_failed(&ticket.id, ticket.status, info).await;
        }
        // The tool reports its conversation id after start and the agent ended before it did (Codex, ADP-07):
        // there is nothing to resume, so the start failed (Retry start begins a new conversation).
        if AT_WORK.contains(&ticket.status)
            && let Some(tool) = info.tool
            && info.tool_session_id.as_deref().is_none_or(str::is_empty)
        {
            return self
                .start_failed(&ticket.id, ticket.status, tool, info)
                .await;
        }
        match reconcile(ticket.status, false) {
            Outcome::Interrupt { keep_label } => {
                self.interrupt(
                    &ticket.id,
                    ticket.status,
                    keep_label,
                    json!({ "reason": reason, "session": info.id, "tool": info.tool }),
                )
                .await
            }
            Outcome::Unchanged | Outcome::NoteSessionEnded | Outcome::InterruptValidation => {
                Ok(None)
            }
        }
    }

    /// A validator session ended while the service runs, not stopped by Otaq (a crash, the tool exiting):
    /// its validating ticket is Interrupted from Validating (VAL-07).
    pub async fn validator_session_ended(
        &self,
        info: &SessionInfo,
    ) -> Result<Option<TicketKey>, BoardError> {
        if info.kind != SessionKind::Validator || info.is_running() {
            return Ok(None);
        }
        let reason = info.end_reason.as_deref().unwrap_or_default();
        // Ended by Otaq on a refused model: its ticket's state is already set (AGT-05).
        if reason == END_STOPPED || reason == END_SERVICE_STOPPED || reason == END_MODEL_REFUSED {
            return Ok(None);
        }
        let Some(key) = &info.ticket else {
            return Ok(None);
        };
        let ticket = match self.inner.board.ticket(&key.to_string()).await {
            Ok(ticket) => ticket,
            Err(BoardError::NotFound(_)) => return Ok(None),
            Err(e) => return Err(e),
        };
        let own = self
            .inner
            .board
            .open_round(&ticket.id)
            .await?
            .and_then(|(_, _, session, _)| session);
        if own.as_deref() != Some(info.id.as_str()) {
            return Ok(None);
        }
        match reconcile_validation(ticket.status, false) {
            Outcome::InterruptValidation => {
                self.interrupt_validation(
                    &ticket.id,
                    ticket.status,
                    json!({ "reason": reason, "session": info.id, "tool": info.tool }),
                )
                .await
            }
            _ => Ok(None),
        }
    }

    /// Interrupted from Validating: the round ends "interrupted" (not counted); Resume starts a fresh one.
    async fn interrupt_validation(
        &self,
        ticket_id: &str,
        before: Status,
        mut payload: serde_json::Value,
    ) -> Result<Option<TicketKey>, BoardError> {
        payload["validation"] = json!(true);
        let key = self
            .interrupt_as(ticket_id, before, Some(Status::Validating), false, payload)
            .await?;
        if key.is_some() {
            self.inner
                .board
                .abandon_round(ticket_id, crate::domain::ValidationOutcome::Interrupted)
                .await?;
        }
        Ok(key)
    }

    /// Watches session ends until the service stops (agent exits, d0018).
    pub async fn run(self, mut stop: watch::Receiver<bool>) {
        let mut listener = self.inner.events.listen();
        loop {
            let event = tokio::select! {
                _ = stop.wait_for(|s| *s) => return,
                event = listener.receiver.recv() => event,
            };
            match event {
                Ok(Event::SessionState { session }) if !session.is_running() => {
                    if let Err(e) = self.agent_session_ended(&session).await {
                        tracing::warn!(session = session.id, error = %e, "can't record an agent's exit");
                    }
                    if let Err(e) = self.validator_session_ended(&session).await {
                        tracing::warn!(session = session.id, error = %e, "can't record a validator's exit");
                    }
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "recovery fell behind on session events");
                }
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    }

    /// Sets a ticket at work to Interrupted, if it still is in `before`. A needs-you ticket keeps its label (the
    /// status it had), its text and its question.
    async fn interrupt(
        &self,
        ticket_id: &str,
        before: Status,
        keep_label: bool,
        payload: serde_json::Value,
    ) -> Result<Option<TicketKey>, BoardError> {
        let from = keep_label.then_some(before);
        self.interrupt_as(ticket_id, before, from, keep_label, payload)
            .await
    }

    /// Interrupted, remembering `from` (the needs-you status it kept, or Validating) and keeping the text when
    /// `keep_label`.
    async fn interrupt_as(
        &self,
        ticket_id: &str,
        before: Status,
        from: Option<Status>,
        keep_label: bool,
        mut payload: serde_json::Value,
    ) -> Result<Option<TicketKey>, BoardError> {
        let board = &self.inner.board;
        let now = now_ms();
        let mut tx = board.store().begin_write().await?;
        let done = sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, interrupted_from = ?,
                                signal_text = CASE WHEN ? THEN signal_text ELSE NULL END,
                                signal_since = CASE WHEN ? THEN signal_since ELSE NULL END,
                                wakeup_at = NULL, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ? AND state = ?"
        ))
        .bind(Status::Interrupted.column().as_str())
        .bind(Status::Interrupted.as_str())
        .bind(from.map(|s| s.as_str()))
        .bind(keep_label)
        .bind(keep_label)
        .bind(now)
        .bind(Status::Interrupted.as_str())
        .bind(now)
        .bind(ticket_id)
        .bind(before.as_str())
        .execute(&mut *tx)
        .await?;
        if done.rows_affected() == 0 {
            return Ok(None);
        }
        payload["from"] = json!(before);
        record_event(
            &mut tx,
            ticket_id,
            EventKind::Interrupted,
            &Actor::current_user(),
            payload,
        )
        .await?;
        tx.commit().await?;
        let ticket = board.ticket_by_id(ticket_id).await?;
        tracing::info!(ticket = %ticket.key, from = %before, "interrupted");
        self.publish(&ticket.id, &ticket.key, Some(Status::Interrupted));
        Ok(Some(ticket.key))
    }

    /// The agent ended before its tool said its conversation's id: Start interrupted at the launch, with the reason;
    /// Retry start runs the start again.
    async fn start_failed(
        &self,
        ticket_id: &str,
        before: Status,
        tool: Tool,
        info: &SessionInfo,
    ) -> Result<Option<TicketKey>, BoardError> {
        let board = &self.inner.board;
        let message = format!(
            "{} stopped before its conversation started, so there is nothing to resume.",
            self.inner.adapters.get(tool).display_name()
        );
        let now = now_ms();
        let mut tx = board.store().begin_write().await?;
        let done = sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, start_problem_step = ?, start_problem_message = ?,
                                signal_text = NULL, signal_since = NULL, wakeup_at = NULL, interrupted_from = NULL,
                                updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ? AND state = ?"
        ))
        .bind(Status::StartInterrupted.column().as_str())
        .bind(Status::StartInterrupted.as_str())
        .bind(StartStep::Launch.as_str())
        .bind(&message)
        .bind(now)
        .bind(Status::StartInterrupted.as_str())
        .bind(now)
        .bind(ticket_id)
        .bind(before.as_str())
        .execute(&mut *tx)
        .await?;
        if done.rows_affected() == 0 {
            return Ok(None);
        }
        record_event(
            &mut tx,
            ticket_id,
            EventKind::StartInterrupted,
            &Actor::current_user(),
            json!({ "step": StartStep::Launch, "message": message, "session": info.id, "from": before }),
        )
        .await?;
        tx.commit().await?;
        let ticket = board.ticket_by_id(ticket_id).await?;
        tracing::warn!(ticket = %ticket.key, "the agent ended before its tool id was known: start failed");
        self.publish(&ticket.id, &ticket.key, Some(Status::StartInterrupted));
        Ok(Some(ticket.key))
    }

    /// The resumed conversation wasn't found: Resume failed, with Retry and Start from scratch.
    async fn resume_failed(
        &self,
        ticket_id: &str,
        before: Status,
        info: &SessionInfo,
    ) -> Result<Option<TicketKey>, BoardError> {
        let board = &self.inner.board;
        let now = now_ms();
        let mut tx = board.store().begin_write().await?;
        let done = sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, wakeup_at = NULL, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ? AND state = ?"
        ))
        .bind(Status::ResumeFailed.column().as_str())
        .bind(Status::ResumeFailed.as_str())
        .bind(now)
        .bind(Status::ResumeFailed.as_str())
        .bind(now)
        .bind(ticket_id)
        .bind(before.as_str())
        .execute(&mut *tx)
        .await?;
        if done.rows_affected() == 0 {
            return Ok(None);
        }
        record_event(
            &mut tx,
            ticket_id,
            EventKind::ResumeFailed,
            &Actor::current_user(),
            json!({
                "session": info.id,
                "tool": info.tool,
                "tool_session_id": info.tool_session_id,
            }),
        )
        .await?;
        tx.commit().await?;
        let ticket = board.ticket_by_id(ticket_id).await?;
        tracing::warn!(ticket = %ticket.key, "resume failed: conversation not found");
        self.publish(&ticket.id, &ticket.key, Some(Status::ResumeFailed));
        Ok(Some(ticket.key))
    }

    /// History notes "session ended (restart)" for a Review / Approved ticket whose agent session the previous run
    /// ended (once).
    async fn note_session_ended(
        &self,
        ticket_id: &str,
        since: i64,
    ) -> Result<Option<TicketKey>, BoardError> {
        let board = &self.inner.board;
        let ended: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM sessions WHERE ticket_id = ? AND kind = 'agent' AND end_reason = ? AND started_at >= ?
             AND NOT EXISTS (SELECT 1 FROM events e WHERE e.ticket_id = sessions.ticket_id AND e.kind = ?
                             AND e.at >= sessions.ended_at)
             ORDER BY started_at DESC LIMIT 1",
        )
        .bind(ticket_id)
        .bind(END_SERVICE_STOPPED)
        .bind(since)
        .bind(EventKind::SessionEnded.as_str())
        .fetch_optional(board.store().pool())
        .await?;
        let Some((session,)) = ended else {
            return Ok(None);
        };
        let mut tx = board.store().begin_write().await?;
        record_event(
            &mut tx,
            ticket_id,
            EventKind::SessionEnded,
            &Actor::current_user(),
            json!({ "reason": REASON_RESTART, "session": session }),
        )
        .await?;
        tx.commit().await?;
        let ticket = board.ticket_by_id(ticket_id).await?;
        self.publish(&ticket.id, &ticket.key, None);
        Ok(Some(ticket.key))
    }

    fn publish(&self, id: &str, key: &TicketKey, new_status: Option<Status>) {
        let project = key.project.to_string();
        self.inner.events.publish(Event::TicketChanged {
            project: project.clone(),
            id: id.to_owned(),
            key: key.clone(),
        });
        if let Some(status) = new_status {
            self.inner.events.publish(Event::CardStatusChanged {
                project,
                id: id.to_owned(),
                key: key.clone(),
                status,
            });
        }
    }
}
