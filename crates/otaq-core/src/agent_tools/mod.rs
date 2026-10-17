//! Agent tools (ADP-01, ADP-02, ADP-06; d0086, d0088): one adapter per agent CLI behind one interface,
//! [`AgentTool`]. The scheduler, board, cards, limits and UI talk only to the interface and never branch on the
//! tool; nothing outside `agent_tools/<tool>/` names a tool's flags, files, payload fields or syntax.
//!
//! Each adapter answers d0088's seven questions, one group of methods each: detect; launch and resume; one-time
//! setup; signals; usage; input quirks; skills. The doc comment of every method says how each tool of version 1
//! fills it, from `reports/Hands-on check of Codex, Copilot and OpenCode (2026-10-17).md` (Codex 0.157.1,
//! Copilot 1.0.98, OpenCode 2.0.17) and the Copilot real-account check of the same day. Only the Claude Code
//! adapter exists ([`claude`]); M23 adds the other three without changing a signature here.
//!
//! The interface is a plain object-safe trait used as `&dyn AgentTool` through the registry, [`Adapters`]. The
//! two steps that run programs and take a while (detect, the tool-wide setup) return a boxed future, so no
//! async-trait crate is needed. OpenCode's step before launch is not a future but a [`PrepareStep`] the service
//! runs: it is a plain command, so launch tests can check it exactly without running the tool (d0088 testing).

pub mod claude;
pub mod events;
pub mod setup;

use std::ffi::OsString;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::domain::{TicketKey, stored_enum};
pub use crate::recovery::TranscriptEntry;
use crate::usage::Tokens;
use crate::usage::limit::LimitKind;
pub use events::{Credits, LimitWindow, ToolEvent};
pub use setup::SetupOutcome;

stored_enum! {
    /// An agent CLI Otaq can run, as stored on sessions. M23 adds `Codex = "codex"`, `Copilot = "copilot"` and
    /// `OpenCode = "opencode"` (d0086). Only [`Adapters::get`] and the names below match on it.
    pub enum Tool ("agent tool") {
        /// Claude Code, the `claude` CLI.
        Claude = "claude",
    }
}

/// The names a view shows for a tool's sessions, without the registry (the view models have none). Each is the
/// same as the adapter's own method.
impl Tool {
    /// [`AgentTool::short_name`]: "Claude".
    pub fn short_name(self) -> &'static str {
        match self {
            Self::Claude => claude::SHORT_NAME,
        }
    }

    /// [`AgentTool::command_name`]: `claude`.
    pub fn command_name(self) -> &'static str {
        match self {
            Self::Claude => claude::command_name(),
        }
    }

    /// [`AgentTool::resume_hint`]: "claude --resume 3be94c10".
    pub fn resume_hint(self, tool_id: &str) -> String {
        match self {
            Self::Claude => claude::resume_hint(tool_id),
        }
    }
}

/// A boxed future, so the trait stays object-safe.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What [`AgentTool::detect`] found (ADP-10).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detection {
    /// Found on PATH.
    pub installed: bool,
    /// Signed in; `None` when the tool can't say without a model call.
    pub signed_in: Option<bool>,
    /// The version as the tool prints it.
    pub version: Option<String>,
}

/// The skill every agent session starts with.
pub const WORK_TICKET_SKILL: &str = "work-ticket";

/// The skill every Project agent starts with (d0068).
pub const PROJECT_AGENT_SKILL: &str = "project-agent";

/// The skill every epic lead starts with (d0083).
pub const EPIC_LEAD_SKILL: &str = "epic-lead";

/// Where things are on this machine, for every launch: what all adapters share, and each adapter's own part.
/// [`LaunchContext::from_process`] finds them for the running service; tests replace each tool's program with an
/// ordinary command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchContext {
    /// The folder of the `otaq` command, put first on the agent's PATH.
    pub bin_dir: PathBuf,
    /// Otaq's plugin folder (one copy of its skills and hooks, ADP-05).
    pub plugin_dir: PathBuf,
    /// The PATH the agent extends (the service's).
    pub path: OsString,
    /// The Claude Code adapter's part: its program and the user's Claude Code folder.
    pub claude: claude::ClaudeLaunch,
}

impl LaunchContext {
    /// The running binary's folder, the plugin next to it (or the repository's in a dev build), and each
    /// tool's program from PATH. `None` when no plugin folder is found.
    pub fn from_process() -> Option<Self> {
        let exe = std::env::current_exe().ok()?;
        let bin_dir = exe.parent()?.to_path_buf();
        Some(Self {
            plugin_dir: claude::find_plugin_dir(&bin_dir)?,
            bin_dir,
            path: std::env::var_os("PATH").unwrap_or_default(),
            claude: claude::ClaudeLaunch::from_process(),
        })
    }
}

/// What an agent session is for. Every kind of agent can run on any tool (d0086).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LaunchRole {
    /// A ticket's agent, in its worktree (SES-03).
    TicketAgent,
    /// A ticket's validator, in its worktree, with no dev-server port (VAL-01).
    Validator,
    /// A Project agent, in the project's own folder, for no ticket (PAG-01).
    ProjectAgent,
    /// An epic lead, in the project's own folder, for one epic (LEAD-01).
    EpicLead,
    /// A finished ticket's conversation resumed to answer another agent (M10).
    Answer,
    /// An epic's short-lived delivery session that runs Open PR or Push (EPC-08).
    Delivery,
}

/// The first prompt of a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirstPrompt {
    /// One of Otaq's own skills (`work-ticket`, `address-review`, `epic-lead`…), named in the tool's syntax by
    /// the adapter ([`AgentTool::skill_reference`]), optionally followed by a message (the epic lead's first
    /// message).
    OtaqSkill {
        name: String,
        message: Option<String>,
    },
    /// Text passed as it is (a workflow skill as stored, "Follow `<path>`").
    Text(String),
}

/// One launch: everything a tool needs to start or resume an agent. Nothing in it is tool-specific.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRequest {
    pub role: LaunchRole,
    /// The working folder: the ticket's worktree, or the project's folder.
    pub cwd: PathBuf,
    /// The project's key.
    pub project: String,
    /// The ticket (`OTAQ_TICKET`): the ticket, or the epic for a delivery session. `None` for Project agents and
    /// epic leads.
    pub ticket: Option<TicketKey>,
    /// The epic an epic lead works on.
    pub epic: Option<TicketKey>,
    /// The ticket's dev-server port (`PORT`); 0 where the role has none (answer, delivery), as today.
    pub port: u16,
    /// The first prompt; a resume may have none.
    pub first_prompt: Option<FirstPrompt>,
    /// The tool id to resume (Resume, Park, Send back, answers); `None` starts a new conversation.
    pub resume: Option<String>,
    /// The model, passed to the tool unchanged (d0088); `None` uses the tool's default. Always `None` in M20.
    pub model: Option<String>,
}

/// What [`AgentTool::launch`] answers: the program to start in the session's terminal, and how the tool id is
/// known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub tool_id: ToolIdMode,
}

/// How a session's tool id becomes known (d0088 "Session ids").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolIdMode {
    /// The adapter chose it and it is on the command line: a fresh id for a new conversation (Claude Code and
    /// Copilot `--session-id <uuid>`), or the resumed id.
    ChosenUpFront(String),
    /// The adapter chose it, and `step` creates the conversation before the program starts (OpenCode: `opencode
    /// api --standalone POST /api/session` with the `ses_…` id, a title and the model, run in the worktree; the
    /// program then gets `--session <id>`).
    CreatedBeforeLaunch { id: String, step: PrepareStep },
    /// The tool makes its own id after start; the session's id stays empty until a
    /// [`ToolEvent::ToolIdKnown`] (Codex: its `SessionStart` hook, or `CODEX_THREAD_ID`).
    CapturedAfterStart,
}

/// A command the service runs to completion before it starts the program, with a time limit. A non-zero exit
/// fails the start with the command's own error output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareStep {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
}

/// Why a launch couldn't be built. The text is shown as the failed start's reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchError(pub String);

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LaunchError {}

/// A launch that can't be built refuses the click as "not available yet", with the adapter's reason.
impl From<LaunchError> for crate::board::BoardError {
    fn from(e: LaunchError) -> Self {
        Self::refused(crate::board::Refusal::NotAvailableYet, e.0)
    }
}

/// What the adapter may need besides the event itself: the session it came from, so it can read the tool's own
/// files (Copilot's `errorOccurred` is only a trigger to read the latest `session.error` in that session's
/// `events.jsonl`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignalContext<'a> {
    /// The session's tool id, when known.
    pub tool_id: Option<&'a str>,
    /// The session's working folder.
    pub cwd: Option<&'a Path>,
}

/// Where a conversation's tokens come from (USG-09).
pub enum TokenSource {
    /// The adapter reads the tool's own files; the usage tick keeps the reader per tool id and calls it again
    /// (Claude Code's transcript and helper-agent files, Codex's session file, Copilot's `events.jsonl`).
    Reader(Box<dyn TokenReader>),
    /// Tokens arrive as [`ToolEvent::Tokens`] (OpenCode's plugin events).
    Events,
    /// The tool doesn't report them: "tokens: not reported" (d0086).
    NotReported,
}

/// Reads one conversation's token counts from the tool's files, a little more on every call. Files only grow,
/// so a reader remembers how far it has read.
pub trait TokenReader: Send {
    /// Reads what was added since the last call. Returns whether any counts changed, or why the conversation's
    /// tokens are unknown.
    fn read(&mut self) -> Result<bool, TokensUnknown>;

    /// Every step counted so far: one per model response (Claude Code, per response id), `token_count` event
    /// (Codex) or `session.usage_record` (Copilot). A repeated step id replaces its earlier counts.
    fn steps(&self) -> Vec<TokenStep>;

    /// The tool's usage windows, when the same files carry them (Copilot's `quotaSnapshots`, one window per
    /// entry). `None` for Claude Code (its status line), Codex (its watched session file) and OpenCode (none).
    fn windows(&self) -> Option<Vec<LimitWindow>> {
        None
    }
}

/// One step's counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenStep {
    /// Unique within the conversation.
    pub id: String,
    /// When it was written, in Unix milliseconds.
    pub at: Option<i64>,
    /// The model that actually answered, where the tool says (Copilot's Auto picks one, ADP-08).
    pub model: Option<String>,
    pub tokens: Tokens,
}

/// Why a conversation's tokens are unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokensUnknown {
    /// No file for the tool id (yet).
    Missing,
    /// The file couldn't be read.
    Unreadable(String),
    /// The file holds nothing the tool would write.
    Unrecognised,
}

/// Where a tool's usage windows come from (USG-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsSource {
    /// A report Otaq receives and hands to [`AgentTool::windows_from_report`]: Claude Code's status-line JSON,
    /// sent by `otaq statusline`.
    Report,
    /// The tool's own files, read anyway: Codex's watched session file ([`ToolEvent::LimitWindows`]),
    /// Copilot's `events.jsonl` ([`TokenReader::windows`]).
    ToolFiles,
    /// Not reported: "Limits not reported" (OpenCode).
    NotReported,
}

/// One thing an adapter's one-time setup wrote into its tool's own config (d0085, d0086), for the Agent tools
/// page ("Otaq added: trust for your projects, its hooks…").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupEntry {
    /// A plain phrase: "trust for your projects".
    pub text: String,
    /// Removed when the tool is disconnected: Otaq-only entries (hook trust, rules, plugins, skills). Folder
    /// trust stays on every tool (d0086).
    pub removed_on_disconnect: bool,
}

/// What a setup or removal did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetupReport {
    /// The entries written (setup) or removed (removal).
    pub done: Vec<SetupEntry>,
    /// The entries that failed, each with a short reason shown as it is.
    pub failed: Vec<(SetupEntry, String)>,
}

/// How text is typed into the tool's input line (ADP-06, d0048). Enter is always `\r`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputQuirks {
    /// The bytes that clear the input line before Otaq types. Never Ctrl+C: it quits Codex and OpenCode on an
    /// empty line and arms exit in Copilot.
    pub clear_line: &'static [u8],
    /// The pause between the text and Enter.
    pub enter_delay: Duration,
    /// Whether [`Self::clear_line`] clears a multi-line draft. When not, SES-07 asks the user instead.
    pub multi_line_clearable: bool,
}

/// How Otaq's skills (one copy, `plugin/skills`) reach the tool (ADP-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillDelivery {
    /// The plugin folder is passed at every launch (Claude Code and Copilot `--plugin-dir`).
    PluginFolderAtLaunch,
    /// The skills folder goes in the launch's config (OpenCode `skills` in `OPENCODE_CONFIG_CONTENT`).
    LaunchConfig,
    /// Installed once by the one-time setup (Codex: a local plugin install into its home folder).
    OneTimeInstall,
}

/// One agent CLI (d0088). Each group of methods answers one of d0088's seven questions.
pub trait AgentTool: Send + Sync {
    /// Which tool this adapter is.
    fn tool(&self) -> Tool;

    // Names.

    /// The name shown to people: "Claude Code", "Codex", "GitHub Copilot", "OpenCode".
    fn display_name(&self) -> &'static str;

    /// The short name used in sentences: "Claude" ("Claude's saved conversation"), "Codex", "Copilot",
    /// "OpenCode".
    fn short_name(&self) -> &'static str;

    /// The command name: `claude` (History's "claude 3be94c10…"), `codex`, `copilot`, `opencode`.
    fn command_name(&self) -> &'static str;

    // 1. Detect.

    /// Installed, signed in and version (ADP-10). Claude Code: `claude --version`, signed in not reported
    /// (`None`), as the Tools checklist does today. Codex: `codex --version`; `codex login status`. Copilot:
    /// `copilot --version`; signed in from its login state (no model call). OpenCode: `opencode --version`;
    /// signed in from its stored credentials, where it can say.
    fn detect(&self) -> BoxFuture<'_, Detection>;

    // 2. Launch and resume.

    /// The program, arguments and environment for `request`, and how the tool id is known. The model goes to the
    /// tool unchanged. Never a permission or bypass flag (d0011); the only pre-approval is Otaq's own `otaq`
    /// commands.
    /// - Claude Code: `claude --plugin-dir <plugin> --allowedTools "Bash(otaq:*)" --settings <status line>
    ///   [--model <m>] (--session-id <new uuid> | --resume <id>) ["<prompt>"]`; tool id chosen up front.
    ///   Writes its status-line settings file (USG-04).
    /// - Codex: `codex --no-daemon -c check_for_update_on_startup=false [-m <m>] "<prompt>"`, resume `codex
    ///   --no-daemon resume <id> …`; tool id captured after start (ADP-07).
    /// - Copilot: `copilot --session-id <uuid> [--model <m>] --plugin-dir <plugin> --allow-tool
    ///   'shell(otaq:*)' -i "<prompt>"`, resume `--resume <id>`; tool id chosen up front (ADP-08).
    /// - OpenCode: `opencode --standalone --session <id> --prompt "<prompt>"` with `OPENCODE_CONFIG_CONTENT`
    ///   (plugin, skills, the `otaq *` shell rule, the model); never `--auto` or `OPENCODE_CONFIG_DIR`; tool id
    ///   created before launch by a [`PrepareStep`] (ADP-09).
    ///
    /// The environment carries Otaq's own variables (`OTAQ_TICKET`, the role, the epic, `PORT`, PATH with the
    /// `otaq` folder first) for every tool.
    fn launch(&self, request: &LaunchRequest) -> Result<LaunchPlan, LaunchError>;

    /// Why no launch can be built on this machine at all, so a caller can refuse before work that only makes
    /// sense for a launch (recreating a worktree); `None` when launches can be built. Claude Code: Otaq's plugin
    /// wasn't found (the same reason [`Self::launch`] gives).
    fn unavailable(&self) -> Option<LaunchError>;

    /// Whether the conversation `tool_id` that ran in `cwd` is still there to resume (answer sessions). Claude
    /// Code: its saved conversation file. Codex: its session file in its home folder. Copilot:
    /// `session-state/<id>/`. OpenCode: the session in its database (`opencode session list`). May read files.
    fn can_resume(&self, tool_id: &str, cwd: &Path) -> bool;

    /// How the user resumes the conversation by hand, for the recovery texts: "claude --resume 3be94c10",
    /// "codex resume …", "copilot --resume …", "opencode --session …". `tool_id` may be shortened.
    fn resume_hint(&self, tool_id: &str) -> String;

    // 3. One-time setup (d0085, d0086).

    /// Records what the tool needs for one added project: folder trust (Claude Code `projects` in its config
    /// file; Codex `[projects."<root>"]` in `config.toml`; Copilot `trustedFolders` in `config.json`). `None` when
    /// the tool keeps no folder trust (OpenCode) or its config can't be located. Synchronous and short; may write
    /// the tool's config. Called for every tool by the service's [`setup::Trust`].
    fn record_project(&self, repo: &Path) -> Option<SetupOutcome>;

    /// The tool-wide setup when the tool is connected: Codex records hook trust before the first launch (the
    /// hash from its app-server's `hooks/list`), writes `rules/otaq.rules` and installs Otaq's local plugin.
    /// Claude Code, Copilot and OpenCode need nothing (an empty report): their hooks, pre-approval and skills
    /// come with every launch.
    fn set_up_tool(&self) -> BoxFuture<'_, SetupReport>;

    /// What Otaq's setup has written into the tool's config, for the Agent tools page. Claude Code: trust for
    /// your projects. Codex: trust for your projects, its hooks, `otaq` pre-approved, its skills. Copilot: trust
    /// for your projects. OpenCode: nothing.
    fn setup_entries(&self) -> Vec<SetupEntry>;

    /// Removes the Otaq-only entries when the tool is disconnected (hook trust, rules, plugins, skills); folder
    /// trust stays on every tool (d0086). Claude Code and Copilot have no Otaq-only entries (an empty report).
    fn remove_setup(&self) -> SetupReport;

    // 4. Signals (ADP-02, ADP-03).

    /// Turns one hook (`otaq hook --tool <tool> <event>`, the event name as text, the JSON from stdin) into
    /// Otaq's events: none, one or several. An unknown name or shape gives [`ToolEvent::Unrecognised`] and is
    /// logged raw at debug level (ADP-11). May read the session's own files through `context`.
    /// - Claude Code: `Notification`, `Stop`, `StopFailure`, `UserPromptSubmit` (SDD §11.4).
    /// - Codex: `SessionStart` (→ tool id known), `PermissionRequest`, `Stop`, `Interrupt`, `UserPromptSubmit`.
    /// - Copilot: its camelCase names and fields where they differ (`sessionId`); `errorOccurred` triggers a
    ///   read of the latest `session.error` in the session's `events.jsonl`; `permissionRequest` is ignored.
    /// - OpenCode: dotted plugin event names (`permission.asked`, `form.created`,
    ///   `session.execution.succeeded`, `session.retry.scheduled`, `session.usage.updated`…), de-duplicated by
    ///   their `id`.
    fn map_hook(&self, event: &str, payload: &Value, context: SignalContext<'_>) -> Vec<ToolEvent>;

    /// A file the service watches while the agent runs, from its tool id: Codex's session file (failed turns,
    /// limits, tokens and windows that no hook reports). `None` for Claude Code, Copilot and OpenCode, whose
    /// signals all come through hooks.
    fn watched_file(&self, tool_id: &str, cwd: &Path) -> Option<PathBuf>;

    /// Turns lines newly added to [`Self::watched_file`] (whole lines only) into Otaq's events. Codex: a
    /// `task_complete` error → failed, or limit reached for `usage_limit_reached` / `usage_limit_exceeded`;
    /// `token_count` → limit windows (`rate_limits`). Nothing for the other tools.
    fn map_watched_lines(&self, lines: &str, context: SignalContext<'_>) -> Vec<ToolEvent>;

    // 5. Usage (USG-09, REC-02).

    /// Where the tokens of conversation `tool_id` in `cwd` come from. Claude Code: its transcript and helper
    /// agents' files. Codex: its session file (`token_count`; cached input is part of its input, so the reader
    /// subtracts it). Copilot: `session-state/<id>/events.jsonl` (`session.usage_record`). OpenCode: its
    /// plugin events ([`TokenSource::Events`]).
    fn token_source(&self, tool_id: &str, cwd: &Path) -> TokenSource;

    /// The last `limit` messages and actions of conversation `tool_id` ("Before the restart", REC-02); `None`
    /// when it can't be read. Claude Code: its transcript. Codex: its session file. Copilot: its
    /// `events.jsonl`. OpenCode: `opencode session export <id>`. May read files.
    fn last_entries(&self, tool_id: &str, cwd: &Path, limit: usize)
    -> Option<Vec<TranscriptEntry>>;

    /// Where the tool's usage windows come from. Claude Code: its status line ([`WindowsSource::Report`]).
    /// Codex and Copilot: their own files. OpenCode: not reported.
    fn windows_source(&self) -> WindowsSource;

    /// The windows in a report (see [`WindowsSource::Report`]); `None` when it has none (a Claude Code API-key
    /// user, or before the first response). Claude Code: `rate_limits.five_hour` (300 minutes) and
    /// `seven_day` (10,080 minutes). Codex could use it for its app-server's `account/rateLimits/read`.
    fn windows_from_report(&self, report: &Value) -> Option<Vec<LimitWindow>>;

    /// The key of the window a limit of `kind` belongs to, for the reset time (USG-04). Claude Code: session →
    /// `five_hour`, weekly → `seven_day`. Codex: `primary` / `secondary` by length. Copilot: `chat`.
    fn limit_window_key(&self, kind: LimitKind) -> Option<&'static str>;

    // 6. Input quirks (ADP-06).

    /// How Otaq types into the tool. Claude Code and Copilot: Ctrl+E Ctrl+U, Enter at once, a multi-line draft
    /// clears. Codex: Ctrl+E Ctrl+U, Enter about 0.1 s after the text (its paste detection), only the current
    /// line of a multi-line draft clears. OpenCode: Ctrl+E Ctrl+U, Enter at once, only the current line clears.
    fn input(&self) -> InputQuirks;

    // 7. Skills (ADP-05).

    /// One of Otaq's own skills named for the agent: `/otaq:commit` (Claude Code, Copilot), `$otaq:commit`
    /// (Codex), OpenCode's own form.
    fn skill_reference(&self, skill: &str) -> String;

    /// How Otaq's skills reach the tool.
    fn skill_delivery(&self) -> SkillDelivery;
}

/// Every adapter, built once at service start. Cheap to clone.
#[derive(Clone)]
pub struct Adapters {
    claude: Arc<claude::ClaudeCode>,
}

impl Adapters {
    pub fn new(claude: claude::ClaudeCode) -> Self {
        Self {
            claude: Arc::new(claude),
        }
    }

    /// Adapters that launch with `launch` and read none of the tools' own files (no PATH, no config folders),
    /// for parts of the service built on their own (tests).
    pub fn launching_only(launch: Option<LaunchContext>, data: crate::data_dir::DataDir) -> Self {
        Self::new(claude::ClaudeCode::new(claude::ClaudeContext {
            launch,
            check: crate::tools::CheckEnv {
                path: OsString::new(),
                claude_config_dir: None,
                managed_settings_dir: PathBuf::new(),
            },
            data,
            trust: None,
        }))
    }

    /// The adapter of `tool`.
    pub fn get(&self, tool: Tool) -> &dyn AgentTool {
        match tool {
            Tool::Claude => self.claude.as_ref(),
        }
    }

    /// The adapter a new session runs on: Claude Code, until named agents choose one (M22).
    pub fn for_new_session(&self) -> &dyn AgentTool {
        self.claude.as_ref()
    }

    /// The adapter of a session's `tool`; a session with none (a stand-in in tests) types as a new session's
    /// tool would.
    pub fn of_session(&self, tool: Option<Tool>) -> &dyn AgentTool {
        tool.map_or_else(|| self.for_new_session(), |tool| self.get(tool))
    }

    /// Why a launch on `tool` (`None`: a new session's tool) can't be built at all ([`AgentTool::unavailable`]).
    pub fn unavailable(&self, tool: Option<Tool>) -> Result<(), LaunchError> {
        let adapter = tool.map_or_else(|| self.for_new_session(), |tool| self.get(tool));
        adapter.unavailable().map_or(Ok(()), Err)
    }

    /// Builds the launch of `request` on `tool` (a resume: the tool the conversation ran on), or on
    /// [`Self::for_new_session`] for a new conversation.
    pub fn launch(
        &self,
        tool: Option<Tool>,
        request: &LaunchRequest,
    ) -> Result<Launched, LaunchError> {
        let adapter = tool.map_or_else(|| self.for_new_session(), |tool| self.get(tool));
        let plan = adapter.launch(request)?;
        let tool_session_id = match &plan.tool_id {
            ToolIdMode::ChosenUpFront(id) => Some(id.clone()),
            // No tool of M20 creates its conversation before launch (OpenCode, M23 runs the step).
            ToolIdMode::CreatedBeforeLaunch { .. } => {
                return Err(LaunchError(format!(
                    "{} needs a step before launch that this version can't run",
                    adapter.display_name()
                )));
            }
            // Stored once the tool reports it.
            ToolIdMode::CapturedAfterStart => None,
        };
        Ok(Launched {
            tool: adapter.tool(),
            tool_session_id,
            model: request.model.clone(),
            plan,
        })
    }
}

/// A launch ready to spawn, with what the session stores (ADP-04): the tool, its id (empty until a tool that
/// reports it after start does) and the model as launched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launched {
    pub tool: Tool,
    pub tool_session_id: Option<String>,
    pub model: Option<String>,
    pub plan: LaunchPlan,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_stores_and_parses_as_claude() {
        assert_eq!(Tool::Claude.as_str(), "claude");
        assert_eq!("claude".parse::<Tool>().unwrap(), Tool::Claude);
        assert_eq!(Tool::ALL, &[Tool::Claude]);
        assert!("codex".parse::<Tool>().is_err());
        assert_eq!(serde_json::to_string(&Tool::Claude).unwrap(), "\"claude\"");
    }

    /// A new session runs on Claude Code; what the launch returns is what the session stores (ADP-04).
    #[test]
    fn the_registry_launches_and_says_what_to_store() {
        let home = tempfile::tempdir().unwrap();
        let data = crate::data_dir::DataDir::new(home.path());
        let request = LaunchRequest {
            role: LaunchRole::TicketAgent,
            cwd: "/wt/SHOP-18".into(),
            project: "SHOP".into(),
            ticket: Some("SHOP-18".parse().unwrap()),
            epic: None,
            port: 4012,
            first_prompt: Some(FirstPrompt::OtaqSkill {
                name: WORK_TICKET_SKILL.into(),
                message: None,
            }),
            resume: Some("c-1".into()),
            model: Some("opus".into()),
        };

        let none = Adapters::launching_only(None, data.clone());
        assert!(none.unavailable(None).is_err());
        assert!(none.launch(None, &request).is_err());

        let launch = LaunchContext {
            bin_dir: "/opt/cb/bin".into(),
            plugin_dir: "/opt/cb/plugin".into(),
            path: OsString::new(),
            claude: claude::ClaudeLaunch {
                program: vec![Tool::Claude.command_name().into()],
                config_dir: None,
            },
        };
        let adapters = Adapters::launching_only(Some(launch), data);
        assert_eq!(adapters.unavailable(Some(Tool::Claude)), Ok(()));
        assert_eq!(adapters.for_new_session().tool(), Tool::Claude);
        assert_eq!(adapters.of_session(None).tool(), Tool::Claude);
        let launched = adapters.launch(Some(Tool::Claude), &request).unwrap();
        assert_eq!(launched.tool, Tool::Claude);
        assert_eq!(launched.tool_session_id.as_deref(), Some("c-1"));
        assert_eq!(launched.model.as_deref(), Some("opus"));
        assert_eq!(
            launched.plan.tool_id,
            ToolIdMode::ChosenUpFront("c-1".into())
        );
    }

    #[test]
    fn tool_names_without_the_registry() {
        assert_eq!(Tool::Claude.short_name(), "Claude");
        assert_eq!(Tool::Claude.command_name(), "claude");
        assert_eq!(
            Tool::Claude.resume_hint("3be94c10"),
            "claude --resume 3be94c10"
        );
    }
}
