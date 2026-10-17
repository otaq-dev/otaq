//! The PTY session manager (SES-04, SVC-01, SVC-05; d0022).
//!
//! The background service owns every PTY: agent sessions now, Shell and the Project terminal later (M9). A
//! session is a program running in a PTY with a cwd and environment. Its output is appended to a bounded
//! in-memory [`Scrollback`] and sent live to every attached client; a client that (re)attaches gets the stored
//! scrollback first, then live output, so it can redraw its terminal (SVC-05). Output never goes to disk
//! (d0045).
//!
//! Each spawn writes a `sessions` row (kind, state, started/ended, end reason, the agent tool, its own session id
//! and the model as launched; d0088); every row is kept (d0045). One agent session per ticket (SES-04).
//!
//! Threads: each session has a reader thread (PTY → scrollback and attached clients) and a waiter thread (child
//! exit → the row, a `session_state` event). Sessions end when their program exits, when Otaq stops them,
//! or when the service stops; the next service start reconciles their tickets ([`crate::recovery`]).

pub mod deliveries;
pub mod env;
pub mod input;
pub mod scrollback;

use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch};

use crate::agent_tools::Tool;
use crate::attention::{ClientId, Focus};
use crate::domain::{TicketKey, stored_enum};
use crate::protocol::Event;
use crate::service::EventBus;
use crate::store::{Store, StoreError, new_id, now_ms};
pub use input::{InputEffect, InputTracker};
pub use scrollback::Scrollback;

stored_enum! {
    /// What runs in a session (the `sessions.kind` column).
    pub enum SessionKind ("session kind") {
        /// A ticket's agent session.
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
        /// The read-only judge of a ticket's submit (d0063): a fresh agent session in the ticket's worktree.
        Validator = "validator",
        /// A Project agent (d0068): an agent session in the project's folder, for no ticket.
        ProjectAgent = "project_agent",
    }
}

impl SessionKind {
    /// Whether the session runs an agent tool, whichever tool it is. Processes inside it are that ticket's agent,
    /// or the Project agent (CLI-01); Shell, Project terminal and Run sessions are the user's own terminals
    /// (CLI-03).
    pub fn runs_agent_tool(self) -> bool {
        matches!(
            self,
            Self::Agent | Self::Answer | Self::Delivery | Self::Validator | Self::ProjectAgent
        )
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
    /// The agent tool it runs; `None` for Shell, Run and Project terminals (ADP-04).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<Tool>,
    /// The tool's own conversation id, kept for Resume, Park and "Before the restart" (d0045, d0088). Empty for
    /// a moment for a tool that reports it after start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_session_id: Option<String>,
    /// The model passed to the tool at launch; `None` when none was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
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

/// A stored conversation to continue (Resume, Park, Send back, answers; d0045, d0088): the agent tool it ran
/// on, the tool's own id and the model as launched, so Resume reopens the same tool and model (ADP-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredConversation {
    pub tool: Tool,
    pub id: String,
    pub model: Option<String>,
}

impl StoredConversation {
    /// The conversation `session` ran, when it ran an agent tool whose id is known.
    pub fn of(session: &SessionInfo) -> Option<Self> {
        Some(Self {
            tool: session.tool?,
            id: session.tool_session_id.clone()?,
            model: session.model.clone(),
        })
    }

    /// The conversation of ticket `ticket_id`'s newest agent session whose tool id is known.
    pub async fn latest_agent(
        pool: &sqlx::SqlitePool,
        ticket_id: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        let row: Option<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT tool, tool_session_id, model FROM sessions
             WHERE ticket_id = ? AND kind = 'agent' AND tool IS NOT NULL AND tool_session_id IS NOT NULL
             ORDER BY started_at DESC, id DESC LIMIT 1",
        )
        .bind(ticket_id)
        .fetch_optional(pool)
        .await?;
        Ok(row.and_then(|(tool, id, model)| {
            Some(Self {
                tool: tool.parse().ok()?,
                id,
                model,
            })
        }))
    }
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
    /// The agent tool it runs, set by the adapter that built the command; `None` for the user's terminals.
    pub tool: Option<Tool>,
    pub tool_session_id: Option<String>,
    pub model: Option<String>,
}

/// The root process of a running agent session, for caller identity (CLI-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRoot {
    pub pid: u32,
    /// Agent, answer, delivery or validator.
    pub kind: SessionKind,
    pub session: String,
    pub ticket: TicketKey,
    pub ticket_id: String,
}

/// The root process of a running Project agent session, for caller identity (CLI-01, d0068).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectAgentRoot {
    pub pid: u32,
    pub session: String,
    /// The project's key.
    pub project: String,
    /// The Project agent's id.
    pub agent: String,
    /// An epic lead's epic (its id and key, d0083), once [`Sessions::set_epic_lead`] said so.
    pub epic: Option<(String, TicketKey)>,
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
    /// A project's variables (ENV-02), set once at service start; without it sessions get none.
    project_env: OnceLock<env::EnvProvider>,
}

struct Live {
    info: Mutex<SessionInfo>,
    ticket_id: Option<String>,
    /// The Project agent the session runs for (d0068), once [`Sessions::set_project_agent`] said so.
    project_agent: Mutex<Option<String>>,
    /// The epic it leads (id and key), once [`Sessions::set_epic_lead`] said so (d0083).
    lead_epic: Mutex<Option<(String, TicketKey)>>,
    pid: Option<u32>,
    /// The program and what it started, stopped together.
    group: crate::process::Group,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    output: Mutex<Output>,
    /// The unsent-input flag (SVC-06, d0048).
    input: Mutex<InputTracker>,
    /// Set when Otaq stops the session; the end reason then says so.
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
                project_env: OnceLock::new(),
            }),
        }
    }

    /// Gives the sessions the project variables (ENV-02, d0082): every spawn asks `provider` for the rows of its
    /// project, then.
    pub fn set_env_provider(&self, provider: env::EnvProvider) {
        let _ = self.inner.project_env.set(provider);
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
        // On Windows portable-pty starts from the environment stored in the registry (a fresh login's), which
        // drops what the service's own environment changed, PATH first of all. Sessions get the service's
        // environment, as on macOS and Linux.
        if cfg!(windows) {
            for (key, value) in std::env::vars_os() {
                command.env(key, value);
            }
        }
        command.args(&spec.args);
        command.cwd(&spec.cwd);
        // The project's variables, read now (ENV-02), then Otaq's own on top ([`env`]'s order rule).
        let rows = match self.inner.project_env.get() {
            Some(provider) => provider(spec.project_id.clone()).await,
            None => Vec::new(),
        };
        // A service started from Finder or a script may have no TERM; terminals need one.
        let term = command.get_env("TERM").map_or_else(
            || "xterm-256color".to_owned(),
            |t| t.to_string_lossy().into_owned(),
        );
        let own: Vec<(String, String)> = [
            ("TERM".to_owned(), term),
            ("COLORTERM".to_owned(), "truecolor".to_owned()),
        ]
        .into_iter()
        .chain(spec.env.iter().cloned())
        .collect();
        let inherited_path = command.get_env("PATH").map(ToOwned::to_owned);
        for (key, value) in env::layered(
            &rows,
            &own,
            inherited_path.as_deref(),
            crate::tools::Os::current(),
        ) {
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
            tool: spec.tool,
            tool_session_id: spec.tool_session_id.clone(),
            model: spec.model.clone(),
            size: Some(spec.size),
        };
        let inserted = sqlx::query(
            "INSERT INTO sessions (id, project_id, ticket_id, kind, pty_id, tool, tool_session_id, model, state,
                                   started_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&info.id)
        .bind(&spec.project_id)
        .bind(spec.ticket.as_ref().map(|t| t.id.clone()))
        .bind(info.kind.as_str())
        .bind(child.process_id().map(|pid| pid.to_string()))
        .bind(info.tool.map(Tool::as_str))
        .bind(&info.tool_session_id)
        .bind(&info.model)
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
            project_agent: Mutex::new(None),
            lead_epic: Mutex::new(None),
            pid: child.process_id(),
            group: crate::process::Group::of_leader(child.process_id()),
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

    /// Relays keystrokes typed by a person (in the app or `otaq attach`) and updates the unsent-input
    /// flag (SVC-06). Each Enter is reported as a `session_enter` event (ATT-02 clears needs-you signals with it).
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

    /// Adds a line of Otaq's own to the session's output, as if the program had printed it: Run's "[…] exited
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

    /// Writes Otaq's own bytes to the program's input as they are. They don't touch the unsent-input
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
    /// per client: the session counts as focused while any client holds it, and `otaq attach` holds it while
    /// attached (ATT-03). Publishes `session_focus` when the session's focus changes, and `ticket_changed` when
    /// the ticket's does on the session's side (a validator is a side of its own, d0063; its card, inbox item and
    /// notification depend on it). An ended session takes no focus.
    pub fn set_focused(
        &self,
        id: &str,
        client: ClientId,
        focused: bool,
    ) -> Result<(), SessionError> {
        let live = self.live(id)?;
        let info = live.info.lock().expect("session info").clone();
        // A Project agent's session is focused like a ticket's (ATT-03, d0068), under its own key.
        let project_agent = live
            .project_agent
            .lock()
            .expect("session project agent")
            .as_ref()
            .map(|agent| format!("project-agent:{agent}"));
        let Some(ticket_id) = live.ticket_id.as_deref().or(project_agent.as_deref()) else {
            return Ok(());
        };
        if focused && !info.is_running() {
            return Ok(());
        }
        let side = if info.kind == SessionKind::Validator {
            crate::attention::Side::Validator
        } else {
            crate::attention::Side::Agent
        };
        let change = self.inner.focus.set(ticket_id, id, side, client, focused);
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

    /// Clears the input line with `clear_line`, the bytes of the session's agent tool (ADP-06), and resets the
    /// flag. Only ever on the user's click ("Clear it and send", SES-07): Otaq never clears the user's text
    /// silently.
    pub fn clear_line(&self, id: &str, clear_line: &[u8]) -> Result<(), SessionError> {
        let live = self.live(id)?;
        let mut input = live.input.lock().expect("session input");
        live.write(id, clear_line)?;
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

    /// The root processes of every running session that runs an agent tool for a ticket.
    pub fn agent_roots(&self) -> Vec<AgentRoot> {
        self.inner
            .live
            .lock()
            .expect("sessions lock")
            .values()
            .filter_map(|l| {
                let info = l.info.lock().expect("session info");
                if !info.is_running() || !info.kind.runs_agent_tool() {
                    return None;
                }
                Some(AgentRoot {
                    pid: l.pid?,
                    kind: info.kind,
                    session: info.id.clone(),
                    ticket: info.ticket.clone()?,
                    ticket_id: l.ticket_id.clone()?,
                })
            })
            .collect()
    }

    /// The running agent session (one that runs an agent tool) whose process group holds `pid` (Windows: its job; see
    /// [`crate::process::Group::contains`]).
    pub fn agent_root_holding(&self, pid: u32) -> Option<AgentRoot> {
        self.inner
            .live
            .lock()
            .expect("sessions lock")
            .values()
            .find_map(|l| {
                let info = l.info.lock().expect("session info");
                if !info.is_running() || !info.kind.runs_agent_tool() || !l.group.contains(pid) {
                    return None;
                }
                Some(AgentRoot {
                    pid: l.pid?,
                    kind: info.kind,
                    session: info.id.clone(),
                    ticket: info.ticket.clone()?,
                    ticket_id: l.ticket_id.clone()?,
                })
            })
    }

    /// Records that the session runs for the Project agent `agent` (its `sessions.project_agent_id`); from then on
    /// its processes are that Project agent's callers.
    pub async fn set_project_agent(&self, id: &str, agent: &str) -> Result<(), SessionError> {
        sqlx::query("UPDATE sessions SET project_agent_id = ? WHERE id = ?")
            .bind(agent)
            .bind(id)
            .execute(self.inner.store.pool())
            .await
            .map_err(|e| SessionError::Store(e.into()))?;
        *self
            .live(id)?
            .project_agent
            .lock()
            .expect("session project agent") = Some(agent.to_owned());
        Ok(())
    }

    /// Records that the Project agent session `id` leads the epic `epic_id` (`key`): from then on its processes are
    /// that epic lead's callers (d0083). In memory only: the lead's row keeps the epic.
    pub fn set_epic_lead(
        &self,
        id: &str,
        epic_id: &str,
        key: &TicketKey,
    ) -> Result<(), SessionError> {
        *self.live(id)?.lead_epic.lock().expect("session lead epic") =
            Some((epic_id.to_owned(), key.clone()));
        Ok(())
    }

    /// The root processes of every running Project agent session.
    pub fn project_agent_roots(&self) -> Vec<ProjectAgentRoot> {
        self.inner
            .live
            .lock()
            .expect("sessions lock")
            .values()
            .filter_map(|l| l.project_agent_root())
            .collect()
    }

    /// The running Project agent session whose process group holds `pid` (Windows: its job).
    pub fn project_agent_holding(&self, pid: u32) -> Option<ProjectAgentRoot> {
        self.inner
            .live
            .lock()
            .expect("sessions lock")
            .values()
            .filter(|l| l.group.contains(pid))
            .find_map(|l| l.project_agent_root())
    }

    /// The running Project terminal whose process group holds `pid` (Windows: its job).
    pub fn project_terminal_holding(&self, pid: u32) -> Option<String> {
        self.inner
            .live
            .lock()
            .expect("sessions lock")
            .values()
            .find_map(|l| {
                let info = l.info.lock().expect("session info");
                (info.is_running() && info.kind == SessionKind::Project && l.group.contains(pid))
                    .then(|| info.id.clone())
            })
    }

    /// The root processes of every running Project terminal, with their session (T-0167: what was created there).
    pub fn project_terminal_roots(&self) -> Vec<(u32, String)> {
        self.inner
            .live
            .lock()
            .expect("sessions lock")
            .values()
            .filter_map(|l| {
                let info = l.info.lock().expect("session info");
                (info.is_running() && info.kind == SessionKind::Project)
                    .then(|| Some((l.pid?, info.id.clone())))
                    .flatten()
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
                    s.tool, s.tool_session_id, s.model
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
                    s.tool, s.tool_session_id, s.model, t.number, t.child_number
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
    /// The session as a running Project agent's root.
    fn project_agent_root(&self) -> Option<ProjectAgentRoot> {
        let info = self.info.lock().expect("session info");
        if !info.is_running() || info.kind != SessionKind::ProjectAgent {
            return None;
        }
        Some(ProjectAgentRoot {
            pid: self.pid?,
            session: info.id.clone(),
            project: info.project.clone(),
            agent: self
                .project_agent
                .lock()
                .expect("session project agent")
                .clone()?,
            epic: self.lead_epic.lock().expect("session lead epic").clone(),
        })
    }

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
        self.group.hang_up();
        // Some programs ignore SIGHUP; the portable-pty killer is the fallback when the group is gone.
        let _ = self.killer.lock().expect("session killer").kill();
    }

    fn kill_now(&self) {
        if *self.ended.borrow() {
            return;
        }
        self.group.kill();
    }
}

/// The reader thread: PTY output → scrollback and attached clients, until end of file.
fn read_output(
    mut reader: Box<dyn Read + Send>,
    live: &Live,
    drained: std::sync::mpsc::Sender<()>,
) {
    let mut buf = vec![0u8; 16 * 1024];
    // Windows: ConPTY starts by asking the terminal where the cursor is and holds all output until it hears back.
    // The service answers that first question itself (top left), so a session runs before any terminal attaches,
    // and keeps it out of the scrollback, so no attached terminal answers it a second time.
    let mut startup_query = if cfg!(windows) { 4 } else { 0 };
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let mut chunk = buf[..n].to_vec();
                if startup_query > 0 {
                    startup_query -= 1;
                    if let Some(at) = find(&chunk, CURSOR_POSITION_QUERY) {
                        chunk.drain(at..at + CURSOR_POSITION_QUERY.len());
                        startup_query = 0;
                        let mut writer = live.writer.lock().expect("session writer");
                        let _ = writer.write_all(b"\x1b[1;1R").and_then(|()| writer.flush());
                    }
                    if chunk.is_empty() {
                        continue;
                    }
                }
                let data: Arc<[u8]> = Arc::from(chunk.as_slice());
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

/// "Where is the cursor?" (DSR 6), which ConPTY sends when a session starts.
const CURSOR_POSITION_QUERY: &[u8] = b"\x1b[6n";

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
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
    tool: Option<String>,
    tool_session_id: Option<String>,
    model: Option<String>,
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
            tool: self.tool.and_then(|t| t.parse().ok()),
            tool_session_id: self.tool_session_id,
            model: self.model,
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
