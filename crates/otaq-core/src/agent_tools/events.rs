//! Otaq's events (ADP-02, d0088): the fixed list every adapter turns its tool's hooks, plugin events or session
//! file into, and the only thing the rest of the app sees of a tool's signals.
//!
//! Beside *tool id known*, the list has *model known* and *model refused* (M22).
//!
//! An adapter never decides a card's status. *Turn ended* says what the turn left running and its last message;
//! whether that is Idle, Working in background or "turn ended after a submit" is Otaq's own rule in `signals/`.

use serde::{Deserialize, Serialize};

use crate::signals::background::BackgroundWork;
use crate::usage::Tokens;
use crate::usage::limit::LimitHit;

/// One thing a tool reported about its agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolEvent {
    /// The agent waits for the user: a permission prompt or another request (Claude Code `Notification` with a
    /// request type; Codex `PermissionRequest`; Copilot `Notification` `permission_prompt`; OpenCode
    /// `permission.asked`).
    NeedsYou {
        /// The tool's prompt text ("Run command: sleep 20").
        prompt: String,
        /// Raised inside a helper agent (a Claude Code subagent): Otaq marks the stored prompt so the card shows
        /// the bot icon.
        #[serde(default)]
        helper_agent: bool,
    },
    /// The tool shows the user a question of its own (Copilot `elicitation_dialog`, OpenCode `form.created`).
    /// Handled like *needs you* with its prompt. Claude Code's elicitation is a *needs you* (SDD §11.4), and
    /// Codex has no question event (a question there is plain text, then a turn end).
    AsksYou { prompt: String },
    /// The agent's turn ended (Claude Code and Copilot `Stop`, Codex `Stop` and `Interrupt`, OpenCode
    /// `session.execution.succeeded` / `.interrupted`).
    TurnEnded {
        /// What the turn left running, as the tool reports it (Claude Code's `background_tasks` and
        /// `session_crons`). Empty for tools that don't report it; Otaq then checks the agent's process tree
        /// (ADP-12).
        #[serde(default)]
        background: BackgroundWork,
        /// The agent's last message, whole, when the tool gives it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_message: Option<String>,
    },
    /// A new turn started (every tool's prompt submit; OpenCode `session.inbox.enqueued` /
    /// `session.execution.started`).
    NewTurn {
        /// The text that started the turn, when the tool gives it (Claude Code's `prompt`): a Project agent keeps
        /// its first request for its fallback name (PAG-04).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt: Option<String>,
    },
    /// The turn failed for a reason other than a usage limit.
    Failed {
        /// The tool's error kind (`invalid_request`, `provider.auth`, …), or "unknown".
        error: String,
        /// The tool's own message, shown as it is.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    /// The turn stopped on a usage limit (d0020, d0046).
    LimitReached {
        hit: LimitHit,
        /// When the limit resets, in Unix milliseconds, when the signal itself says so (Copilot's `resetDate`, a
        /// Codex limit error). `None` leaves it to the windows or the limit text (USG-04).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resets_at: Option<i64>,
    },
    /// The conversation's token totals so far (OpenCode `session.usage.updated`). Tools whose tokens are read
    /// from a file (`TokenSource::Reader`) don't send this.
    Tokens { totals: Tokens },
    /// The tool's current usage windows (Codex `token_count.rate_limits` in its session file). A report
    /// replaces the tool's set.
    LimitWindows { windows: Vec<LimitWindow> },
    /// The tool's own conversation id, for a tool that reports it after start (Codex `SessionStart`, or
    /// `CODEX_THREAD_ID`). Stored only while the session's tool id is empty.
    ToolIdKnown { tool_id: String },
    /// The exact model the session runs on, as the tool reports it (AGT-04; Claude Code's `SessionStart` `model`,
    /// or its status line's `model.id`): `claude-opus-5-5`. The tool may report it again (every status line,
    /// a resumed conversation's `SessionStart`); the first value a session gets is the one kept, a later
    /// different one is logged and ignored.
    ModelKnown { model: String },
    /// The tool started but can't run the model it was given: its first API call failed on the model (AGT-05,
    /// SDD §11.4; Claude Code's `StopFailure` with `model_not_found`, "There's an issue with the selected model
    /// (…). It may not exist or you may not have access to it."). Not a [`Self::Failed`].
    ModelRefused {
        /// The model the tool names in its message (`claude-fable-5-1`), when it names one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        /// The tool's own message, shown as it is.
        message: String,
    },
    /// An event name or payload the adapter doesn't know (ADP-11). The adapter has logged the raw payload at
    /// debug level; nothing changes.
    Unrecognised { event: String },
}

/// One usage window a tool reports (USG-09, d0088): how much of it is used and when it resets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitWindow {
    /// The tool's own key: `five_hour`, `seven_day` (Claude Code); `primary`, `secondary` (Codex); `chat`
    /// (Copilot).
    pub key: String,
    /// The window's length in minutes (300, 10,080), or `None` for a window without a fixed length (a Copilot
    /// plan month, a session).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_minutes: Option<u32>,
    /// The share used, 0–100.
    pub used_percent: f64,
    /// When the window resets, in Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<i64>,
    /// Credits, where the tool counts them (Copilot's AI credits).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<Credits>,
}

/// A window's credits.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Credits {
    pub used: f64,
    /// The window's allowance, when the tool gives it (Copilot Free: 200).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
}
