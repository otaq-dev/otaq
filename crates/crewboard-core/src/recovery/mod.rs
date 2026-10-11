//! Failure and recovery (SDD §17; d0018, d0022, d0045): **detect and report, never self-heal**.
//!
//! - **Startup reconcile** (REC-01): when the service starts (after a reboot, a crash or a user-clicked restart),
//!   every ticket whose agent was at work is compared with its session by [`reconcile::reconcile`]. A fresh service
//!   has no sessions yet, so they are all gone: tickets at work become **Interrupted** (orange ones keep their label
//!   and question), Ready for Review and Approved tickets only get a History note. Run and Shell sessions end as
//!   "stopped by restart". Starts cut short are Start interrupted already ([`crate::start::Starter::interrupt_cut_short`]),
//!   so reconcile leaves them alone.
//! - **`claude` exited** (d0018): an agent session that ends on its own while its ticket is at work interrupts the
//!   ticket the same way. Sessions Crewboard stopped itself (Park, Close, Done) don't.
//! - **Nothing resumes automatically.** Resume is the user's click (T-0112).
//!
//! Each service start is recorded in `service_runs`; a previous run without a clean stop means this start followed
//! a crash or a reboot (REC-06's summary).

pub mod cleanup;
pub mod reconcile;
pub mod transcript;

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{broadcast, watch};

use crate::board::tickets::{EventKind, record_event};
use crate::board::{Board, BoardError};
use crate::domain::{Actor, Status, TicketKey};
use crate::protocol::Event;
use crate::service::EventBus;
use crate::session::{
    END_SERVICE_STOPPED, END_STOPPED, END_STOPPED_BY_RESTART, SessionInfo, SessionKind, Sessions,
};
use crate::store::tickets::set_state_since;
use crate::store::{Store, StoreError, new_id, now_ms};
pub use reconcile::{AT_WORK, Outcome, reconcile, reconcile_validation};
pub use transcript::{BeforeRestart, TranscriptEntry};

/// "Before the restart" for a ticket (REC-02): the last `limit` messages and actions of its latest agent
/// conversation, read from Claude Code's folder `claude_dir` (`None`: no folder known, so not available).
/// Read-only; never an error that blocks the ticket.
pub async fn before_restart(
    board: &Board,
    claude_dir: Option<&std::path::Path>,
    key: &str,
    limit: usize,
) -> Result<BeforeRestart, BoardError> {
    let ticket = board.ticket(key).await?;
    let latest: Option<String> = sqlx::query_scalar(
        "SELECT claude_session_id FROM sessions
         WHERE ticket_id = ? AND kind = 'agent' AND claude_session_id IS NOT NULL
         ORDER BY started_at DESC, id DESC LIMIT 1",
    )
    .bind(&ticket.id)
    .fetch_optional(board.store().pool())
    .await?;
    let (Some(session_id), Some(claude_dir)) = (latest.clone(), claude_dir) else {
        return Ok(BeforeRestart::NotAvailable {
            claude_session_id: latest,
        });
    };
    let cwd = match ticket.work.as_ref().and_then(|w| w.worktree.clone()) {
        Some(worktree) => std::path::PathBuf::from(worktree),
        None => {
            std::path::PathBuf::from(board.project(ticket.key.project.as_str()).await?.repo_path)
        }
    };
    let claude_dir = claude_dir.to_owned();
    let id = session_id.clone();
    let entries = tokio::task::spawn_blocking(move || {
        transcript::find_transcript(&claude_dir, &cwd, &id)
            .and_then(|path| transcript::read_transcript(&path, limit))
    })
    .await
    .ok()
    .flatten();
    Ok(match entries {
        Some(entries) => BeforeRestart::Available {
            claude_session_id: session_id,
            entries,
        },
        None => BeforeRestart::NotAvailable {
            claude_session_id: Some(session_id),
        },
    })
}

/// Why a ticket was interrupted, as written in its History.
pub const REASON_RESTART: &str = "restart";

/// `tickets.return_mode`: the next start continues the stored conversation (Resume, Retry).
pub const RETURN_RESUME: &str = "resume";
/// `tickets.return_mode`: the next start is a new conversation in the same worktree (Start from scratch).
pub const RETURN_SCRATCH: &str = "scratch";

/// What `claude --resume <id>` prints when it can't find the conversation, compared without case ("No
/// conversation found with session ID: …"). Checked by hand against the installed CLI (SDD §26).
pub const CONVERSATION_NOT_FOUND: &str = "no conversation found";

/// Whether a session's output says its conversation couldn't be found (REC-03).
pub fn conversation_not_found(output: &[u8]) -> bool {
    String::from_utf8_lossy(output)
        .to_lowercase()
        .contains(CONVERSATION_NOT_FOUND)
}

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
    /// It stopped cleanly (`crewboard daemon stop`, a user-clicked restart). Otherwise it crashed or the machine
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
    events: EventBus,
    report: Mutex<StartupReport>,
    /// The Run / Shell / Project terminal sessions the last startup found stopped by the restart.
    stopped: Mutex<Vec<SessionInfo>>,
    restart_hooks: Mutex<Vec<RestartHook>>,
}

impl Recovery {
    pub fn new(board: Board, sessions: Sessions, events: EventBus) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                sessions,
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

    /// An agent session ended while the service runs. If `claude` exited on its own (not stopped by Crewboard)
    /// while its ticket was at work, the ticket is Interrupted (d0018). Returns the ticket's key when it was.
    pub async fn agent_session_ended(
        &self,
        info: &SessionInfo,
    ) -> Result<Option<TicketKey>, BoardError> {
        if info.kind != SessionKind::Agent || info.is_running() {
            return Ok(None);
        }
        let reason = info.end_reason.as_deref().unwrap_or_default();
        if reason == END_STOPPED || reason == END_SERVICE_STOPPED {
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
        // `claude --resume` that can't find the conversation: Resume failed, never a fallback (REC-03).
        let output = self
            .inner
            .sessions
            .attach(&info.id)
            .map(|a| a.scrollback)
            .unwrap_or_default();
        if AT_WORK.contains(&ticket.status) && conversation_not_found(&output) {
            return self.resume_failed(&ticket.id, ticket.status, info).await;
        }
        match reconcile(ticket.status, false) {
            Outcome::Interrupt { keep_label } => {
                self.interrupt(
                    &ticket.id,
                    ticket.status,
                    keep_label,
                    json!({ "reason": reason, "session": info.id }),
                )
                .await
            }
            Outcome::Unchanged | Outcome::NoteSessionEnded | Outcome::InterruptValidation => {
                Ok(None)
            }
        }
    }

    /// A validator session ended while the service runs, not stopped by Crewboard (a crash, `claude` exiting):
    /// its validating ticket is Interrupted from Validating (VAL-07).
    pub async fn validator_session_ended(
        &self,
        info: &SessionInfo,
    ) -> Result<Option<TicketKey>, BoardError> {
        if info.kind != SessionKind::Validator || info.is_running() {
            return Ok(None);
        }
        let reason = info.end_reason.as_deref().unwrap_or_default();
        if reason == END_STOPPED || reason == END_SERVICE_STOPPED {
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
                    json!({ "reason": reason, "session": info.id }),
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

    /// Sets a ticket at work to Interrupted, if it still is in `before`. An orange ticket keeps its label (the
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

    /// Interrupted, remembering `from` (the orange status it kept, or Validating) and keeping the text when
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
        let mut tx = board.store().pool().begin().await?;
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

    /// The resumed conversation wasn't found: Resume failed, with Retry and Start from scratch.
    async fn resume_failed(
        &self,
        ticket_id: &str,
        before: Status,
        info: &SessionInfo,
    ) -> Result<Option<TicketKey>, BoardError> {
        let board = &self.inner.board;
        let now = now_ms();
        let mut tx = board.store().pool().begin().await?;
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
            json!({ "session": info.id, "claude_session_id": info.claude_session_id }),
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
        let mut tx = board.store().pool().begin().await?;
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
