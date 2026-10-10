//! The PTY session manager (SES-04, SVC-01, SVC-05; d0022).
//!
//! The background service owns every PTY: agent sessions now, Shell and the Project terminal later (M9). A
//! session is a program running in a PTY with a cwd and environment. Its output is appended to a bounded
//! in-memory [`Scrollback`] and sent live to every attached client; a client that (re)attaches gets the stored
//! scrollback first, then live output, so it can redraw its terminal (SVC-05). Output never goes to disk
//! (d0045).
//!
//! Each spawn writes a `sessions` row (kind, state, started/ended, end reason, Claude session ID); every row is
//! kept (d0045). One agent session per ticket (SES-04).
//!
//! Threads: each session has a reader thread (PTY → scrollback and attached clients) and a waiter thread (child
//! exit → the row, a `session_state` event). Sessions end when their program exits, when Crewboard stops them,
//! or when the service stops; the next service start reconciles their tickets ([`crate::recovery`]).

pub mod deliveries;
pub mod input;
pub mod launch;
pub mod scrollback;
pub mod statusline;

use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch};

use crate::attention::{ClientId, Focus};
use crate::domain::{TicketKey, stored_enum};
use crate::protocol::Event;
use crate::service::EventBus;
use crate::store::{Store, StoreError, new_id, now_ms};
pub use input::{CLEAR_LINE, InputEffect, InputTracker};
pub use scrollback::Scrollback;

stored_enum! {
    /// What runs in a session (the `sessions.kind` column).
    pub enum SessionKind ("session kind") {
        /// A ticket's `claude` session.
        Agent = "agent",
        /// A finished ticket resumed to answer another agent's question (M10).
        Answer = "answer",
        /// The short-lived session that opens an epic's PR (M10).
        Delivery = "delivery",
        /// A ticket's Shell tab (M9).
        Shell = "shell",
        /// The Project terminal (M9).
        Project = "project",
        /// A Run command (M9).
        Run = "run",
    }
}

impl SessionKind {
    /// Whether the session runs `claude` for a ticket. Processes inside it are that ticket's agent (CLI-01);
    /// Shell, Project terminal and Run sessions are the user's own terminals (CLI-03).
    pub fn runs_claude(self) -> bool {
        matches!(self, Self::Agent | Self::Answer | Self::Delivery)
    }

    /// Run, Shell and the Project terminal: the user's own processes, stopped by a restart and never restarted
    /// on their own.
    pub fn is_users_terminal(self) -> bool {
        matches!(self, Self::Run | Self::Shell | Self::Project)
    }
}

stored_enum! {
    pub enum SessionState ("session state") {
        Running = "running",
        Ended = "ended",
    }
}

/// Why sessions end, as stored in `end_reason` (besides "exit N" and "signal SIGNAME").
pub const END_STOPPED: &str = "stopped";
pub const END_SERVICE_STOPPED: &str = "service stopped";
/// Run, Shell and Project terminal sessions ended by the service stopping: shown as "stopped by restart" and never
/// started again on their own (REC-01, d0035, d0045).
pub const END_STOPPED_BY_RESTART: &str = "stopped by restart";

/// A terminal's size in character cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

impl TerminalSize {
    fn pty(self) -> PtySize {
        PtySize {
            rows: self.rows.max(1),
            cols: self.cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

/// A session as clients see it: a `sessions` row, plus its size while it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub kind: SessionKind,
    /// The project's key.
    pub project: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticket: Option<TicketKey>,
    pub state: SessionState,
    pub started_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_reason: Option<String>,
    /// Kept for Resume, Park and "Before the restart" (d0045).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<TerminalSize>,
}

impl SessionInfo {
    pub fn is_running(&self) -> bool {
        self.state == SessionState::Running
    }
}

/// The ticket a session belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTicket {
    pub id: String,
    pub key: TicketKey,
}

/// What to run.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub kind: SessionKind,
    pub project_id: String,
    pub project_key: String,
    pub ticket: Option<SessionTicket>,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Added to (or replacing in) the service's environment, which the session inherits.
    pub env: Vec<(String, String)>,
    pub size: TerminalSize,
    pub claude_session_id: Option<String>,
}

/// The root process of a running `claude` session, for caller identity (CLI-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRoot {
    pub pid: u32,
    pub session: String,
    pub ticket: TicketKey,
    pub ticket_id: String,
}

/// A piece of output and where it sits in the session's whole output.
#[derive(Debug, Clone)]
pub struct OutputChunk {
    pub offset: u64,
    pub data: Arc<[u8]>,
}

/// What a client gets when it attaches: the stored output, then live chunks from `output`.
#[derive(Debug)]
pub struct Attached {
    pub info: SessionInfo,
    pub scrollback: Vec<u8>,
    /// The offset of the first scrollback byte.
    pub start: u64,
    /// The offset after the last scrollback byte; live chunks continue from here.
    pub end: u64,
    pub output: broadcast::Receiver<OutputChunk>,
}

/// Live chunks kept for attached clients that are behind; one further behind is disconnected.
const OUTPUT_CHANNEL: usize = 1024;
/// How long a stopped session gets to exit after SIGHUP before SIGKILL.
const KILL_GRACE: Duration = Duration::from_secs(2);
/// How long the waiter lets the reader drain the last output after the program exited.
const DRAIN_WAIT: Duration = Duration::from_millis(500);

/// Owns every live session. Cheap to clone.
#[derive(Clone)]
pub struct Sessions {
    inner: Arc<Inner>,
}

struct Inner {
    store: Store,
    events: EventBus,
    scrollback_limit: usize,
    /// Which clients' terminals have focus on which session (ATT-03); shared with the board.
    focus: Focus,
    /// Running sessions, and the last ended one per ticket and kind (so its scrollback can still be shown).
    live: Mutex<HashMap<String, Arc<Live>>>,
}

struct Live {
    info: Mutex<SessionInfo>,
    ticket_id: Option<String>,
    pid: Option<u32>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    output: Mutex<Output>,
    /// The unsent-input flag (SVC-06, d0048).
    input: Mutex<InputTracker>,
    /// Set when Crewboard stops the session; the end reason then says so.
    stop_reason: Mutex<Option<&'static str>>,
    ended: watch::Sender<bool>,
}

struct Output {
    scrollback: Scrollback,
    live: broadcast::Sender<OutputChunk>,
}

impl Sessions {
    pub fn new(store: Store, events: EventBus) -> Self {
        Self::with_scrollback_limit(store, events, scrollback::DEFAULT_LIMIT)
    }

    pub fn with_scrollback_limit(store: Store, events: EventBus, limit: usize) -> Self {
        Self::build(store, events, limit, Focus::default())
    }

    /// Sessions whose terminal focus is kept in `focus`: the service passes the board's, so ticket views and
    /// needs-you items follow the focused-terminal rule (ATT-03).
    pub fn with_focus(store: Store, events: EventBus, focus: Focus) -> Self {
        Self::build(store, events, scrollback::DEFAULT_LIMIT, focus)
    }

    fn build(store: Store, events: EventBus, limit: usize, focus: Focus) -> Self {
        Self {
            inner: Arc::new(Inner {
                store,
                events,
                scrollback_limit: limit,
                focus,
                live: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Marks sessions left running by a service that is gone as ended (agents "service stopped", Run / Shell /
    /// Project terminal "stopped by restart"). Called once when the service starts.
    pub async fn end_leftovers(store: &Store) -> Result<u64, StoreError> {
        let mut ended = 0;
        for (kinds, reason) in [(false, END_SERVICE_STOPPED), (true, END_STOPPED_BY_RESTART)] {
            let done = sqlx::query(
                "UPDATE sessions SET state = ?, ended_at = ?, end_reason = ?
                 WHERE ended_at IS NULL AND (kind IN ('run', 'shell', 'project')) = ?",
            )
            .bind(SessionState::Ended.as_str())
            .bind(now_ms())
            .bind(reason)
            .bind(kinds)
            .execute(store.pool())
            .await?;
            ended += done.rows_affected();
        }
        Ok(ended)
    }

    /// Starts `spec.program` in a new PTY and records its `sessions` row. A second running agent session for
    /// the same ticket is refused.
    pub async fn spawn(&self, spec: SpawnSpec) -> Result<SessionInfo, SessionError> {
        if spec.kind == SessionKind::Agent
            && let Some(ticket) = &spec.ticket
            && let Some(running) = self.running_of(&ticket.id, SessionKind::Agent)
        {
            return Err(SessionError::AlreadyRunning {
                ticket: ticket.key.clone(),
                session: running.id,
            });
        }

        let pty = native_pty_system()
            .openpty(spec.size.pty())
            .map_err(|e| SessionError::Spawn(e.to_string()))?;
        let mut command = CommandBuilder::new(&spec.program);
        command.args(&spec.args);
        command.cwd(&spec.cwd);
        // A service started from Finder or a script may have no TERM; terminals need one.
        if command.get_env("TERM").is_none() {
            command.env("TERM", "xterm-256color");
        }
        command.env("COLORTERM", "truecolor");
        for (key, value) in &spec.env {
            command.env(key, value);
        }
        let mut child = pty
            .slave
            .spawn_command(command)
            .map_err(|e| SessionError::Spawn(format!("can't start {}: {e}", spec.program)))?;
        // The reader only sees end-of-file once every slave handle is closed.
        drop(pty.slave);
        let reader = pty
            .master
            .try_clone_reader()
            .map_err(|e| SessionError::Spawn(e.to_string()))?;
        let writer = pty
            .master
            .take_writer()
            .map_err(|e| SessionError::Spawn(e.to_string()))?;

        let info = SessionInfo {
            id: new_id(),
            kind: spec.kind,
            project: spec.project_key.clone(),
            ticket: spec.ticket.as_ref().map(|t| t.key.clone()),
            state: SessionState::Running,
            started_at: now_ms(),
            ended_at: None,
            end_reason: None,
            claude_session_id: spec.claude_session_id.clone(),
            size: Some(spec.size),
        };
        let inserted = sqlx::query(
            "INSERT INTO sessions (id, project_id, ticket_id, kind, pty_id, claude_session_id, state, started_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&info.id)
        .bind(&spec.project_id)
        .bind(spec.ticket.as_ref().map(|t| t.id.clone()))
        .bind(info.kind.as_str())
        .bind(child.process_id().map(|pid| pid.to_string()))
        .bind(&info.claude_session_id)
        .bind(info.state.as_str())
        .bind(info.started_at)
        .execute(self.inner.store.pool())
        .await;
        if let Err(e) = inserted {
            let _ = child.kill();
            return Err(SessionError::Store(e.into()));
        }

        let (ended, _) = watch::channel(false);
        let live = Arc::new(Live {
            info: Mutex::new(info.clone()),
            ticket_id: spec.ticket.as_ref().map(|t| t.id.clone()),
            pid: child.process_id(),
            killer: Mutex::new(child.clone_killer()),
            master: Mutex::new(pty.master),
            writer: Mutex::new(writer),
            output: Mutex::new(Output {
                scrollback: Scrollback::new(self.inner.scrollback_limit),
                live: broadcast::channel(OUTPUT_CHANNEL).0,
            }),
            input: Mutex::new(InputTracker::new()),
            stop_reason: Mutex::new(None),
            ended,
        });
        self.inner
            .live
            .lock()
            .expect("sessions lock")
            .insert(info.id.clone(), live.clone());

        let (drained, drained_rx) = std::sync::mpsc::channel::<()>();
        let reading = live.clone();
        std::thread::Builder::new()
            .name(format!("pty-read-{}", info.id))
            .spawn(move || read_output(reader, &reading, drained))
            .map_err(|e| SessionError::Spawn(e.to_string()))?;
        let weak = Arc::downgrade(&self.inner);
        let runtime = tokio::runtime::Handle::current();
        let id = info.id.clone();
        std::thread::Builder::new()
            .name(format!("pty-wait-{id}"))
            .spawn(move || {
                let status = child.wait();
                let _ = drained_rx.recv_timeout(DRAIN_WAIT);
                let reason = match status {
                    Ok(status) => match status.signal() {
                        Some(signal) => format!("signal {signal}"),
                        None => format!("exit {}", status.exit_code()),
                    },
                    Err(e) => format!("lost: {e}"),
                };
                runtime.spawn(async move {
                    if let Some(inner) = weak.upgrade() {
                        inner.finish(&id, reason).await;
                    }
                });
            })
            .map_err(|e| SessionError::Spawn(e.to_string()))?;

        tracing::info!(session = %info.id, kind = %info.kind, ticket = ?info.ticket, program = %spec.program, "session started");
        self.inner.events.publish(Event::SessionState {
            session: info.clone(),
        });
        Ok(info)
    }

    /// The stored output and a live feed from right after it. Works for ended sessions still in memory too.
    pub fn attach(&self, id: &str) -> Result<Attached, SessionError> {
        let live = self.live(id)?;
        let info = live.info.lock().expect("session info").clone();
        let output = live.output.lock().expect("session output");
        let (scrollback, start) = output.scrollback.contents();
        Ok(Attached {
            info,
            end: output.scrollback.end(),
            scrollback,
            start,
            output: output.live.subscribe(),
        })
    }

    /// Resizes the PTY; the program gets SIGWINCH.
    pub fn resize(&self, id: &str, size: TerminalSize) -> Result<(), SessionError> {
        let live = self.live(id)?;
        let mut info = live.info.lock().expect("session info");
        if !info.is_running() {
            return Err(SessionError::Ended(id.to_owned()));
        }
        live.master
            .lock()
            .expect("session pty")
            .resize(size.pty())
            .map_err(|e| SessionError::Io(e.to_string()))?;
        info.size = Some(size);
        Ok(())
    }

    /// Relays keystrokes typed by a person (in the app or `crewboard attach`) and updates the unsent-input
    /// flag (SVC-06). Each Enter is reported as a `session_enter` event (ATT-02 clears orange signals with it).
    pub fn input(&self, id: &str, bytes: &[u8]) -> Result<InputEffect, SessionError> {
        let live = self.live(id)?;
        let effect = {
            // Held while writing, so the flag follows the order the bytes reach the program.
            let mut input = live.input.lock().expect("session input");
            live.write(id, bytes)?;
            input.feed(bytes)
        };
        if effect.enters > 0 {
            let ticket = live.info.lock().expect("session info").ticket.clone();
            for _ in 0..effect.enters {
                self.inner.events.publish(Event::SessionEnter {
                    session: id.to_owned(),
                    ticket: ticket.clone(),
                });
            }
        }
        Ok(effect)
    }

    /// Adds a line of Crewboard's own to the session's output, as if the program had printed it: Run's "[…] exited
    /// with code 1" (RUN-01). It goes to the scrollback and to attached clients, also after the session ended,
    /// and starts on a line of its own.
    pub fn append_line(&self, id: &str, line: &str) -> Result<(), SessionError> {
        let live = self.live(id)?;
        let mut output = live.output.lock().expect("session output");
        let mut bytes = Vec::with_capacity(line.len() + 4);
        if output.scrollback.last().is_some_and(|b| b != b'\n') {
            bytes.extend_from_slice(b"\r\n");
        }
        bytes.extend_from_slice(line.as_bytes());
        bytes.extend_from_slice(b"\r\n");
        let data: Arc<[u8]> = Arc::from(bytes.as_slice());
        let offset = output.scrollback.push(&data);
        let _ = output.live.send(OutputChunk { offset, data });
        Ok(())
    }

    /// Writes Crewboard's own bytes to the program's input as they are. They don't touch the unsent-input
    /// flag: only a person's typing does.
    pub fn write(&self, id: &str, bytes: &[u8]) -> Result<(), SessionError> {
        self.live(id)?.write(id, bytes)
    }

    /// Whether the session's input line may hold text the user hasn't sent (d0048).
    pub fn has_unsent_input(&self, id: &str) -> Result<bool, SessionError> {
        Ok(self
            .live(id)?
            .input
            .lock()
            .expect("session input")
            .has_unsent_text())
    }

    /// A client reports that its terminal for this session gained or lost keyboard focus (SES-10). Focus is kept
    /// per client: the session counts as focused while any client holds it, and `crewboard attach` holds it while
    /// attached (ATT-03). Publishes `session_focus` when the session's focus changes, and `ticket_changed` when
    /// the ticket's does (its card, inbox item and notification depend on it). An ended session takes no focus.
    pub fn set_focused(
        &self,
        id: &str,
        client: ClientId,
        focused: bool,
    ) -> Result<(), SessionError> {
        let live = self.live(id)?;
        let info = live.info.lock().expect("session info").clone();
        let Some(ticket_id) = live.ticket_id.as_deref() else {
            return Ok(());
        };
        if focused && !info.is_running() {
            return Ok(());
        }
        let change = self.inner.focus.set(ticket_id, id, client, focused);
        if change.session_changed {
            self.inner.events.publish(Event::SessionFocus {
                session: id.to_owned(),
                ticket: info.ticket.clone(),
                focused: change.session_focused,
            });
        }
        if change.ticket_changed {
            self.inner.ticket_focus_changed(ticket_id, info.ticket);
        }
        Ok(())
    }

    /// A client disconnected: every focus it held is lost (ATT-03).
    pub fn client_gone(&self, client: ClientId) {
        let released = self.inner.focus.client_gone(client);
        self.inner.publish_released(released);
    }

    /// Whether a client's terminal for this session has focus.
    pub fn is_focused(&self, id: &str) -> Result<bool, SessionError> {
        self.live(id)?;
        Ok(self.inner.focus.session_focused(id))
    }

    /// Clears the input line with [`CLEAR_LINE`] and resets the flag. Only ever on the user's click ("Clear
    /// it and send", SES-07): Crewboard never clears the user's text silently.
    pub fn clear_line(&self, id: &str) -> Result<(), SessionError> {
        let live = self.live(id)?;
        let mut input = live.input.lock().expect("session input");
        live.write(id, CLEAR_LINE)?;
        input.clear_line();
        Ok(())
    }

    /// The session as it is now (running, or ended and still in memory).
    pub fn info(&self, id: &str) -> Result<SessionInfo, SessionError> {
        Ok(self.live(id)?.info.lock().expect("session info").clone())
    }

    /// Every session in memory: the running ones and the last ended one per ticket and kind.
    pub fn in_memory(&self) -> Vec<SessionInfo> {
        let mut all: Vec<SessionInfo> = self
            .inner
            .live
            .lock()
            .expect("sessions lock")
            .values()
            .map(|l| l.info.lock().expect("session info").clone())
            .collect();
        all.sort_by(|a, b| a.started_at.cmp(&b.started_at).then(a.id.cmp(&b.id)));
        all
    }

    /// The root processes of every running session that runs `claude` for a ticket.
    pub fn agent_roots(&self) -> Vec<AgentRoot> {
        self.inner
            .live
            .lock()
            .expect("sessions lock")
            .values()
            .filter_map(|l| {
                let info = l.info.lock().expect("session info");
                if !info.is_running() || !info.kind.runs_claude() {
                    return None;
                }
                Some(AgentRoot {
                    pid: l.pid?,
                    session: info.id.clone(),
                    ticket: info.ticket.clone()?,
                    ticket_id: l.ticket_id.clone()?,
                })
            })
            .collect()
    }

    /// The ticket's running session of this kind.
    pub fn running_of(&self, ticket_id: &str, kind: SessionKind) -> Option<SessionInfo> {
        self.latest_of(ticket_id, kind)
            .filter(SessionInfo::is_running)
    }

    /// The ticket's newest session of this kind still in memory (running or just ended).
    pub fn latest_of(&self, ticket_id: &str, kind: SessionKind) -> Option<SessionInfo> {
        self.inner
            .live
            .lock()
            .expect("sessions lock")
            .values()
            .filter(|l| l.ticket_id.as_deref() == Some(ticket_id))
            .map(|l| l.info.lock().expect("session info").clone())
            .filter(|i| i.kind == kind)
            .max_by(|a, b| a.started_at.cmp(&b.started_at).then(a.id.cmp(&b.id)))
    }

    /// Every stored session of a ticket, newest first, with the size of the running one.
    pub async fn history(&self, ticket: &SessionTicket) -> Result<Vec<SessionInfo>, SessionError> {
        let rows: Vec<SessionRow> = sqlx::query_as(
            "SELECT s.id, s.kind, p.key AS project_key, s.state, s.started_at, s.ended_at, s.end_reason,
                    s.claude_session_id
             FROM sessions s JOIN projects p ON p.id = s.project_id
             WHERE s.ticket_id = ? ORDER BY s.started_at DESC, s.id DESC",
        )
        .bind(&ticket.id)
        .fetch_all(self.inner.store.pool())
        .await
        .map_err(|e| SessionError::Store(e.into()))?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let mut info = row.into_info(Some(ticket.key.clone()))?;
                if let Ok(live) = self.info(&info.id) {
                    info.size = live.size;
                }
                Some(info)
            })
            .collect())
    }

    /// A session as stored (running, or ended long ago), with its ticket's key.
    pub async fn stored(&self, id: &str) -> Result<SessionInfo, SessionError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            #[sqlx(flatten)]
            session: SessionRow,
            number: Option<i64>,
            child_number: Option<i64>,
        }
        let row: Row = sqlx::query_as(
            "SELECT s.id, s.kind, p.key AS project_key, s.state, s.started_at, s.ended_at, s.end_reason,
                    s.claude_session_id, t.number, t.child_number
             FROM sessions s JOIN projects p ON p.id = s.project_id LEFT JOIN tickets t ON t.id = s.ticket_id
             WHERE s.id = ?",
        )
        .bind(id)
        .fetch_optional(self.inner.store.pool())
        .await
        .map_err(|e| SessionError::Store(e.into()))?
        .ok_or_else(|| SessionError::NotFound(id.to_owned()))?;
        let ticket = row.number.and_then(|n| {
            crate::board::tickets::key_from_parts(
                &row.session.project_key,
                n,
                row.child_number.unwrap_or(0),
            )
        });
        row.session
            .into_info(ticket)
            .ok_or_else(|| SessionError::NotFound(id.to_owned()))
    }

    /// Waits until the session has ended; returns it as it ended. `None` for a session not in memory.
    pub async fn wait_ended(&self, id: &str) -> Option<SessionInfo> {
        let live = self.live(id).ok()?;
        let mut ended = live.ended.subscribe();
        let _ = ended.wait_for(|e| *e).await;
        Some(live.info.lock().expect("session info").clone())
    }

    /// Stops a session: SIGHUP to its process group, SIGKILL if it hasn't exited after a grace period.
    pub async fn stop(&self, id: &str) -> Result<(), SessionError> {
        let live = self.live(id)?;
        live.stop(END_STOPPED);
        let mut ended = live.ended.subscribe();
        if tokio::time::timeout(KILL_GRACE, ended.wait_for(|e| *e))
            .await
            .is_err()
        {
            live.kill_now();
            let _ = ended.wait_for(|e| *e).await;
        }
        Ok(())
    }

    /// Stops every running session as the service stops, and records why.
    pub async fn shutdown(&self) {
        let running: Vec<Arc<Live>> = self
            .inner
            .live
            .lock()
            .expect("sessions lock")
            .values()
            .filter(|l| l.info.lock().expect("session info").is_running())
            .cloned()
            .collect();
        for live in &running {
            let kind = live.info.lock().expect("session info").kind;
            live.stop(if kind.is_users_terminal() {
                END_STOPPED_BY_RESTART
            } else {
                END_SERVICE_STOPPED
            });
        }
        for live in &running {
            let mut ended = live.ended.subscribe();
            if tokio::time::timeout(KILL_GRACE, ended.wait_for(|e| *e))
                .await
                .is_err()
            {
                live.kill_now();
                let _ = tokio::time::timeout(KILL_GRACE, ended.wait_for(|e| *e)).await;
            }
        }
        // Anything that still didn't report its end is recorded as ended now.
        if let Err(e) = Self::end_leftovers(&self.inner.store).await {
            tracing::warn!(error = %e, "can't record the end of sessions");
        }
    }

    fn live(&self, id: &str) -> Result<Arc<Live>, SessionError> {
        self.inner
            .live
            .lock()
            .expect("sessions lock")
            .get(id)
            .cloned()
            .ok_or_else(|| SessionError::NotFound(id.to_owned()))
    }
}

impl Inner {
    /// Records a session's end: its row, its info, a `session_state` event; drops older ended sessions of the
    /// same ticket and kind from memory.
    async fn finish(&self, id: &str, exit: String) {
        let Some(live) = self.live.lock().expect("sessions lock").get(id).cloned() else {
            return;
        };
        let reason = live
            .stop_reason
            .lock()
            .expect("stop reason")
            .map_or(exit, str::to_owned);
        let ended_at = now_ms();
        let info = {
            let mut info = live.info.lock().expect("session info");
            info.state = SessionState::Ended;
            info.ended_at = Some(ended_at);
            info.end_reason = Some(reason.clone());
            info.size = None;
            info.clone()
        };
        if let Err(e) = sqlx::query(
            "UPDATE sessions SET state = ?, ended_at = ?, end_reason = ? WHERE id = ? AND ended_at IS NULL",
        )
        .bind(SessionState::Ended.as_str())
        .bind(ended_at)
        .bind(&reason)
        .bind(id)
        .execute(self.store.pool())
        .await
        {
            tracing::warn!(session = id, error = %e, "can't record the session's end");
        }
        {
            let mut all = self.live.lock().expect("sessions lock");
            all.retain(|other_id, other| {
                let other_info = other.info.lock().expect("session info");
                other_id == id
                    || other_info.is_running()
                    || other.ticket_id.is_none()
                    || other.ticket_id != live.ticket_id
                    || other_info.kind != info.kind
            });
        }
        tracing::info!(session = id, reason = %reason, "session ended");
        live.ended.send_replace(true);
        self.events.publish(Event::SessionState { session: info });
        let released = self.focus.session_gone(id);
        self.publish_released(released);
    }

    /// Tells clients that sessions lost their last focus holder, and which tickets lost focus.
    fn publish_released(&self, released: Vec<crate::attention::Released>) {
        for r in released {
            let ticket = self
                .live
                .lock()
                .expect("sessions lock")
                .get(&r.session)
                .and_then(|l| l.info.lock().expect("session info").ticket.clone());
            self.events.publish(Event::SessionFocus {
                session: r.session.clone(),
                ticket: ticket.clone(),
                focused: false,
            });
            if r.ticket_lost_focus {
                self.ticket_focus_changed(&r.ticket_id, ticket);
            }
        }
    }

    /// A ticket's focus changed: its card, inbox item and notification may change with it.
    fn ticket_focus_changed(&self, ticket_id: &str, key: Option<TicketKey>) {
        if let Some(key) = key {
            self.events.publish(Event::TicketChanged {
                project: key.project.to_string(),
                id: ticket_id.to_owned(),
                key,
            });
        }
    }
}

impl Drop for Inner {
    /// A manager dropped without [`Sessions::shutdown`] (tests) still leaves no process behind.
    fn drop(&mut self) {
        if let Ok(all) = self.live.lock() {
            for live in all.values() {
                live.kill_now();
            }
        }
    }
}

impl Live {
    fn write(&self, id: &str, bytes: &[u8]) -> Result<(), SessionError> {
        if !self.info.lock().expect("session info").is_running() {
            return Err(SessionError::Ended(id.to_owned()));
        }
        let mut writer = self.writer.lock().expect("session writer");
        writer
            .write_all(bytes)
            .and_then(|()| writer.flush())
            .map_err(|e| SessionError::Io(e.to_string()))
    }

    fn stop(&self, reason: &'static str) {
        self.stop_reason
            .lock()
            .expect("stop reason")
            .get_or_insert(reason);
        self.signal(rustix::process::Signal::HUP);
        // Some programs ignore SIGHUP; the portable-pty killer is the fallback when the group is gone.
        let _ = self.killer.lock().expect("session killer").kill();
    }

    fn kill_now(&self) {
        if *self.ended.borrow() {
            return;
        }
        self.signal(rustix::process::Signal::KILL);
    }

    /// Signals the session's whole process group (the program is its session leader).
    fn signal(&self, signal: rustix::process::Signal) {
        let pid = self
            .pid
            .and_then(|p| i32::try_from(p).ok())
            .and_then(rustix::process::Pid::from_raw);
        if let Some(pid) = pid {
            let _ = rustix::process::kill_process_group(pid, signal);
        }
    }
}

/// The reader thread: PTY output → scrollback and attached clients, until end of file.
fn read_output(
    mut reader: Box<dyn Read + Send>,
    live: &Live,
    drained: std::sync::mpsc::Sender<()>,
) {
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let data: Arc<[u8]> = Arc::from(&buf[..n]);
                let mut output = live.output.lock().expect("session output");
                let offset = output.scrollback.push(&data);
                let _ = output.live.send(OutputChunk { offset, data });
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            // macOS reports EIO once the program and its children closed the terminal.
            Err(_) => break,
        }
    }
    let _ = drained.send(());
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: String,
    kind: String,
    project_key: String,
    state: String,
    started_at: i64,
    ended_at: Option<i64>,
    end_reason: Option<String>,
    claude_session_id: Option<String>,
}

impl SessionRow {
    fn into_info(self, ticket: Option<TicketKey>) -> Option<SessionInfo> {
        Some(SessionInfo {
            id: self.id,
            kind: self.kind.parse().ok()?,
            project: self.project_key,
            ticket,
            state: self.state.parse().ok()?,
            started_at: self.started_at,
            ended_at: self.ended_at,
            end_reason: self.end_reason,
            claude_session_id: self.claude_session_id,
            size: None,
        })
    }
}

#[derive(Debug)]
pub enum SessionError {
    /// The ticket already has a running agent session (SES-04).
    AlreadyRunning {
        ticket: TicketKey,
        session: String,
    },
    NotFound(String),
    /// The session has ended; it takes no input or resize.
    Ended(String),
    Spawn(String),
    Io(String),
    Store(StoreError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning { ticket, .. } => {
                write!(f, "{ticket} already has a running agent session")
            }
            Self::NotFound(id) => write!(f, "no session {id}"),
            Self::Ended(id) => write!(f, "session {id} has ended"),
            Self::Spawn(message) | Self::Io(message) => f.write_str(message),
            Self::Store(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SessionError {}
