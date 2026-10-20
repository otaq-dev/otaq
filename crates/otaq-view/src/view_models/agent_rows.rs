//! Agent rows (AGT-04, AGT-05, VAL-14, USG-01; d0108, d0086, d0088, d0077): the stages of a ticket's flow that run an
//! agent and the Epic lead row of an epic, as the ⌘N form, the editor, the panel's Ticket tab and the epic forms show
//! them, from the facts of `ticket.agents` ([`TicketAgents`]) and the tools' cached model lists.
//!
//! Until T-0411 draws the ticket's path and its choices, the rows are read-only (d0108: the stage names the agent):
//!
//! - **A row** is the stage's name ("Dev"), the agent's name and its model as Settings shows it ("Opus · always the
//!   newest", "tool default"); with several tools the tool joins the line and the avatar shows.
//! - **Started** (AGT-04): the agent and the exact model the session ran on, "Strong" · "claude-opus-5-5 · started",
//!   with the lock; "(deleted)" once the agent is gone. "Sonnet → claude-sonnet-5-6 · new since the last run
//!   (claude-sonnet-5-5)" while its session runs; the swap note "asked for gpt-5.5, ran on auto" (AGT-05, Q-J).
//! - **The token line** in the panel's header (VAL-14, Q-G): "Strong 61k · Light 23k (1 round)"; with several tools
//!   "Strong · Claude Code 58k · Verifier · Copilot 9k (1 round)".
//! - **History lines**: session starts and ends with the agent and the exact model, M31's row changes and moves as
//!   they were recorded, and a Done ticket's "Developer: Light (deleted) · claude-haiku-5-5 · 48k".

use otaq_core::agent_tools::Tool;
use otaq_core::agent_tools::follow::FollowChoice;
use otaq_core::agent_tools::models::{ModelKind, ModelList};
use otaq_core::agents::facts::{StageAgent, TicketAgents};
use otaq_core::agents::mark::is_swap;
use otaq_core::agents::store::SessionAgentRun;
use otaq_core::agents::{Agent, SessionRole};
use otaq_core::domain::{Kind, StatusSet};
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::agents::{RoleId, TOOL_DEFAULT, model_label};
use crate::view_models::card::tokens_label;
use crate::view_models::tool_avatar::ToolAvatarView;
use crate::view_models::tool_problems::{self, Callout, Tools, tool_word};

pub const HEADING: &str = "Agents";
pub const TICKET_HINT: &str = "the agent each stage runs";
pub const EPIC_HINT: &str = "who leads the epic's children";
pub const SET_BY_YOU: &str = "set by you";
pub const LET_AGENTS_PICK: &str = "Let agents pick again";
/// The lock's tooltip on a started row (AGT-04).
pub const LOCKED: &str = "Locked: Send back continues the same conversation";
/// The lock's tooltip while a check's round runs.
pub const ROUND_LOCKED: &str = "Locked until this round ends";

/// Who set a row: neutral "set by you", or an agent's pick in violet (d0077). M31's recorded row changes only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentRowSetBy")]
pub struct SetByView {
    pub text: String,
    /// Agent-made: violet.
    pub agent_made: bool,
    pub icon: Option<IconName>,
}

/// One item of a row's picker (T-0411 offers the ticket's choices again).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentPickerItem")]
pub struct PickerItem {
    pub agent: Option<String>,
    pub label: String,
    pub detail: String,
    pub selected: bool,
    pub avatar: Option<ToolAvatarView>,
}

/// One agent row (`AgentRow` on the page, T-0344).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentRowView")]
pub struct RowView {
    pub role: RoleId,
    /// The stage's name ("Dev", "Validate"), or "Epic lead".
    pub label: String,
    /// Primary: "Strong", "Light (deleted)".
    pub value: String,
    /// Secondary: "Fable · always the newest", "tool default", "claude-opus-5-5 · started".
    pub detail: Option<String>,
    pub set_by: Option<SetByView>,
    pub let_agents_pick: Option<&'static str>,
    /// Started: the lock icon's tooltip (AGT-04).
    pub lock: Option<&'static str>,
    /// `None`: read-only (every row until T-0411).
    pub picker: Option<Vec<PickerItem>>,
    pub change: Option<&'static str>,
    /// "Sonnet → claude-sonnet-5-6 · new since the last run (claude-sonnet-5-5)" while the session runs.
    pub moved_on: Option<String>,
    pub next_lead: Option<&'static str>,
    /// The tool's avatar before the value, with several tools connected (SHL-08).
    pub avatar: Option<ToolAvatarView>,
    /// "asked for gpt-5.5, ran on auto": the session's tool quietly ran another model (AGT-05), neutral.
    pub swap: Option<String>,
    /// "on Codex · Claude Code limit": the session runs on its tool's fallback (AGM-03), amber.
    pub moved: Option<String>,
}

/// One child in the epic panel's read-only list (d0086).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentChildRow")]
pub struct ChildRow {
    pub key: String,
    pub number: String,
    pub value: String,
    pub set_by: Option<SetByView>,
    pub note: Option<&'static str>,
}

/// What a row picked in the ⌘N form shows until the ticket is created (T-0411).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentRowPicked")]
pub struct PickedView {
    pub set_by: SetByView,
    pub let_agents_pick: &'static str,
}

/// The "Agents" block of a form or the Ticket tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentBlock")]
pub struct Block {
    pub heading: &'static str,
    pub hint: &'static str,
    pub rows: Vec<RowView>,
    pub note: Option<&'static str>,
    pub note_icon: Option<IconName>,
    pub children: Vec<ChildRow>,
    pub children_heading: Option<&'static str>,
    pub children_note: Option<&'static str>,
    pub picked: Option<PickedView>,
}

// ---------------------------------------------------------------------------------------------------------------
// The model as Settings shows it

/// What a model passed resolves to today, from the list ("opus" → "claude-opus-5-5"; tool default → the list's
/// default entry's): the started row's model until the tool reports the one that ran.
fn resolves_to(model: Option<&str>, models: Option<&ModelList>) -> Option<String> {
    let list = models?;
    let entry = match model {
        Some(model) => list.entry(model)?,
        None => list.tool_default()?,
    };
    entry.resolves_to.clone().or_else(|| match entry.kind {
        ModelKind::Exact => Some(entry.id.clone()),
        _ => None,
    })
}

/// The exact model a session ran on: the tool's report, else what its model resolves to, else the model passed
/// (or "tool default").
fn exact_model(session: &SessionAgentRun, models: Option<&ModelList>) -> String {
    session
        .model_ran
        .clone()
        .or_else(|| resolves_to(session.model.as_deref(), models))
        .or_else(|| session.model.clone())
        .unwrap_or_else(|| TOOL_DEFAULT.to_owned())
}

/// The display name of a follow choice ("Sonnet" for `sonnet`), for "Sonnet → claude-sonnet-5-6".
fn follow_name(model: Option<&str>, models: Option<&ModelList>) -> Option<String> {
    let entry = models?.entry(model?)?;
    (entry.kind == ModelKind::Follow).then(|| entry.display_name.clone())
}

/// The tool a session ran on: its own, else its agent's now, else the first.
fn session_tool(t: &TicketAgents, session: &SessionAgentRun, tools: &Tools) -> Tool {
    session
        .tool
        .or_else(|| {
            let id = session.agent_id.as_deref()?;
            t.agents
                .iter()
                .find(|a| a.id == id)
                .and_then(|a| tools.tool_of(a))
        })
        .unwrap_or(Tool::ALL[0])
}

/// Whether a line about `name` on `tool` names the tool: only with several tools connected, and never when it is
/// the agent's name (AGT-04: "Codex · gpt-5.5 · started").
fn names_tool(tools: &Tools, tool: Tool, name: &str) -> bool {
    tools.several()
        && !name.eq_ignore_ascii_case(tool.display_name())
        && !name.eq_ignore_ascii_case(tool_word(tool))
}

/// A row's secondary line with the tool first where it is named: "Claude Code · claude-opus-5-5 · started".
fn with_tool(tools: &Tools, tool: Tool, name: &str, detail: String) -> String {
    if names_tool(tools, tool, name) {
        format!("{} \u{b7} {detail}", tool.display_name())
    } else {
        detail
    }
}

/// "Strong · Claude Code", "Verifier · Copilot": a name with its tool in a sentence, where it is named.
fn name_with_tool(tools: &Tools, tool: Tool, name: &str) -> String {
    if names_tool(tools, tool, name) {
        format!("{name} \u{b7} {}", tool_word(tool))
    } else {
        name.to_owned()
    }
}

/// "asked for gpt-5.5, ran on auto" (AGT-05, d0088): the session's tool swaps a refused model quietly, the model
/// passed was an exact one, and the tool reported another. Copilot's Auto or a follow choice picking a model is
/// never a swap (Q-J).
pub fn swap_note(session: &SessionAgentRun, tool: Tool, tools: &Tools) -> Option<String> {
    let asked = session.model.as_deref()?;
    let ran = session.model_ran.as_deref()?;
    let exact = match tools.list(tool).and_then(|l| l.entry(asked)) {
        Some(entry) => entry.kind == ModelKind::Exact,
        None => FollowChoice::parse(asked).is_none(),
    };
    // The gate's own rule (`mark::is_swap`): a variant of the asked model (`#high`) is the same model.
    is_swap(tool.swaps_refused_model(), exact, Some(asked), ran)
        .then(|| format!("asked for {asked}, ran on {ran}"))
}

/// "Sonnet → claude-sonnet-5-6 · new since the last run (claude-sonnet-5-5)": the running session's exact model
/// differs from the previous run on the same agent (AGT-05).
fn moved_on(session: &SessionAgentRun, models: Option<&ModelList>) -> Option<String> {
    let moved = session.moved_on.as_ref()?;
    let name =
        follow_name(session.model.as_deref(), models).or_else(|| session.agent_name.clone())?;
    Some(format!(
        "{name} \u{2192} {} \u{b7} new since the last run ({})",
        moved.now, moved.before
    ))
}

// ---------------------------------------------------------------------------------------------------------------
// Rows

/// An agent's model with its tool where it is named: "Codex · gpt-5.5", "GitHub Copilot · gpt-5.5".
fn agent_detail(agent: &Agent, tools: &Tools) -> (String, Option<ToolAvatarView>) {
    match agent.model.on(tools.default_model.as_ref()) {
        Some(on) => (
            with_tool(
                tools,
                on.tool,
                &agent.name,
                model_label(on.model.as_deref(), tools.list(on.tool)),
            ),
            tools.avatar(on.tool, on.model.as_deref()),
        ),
        None => ("no default model".to_owned(), None),
    }
}

fn sessions_of(
    t: &TicketAgents,
    role: SessionRole,
) -> impl DoubleEndedIterator<Item = &SessionAgentRun> {
    t.sessions.iter().filter(move |s| s.role == role)
}

/// Rounds that counted (a pass or a fail), and the one running.
fn counted_rounds(t: &TicketAgents) -> u32 {
    sessions_of(t, SessionRole::Validator)
        .filter(|s| s.running || matches!(s.round_outcome.as_deref(), Some("pass" | "fail")))
        .count() as u32
}

fn empty_row(role: SessionRole, label: String) -> RowView {
    RowView {
        role: role.into(),
        label,
        value: String::new(),
        detail: None,
        set_by: None,
        let_agents_pick: None,
        lock: None,
        picker: None,
        change: None,
        moved_on: None,
        next_lead: None,
        avatar: None,
        swap: None,
        moved: None,
    }
}

/// A row for `role`'s agent `agent`: the last session's (started), else the agent as it is now.
fn row_view(
    t: &TicketAgents,
    role: SessionRole,
    label: String,
    agent: Option<&Agent>,
    tools: &Tools,
) -> RowView {
    let mut view = empty_row(role, label);
    if let Some(session) = sessions_of(t, role).next_back() {
        let tool = session_tool(t, session, tools);
        let models = tools.list(tool);
        let exact = exact_model(session, models);
        let name = session.agent_name.clone().unwrap_or_default();
        let deleted = session
            .agent_id
            .as_deref()
            .is_some_and(|id| !t.agents.iter().any(|a| a.id == id));
        let round = role == SessionRole::Validator && session.running;
        view.value = if deleted {
            format!("{name} (deleted)")
        } else {
            name.clone()
        };
        view.detail = Some(with_tool(
            tools,
            tool,
            &name,
            format!(
                "{exact} \u{b7} {}",
                if round { "validating" } else { "started" }
            ),
        ));
        view.lock = Some(if round { ROUND_LOCKED } else { LOCKED });
        view.avatar = tools.avatar(tool, Some(&exact));
        view.swap = swap_note(session, tool, tools);
        if session.running {
            view.moved_on = moved_on(session, models);
        }
        return view;
    }
    if let Some(agent) = agent {
        let (detail, avatar) = agent_detail(agent, tools);
        view.value = agent.name.clone();
        view.detail = Some(detail);
        view.avatar = avatar;
    }
    view
}

fn finished(t: &TicketAgents) -> bool {
    t.status.is_some_and(|s| s.set() == StatusSet::Done)
}

/// The stages of `t` with their own row: every agent stage and agent check, not the Delivery (its agent opens the
/// PR, in the Developer's place); one row per session role, the first stage's.
fn shown_stages(t: &TicketAgents) -> Vec<&StageAgent> {
    let mut shown: Vec<&StageAgent> = Vec::new();
    for stage in t.stages.iter().filter(|s| !s.delivery) {
        if !shown.iter().any(|s| s.role == stage.role) {
            shown.push(stage);
        }
    }
    shown
}

/// Every row of `t`: a ticket's stages in flow order, an epic's lead.
pub fn rows(t: &TicketAgents, tools: &Tools) -> Vec<RowView> {
    match t.kind {
        Kind::Epic => vec![row_view(
            t,
            SessionRole::EpicLead,
            SessionRole::EpicLead.label().to_owned(),
            t.lead.as_ref(),
            tools,
        )],
        Kind::Ticket => shown_stages(t)
            .into_iter()
            .map(|s| row_view(t, s.role, s.stage.clone(), s.agent.as_ref(), tools))
            .collect(),
    }
}

/// The callouts of `t`'s held roles (§7.2, SCH-04 `[M23]`): Model unavailable, a tool that isn't available or is
/// at its limit, each with its buttons.
pub fn callouts(t: &TicketAgents, tools: &Tools) -> Vec<Callout> {
    tool_problems::hold_callouts(&t.holds, &t.agents, tools)
}

/// The "Agents" block: the rows.
pub fn block(t: &TicketAgents, tools: &Tools) -> Block {
    Block {
        heading: HEADING,
        hint: match t.kind {
            Kind::Ticket => TICKET_HINT,
            Kind::Epic => EPIC_HINT,
        },
        rows: rows(t, tools),
        note: None,
        note_icon: None,
        children: Vec::new(),
        children_heading: None,
        children_note: None,
        picked: None,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The token line

fn tokens_text(tokens: Option<u64>) -> String {
    tokens.map_or_else(|| "\u{2014}".to_owned(), tokens_label)
}

/// One part of the line: each agent in turn with its tokens ([`TicketAgents::tokens`]), "Fable test 2k → Strong
/// 50k"; a part on no agent is named after the role.
fn agent_part(t: &TicketAgents, role: SessionRole, tools: &Tools) -> Option<String> {
    let parts: Vec<String> = t
        .tokens
        .iter()
        .filter(|p| p.role == role)
        .map(|p| {
            let name = p.agent.clone().unwrap_or_else(|| role.label().to_owned());
            let name = match p.tool {
                Some(tool) => name_with_tool(tools, tool, &name),
                None => name,
            };
            format!("{name} {}", tokens_text(p.tokens.map(|t| t.shown())))
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(" \u{2192} "))
}

/// The panel header's tokens by agent (VAL-14, d0086, Q-G): "Strong 182k · no validation yet", "Strong 61k · Light
/// 23k (1 round)", "Strong 61k · Fable test 1k (no result)"; an epic's lead "Strong 48k". `None` before any agent
/// session. The tool is never named while one tool is connected; with several, "Strong · Claude Code 58k ·
/// Verifier · Copilot 9k (1 round)" (USG-10, d0086), the tool left out where it is the agent's name.
pub fn token_line(t: &TicketAgents, tools: &Tools) -> Option<String> {
    if t.kind == Kind::Epic {
        return agent_part(t, SessionRole::EpicLead, tools);
    }
    let developer = agent_part(t, SessionRole::Developer, tools)?;
    let Some(validator) = agent_part(t, SessionRole::Validator, tools) else {
        return Some(format!("{developer} \u{b7} no validation yet"));
    };
    let rounds = counted_rounds(t);
    let suffix = if rounds == 0 {
        "(no result)".to_owned()
    } else {
        format!(
            "({rounds} {})",
            if rounds == 1 { "round" } else { "rounds" }
        )
    };
    Some(format!("{developer} \u{b7} {validator} {suffix}"))
}

// ---------------------------------------------------------------------------------------------------------------
// History

/// A History line of the agents (`history.rs` adds them to the table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryLine {
    /// The title of a session's own row: "Session started on Sonnet · claude-sonnet-5-6 (Sonnet →
    /// claude-sonnet-5-6)", and the swap note in its detail ("asked for gpt-5.5, ran on auto").
    Start {
        session: String,
        title: String,
        note: Option<String>,
    },
    /// A line of its own: "Session ended · Sonnet · claude-sonnet-5-5 · 54k", "Developer: Light (deleted) ·
    /// claude-haiku-5-5 · 48k".
    Line {
        at: i64,
        title: String,
        tokens: String,
    },
}

/// One line per session start and end with the exact model, and a Done or Closed ticket's agents as they ran.
pub fn history_lines(t: &TicketAgents, tools: &Tools) -> Vec<HistoryLine> {
    let mut lines = Vec::new();
    for session in &t.sessions {
        let Some(name) = session.agent_name.clone().filter(|n| !n.is_empty()) else {
            continue;
        };
        let tool = session_tool(t, session, tools);
        let models = tools.list(tool);
        let exact = exact_model(session, models);
        let name = name_with_tool(tools, tool, &name);
        let mut title = format!("Session started on {name} \u{b7} {exact}");
        if let Some(moved) = &session.moved_on {
            let follow = follow_name(session.model.as_deref(), models).unwrap_or(name.clone());
            title.push_str(&format!(" ({follow} \u{2192} {})", moved.now));
        }
        lines.push(HistoryLine::Start {
            session: session.id.clone(),
            title,
            note: swap_note(session, tool, tools),
        });
        if let Some(at) = session.ended_at {
            lines.push(HistoryLine::Line {
                at,
                title: format!(
                    "Session ended \u{b7} {name} \u{b7} {exact} \u{b7} {}",
                    tokens_text(session.tokens)
                ),
                tokens: "\u{2014}".to_owned(),
            });
        }
    }
    if finished(t) {
        for role in SessionRole::ALL {
            let ran: Vec<&SessionAgentRun> = sessions_of(t, *role).collect();
            let Some(last) = ran.last() else { continue };
            let tool = session_tool(t, last, tools);
            let exact = exact_model(last, tools.list(tool));
            let deleted = last
                .agent_id
                .as_deref()
                .is_some_and(|id| !t.agents.iter().any(|a| a.id == id));
            let name = last.agent_name.clone().unwrap_or_default();
            let tokens: Option<u64> = ran.iter().filter_map(|s| s.tokens).reduce(|a, b| a + b);
            let exact = if names_tool(tools, tool, &name) {
                format!("{} \u{b7} {exact}", tool_word(tool))
            } else {
                exact
            };
            lines.push(HistoryLine::Line {
                at: last.ended_at.unwrap_or(last.started_at),
                title: format!(
                    "{}: {name}{} \u{b7} {exact} \u{b7} {}",
                    role.label(),
                    if deleted { " (deleted)" } else { "" },
                    tokens_text(tokens)
                ),
                tokens: "\u{2014}".to_owned(),
            });
        }
    }
    lines
}

fn text(p: &serde_json::Value, field: &str) -> Option<String> {
    p.get(field)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// A recorded role ("developer", "lead") as History says it.
fn role_label(p: &serde_json::Value) -> &'static str {
    text(p, "role")
        .and_then(|r| r.parse::<SessionRole>().ok())
        .map_or("Agent", SessionRole::label)
}

/// Why a recorded row moved: "Claude Code limit", "Codex not available"; `None` for the user's own move.
fn move_reason(reason: &str, tool: Option<Tool>) -> Option<String> {
    let tool = tool.map_or("its tool", tool_word);
    match reason {
        "tool_limit" => Some(format!("{tool} limit")),
        "tool_unavailable" => Some(format!("{tool} not available")),
        _ => None,
    }
}

/// A recorded row's move (AGT-08, M31's `agent_row_set` with `moved_reason`): "Moved to Strong GPT (Codex): Claude
/// Code limit" by the automatic fallback; "Developer: Strong GPT (moved by you from Strong · Claude Code limit)" by
/// Use fallback or Move waiting to…. `None` for a row change that isn't a move.
pub fn move_line(p: &serde_json::Value) -> Option<String> {
    let reason = text(p, "moved_reason")?;
    let role = role_label(p);
    let agent = text(p, "agent").unwrap_or_default();
    let from = text(p, "from").unwrap_or_default();
    let why = move_reason(&reason, text(p, "moved_tool").and_then(|t| t.parse().ok()));
    let by_user = p
        .get("moved_by_user")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if by_user || why.is_none() {
        return Some(match why {
            Some(why) => format!("{role}: {agent} (moved by you from {from} \u{b7} {why})"),
            None => format!("{role}: {agent} (moved by you from {from})"),
        });
    }
    let target = match text(p, "tool").and_then(|t| t.parse::<Tool>().ok()) {
        Some(tool) => format!("{agent} ({})", tool_word(tool)),
        None => agent,
    };
    Some(format!("Moved to {target}: {}", why.unwrap_or_default()))
}

/// Use fallback's History line (AGM-03, `fallback_used`): "Developer started on Codex: Claude Code limit (by
/// you)".
pub fn fallback_line(p: &serde_json::Value) -> String {
    let tool = |field| {
        text(p, field)
            .and_then(|t| t.parse::<Tool>().ok())
            .map_or("its fallback", tool_word)
    };
    let why = text(p, "reason")
        .and_then(|r| move_reason(&r, text(p, "from_tool").and_then(|t| t.parse().ok())))
        .or_else(|| {
            (text(p, "reason").as_deref() == Some("model_unavailable"))
                .then(|| "Model unavailable".to_owned())
        });
    match why {
        Some(why) => format!(
            "{} started on {}: {why} (by you)",
            role_label(p),
            tool("tool")
        ),
        None => format!("{} started on {} (by you)", role_label(p), tool("tool")),
    }
}

/// A recorded row change's History line (M31's `agent_row_set`): "Developer: Strong (set by you)", "Developer: Let
/// agents pick again (by you)"; an agent's pick "Developer: Light (picked by Checkout slowness)" with its reason; on
/// an epic for a child "Wishlist lead picked Standard for SHOP-40.4" or "Wishlist lead re-picked SHOP-40.3: Strong →
/// Light".
pub fn row_change_line(p: &serde_json::Value) -> (String, Option<String>) {
    if let Some(line) = move_line(p) {
        return (line, None);
    }
    let role = role_label(p);
    let agent = text(p, "agent");
    let from = text(p, "from");
    let reason = text(p, "reason");
    match (text(p, "set_by").as_deref(), text(p, "picked_by")) {
        (Some("agent"), Some(by)) => {
            let agent = agent.unwrap_or_default();
            match (text(p, "child"), from) {
                (Some(child), Some(from)) => (
                    format!("{by} re-picked {child}: {from} \u{2192} {agent}"),
                    reason,
                ),
                (Some(child), None) => (format!("{by} picked {agent} for {child}"), reason),
                (None, _) => (format!("{role}: {agent} (picked by {by})"), reason),
            }
        }
        _ => match agent {
            Some(agent) => (format!("{role}: {agent} ({SET_BY_YOU})"), None),
            None => (format!("{role}: {LET_AGENTS_PICK} (by you)"), None),
        },
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The gallery's "05/08 · Agent rows" (drawn by T-0344)

/// One sample: its caption, the block and the header's token line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentRowsSample")]
pub struct GallerySample {
    pub caption: &'static str,
    pub block: Block,
    pub token_line: Option<String>,
    /// The held roles' callouts above the bar (M23).
    pub callouts: Vec<Callout>,
}

/// The rows' states: not started, started, a round running, the epic lead; with several tools the tool in the
/// rows, the swap, Model unavailable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentRowsGallery")]
pub struct Gallery {
    pub samples: Vec<GallerySample>,
}

pub fn gallery() -> Gallery {
    use fixtures::*;
    let tools = Tools::one(Some(models()));
    let sample = |caption, t: TicketAgents| GallerySample {
        caption,
        block: block(&t, &tools),
        token_line: token_line(&t, &tools),
        callouts: callouts(&t, &tools),
    };
    let all = several_tools();
    let several = |caption, t: TicketAgents| GallerySample {
        caption,
        block: block(&t, &all),
        token_line: token_line(&t, &all),
        callouts: callouts(&t, &all),
    };
    Gallery {
        samples: vec![
            sample("\u{2318}N \u{b7} fresh install", fresh_install()),
            sample("Developer once started (SHOP-19)", shop_19()),
            sample(
                "Validator while a round runs (SHOP-29)",
                shop_29_validating(),
            ),
            sample("Epic lead once the lead starts (SHOP-40)", shop_40_lead()),
            several(
                "Several tools \u{b7} asked for gpt-5.5, ran on auto (SHOP-29)",
                shop_29_swapped(),
            ),
            several(
                "Several tools \u{b7} Model unavailable (SHOP-49)",
                shop_49_model_unavailable(),
            ),
        ],
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Fixtures (the gallery's "05/08 · Agent rows" and the tests)

pub mod fixtures {
    //! The sample of *update Y* ("The example"), on agents: Developer and Validator (the default flow's), Strong on
    //! `opus`, Light on `haiku`, and Claude Code's list.

    use otaq_core::agent_tools::Tool;
    use otaq_core::agent_tools::models::{ModelEntry, ModelKind, ModelList};
    use otaq_core::agents::facts::{StageAgent, TicketAgents};
    use otaq_core::agents::store::SessionAgentRun;
    use otaq_core::agents::{Agent, AgentHold, HoldReason, Role, SessionRole};
    use otaq_core::domain::{Kind, Status};
    use otaq_core::usage::{AgentTokens, Tokens};

    use crate::view_models::agents::fixtures::{agent, on};
    use crate::view_models::tool_problems::Tools;

    pub const CLAUDE: Tool = Tool::ALL[0];
    pub const CODEX: Tool = Tool::ALL[1];
    pub const COPILOT: Tool = Tool::ALL[2];

    pub fn models() -> ModelList {
        let entry = |id: &str, display: &str, resolves: &str, kind| ModelEntry {
            id: id.into(),
            display_name: display.into(),
            description: None,
            resolves_to: Some(resolves.into()),
            kind,
            family: None,
            released_at: None,
        };
        ModelList {
            entries: vec![
                entry(
                    "default",
                    "Default (recommended)",
                    "claude-opus-5-5",
                    ModelKind::ToolDefault,
                ),
                entry("opus", "Opus", "claude-opus-5-5", ModelKind::Follow),
                entry("sonnet", "Sonnet", "claude-sonnet-5-6", ModelKind::Follow),
                entry("haiku", "Haiku", "claude-haiku-5-5", ModelKind::Follow),
            ],
        }
    }

    /// The list: Strong, Light, Verifier on Copilot, Old GPT on a model Codex no longer lists, and the lead.
    pub fn list() -> Vec<Agent> {
        let mut lead = agent("lead", "Epic lead", Some("opus"));
        lead.role = Some(Role::EpicLead);
        vec![
            agent("strong", "Strong", Some("opus")),
            agent("light", "Light", Some("haiku")),
            on(COPILOT, agent("verifier", "Verifier", Some("gpt-5.5"))),
            on(CODEX, agent("old-gpt", "Old GPT", Some("gpt-5.4"))),
            lead,
        ]
    }

    pub fn find(agents: &[Agent], id: &str) -> Agent {
        agents.iter().find(|a| a.id == id).cloned().expect("agent")
    }

    /// A stage running `agent`.
    pub fn stage(agents: &[Agent], name: &str, role: SessionRole, agent: &str) -> StageAgent {
        StageAgent {
            stage_id: name.to_lowercase(),
            stage: name.into(),
            role,
            delivery: false,
            agent: agents.iter().find(|a| a.id == agent).cloned(),
        }
    }

    /// A session of `role` on `agent` (its name at start), with the model passed and the one that ran.
    pub fn session(
        id: &str,
        role: SessionRole,
        agent: &Agent,
        tool: Tool,
        model: Option<&str>,
        ran: Option<&str>,
        tokens: u64,
    ) -> SessionAgentRun {
        SessionAgentRun {
            id: id.into(),
            role,
            running: true,
            started_at: 10,
            ended_at: None,
            tool: Some(tool),
            agent_id: Some(agent.id.clone()),
            agent_name: Some(agent.name.clone()),
            model: model.map(str::to_owned),
            model_ran: ran.map(str::to_owned),
            effort: None,
            tokens: Some(tokens),
            round_outcome: None,
            moved_on: None,
        }
    }

    pub fn ended(mut s: SessionAgentRun, started: i64, ended: i64) -> SessionAgentRun {
        s.running = false;
        s.started_at = started;
        s.ended_at = Some(ended);
        s
    }

    pub fn round(mut s: SessionAgentRun, outcome: &str) -> SessionAgentRun {
        s.running = false;
        s.ended_at = Some(s.started_at + 1);
        s.round_outcome = Some(outcome.into());
        s
    }

    /// The tokens by role and agent, as `Usage::agents_of` sums them.
    pub fn tokens_of(sessions: &[SessionAgentRun]) -> Vec<AgentTokens> {
        let mut out: Vec<AgentTokens> = Vec::new();
        for s in sessions {
            let tokens = Tokens {
                input: s.tokens.unwrap_or(0),
                ..Tokens::default()
            };
            match out
                .iter_mut()
                .find(|p| p.role == s.role && p.agent == s.agent_name && p.tool == s.tool)
            {
                Some(p) => {
                    if let Some(t) = &mut p.tokens {
                        t.input += tokens.input;
                    }
                }
                None => out.push(AgentTokens {
                    role: s.role,
                    agent: s.agent_name.clone(),
                    tool: s.tool.or(Some(Tool::ALL[0])),
                    tokens: Some(tokens),
                }),
            }
        }
        out.sort_by_key(|t| SessionRole::ALL.iter().position(|r| *r == t.role));
        out
    }

    pub fn ticket(
        key: &str,
        status: Status,
        agents: Vec<Agent>,
        stages: Vec<StageAgent>,
        sessions: Vec<SessionAgentRun>,
    ) -> TicketAgents {
        TicketAgents {
            key: Some(key.parse().expect("key")),
            kind: Kind::Ticket,
            status: Some(status),
            agents,
            stages,
            lead: None,
            tokens: tokens_of(&sessions),
            sessions,
            holds: Vec::new(),
        }
    }

    /// Dev on Strong, Validate on Light.
    fn stages(agents: &[Agent], validator: &str) -> Vec<StageAgent> {
        vec![
            stage(agents, "Dev", SessionRole::Developer, "strong"),
            stage(agents, "Validate", SessionRole::Validator, validator),
        ]
    }

    /// The ⌘N form on a fresh install: nothing ran.
    pub fn fresh_install() -> TicketAgents {
        let agents = list();
        TicketAgents {
            key: None,
            status: None,
            ..ticket(
                "SHOP-1",
                Status::Backlog,
                agents.clone(),
                stages(&agents, "light"),
                Vec::new(),
            )
        }
    }

    /// SHOP-19 "Price table", started on Strong: "Strong · claude-opus-5-5 · started" (`CTAWe`).
    pub fn shop_19() -> TicketAgents {
        let agents = list();
        let strong = find(&agents, "strong");
        ticket(
            "SHOP-19",
            Status::Working,
            agents.clone(),
            stages(&agents, "light"),
            vec![session(
                "s19",
                SessionRole::Developer,
                &strong,
                CLAUDE,
                Some("opus"),
                Some("claude-opus-5-5"),
                182_000,
            )],
        )
    }

    /// SHOP-29 "Coupon field" validating on Light after Strong's session.
    pub fn shop_29_validating() -> TicketAgents {
        let agents = list();
        let (strong, light) = (find(&agents, "strong"), find(&agents, "light"));
        ticket(
            "SHOP-29",
            Status::Checking,
            agents.clone(),
            stages(&agents, "light"),
            vec![
                ended(
                    session(
                        "d29",
                        SessionRole::Developer,
                        &strong,
                        CLAUDE,
                        Some("opus"),
                        Some("claude-opus-5-5"),
                        61_000,
                    ),
                    1,
                    5,
                ),
                session(
                    "v29",
                    SessionRole::Validator,
                    &light,
                    CLAUDE,
                    Some("haiku"),
                    None,
                    23_000,
                ),
            ],
        )
    }

    /// SHOP-40 "Wishlist", its lead running on the Epic lead.
    pub fn shop_40_lead() -> TicketAgents {
        let agents = list();
        let lead = find(&agents, "lead");
        let sessions = vec![session(
            "l40",
            SessionRole::EpicLead,
            &lead,
            CLAUDE,
            Some("opus"),
            Some("claude-opus-5-5"),
            48_000,
        )];
        TicketAgents {
            kind: Kind::Epic,
            lead: Some(lead),
            ..ticket("SHOP-40", Status::Working, agents, Vec::new(), sessions)
        }
    }

    /// Claude Code, Codex and Copilot connected, with their lists (Codex no longer lists gpt-5.4).
    pub fn several_tools() -> Tools {
        let mut tools = crate::view_models::tool_problems::fixtures::several();
        let exact = |id: &str| ModelEntry {
            id: id.into(),
            display_name: id.into(),
            description: None,
            resolves_to: None,
            kind: ModelKind::Exact,
            family: None,
            released_at: None,
        };
        let tool_default = |resolves: Option<&str>| ModelEntry {
            id: "default".into(),
            display_name: "Default".into(),
            resolves_to: resolves.map(str::to_owned),
            kind: ModelKind::ToolDefault,
            ..exact("default")
        };
        let auto = ModelEntry {
            display_name: "Auto \u{b7} balance".into(),
            kind: ModelKind::Follow,
            ..exact("auto@balance")
        };
        tools.lists = vec![
            (CLAUDE, models()),
            (
                CODEX,
                ModelList {
                    entries: vec![
                        tool_default(Some("gpt-5.5")),
                        exact("gpt-5.5"),
                        exact("gpt-5.5-pro"),
                    ],
                },
            ),
            (
                COPILOT,
                ModelList {
                    entries: vec![tool_default(None), auto, exact("gpt-5.5")],
                },
            ),
        ];
        tools
    }

    /// SHOP-29 "Coupon field" validating on Verifier, which Copilot Free ran on auto (05 `sFZiP`, Q-J).
    pub fn shop_29_swapped() -> TicketAgents {
        let agents = list();
        let (strong, verifier) = (find(&agents, "strong"), find(&agents, "verifier"));
        ticket(
            "SHOP-29",
            Status::Checking,
            agents.clone(),
            stages(&agents, "verifier"),
            vec![
                ended(
                    session(
                        "d29",
                        SessionRole::Developer,
                        &strong,
                        CLAUDE,
                        Some("opus"),
                        Some("claude-opus-5-5"),
                        61_000,
                    ),
                    1,
                    5,
                ),
                session(
                    "v29",
                    SessionRole::Validator,
                    &verifier,
                    COPILOT,
                    Some("gpt-5.5"),
                    Some("auto"),
                    23_000,
                ),
            ],
        )
    }

    /// SHOP-49 "Export orders as CSV" with Dev on Old GPT, held: Codex no longer lists gpt-5.4 (05 `sFZiP`, §7.2).
    pub fn shop_49_model_unavailable() -> TicketAgents {
        let agents = list();
        let mut stages = stages(&agents, "verifier");
        stages[0] = stage(&agents, "Dev", SessionRole::Developer, "old-gpt");
        let mut t = ticket("SHOP-49", Status::Queued, agents, stages, Vec::new());
        t.holds = vec![AgentHold {
            id: "h49".into(),
            ticket_id: Some("t49".into()),
            project_id: None,
            role: SessionRole::Developer,
            reason: HoldReason::ModelUnavailable,
            tool: CODEX,
            model: Some("gpt-5.4".into()),
            agent_id: Some("old-gpt".into()),
            fallback_tool: None,
            fallback_held: false,
            fallback_asked: false,
            since: 0,
        }];
        t
    }
}

#[cfg(test)]
mod tests {
    use otaq_core::domain::Status;
    use serde_json::json;

    use super::fixtures::*;
    use super::*;

    fn tools() -> Tools {
        Tools::one(Some(models()))
    }

    #[test]
    fn rows_name_each_stages_agent_and_its_model() {
        let rows = rows(&fresh_install(), &tools());
        let shown: Vec<_> = rows
            .iter()
            .map(|r| (r.label.as_str(), r.value.as_str(), r.detail.as_deref()))
            .collect();
        assert_eq!(
            shown,
            [
                ("Dev", "Strong", Some("Opus \u{b7} always the newest")),
                ("Validate", "Light", Some("Haiku \u{b7} always the newest")),
            ]
        );
        assert!(rows.iter().all(|r| r.picker.is_none() && r.lock.is_none()));
    }

    #[test]
    fn a_started_row_shows_the_exact_model_and_the_lock() {
        let rows = rows(&shop_19(), &tools());
        assert_eq!(
            (
                rows[0].value.as_str(),
                rows[0].detail.as_deref(),
                rows[0].lock
            ),
            (
                "Strong",
                Some("claude-opus-5-5 \u{b7} started"),
                Some(LOCKED)
            )
        );
        let rows = super::rows(&shop_29_validating(), &tools());
        assert_eq!(
            (rows[1].detail.as_deref(), rows[1].lock),
            (
                Some("claude-haiku-5-5 \u{b7} validating"),
                Some(ROUND_LOCKED)
            )
        );
        let mut deleted = shop_19();
        deleted.agents.retain(|a| a.id != "strong");
        assert_eq!(super::rows(&deleted, &tools())[0].value, "Strong (deleted)");
    }

    #[test]
    fn several_tools_name_the_tool_and_the_swap() {
        let all = several_tools();
        let t = shop_29_swapped();
        let rows = rows(&t, &all);
        assert_eq!(
            rows[0].detail.as_deref(),
            Some("Claude Code \u{b7} claude-opus-5-5 \u{b7} started")
        );
        assert!(rows[0].avatar.is_some());
        assert_eq!(
            rows[1].swap.as_deref(),
            Some("asked for gpt-5.5, ran on auto")
        );
        assert_eq!(
            token_line(&t, &all).as_deref(),
            Some("Strong \u{b7} Claude Code 61k \u{b7} Verifier \u{b7} Copilot 23k (1 round)")
        );
        let held = shop_49_model_unavailable();
        let c = callouts(&held, &all);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].heading, "Model unavailable");
    }

    #[test]
    fn the_token_line_names_each_agent() {
        assert_eq!(
            token_line(&shop_19(), &tools()).as_deref(),
            Some("Strong 182k \u{b7} no validation yet")
        );
        assert_eq!(
            token_line(&shop_29_validating(), &tools()).as_deref(),
            Some("Strong 61k \u{b7} Light 23k (1 round)")
        );
        assert_eq!(
            token_line(&shop_40_lead(), &tools()).as_deref(),
            Some("Epic lead 48k")
        );
        assert_eq!(token_line(&fresh_install(), &tools()), None);
    }

    #[test]
    fn the_epic_has_one_lead_row() {
        let block = block(&shop_40_lead(), &tools());
        assert_eq!(block.hint, EPIC_HINT);
        assert_eq!(block.rows.len(), 1);
        assert_eq!(
            (block.rows[0].role, block.rows[0].label.as_str()),
            (RoleId::EpicLead, "Epic lead")
        );
    }

    #[test]
    fn history_names_the_agent_and_the_exact_model() {
        let mut t = shop_29_validating();
        t.status = Some(Status::Merged);
        t.sessions[1] = ended(t.sessions[1].clone(), 6, 9);
        let lines = history_lines(&t, &tools());
        let titles: Vec<_> = lines
            .iter()
            .map(|l| match l {
                HistoryLine::Start { title, .. } | HistoryLine::Line { title, .. } => {
                    title.as_str()
                }
            })
            .collect();
        assert_eq!(
            titles,
            [
                "Session started on Strong \u{b7} claude-opus-5-5",
                "Session ended \u{b7} Strong \u{b7} claude-opus-5-5 \u{b7} 61k",
                "Session started on Light \u{b7} claude-haiku-5-5",
                "Session ended \u{b7} Light \u{b7} claude-haiku-5-5 \u{b7} 23k",
                "Developer: Strong \u{b7} claude-opus-5-5 \u{b7} 61k",
                "Validator: Light \u{b7} claude-haiku-5-5 \u{b7} 23k",
            ]
        );
    }

    /// M31's recorded row changes still read in History.
    #[test]
    fn recorded_row_changes_read_as_history_lines() {
        assert_eq!(
            row_change_line(&json!({"role": "developer", "agent": "Strong", "set_by": "user"})).0,
            "Developer: Strong (set by you)"
        );
        assert_eq!(
            row_change_line(&json!({"role": "validator", "set_by": "user"})).0,
            "Validator: Let agents pick again (by you)"
        );
        assert_eq!(
            row_change_line(&json!({
                "role": "developer", "agent": "Light", "set_by": "agent",
                "picked_by": "Checkout slowness", "reason": "footer text only"
            })),
            (
                "Developer: Light (picked by Checkout slowness)".to_owned(),
                Some("footer text only".to_owned())
            )
        );
        assert_eq!(
            move_line(&json!({
                "role": "developer", "agent": "Strong GPT", "from": "Strong", "tool": "codex",
                "moved_reason": "tool_limit", "moved_tool": "claude"
            }))
            .as_deref(),
            Some("Moved to Strong GPT (Codex): Claude Code limit")
        );
    }

    #[test]
    fn use_fallback_reads_as_a_history_line() {
        assert_eq!(
            fallback_line(&json!({
                "role": "developer", "tool": "codex", "from_tool": "claude", "reason": "tool_limit"
            })),
            "Developer started on Codex: Claude Code limit (by you)"
        );
    }

    #[test]
    fn the_gallery_has_every_state() {
        let g = gallery();
        assert_eq!(g.samples.len(), 6);
        assert!(g.samples.iter().all(|s| !s.block.rows.is_empty()));
    }
}
