//! Agent tools (ADP-01, ADP-02, ADP-06; d0086, d0088): one adapter per agent CLI behind one interface,
//! [`AgentTool`]. The scheduler, board, cards, limits and UI talk only to the interface and never branch on the
//! tool; nothing outside `agent_tools/<tool>/` names a tool's flags, files, payload fields or syntax.
//!
//! Each adapter answers d0088's seven questions, one group of methods each: detect; launch and resume; one-time
//! setup; signals; usage; input quirks; skills. The doc comment of every method says how each tool of version 1
//! fills it, from `reports/Hands-on check of Codex, Copilot and OpenCode (2026-10-17).md` (Codex 0.157.1,
//! Copilot 1.0.98, OpenCode 2.0.17) and the Copilot real-account check of the same day. Each tool has its own
//! folder: [`claude`], [`codex`], [`copilot`], [`opencode`] (M23 builds the last three's groups file by file).
//!
//! The interface is a plain object-safe trait used as `&dyn AgentTool` through the registry, [`Adapters`]. The
//! two steps that run programs and take a while (detect, the tool-wide setup) return a boxed future, so no
//! async-trait crate is needed. OpenCode's step before launch is not a future but a [`PrepareStep`] the service
//! runs: it is a plain command, so launch tests can check it exactly without running the tool (d0088 testing).

pub mod catalog;
pub mod claude;
pub mod codex;
pub mod connected;
pub mod copilot;
pub mod events;
pub mod follow;
pub mod models;
pub mod opencode;
pub mod setup;

use std::ffi::{OsStr, OsString};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::board::settings::{WorkflowSkills, WorkflowStep};
use crate::domain::{TicketKey, stored_enum};
pub use crate::recovery::TranscriptEntry;
pub use crate::tools::Os;
use crate::usage::Tokens;
use crate::usage::limit::LimitKind;
pub use events::{Credits, LimitWindow, ToolEvent};
pub use models::{ModelKind, ModelList, ModelListError};
pub use setup::SetupOutcome;

stored_enum! {
    /// An agent CLI Otaq can run, as stored on sessions and named agents, in d0086's order (Claude Code, Codex,
    /// Copilot, OpenCode). Only [`Adapters::get`] and the names below match on it.
    pub enum Tool ("agent tool") {
        /// Claude Code, the `claude` CLI.
        Claude = "claude",
        /// Codex, the `codex` CLI (ADP-07).
        Codex = "codex",
        /// GitHub Copilot, the `copilot` CLI (ADP-08).
        Copilot = "copilot",
        /// OpenCode, the `opencode` CLI (ADP-09).
        OpenCode = "opencode",
    }
}

/// The names a view shows for a tool's sessions, without the registry (the view models have none). Each is the
/// same as the adapter's own method.
impl Tool {
    /// [`AgentTool::display_name`]: "Claude Code", "GitHub Copilot".
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Claude => claude::DISPLAY_NAME,
            Self::Codex => codex::DISPLAY_NAME,
            Self::Copilot => copilot::DISPLAY_NAME,
            Self::OpenCode => opencode::DISPLAY_NAME,
        }
    }

    /// [`AgentTool::short_name`]: "Claude", "Copilot".
    pub fn short_name(self) -> &'static str {
        match self {
            Self::Claude => claude::SHORT_NAME,
            Self::Codex => codex::SHORT_NAME,
            Self::Copilot => copilot::SHORT_NAME,
            Self::OpenCode => opencode::SHORT_NAME,
        }
    }

    /// [`AgentTool::command_name`]: `claude`, `codex`, `copilot`, `opencode`.
    pub fn command_name(self) -> &'static str {
        match self {
            Self::Claude => claude::command_name(),
            Self::Codex => codex::COMMAND_NAME,
            Self::Copilot => copilot::COMMAND_NAME,
            Self::OpenCode => opencode::COMMAND_NAME,
        }
    }

    /// [`AgentTool::resume_hint`]: "claude --resume 3be94c10".
    pub fn resume_hint(self, tool_id: &str) -> String {
        match self {
            Self::Claude => claude::resume_hint(tool_id),
            Self::Codex => codex::launch::resume_hint(tool_id),
            Self::Copilot => copilot::launch::resume_hint(tool_id),
            Self::OpenCode => opencode::launch::resume_hint(tool_id),
        }
    }

    /// [`AgentTool::swaps_refused_model`]: the tool runs another model quietly when it can't run the one asked for
    /// (the panel's "asked for gpt-5.5, ran on auto").
    pub fn swaps_refused_model(self) -> bool {
        match self {
            Self::Claude | Self::Codex => false,
            Self::Copilot | Self::OpenCode => true,
        }
    }
}

/// A boxed future, so the trait stays object-safe.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What [`AgentTool::detect`] found (ADP-10).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detection {
    /// Found on PATH (and, for the M23 tools, its version read).
    pub installed: bool,
    /// Signed in; `None` when the tool can't say without a model call.
    pub signed_in: Option<bool>,
    /// The version as the tool prints it.
    pub version: Option<String>,
    /// Found, but older than Otaq can run: "Found · update needed" (Copilot before 1.0.98, Q-G). Never launched.
    pub update_needed: bool,
    /// How the tool is signed in, where it says ("signed in with ChatGPT"), for the Connected line.
    pub account: Option<String>,
    /// **Sign in…** is offered although the tool counts as Connected: it runs, but signing in gives it more
    /// (OpenCode with only its free models, Q-K).
    pub sign_in_offered: bool,
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

/// Where Codex's, Copilot's and OpenCode's adapters find things on this machine: what launches share, the PATH
/// detect searches, and the user's home folder, under which each tool keeps its own config (each adapter finds its
/// folder there, or from its own variable such as `CODEX_HOME`). With `home` `None` (tests, parts of the service
/// built on their own) an adapter reads and writes none of its tool's files.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Otaq's folders and PATH for launches; `None` when Otaq's plugin wasn't found.
    pub launch: Option<LaunchContext>,
    /// The PATH searched for the tool's program by detect.
    pub path: OsString,
    /// The user's home folder; `None` reads no tool config.
    pub home: Option<PathBuf>,
    /// Otaq's data folder.
    pub data: crate::data_dir::DataDir,
}

/// How long a tool's `--version` may take (as the Tools checklist).
const VERSION_TIMEOUT: Duration = Duration::from_secs(15);

/// The part of detect every tool shares (ADP-10): `command` found on `path`, and what its `--version` printed on
/// stdout (`None` when it failed or took longer than [`VERSION_TIMEOUT`]). `None` when it isn't on `path`.
pub(crate) async fn version_output(command: &str, path: &OsStr) -> Option<Option<String>> {
    let program = crate::tools::find_on_path(command, path)?;
    let mut run = tokio::process::Command::new(&program);
    run.arg("--version")
        .env("PATH", path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(VERSION_TIMEOUT, run.output()).await;
    Some(match output {
        Ok(Ok(output)) if output.status.success() => {
            Some(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        _ => None,
    })
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
    /// A workflow skill of `step` as Settings stores it (Validate, Open PR, Push; REV-09), named by the adapter for
    /// the launch's folder ([`AgentTool::workflow_skill`]): Otaq's own in the tool's syntax, the user's by its
    /// SKILL.md path ("Follow `<path>`") when found, else as stored, or the step's built-in on a tool that can't
    /// run it by name.
    WorkflowSkill { step: WorkflowStep, stored: String },
    /// Text passed as it is (the PR resume's "push now").
    Text(String),
}

/// A workflow skill (Commit, Push, Open PR, Validate; REV-09) as named to one agent (ADP-05, d0086). Serialised
/// as the name itself, `{"file": "<path>"}`, or `{"skill": "$otaq:commit", "instead_of": "/their-plugin:commit"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SkillRef {
    /// Run by name: Otaq's own in the tool's syntax (`/otaq:commit`), or the user's as stored when no SKILL.md
    /// was found for it and the tool runs skills by name (`/my-commit`, `/their-plugin:commit` on Claude Code).
    Named(String),
    /// The user's skill, found as a file: the agent follows its SKILL.md. Otaq copies nothing.
    File { file: PathBuf },
    /// Otaq's built-in for the step (`skill`, in the tool's syntax) instead of the stored `instead_of`, which this
    /// tool can't run: not Otaq's, not found as a file, and the tool doesn't run skills by name (d0086: "my-commit:
    /// Claude Code only, other tools use the built-in").
    BuiltIn { skill: String, instead_of: String },
}

impl SkillRef {
    /// How a first prompt or a typed delivery runs it: the name, or "Follow `<path>`".
    pub fn prompt(&self) -> String {
        match self {
            Self::Named(name) | Self::BuiltIn { skill: name, .. } => name.clone(),
            Self::File { file } => format!("Follow `{}`", file.display()),
        }
    }
}

/// The folder name a user's skill would have, from its stored value (`/my-commit` → `my-commit`); `None` for a
/// value that can't be one folder (`/their-plugin:commit`, a plugin's skill).
fn skill_folder_name(stored: &str) -> Option<&str> {
    let name = stored.trim().trim_start_matches('/');
    let plain = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    (plain && name.starts_with(|c: char| c.is_ascii_alphanumeric())).then_some(name)
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
    /// The model, passed to the tool unchanged (d0088); `None` uses the tool's default. A new session's comes from
    /// its resolved named agent, a resume's from the session it continues ([`crate::named_agents::RunsOn`]).
    pub model: Option<String>,
    /// The ticket's or the epic's own title ("Add coupon codes"), for a tool that names its conversations (OpenCode's
    /// session list: "Otaq · SHOP-18 · Add coupon codes"); `None` for a Project agent.
    pub title: Option<String>,
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

/// A one-off read-only question ([`AgentTool::one_off`]): `prompt`, asked in `cwd`, on `model` (passed to the tool
/// unchanged; `None` passes no model option, d0088).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneOffRequest {
    pub cwd: PathBuf,
    pub prompt: String,
    pub model: Option<String>,
}

/// What a one-off question runs to completion: the program and its arguments, in `cwd`, with `env` added to the
/// service's environment (OpenCode's read-only rules). Its progress and answer come on stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneOffCommand {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
}

/// One line of a one-off question's output as its tool's adapter reads it ([`AgentTool::one_off_line`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OneOffLine {
    /// Tools the agent used, in order: progress.
    Tools(Vec<OneOffTool>),
    /// The final answer, or the tool's error instead of one.
    Answer { text: String, is_error: bool },
}

/// A tool a one-off question used: its name as the tool says it (`Read`, `shell`) and what it looked at (a path,
/// a pattern, a command), when it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneOffTool {
    pub name: String,
    pub target: Option<String>,
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

/// A hook an adapter doesn't map (yet): [`ToolEvent::Unrecognised`], with the payload logged raw at debug level
/// (ADP-11).
pub(crate) fn unrecognised(tool: Tool, event: &str, payload: &Value) -> Vec<ToolEvent> {
    tracing::debug!(%tool, event, payload = %payload, "a hook the adapter doesn't recognise");
    vec![ToolEvent::Unrecognised {
        event: event.to_owned(),
    }]
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

    /// What **Sign in…** runs in a terminal for a found tool that isn't signed in (d0086); `None` when the tool has
    /// no sign-in command of its own. Claude Code: `None` ("Run claude and sign in" stays the checklist's text).
    /// Codex: `codex login`. Copilot: its lane verifies it (T-0353). OpenCode: `opencode auth login --standalone`.
    fn login_command(&self) -> Option<Vec<String>> {
        None
    }

    /// **How to install**: the copyable command for `os` (SET-02, d0086). Otaq never runs it.
    fn install_command(&self, os: Os) -> &'static str;

    /// The permission warning for this tool (SET-02, d0011): `Some(the warning's title)` when the user's own
    /// settings make new sessions stop on permission prompts, `None` otherwise or when it can't be told. Claude
    /// Code: its `permissions.defaultMode`, as the Tools checklist evaluated it. Reads the tool's settings
    /// read-only.
    fn permission_warning(&self) -> Option<String> {
        None
    }

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

    /// Whether a resumed session's terminal output says the tool couldn't find the conversation (REC-03: Resume
    /// failed, never a fallback). Claude Code: `claude --resume <id>` prints "No conversation found with session
    /// ID: …".
    fn conversation_not_found(&self, output: &[u8]) -> bool;

    /// How the user resumes the conversation by hand, for the recovery texts: "claude --resume 3be94c10",
    /// "codex resume …", "copilot --resume …", "opencode --session …". `tool_id` may be shortened.
    fn resume_hint(&self, tool_id: &str) -> String;

    /// The models the tool says it can run, read from the tool itself (AGT-02, d0086): the picker offers only
    /// these. Started only by the service ([`models::ModelLists`]), never by a test; makes no model call.
    /// - Claude Code: the SDK `initialize` control request (`claude -p --input-format stream-json
    ///   --output-format stream-json --verbose`, the request on stdin), its `models[]`: `default` (the tool
    ///   default), its aliases `opus`, `sonnet`, `haiku`, `fable`, each with what it resolves to today; 10 s at
    ///   most.
    fn list_models(&self) -> BoxFuture<'_, Result<ModelList, ModelListError>>;

    /// The command of a one-off read-only question, not shown in the pane and kept for no Resume ("Ask the agent"
    /// for the run command, RUN-02, d0086): only the tool's reading tools; the answer is the final result. Run by
    /// [`crate::terminals::suggest`], which reads its output.
    /// - Claude Code: `claude -p --output-format stream-json --verbose --tools Read,Glob,Grep [--model <m>]
    ///   <prompt>`; refused without a launch context.
    /// - OpenCode: `opencode run --standalone --format json [-m <m>] <prompt>` with rules that allow only reading
    ///   in `OPENCODE_CONFIG_CONTENT`.
    fn one_off(&self, request: &OneOffRequest) -> Result<OneOffCommand, LaunchError>;

    /// One line of [`Self::one_off`]'s output: the tools it used, its answer, or `None` for anything else.
    /// - Claude Code: stream-json `assistant` `tool_use` blocks, then the `result` (`is_error`).
    /// - Codex: `codex exec --json` `item.started` `command_execution`, the last `item.completed`
    ///   `agent_message`, `turn.failed`.
    /// - OpenCode: `opencode run --format json` `tool_use` parts, the last `text` part, an `error` line.
    fn one_off_line(&self, _line: &str) -> Option<OneOffLine> {
        None
    }

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

    /// Whether a Connected tool's one-time setup is missing from its config (cheap: one file read, no process), so
    /// the next check runs [`Self::set_up_tool`] again. Codex: Otaq's plugin isn't installed from this plugin
    /// folder, or one of its hooks has no recorded trust. The others: never.
    fn setup_needed(&self) -> bool {
        false
    }

    // 4. Signals (ADP-02, ADP-03).

    /// Turns one hook (`otaq hook --tool <tool> <event>`, the event name as text, the JSON from stdin) into
    /// Otaq's events: none, one or several. An unknown name or shape gives [`ToolEvent::Unrecognised`] and is
    /// logged raw at debug level (ADP-11). May read the session's own files through `context`.
    /// - Claude Code: `Notification`, `Stop`, `StopFailure`, `UserPromptSubmit`, `SessionStart` (SDD §11.4), and
    ///   `Status`, the status line's model sent by `otaq statusline`.
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
    /// `five_hour`, weekly → `seven_day`. Codex: `session` / `weekly` (its windows keyed by length). Copilot:
    /// `chat`.
    fn limit_window_key(&self, kind: LimitKind) -> Option<&'static str>;

    /// When a limit message says the limit resets, after `now` (Unix milliseconds): the reset time's fallback
    /// when no window gives it (USG-04). Claude Code: "You've hit your session limit · resets 3:45pm" → the next
    /// local 3:45 pm. Tools whose limit signal carries the reset itself ([`ToolEvent::LimitReached`]'s
    /// `resets_at`: Copilot's `resetDate`, a Codex limit error) answer `None`.
    fn limit_reset_from_text(&self, text: &str, now: i64) -> Option<i64>;

    // 6. Input quirks (ADP-06).

    /// How Otaq types into the tool. Claude Code and Copilot: Ctrl+E Ctrl+U, Enter at once, a multi-line draft
    /// clears. Codex: Ctrl+E Ctrl+U, Enter about 0.1 s after the text (its paste detection), only the current
    /// line of a multi-line draft clears. OpenCode: Ctrl+E Ctrl+U, Enter at once, only the current line clears.
    fn input(&self) -> InputQuirks;

    /// Whether a typed key while the tool shows a prompt of its own answers that prompt (Copilot: Esc at its
    /// permission prompt ends the turn without a hook, d0086), so Otaq checks the turn after it. Only Copilot.
    fn typed_key_answers_prompt(&self) -> bool {
        false
    }

    // Turn ends and models (ADP-12, AGT-05).

    /// Whether the tool's turn end says what the turn left running (Claude Code's `background_tasks`). When not
    /// (Codex, Copilot, OpenCode), Otaq checks the agent's process tree (ADP-12).
    fn turn_end_reports_background(&self) -> bool {
        true
    }

    /// Whether the tool quietly runs another model when it refuses the one asked for (AGT-05: Copilot, OpenCode),
    /// so "asked for X, ran on Y" is read from the model that ran. Claude Code and Codex refuse instead.
    fn swaps_refused_model(&self) -> bool {
        false
    }

    // 7. Skills (ADP-05).

    /// One of Otaq's own skills named for the agent: `/otaq:commit` (Claude Code, Copilot), `$otaq:commit`
    /// (Codex), OpenCode's own form.
    fn skill_reference(&self, skill: &str) -> String;

    /// How Otaq's skills reach the tool.
    fn skill_delivery(&self) -> SkillDelivery;

    /// The SKILL.md of the user's skill `name` (a folder name, `my-commit`) where this tool would find it for an
    /// agent working in `cwd`, the first that exists; `None` when there is none. Read-only (d0088: Otaq copies
    /// nothing). From the hands-on check's row 6:
    /// - Claude Code: the project's `.claude/skills/<name>/SKILL.md` (in `cwd`), then the user's
    ///   `<Claude config folder>/skills/<name>/SKILL.md` (`$CLAUDE_CONFIG_DIR` or `~/.claude`).
    /// - Codex: `$CODEX_HOME/skills`, `~/.agents/skills`, the repo's `.agents/skills` and `.codex/skills`.
    /// - Copilot: the project's `.github/skills`, `.agents/skills` and `.claude/skills`; the user's
    ///   `~/.copilot/skills` and `~/.agents/skills` (not `~/.claude/skills`).
    /// - OpenCode: the project's `.claude/skills`, `.agents/skills` and `.opencode/skills`; the user's
    ///   `~/.claude/skills` and `~/.agents/skills`.
    fn user_skill_file(&self, name: &str, cwd: &Path) -> Option<PathBuf>;

    /// Whether the tool runs a skill it can't find as a file by its name (`/their-plugin:commit`, a skill of
    /// another plugin; d0086). Claude Code only.
    fn runs_skills_by_name(&self) -> bool {
        true
    }

    /// How the workflow skill `stored` of `step` (as Settings stores it) is named to an agent of this tool working
    /// in `cwd` (ADP-05, d0086): Otaq's own (today's defaults, `/otaq:commit`) by [`Self::skill_reference`]; the
    /// user's by its SKILL.md path when [`Self::user_skill_file`] finds it; else by name as stored where the tool
    /// runs skills by name ([`Self::runs_skills_by_name`], Claude Code), and the step's built-in on the others.
    fn workflow_skill(&self, step: WorkflowStep, stored: &str, cwd: &Path) -> SkillRef {
        if let Some(own) = WorkflowSkills::own_skill_of(stored) {
            return SkillRef::Named(self.skill_reference(own));
        }
        // An imported skill comes as its copy's SKILL.md path (`ResolvedSkill::stored`, d0092).
        let path = Path::new(stored.trim());
        if path.is_absolute() && path.is_file() {
            return SkillRef::File {
                file: path.to_owned(),
            };
        }
        if let Some(file) =
            skill_folder_name(stored).and_then(|name| self.user_skill_file(name, cwd))
        {
            return SkillRef::File { file };
        }
        let stored = stored.trim().to_owned();
        if self.runs_skills_by_name() {
            SkillRef::Named(stored)
        } else {
            SkillRef::BuiltIn {
                skill: self.skill_reference(step.own_skill()),
                instead_of: stored,
            }
        }
    }

    // Model families (AGT-02, d0086).

    /// Where the tool's "newest in a family" choices take their families and release dates from
    /// ([`catalog`]); `None` when the tool has none: Claude Code (its aliases already follow), Copilot (Auto).
    /// - Codex: [`catalog::FamilySource::ModelsDev`] `openai` (its ids are models.dev's `openai` ids exactly).
    /// - OpenCode: [`catalog::FamilySource::ListCarriesFamily`] with the models.dev fallback.
    fn family_source(&self) -> Option<catalog::FamilySource> {
        None
    }
}

/// Every adapter, built once at service start. Cheap to clone.
#[derive(Clone)]
pub struct Adapters {
    claude: Arc<claude::ClaudeCode>,
    codex: Arc<codex::Codex>,
    copilot: Arc<copilot::Copilot>,
    opencode: Arc<opencode::OpenCode>,
}

impl Adapters {
    /// Claude Code's adapter, and the other three tools' on `tools`.
    pub fn new(claude: claude::ClaudeCode, tools: ToolContext) -> Self {
        Self {
            claude: Arc::new(claude),
            codex: Arc::new(codex::Codex::new(tools.clone())),
            copilot: Arc::new(copilot::Copilot::new(tools.clone())),
            opencode: Arc::new(opencode::OpenCode::new(tools)),
        }
    }

    /// Adapters that launch with `launch` and read none of the tools' own files (no PATH, no config folders),
    /// for parts of the service built on their own (tests).
    pub fn launching_only(launch: Option<LaunchContext>, data: crate::data_dir::DataDir) -> Self {
        let tools = ToolContext {
            launch: launch.clone(),
            path: OsString::new(),
            home: None,
            data: data.clone(),
        };
        Self::new(
            claude::ClaudeCode::new(claude::ClaudeContext {
                launch,
                check: crate::tools::CheckEnv {
                    path: OsString::new(),
                    claude_config_dir: None,
                    managed_settings_dir: PathBuf::new(),
                },
                data,
                trust: None,
            }),
            tools,
        )
    }

    /// The adapter of `tool`.
    pub fn get(&self, tool: Tool) -> &dyn AgentTool {
        match tool {
            Tool::Claude => self.claude.as_ref(),
            Tool::Codex => self.codex.as_ref(),
            Tool::Copilot => self.copilot.as_ref(),
            Tool::OpenCode => self.opencode.as_ref(),
        }
    }

    /// The adapter a new session runs on when no named agent names a tool (none exist yet; M22's sessions without
    /// a tool in tests): Claude Code. New sessions always launch on their resolved agent's tool.
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
        let (tool_session_id, prepare) = match &plan.tool_id {
            ToolIdMode::ChosenUpFront(id) => (Some(id.clone()), None),
            // The session runs the step before the program (OpenCode creates its conversation this way).
            ToolIdMode::CreatedBeforeLaunch { id, step } => (Some(id.clone()), Some(step.clone())),
            // Stored once the tool reports it.
            ToolIdMode::CapturedAfterStart => (None, None),
        };
        Ok(Launched {
            tool: adapter.tool(),
            tool_session_id,
            model: request.model.clone(),
            prepare,
            plan,
        })
    }
}

/// A launch ready to spawn, with what the session stores (ADP-04): the tool, its id (empty until a tool that
/// reports it after start does) and the model as launched, and the step the session runs before the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launched {
    pub tool: Tool,
    pub tool_session_id: Option<String>,
    pub model: Option<String>,
    /// [`ToolIdMode::CreatedBeforeLaunch`]'s step, for [`crate::session::Sessions::prepared`].
    pub prepare: Option<PrepareStep>,
    pub plan: LaunchPlan,
}

/// What the adapters' tests share: a tool context on a test PATH, and a stand-in program on it that prints what a
/// tool's `--version` would (never the tool itself).
#[cfg(test)]
mod test_support {
    use std::path::Path;

    use super::ToolContext;

    /// A context whose PATH is `bin` only, with `home` as the home folder.
    pub fn context(bin: &Path, home: Option<&Path>) -> ToolContext {
        ToolContext {
            launch: None,
            path: bin.as_os_str().to_owned(),
            home: home.map(Path::to_path_buf),
            data: crate::data_dir::DataDir::new(bin.join("data")),
        }
    }

    /// An executable `name` in `bin` that prints `stdout` and exits 0.
    pub fn stand_in(bin: &Path, name: &str, stdout: &str) {
        use std::os::unix::fs::PermissionsExt;
        let file = bin.join(name);
        std::fs::write(&file, format!("#!/bin/sh\nprintf '%s' '{stdout}'\n")).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// A SKILL.md at `<root>/skills/<name>/`.
    pub fn write_skill(root: &Path, name: &str) -> std::path::PathBuf {
        let file = root.join("skills").join(name).join("SKILL.md");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "---\nname: x\ndescription: x\n---\n").unwrap();
        file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four tools, stored by their command names, in d0086's order.
    #[test]
    fn tools_store_and_parse_by_name() {
        assert_eq!(
            Tool::ALL,
            &[Tool::Claude, Tool::Codex, Tool::Copilot, Tool::OpenCode]
        );
        for (tool, name) in Tool::ALL
            .iter()
            .zip(["claude", "codex", "copilot", "opencode"])
        {
            assert_eq!(tool.as_str(), name);
            assert_eq!(name.parse::<Tool>().unwrap(), *tool);
            assert_eq!(serde_json::to_string(tool).unwrap(), format!("\"{name}\""));
            assert_eq!(tool.command_name(), name);
        }
        assert!("cursor".parse::<Tool>().is_err());
        assert!("OpenCode".parse::<Tool>().is_err());
    }

    /// Every adapter is the registry's for its tool, says the same names as [`Tool`], and never clears a line with
    /// Ctrl+C (ADP-06).
    #[test]
    fn the_registry_has_every_tool_and_no_tool_types_ctrl_c() {
        let home = tempfile::tempdir().unwrap();
        let adapters = Adapters::launching_only(None, crate::data_dir::DataDir::new(home.path()));
        for &tool in Tool::ALL {
            let adapter = adapters.get(tool);
            assert_eq!(adapter.tool(), tool);
            assert_eq!(adapter.display_name(), tool.display_name());
            assert_eq!(adapter.short_name(), tool.short_name());
            assert_eq!(adapter.command_name(), tool.command_name());
            assert_eq!(
                adapter.resume_hint("3be94c10"),
                tool.resume_hint("3be94c10")
            );
            assert_eq!(adapter.swaps_refused_model(), tool.swaps_refused_model());
            let quirks = adapter.input();
            assert!(!quirks.clear_line.contains(&0x03), "{tool} types Ctrl+C");
            assert_eq!(quirks.clear_line, b"\x05\x15", "{tool}");
        }
    }

    /// OpenCode's launch (T-0354) needs Otaq's plugin folder; its one-off question doesn't (T-0355).
    #[test]
    fn opencode_launches_need_the_plugin_folder() {
        let home = tempfile::tempdir().unwrap();
        let adapters = Adapters::launching_only(None, crate::data_dir::DataDir::new(home.path()));
        let question = OneOffRequest {
            cwd: "/r".into(),
            prompt: "Suggest".into(),
            model: None,
        };
        assert!(adapters.unavailable(Some(Tool::OpenCode)).is_err());
        assert!(adapters.get(Tool::OpenCode).one_off(&question).is_ok());
    }

    /// A tool whose conversation is made before launch (OpenCode): the session stores the id like a chosen one
    /// and gets the step to run first; a tool without a step launches as before (ADP-04).
    #[test]
    fn a_step_before_launch_goes_with_the_launch() {
        let home = tempfile::tempdir().unwrap();
        let data = crate::data_dir::DataDir::new(home.path());
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
        let request = LaunchRequest {
            role: LaunchRole::TicketAgent,
            cwd: "/wt/SHOP-50".into(),
            project: "SHOP".into(),
            ticket: Some("SHOP-50".parse().unwrap()),
            epic: None,
            port: 4012,
            first_prompt: Some(FirstPrompt::OtaqSkill {
                name: WORK_TICKET_SKILL.into(),
                message: None,
            }),
            resume: None,
            model: None,
            title: None,
        };
        let launched = adapters.launch(Some(Tool::OpenCode), &request).unwrap();
        let ToolIdMode::CreatedBeforeLaunch { id, step } = &launched.plan.tool_id else {
            panic!("{:?}", launched.plan.tool_id)
        };
        assert_eq!(launched.tool_session_id.as_ref(), Some(id));
        assert_eq!(launched.prepare.as_ref(), Some(step));
        let claude = adapters.launch(Some(Tool::Claude), &request).unwrap();
        assert_eq!(claude.prepare, None);
        assert!(claude.tool_session_id.is_some());
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
            title: None,
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
        assert_eq!(Tool::Claude.display_name(), "Claude Code");
        assert_eq!(Tool::Claude.command_name(), "claude");
        assert_eq!(Tool::Copilot.display_name(), "GitHub Copilot");
        assert_eq!(Tool::Copilot.short_name(), "Copilot");
        assert_eq!(Tool::Codex.resume_hint("019a"), "codex resume 019a");
        assert_eq!(Tool::Copilot.resume_hint("5f1c"), "copilot --resume 5f1c");
        assert_eq!(
            Tool::OpenCode.resume_hint("ses_1"),
            "opencode --session ses_1"
        );
        assert_eq!(
            Tool::Claude.resume_hint("3be94c10"),
            "claude --resume 3be94c10"
        );
    }
}
