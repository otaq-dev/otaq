//! Agent rows (AGT-04, AGT-05, VAL-14, USG-01; d0086, d0088, d0077; *05 · Ticket side panel → Column 1 · locked
//! rows, Column 2 · model problems and newest, Row · Developer and Validator rows · every state, Row · Tokens per
//! agent*; *08 · Ticket editor → Row · Agents field*): the Developer and Validator rows of a ticket and the Epic lead
//! row of an epic, as the ⌘N form, the editor, the panel's Ticket tab and the epic forms show them, from the facts of
//! `ticket.agents` ([`TicketAgents`]) and the tool's cached model list.
//!
//! - **A row** is its label, the value (the named agent's name, or "Project default (Claude Code)"), the model as
//!   Settings shows it (`named_agents::agent_model`: "Opus · always the newest", "tool default"), who set it ("set by you" with
//!   *Let agents pick again*, or the violet "picked by Checkout slowness: footer text only"), and its picker.
//! - **Locked** (AGT-04): "Strong" · "claude-opus-5-5 · started" with the lock and its tooltip, the exact model from
//!   the session (until the tool reports it, the list's "resolves to"); "Light (started on claude-haiku-5-5)" once
//!   the entry's model changed after the start; "Light (deleted)" once it is gone. The tool is never named while one
//!   tool is connected; with several it joins the line ("Strong" · "Claude Code · claude-opus-5-5 · started"), except
//!   when it is the agent's name ("Codex" · "gpt-5.5 · started").
//! - **Several tools** (M23, d0086): the avatar before each row and picker item, the tool in each line; the swap
//!   note "asked for gpt-5.5, ran on auto" under a row whose tool quietly ran another model than the exact one asked
//!   for (AGT-05, Q-J); a moved row's "moved from Strong: Claude Code limit" (AGT-08), *set by you* staying where it
//!   was.
//! - **One named agent:** read-only rows with the bare name and "Your only agent…" under them.
//! - **"Moved on":** "Sonnet → claude-sonnet-5-6 · new since the last run (claude-sonnet-5-5)" under a locked row
//!   while its session runs.
//! - **The token line** in the panel's header (VAL-14, Q-G): "Strong 61k · Light 23k (1 round)"; with several tools
//!   "Strong · Claude Code 58k · Verifier · Copilot 9k (1 round)", and a moved row "Strong · Claude Code 120k →
//!   Strong GPT · Codex 80k".
//! - **History lines**: session starts and ends with the named agent and the exact model ("Session started on
//!   Verifier · Copilot · auto", with "asked for gpt-5.5, ran on auto"), row changes and moves ("Moved to Strong GPT
//!   (Codex): Claude Code limit"), and a Done ticket's "Developer: Light (deleted) · claude-haiku-5-5 · 48k".

use crate::view_models::legacy_column::{Column, ColumnOf};
use otaq_core::agent_tools::Tool;
use otaq_core::agent_tools::follow::FollowChoice;
use otaq_core::agent_tools::models::{ModelKind, ModelList};
use otaq_core::domain::{Kind, Status, TicketKey};
use otaq_core::named_agents::mark::is_swap;
use otaq_core::named_agents::rows::{RowFacts, SessionAgentRun, TicketAgents};
use otaq_core::named_agents::{CameFrom, MoveReason, NamedAgent, Role, RowLock, RowMove, RowSetBy};
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::card::tokens_label;
use crate::view_models::named_agents::{RoleId, TOOL_DEFAULT, model_label};
use crate::view_models::tool_avatar::ToolAvatarView;
use crate::view_models::tool_problems::{self, Callout, Tools, tool_word};

pub const HEADING: &str = "Agents";
pub const TICKET_HINT: &str = "who writes the code and who checks it";
pub const EPIC_HINT: &str = "who leads the epic's children";
/// Under the rows while only the starter entry exists (*08 → ⌘N · fresh install, one agent*).
pub const ONLY_AGENT: &str =
    "Your only agent. Add another in Settings \u{2192} Agents to choose per ticket.";
/// Under a ticket's rows before its start (*08 → Ticket tab · edit (SHOP-48, Backlog) · set by you*).
pub const NOT_STARTED: &str = "Not started: both rows can change.";
pub const SET_BY_YOU: &str = "set by you";
pub const PROJECT_DEFAULT: &str = "project default";
pub const LET_AGENTS_PICK: &str = "Let agents pick again";
pub const CHANGE: &str = "Change";
/// Under an ended lead's row (*Agent row · Epic lead*): the pick applies to the next lead.
pub const NEXT_LEAD: &str = "Change for the next lead";
/// Under an epic panel's children (*08 → Epic panel · lead running*).
pub const CHILDREN_NOTE: &str =
    "Each child's own Developer and Validator rows live on the child; this list is a summary.";

/// Who set a row: neutral "set by you" / "project default", or an agent's pick in violet (d0077).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentRowSetBy")]
pub struct SetByView {
    pub text: String,
    /// Agent-made: violet.
    pub agent_made: bool,
    /// Before the text in a row: the person for *set by you*, the bot for an agent's pick, none for "project
    /// default" (*05 → Row · Developer and Validator rows · every state*).
    pub icon: Option<IconName>,
}

/// One item of a row's picker: "Project default (Claude Code)" (agent `null`) first, then each named agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentPickerItem")]
pub struct PickerItem {
    pub agent: Option<String>,
    pub label: String,
    /// The model as Settings shows it; with several tools after the tool ("Codex · gpt-5.5").
    pub detail: String,
    pub selected: bool,
    /// The tool's avatar, with several tools connected.
    pub avatar: Option<ToolAvatarView>,
}

/// One agent row (`AgentRow` on the page, T-0344).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentRowView")]
pub struct RowView {
    pub role: RoleId,
    /// "Developer", "Validator", "Epic lead".
    pub label: &'static str,
    /// Primary: "Strong", "Project default (Claude Code)", "Light (started on claude-haiku-5-5)", "Light (deleted)".
    pub value: String,
    /// Secondary: "Fable · always the newest", "tool default", "claude-opus-5-5 · started".
    pub detail: Option<String>,
    pub set_by: Option<SetByView>,
    /// "Let agents pick again" on a row set by you that can change.
    pub let_agents_pick: Option<&'static str>,
    /// Locked: the lock icon's tooltip (AGT-04).
    pub lock: Option<&'static str>,
    /// The picker's items; `None` for a read-only row (locked, or one named agent: no chevron).
    pub picker: Option<Vec<PickerItem>>,
    /// The Change button's label beside an editable row.
    pub change: Option<&'static str>,
    /// "Sonnet → claude-sonnet-5-6 · new since the last run (claude-sonnet-5-5)" while the session runs.
    pub moved_on: Option<String>,
    /// "Change for the next lead", on its own line under an ended lead's row.
    pub next_lead: Option<&'static str>,
    /// The tool's avatar before the value, with several tools connected (SHL-08).
    pub avatar: Option<ToolAvatarView>,
    /// "asked for gpt-5.5, ran on auto": the session's tool quietly ran another model (AGT-05), neutral.
    pub swap: Option<String>,
    /// "moved from Strong: Claude Code limit" (AGT-08), amber; *set by you* stays where it was.
    pub moved: Option<String>,
}

/// One child in the epic panel's read-only list (d0086): "40.2" · "Light" · "picked by Wishlist lead".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentChildRow")]
pub struct ChildRow {
    pub key: String,
    /// "40.2".
    pub number: String,
    pub value: String,
    pub set_by: Option<SetByView>,
    /// "merged" for a merged child.
    pub note: Option<&'static str>,
}

/// What a row picked in the ⌘N form shows until the ticket is created (the pick is set right after): *set by
/// you* with Let agents pick again, beside the picked item's name and model.
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
    /// "who writes the code and who checks it" / "who leads the epic's children".
    pub hint: &'static str,
    pub rows: Vec<RowView>,
    /// "Your only agent. …" or "Not started: both rows can change.".
    pub note: Option<&'static str>,
    /// The info icon before "Your only agent. …" (08 `ehjFV`); none before "Not started: …".
    pub note_icon: Option<IconName>,
    /// An epic panel's children, read-only.
    pub children: Vec<ChildRow>,
    pub children_heading: Option<&'static str>,
    /// "Each child's own Developer and Validator rows live on the child; …" under the children.
    pub children_note: Option<&'static str>,
    /// The ⌘N form (no ticket yet, a choice to make): how a row picked there reads.
    pub picked: Option<PickedView>,
}

// ---------------------------------------------------------------------------------------------------------------
// The model as Settings shows it

/// What a model passed resolves to today, from the list ("opus" → "claude-opus-5-5"; tool default → the list's
/// default entry's): the locked row's model until the tool reports the one that ran.
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

/// The tool a session ran on: its own, else its named agent's, else the first.
fn session_tool(t: &TicketAgents, session: &SessionAgentRun) -> Tool {
    session
        .tool
        .or_else(|| {
            let id = session.named_agent_id.as_deref()?;
            t.agents.iter().find(|a| a.id == id).map(|a| a.tool)
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

/// An entry's model as Settings shows it, from its own tool's list.
fn agent_model(agent: &NamedAgent, tools: &Tools) -> String {
    model_label(agent.model.as_deref(), tools.list(agent.tool))
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

/// Why a row moved, as the panel and History say it: "Claude Code limit", "Codex not available".
fn move_reason(reason: MoveReason, tool: Option<Tool>) -> Option<String> {
    let tool = tool.map_or("its tool", tool_word);
    match reason {
        MoveReason::ToolLimit => Some(format!("{tool} limit")),
        MoveReason::ToolUnavailable => Some(format!("{tool} not available")),
        MoveReason::UserMove => None,
    }
}

/// "moved from Strong: Claude Code limit" under a moved row (AGT-08); "moved from Strong by you" for the user's own
/// move.
fn moved_text(moved: &RowMove) -> String {
    match move_reason(moved.reason, moved.tool) {
        Some(reason) => format!("moved from {}: {reason}", moved.from_name),
        None => format!("moved from {} by you", moved.from_name),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Rows

fn set_by_view(set_by: &RowSetBy) -> SetByView {
    match set_by {
        RowSetBy::User => SetByView {
            text: SET_BY_YOU.to_owned(),
            agent_made: false,
            icon: Some(IconName::User),
        },
        RowSetBy::Agent { picked_by, reason } => SetByView {
            text: if reason.trim().is_empty() {
                format!("picked by {picked_by}")
            } else {
                format!("picked by {picked_by}: {reason}")
            },
            agent_made: true,
            icon: Some(IconName::Bot),
        },
    }
}

/// The named agent a session ran on, by name: the session's own (saved at start), else the row's resolution.
fn session_name(session: &SessionAgentRun, facts: &RowFacts) -> String {
    session
        .named_agent_name
        .clone()
        .or_else(|| facts.resolved.as_ref().map(|r| r.agent.name.clone()))
        .unwrap_or_default()
}

fn sessions_of(t: &TicketAgents, role: Role) -> impl DoubleEndedIterator<Item = &SessionAgentRun> {
    t.sessions.iter().filter(move |s| s.role == role)
}

/// Rounds that counted (a pass or a fail), and the one running.
fn counted_rounds(t: &TicketAgents) -> u32 {
    sessions_of(t, Role::Validator)
        .filter(|s| s.running || matches!(s.round_outcome.as_deref(), Some("pass" | "fail")))
        .count() as u32
}

/// "Sonnet → claude-sonnet-5-6 · new since the last run (claude-sonnet-5-5)": the running session's exact model
/// differs from the previous run of the same role on the same named agent (AGT-05).
fn moved_on(session: &SessionAgentRun, models: Option<&ModelList>) -> Option<String> {
    let moved = session.moved_on.as_ref()?;
    let name = follow_name(session.model.as_deref(), models)
        .or_else(|| session.named_agent_name.clone())?;
    Some(format!(
        "{name} \u{2192} {} \u{b7} new since the last run ({})",
        moved.now, moved.before
    ))
}

/// The locked row's value: the name, "(started on …)" once the entry's model changed since, "(deleted)" once gone.
fn ran_on(
    t: &TicketAgents,
    session: &SessionAgentRun,
    facts: &RowFacts,
    exact: &str,
) -> (String, bool) {
    let name = session_name(session, facts);
    let Some(id) = session.named_agent_id.as_deref() else {
        return (name, false);
    };
    match t.agents.iter().find(|a| a.id == id) {
        None => (format!("{name} (deleted)"), true),
        Some(agent) if agent.model != session.model => {
            (format!("{name} (started on {exact})"), true)
        }
        Some(_) => (name, false),
    }
}

/// An entry's model with its tool where it is named: "Codex · gpt-5.5", "GitHub Copilot · gpt-5.5".
fn agent_detail(agent: &NamedAgent, tools: &Tools) -> String {
    with_tool(tools, agent.tool, &agent.name, agent_model(agent, tools))
}

fn picker(t: &TicketAgents, facts: &RowFacts, tools: &Tools) -> Vec<PickerItem> {
    let chosen = facts.row.as_ref().and_then(|r| r.agent.as_deref());
    let chosen = chosen.filter(|id| t.agents.iter().any(|a| a.id == *id));
    let default = facts.default.as_ref();
    std::iter::once(PickerItem {
        agent: None,
        label: format!(
            "Project default ({})",
            default.map(|a| a.name.as_str()).unwrap_or_default()
        ),
        detail: default.map(|a| agent_detail(a, tools)).unwrap_or_default(),
        selected: chosen.is_none(),
        avatar: default.and_then(|a| tools.avatar(a.tool, a.model.as_deref())),
    })
    .chain(t.agents.iter().map(|a| PickerItem {
        agent: Some(a.id.clone()),
        label: a.name.clone(),
        detail: agent_detail(a, tools),
        selected: chosen == Some(a.id.as_str()),
        avatar: tools.avatar(a.tool, a.model.as_deref()),
    }))
    .collect()
}

fn row_view(t: &TicketAgents, facts: &RowFacts, tools: &Tools) -> Option<RowView> {
    if facts.role == Role::ProjectAgent {
        return None;
    }
    let role = RoleId::from(facts.role);
    let mut view = RowView {
        role,
        label: facts.role.label(),
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
        moved: facts.moved.as_ref().map(moved_text),
    };
    let came_from = facts.resolved.as_ref().map(|r| &r.came_from);
    let who = || match came_from {
        Some(CameFrom::Row { set_by }) => set_by_view(set_by),
        _ => SetByView {
            text: PROJECT_DEFAULT.to_owned(),
            agent_made: false,
            icon: None,
        },
    };

    // Locked: what the session ran on (AGT-04).
    let session = match facts.lock {
        Some(RowLock::RoundRunning) => sessions_of(t, facts.role).rfind(|s| s.running),
        Some(RowLock::Started) => sessions_of(t, facts.role).next_back(),
        None => None,
    };
    if let Some(lock) = facts.lock {
        view.lock = Some(lock.tooltip());
        view.set_by = Some(who());
        match session {
            Some(session) => {
                let tool = session_tool(t, session);
                let models = tools.list(tool);
                let exact = exact_model(session, models);
                let (value, changed) = ran_on(t, session, facts, &exact);
                let name = session_name(session, facts);
                let state = match lock {
                    RowLock::RoundRunning => "validating",
                    RowLock::Started => "started",
                };
                view.detail = (!changed)
                    .then(|| with_tool(tools, tool, &name, format!("{exact} \u{b7} {state}")));
                view.value = value;
                view.avatar = tools.avatar(tool, Some(&exact));
                view.swap = swap_note(session, tool, tools);
                if session.running {
                    view.moved_on = moved_on(session, models);
                }
            }
            None => {
                // Locked before a session is listed (the lead's row exists before its first session).
                let agent = facts.resolved.as_ref().map(|r| &r.agent);
                view.value = agent.map(|a| a.name.clone()).unwrap_or_default();
                view.detail = agent.map(|a| agent_detail(a, tools));
                view.avatar = agent.and_then(|a| tools.avatar(a.tool, a.model.as_deref()));
            }
        }
        return Some(view);
    }

    // One named agent: read-only, the bare name (AGT-04, 08 `ehjFV`).
    if t.agents.len() <= 1 {
        let agent = facts.resolved.as_ref().map(|r| &r.agent);
        view.value = agent.map(|a| a.name.clone()).unwrap_or_default();
        view.detail = agent.map(|a| agent_detail(a, tools));
        view.avatar = agent.and_then(|a| tools.avatar(a.tool, a.model.as_deref()));
        return Some(view);
    }

    match (came_from, facts.resolved.as_ref()) {
        (Some(CameFrom::Row { set_by }), Some(resolved)) => {
            view.value = resolved.agent.name.clone();
            view.detail = Some(agent_detail(&resolved.agent, tools));
            view.avatar = tools.avatar(resolved.agent.tool, resolved.agent.model.as_deref());
            let mut shown = set_by_view(set_by);
            if *set_by == RowSetBy::User {
                view.let_agents_pick = Some(LET_AGENTS_PICK);
                // The Validator after a failed round: the pick is for the next one (*SHOP-30*).
                let rounds = counted_rounds(t);
                if facts.role == Role::Validator && rounds > 0 && !finished(t) {
                    shown.text = format!("{SET_BY_YOU} \u{b7} applies to round {}", rounds + 1);
                }
            }
            view.set_by = Some(shown);
        }
        _ => {
            let default = facts.default.as_ref();
            view.value = format!(
                "Project default ({})",
                default.map(|a| a.name.as_str()).unwrap_or_default()
            );
            view.detail = default.map(|a| agent_detail(a, tools));
            view.avatar = default.and_then(|a| tools.avatar(a.tool, a.model.as_deref()));
        }
    }
    // An ended lead: the next lead runs on the new pick (*Agent row · Epic lead*).
    if facts.role == Role::EpicLead
        && let Some(last) = sessions_of(t, Role::EpicLead).next_back()
        && !last.running
    {
        view.next_lead = Some(NEXT_LEAD);
        let tool = session_tool(t, last);
        view.detail = Some(with_tool(
            tools,
            tool,
            &session_name(last, facts),
            format!("{} \u{b7} ended", exact_model(last, tools.list(tool))),
        ));
    }
    view.picker = Some(picker(t, facts, tools));
    view.change = Some(CHANGE);
    Some(view)
}

fn finished(t: &TicketAgents) -> bool {
    t.status.is_some_and(|s| s.column() == Column::Done)
}

/// Every row of `t`, in the kind's order.
pub fn rows(t: &TicketAgents, tools: &Tools) -> Vec<RowView> {
    t.rows
        .iter()
        .filter_map(|facts| row_view(t, facts, tools))
        .collect()
}

/// The callouts of `t`'s held roles (§7.2, SCH-04 `[M23]`): Model unavailable, a tool that isn't available or is
/// at its limit, each with its buttons.
pub fn callouts(t: &TicketAgents, tools: &Tools) -> Vec<Callout> {
    tool_problems::hold_callouts(&t.holds, &t.agents, tools)
}

fn child_number(key: &TicketKey) -> String {
    match key.child {
        Some(child) => format!("{}.{child}", key.number),
        None => key.number.to_string(),
    }
}

/// The "Agents" block: the rows, the note under them, and an epic's children.
pub fn block(t: &TicketAgents, tools: &Tools) -> Block {
    let rows = rows(t, tools);
    let note = if t.agents.len() <= 1 {
        Some(ONLY_AGENT)
    } else if t.kind == Kind::Ticket
        && t.key.is_some()
        && t.sessions.is_empty()
        && rows.iter().all(|r| r.lock.is_none())
    {
        Some(NOT_STARTED)
    } else {
        None
    };
    let children: Vec<ChildRow> = t
        .children
        .iter()
        .map(|c| ChildRow {
            key: c.key.to_string(),
            number: child_number(&c.key),
            value: c
                .developer
                .as_ref()
                .map(|r| r.agent.name.clone())
                .unwrap_or_default(),
            set_by: c.developer.as_ref().and_then(|r| match &r.came_from {
                CameFrom::Row { set_by } => Some(set_by_view(set_by)),
                _ => None,
            }),
            note: (c.status == Status::Merged).then_some("merged"),
        })
        .collect();
    Block {
        heading: HEADING,
        hint: match t.kind {
            Kind::Ticket => TICKET_HINT,
            Kind::Epic => EPIC_HINT,
        },
        rows,
        note,
        note_icon: (note == Some(ONLY_AGENT)).then_some(IconName::Info),
        children_heading: (!children.is_empty()).then_some("Children"),
        children_note: (!children.is_empty()).then_some(CHILDREN_NOTE),
        children,
        picked: (t.key.is_none() && t.agents.len() > 1).then(|| PickedView {
            set_by: set_by_view(&RowSetBy::User),
            let_agents_pick: LET_AGENTS_PICK,
        }),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The token line

fn tokens_text(tokens: Option<u64>) -> String {
    tokens.map_or_else(|| "\u{2014}".to_owned(), tokens_label)
}

/// One part of the line: each agent in turn with its tokens ([`TicketAgents::tokens`]), "Fable test 2k → Claude
/// Code 50k"; a part on no named agent is named after the row.
fn agent_part(t: &TicketAgents, role: Role, tools: &Tools) -> Option<String> {
    let row_agent = t
        .rows
        .iter()
        .find(|r| r.role == role)
        .and_then(|r| r.resolved.as_ref())
        .map(|r| &r.agent);
    let parts: Vec<String> = t
        .tokens
        .iter()
        .filter(|p| p.role == role)
        .map(|p| {
            let name = p
                .agent
                .clone()
                .or_else(|| row_agent.map(|a| a.name.clone()))
                .unwrap_or_default();
            let name = match p.tool.or(row_agent.map(|a| a.tool)) {
                Some(tool) => name_with_tool(tools, tool, &name),
                None => name,
            };
            format!("{name} {}", tokens_text(p.tokens.map(|t| t.shown())))
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(" \u{2192} "))
}

/// The panel header's tokens by agent (VAL-14, d0086, Q-G): "Strong 182k · validation —", "Strong 61k · Light 23k
/// (1 round)", "Claude Code 61k · Fable test 1k (no result)"; an epic's lead "Strong 48k". `None` before any agent
/// session. The tool is never named while one tool is connected; with several, "Strong · Claude Code 58k · Verifier ·
/// Copilot 9k (1 round)" (USG-10, d0086), the tool left out where it is the agent's name ("Codex 182k").
pub fn token_line(t: &TicketAgents, tools: &Tools) -> Option<String> {
    if t.kind == Kind::Epic {
        return agent_part(t, Role::EpicLead, tools);
    }
    let developer = agent_part(t, Role::Developer, tools)?;
    let Some(validator) = agent_part(t, Role::Validator, tools) else {
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

/// A History line of the named agents (`history.rs` adds them to the table).
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
        let Some(facts) = t.rows.iter().find(|r| r.role == session.role) else {
            continue;
        };
        let name = session_name(session, facts);
        if name.is_empty() {
            continue;
        }
        let tool = session_tool(t, session);
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
        for facts in &t.rows {
            let ran: Vec<&SessionAgentRun> = sessions_of(t, facts.role).collect();
            let Some(last) = ran.last() else { continue };
            let tool = session_tool(t, last);
            let exact = exact_model(last, tools.list(tool));
            let deleted = last
                .named_agent_id
                .as_deref()
                .is_some_and(|id| !t.agents.iter().any(|a| a.id == id));
            let name = session_name(last, facts);
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
                    facts.role.label(),
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

/// A row change's History line (`agent_row_set`): "Developer: Strong (set by you)", "Developer: Let agents pick
/// again (by you)"; an agent's pick "Developer: Light (picked by Checkout slowness)" with its reason; on an epic for a
/// child "Wishlist lead picked Standard for SHOP-40.4" or "Wishlist lead re-picked SHOP-40.3: Strong → Light".
/// A row's move (AGT-08), from the `agent_row_set` event [`otaq_core::named_agents::rows::row_move_event`]
/// writes: "Moved to Strong GPT (Codex): Claude Code limit" by the automatic fallback; "Developer: Strong GPT (moved
/// by you from Strong · Claude Code limit)" by Use fallback or Move waiting to…. `None` for a row change that isn't
/// a move.
pub fn move_line(p: &serde_json::Value) -> Option<String> {
    let reason: MoveReason = text(p, "moved_reason")?.parse().ok()?;
    let role = text(p, "role")
        .and_then(|r| r.parse::<Role>().ok())
        .map_or("Agent", Role::label);
    let agent = text(p, "agent").unwrap_or_default();
    let from = text(p, "from").unwrap_or_default();
    let why = move_reason(reason, text(p, "moved_tool").and_then(|t| t.parse().ok()));
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

pub fn row_change_line(p: &serde_json::Value) -> (String, Option<String>) {
    if let Some(line) = move_line(p) {
        return (line, None);
    }
    let role = text(p, "role")
        .and_then(|r| r.parse::<Role>().ok())
        .map_or("Agent", Role::label);
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

/// Every row state of the frames: one agent, set by you, picked by an agent, locked while started and while a
/// round runs, moved on, edited and deleted after the start, the Validator after a failed round, the epic lead; and
/// with several tools (M23): the tool in locked rows, the tool as the name, the swap, a move, Model unavailable.
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
    let mut edited = shop_19();
    let light = find(&edited.agents, "light");
    edited.rows[0] = row(
        &edited.agents,
        Role::Developer,
        Some(("light", picked("Wishlist lead", ""))),
        "cc",
        Some(RowLock::Started),
    );
    edited.sessions = vec![session(
        "s",
        Role::Developer,
        &light,
        Some("haiku"),
        Some("claude-haiku-5-5"),
        44_000,
    )];
    if let Some(agent) = edited.agents.iter_mut().find(|a| a.id == "light") {
        agent.model = Some("sonnet".into());
    }
    let mut deleted = edited.clone();
    deleted.agents.retain(|a| a.id != "light");
    let mut failed = shop_48_set_by_you();
    failed.status = Some(Status::CheckFailed);
    failed.rows[1] = row(
        &failed.agents,
        Role::Validator,
        Some(("strong", RowSetBy::User)),
        "cc",
        None,
    );
    failed.rows[0].lock = Some(RowLock::Started);
    let strong = find(&failed.agents, "strong");
    failed.sessions = vec![
        ended(
            session(
                "d",
                Role::Developer,
                &strong,
                Some("opus"),
                Some("claude-opus-5-5"),
                141_000,
            ),
            1,
            9,
        ),
        round(
            session("v1", Role::Validator, &light, Some("haiku"), None, 15_000),
            "fail",
        ),
        round(
            session("v2", Role::Validator, &light, Some("haiku"), None, 16_000),
            "fail",
        ),
    ];
    for t in [&mut edited, &mut deleted, &mut failed] {
        t.tokens = tokens_of(&t.sessions);
    }
    Gallery {
        samples: vec![
            sample("\u{2318}N \u{b7} fresh install, one agent", fresh_install()),
            sample("Set by you (SHOP-48, Backlog)", shop_48_set_by_you()),
            sample("Picked by an agent (SHOP-24)", shop_24_picked()),
            sample("Developer once started (SHOP-19)", shop_19()),
            sample(
                "Validator while a round runs (SHOP-29)",
                shop_29_validating(),
            ),
            sample(
                "\u{201c}Always the newest\u{201d} moved on (SHOP-22)",
                shop_22_moved_on(),
            ),
            sample("After the named agent was edited (SHOP-40.2)", edited),
            sample("After the named agent was deleted", deleted),
            sample(
                "Validator editable again after a failed round (SHOP-30)",
                failed,
            ),
            sample("Epic lead once the lead starts (SHOP-40)", shop_40_lead()),
            several(
                "Several tools \u{b7} locked rows with the tool (SHOP-31)",
                shop_31_several(),
            ),
            several(
                "Several tools \u{b7} the tool is the agent's name (SHOP-19)",
                shop_19_codex(),
            ),
            several(
                "Several tools \u{b7} asked for gpt-5.5, ran on auto (SHOP-29)",
                shop_29_swapped(),
            ),
            several(
                "Several tools \u{b7} moved by a fallback (SHOP-31)",
                shop_31_moved(),
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
    //! The sample of *update Y* ("The example"): Claude Code (the starter), Strong on `opus`, Light on `haiku`,
    //! Sonnet on `sonnet`, Fable test on `fable`, and Claude Code's list.

    use otaq_core::agent_tools::Tool;
    use otaq_core::agent_tools::models::{ModelEntry, ModelKind, ModelList};
    use otaq_core::domain::{Kind, ProjectKey, Status, TicketKey};
    use otaq_core::named_agents::rows::{ChildAgents, RowFacts, SessionAgentRun, TicketAgents};
    use otaq_core::named_agents::{
        AgentHold, AgentRow, CameFrom, HoldReason, MoveReason, NamedAgent, Resolved, Role, RowLock,
        RowMove, RowSetBy,
    };

    use crate::view_models::tool_problems::Tools;
    use otaq_core::session::MovedOn;
    use otaq_core::usage::{AgentTokens, Tokens};

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
                entry("fable", "Fable", "claude-fable-5-1", ModelKind::Follow),
                entry("sonnet", "Sonnet", "claude-sonnet-5-6", ModelKind::Follow),
                entry("haiku", "Haiku", "claude-haiku-5-5", ModelKind::Follow),
            ],
        }
    }

    pub fn agent(id: &str, name: &str, model: Option<&str>) -> NamedAgent {
        NamedAgent {
            id: id.into(),
            name: name.into(),
            tool: Tool::ALL[0],
            model: model.map(str::to_owned),
            use_for: String::new(),
            fallback: None,
            mark: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn starter() -> NamedAgent {
        agent("cc", "Claude Code", None)
    }

    /// The list you made (15 `FYvzM`).
    pub fn list() -> Vec<NamedAgent> {
        vec![
            starter(),
            agent("strong", "Strong", Some("opus")),
            agent("light", "Light", Some("haiku")),
            agent("sonnet", "Sonnet", Some("sonnet")),
            agent("fable", "Fable test", Some("fable")),
        ]
    }

    pub fn find(agents: &[NamedAgent], id: &str) -> NamedAgent {
        agents.iter().find(|a| a.id == id).cloned().expect("agent")
    }

    /// A row: `agent` set by `set_by`, or empty (`None`) on `default`.
    pub fn row(
        agents: &[NamedAgent],
        role: Role,
        agent: Option<(&str, RowSetBy)>,
        default: &str,
        lock: Option<RowLock>,
    ) -> RowFacts {
        let default = find(agents, default);
        match agent {
            Some((id, set_by)) => RowFacts {
                role,
                row: Some(AgentRow {
                    role,
                    agent: Some(id.into()),
                    set_by: set_by.clone(),
                    set_at: 1,
                }),
                resolved: agents.iter().find(|a| a.id == id).map(|a| Resolved {
                    agent: a.clone(),
                    came_from: CameFrom::Row { set_by },
                }),
                default: Some(default),
                lock,
                moved: None,
            },
            None => RowFacts {
                role,
                row: None,
                resolved: Some(Resolved {
                    agent: default.clone(),
                    came_from: CameFrom::ProjectDefault,
                }),
                default: Some(default),
                lock,
                moved: None,
            },
        }
    }

    pub fn picked(by: &str, reason: &str) -> RowSetBy {
        RowSetBy::Agent {
            picked_by: by.into(),
            reason: reason.into(),
        }
    }

    /// A session of `role` on `agent` (its name at start), with the model passed and the one that ran.
    pub fn session(
        id: &str,
        role: Role,
        agent: &NamedAgent,
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
            tool: Some(agent.tool),
            named_agent_id: Some(agent.id.clone()),
            named_agent_name: Some(agent.name.clone()),
            model: model.map(str::to_owned),
            model_ran: ran.map(str::to_owned),
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
                .find(|p| p.role == s.role && p.agent == s.named_agent_name && p.tool == s.tool)
            {
                Some(p) => {
                    if let Some(t) = &mut p.tokens {
                        t.input += tokens.input;
                    }
                }
                None => out.push(AgentTokens {
                    role: s.role,
                    agent: s.named_agent_name.clone(),
                    tool: s.tool.or(Some(Tool::ALL[0])),
                    tokens: Some(tokens),
                }),
            }
        }
        out.sort_by_key(|t| Role::ALL.iter().position(|r| *r == t.role));
        out
    }

    pub fn ticket(
        key: &str,
        status: Status,
        agents: Vec<NamedAgent>,
        rows: Vec<RowFacts>,
        sessions: Vec<SessionAgentRun>,
    ) -> TicketAgents {
        TicketAgents {
            key: Some(key.parse().expect("key")),
            kind: Kind::Ticket,
            status: Some(status),
            agents,
            rows,
            tokens: tokens_of(&sessions),
            sessions,
            children: Vec::new(),
            holds: Vec::new(),
        }
    }

    /// SHOP-19 "Price table", started on Strong (set by you): "Strong · claude-opus-5-5 · started" (`CTAWe`).
    pub fn shop_19() -> TicketAgents {
        let agents = list();
        let strong = find(&agents, "strong");
        ticket(
            "SHOP-19",
            Status::Working,
            agents.clone(),
            vec![
                row(
                    &agents,
                    Role::Developer,
                    Some(("strong", RowSetBy::User)),
                    "cc",
                    Some(RowLock::Started),
                ),
                row(&agents, Role::Validator, None, "cc", None),
            ],
            vec![session(
                "s19",
                Role::Developer,
                &strong,
                Some("opus"),
                Some("claude-opus-5-5"),
                182_000,
            )],
        )
    }

    /// SHOP-29 "Coupon field" validating on Light (`CTAWe`): "Strong 61k · Light 23k (1 round)".
    pub fn shop_29_validating() -> TicketAgents {
        let agents = list();
        let (strong, light) = (find(&agents, "strong"), find(&agents, "light"));
        ticket(
            "SHOP-29",
            Status::Checking,
            agents.clone(),
            vec![
                row(
                    &agents,
                    Role::Developer,
                    None,
                    "strong",
                    Some(RowLock::Started),
                ),
                row(
                    &agents,
                    Role::Validator,
                    Some(("light", RowSetBy::User)),
                    "cc",
                    Some(RowLock::RoundRunning),
                ),
            ],
            vec![
                ended(
                    session(
                        "d29",
                        Role::Developer,
                        &strong,
                        Some("opus"),
                        Some("claude-opus-5-5"),
                        61_000,
                    ),
                    1,
                    5,
                ),
                session(
                    "v29",
                    Role::Validator,
                    &light,
                    Some("haiku"),
                    Some("claude-haiku-5-5"),
                    23_000,
                ),
            ],
        )
    }

    /// SHOP-22 "Fix flaky login redirect" on Sonnet, whose `sonnet` moved on since its last run (`Fbq9o`).
    pub fn shop_22_moved_on() -> TicketAgents {
        let agents = list();
        let sonnet = find(&agents, "sonnet");
        ticket(
            "SHOP-22",
            Status::NeedsYou,
            agents.clone(),
            vec![
                row(
                    &agents,
                    Role::Developer,
                    Some(("sonnet", RowSetBy::User)),
                    "cc",
                    Some(RowLock::Started),
                ),
                row(&agents, Role::Validator, None, "cc", None),
            ],
            vec![
                ended(
                    session(
                        "s22a",
                        Role::Developer,
                        &sonnet,
                        Some("sonnet"),
                        Some("claude-sonnet-5-5"),
                        54_000,
                    ),
                    1,
                    5,
                ),
                SessionAgentRun {
                    moved_on: Some(MovedOn {
                        before: "claude-sonnet-5-5".into(),
                        now: "claude-sonnet-5-6".into(),
                    }),
                    ..session(
                        "s22b",
                        Role::Developer,
                        &sonnet,
                        Some("sonnet"),
                        Some("claude-sonnet-5-6"),
                        42_000,
                    )
                },
            ],
        )
    }

    /// SHOP-48 "Add gift wrap option" in Backlog, Developer set by you to Fable test (08 `Mtf3v`).
    pub fn shop_48_set_by_you() -> TicketAgents {
        let agents = list();
        ticket(
            "SHOP-48",
            Status::Backlog,
            agents.clone(),
            vec![
                row(
                    &agents,
                    Role::Developer,
                    Some(("fable", RowSetBy::User)),
                    "cc",
                    None,
                ),
                row(&agents, Role::Validator, None, "cc", None),
            ],
            Vec::new(),
        )
    }

    /// SHOP-24 "Update footer links", picked by the Project agent "Checkout slowness" (`A22yR`).
    pub fn shop_24_picked() -> TicketAgents {
        let agents = list();
        ticket(
            "SHOP-24",
            Status::Backlog,
            agents.clone(),
            vec![
                row(
                    &agents,
                    Role::Developer,
                    Some(("light", picked("Checkout slowness", "footer text only"))),
                    "cc",
                    None,
                ),
                row(&agents, Role::Validator, None, "cc", None),
            ],
            Vec::new(),
        )
    }

    /// A fresh install: only the starter entry (08 `ehjFV`).
    pub fn fresh_install() -> TicketAgents {
        let agents = vec![starter()];
        TicketAgents {
            key: None,
            kind: Kind::Ticket,
            status: None,
            rows: vec![
                row(&agents, Role::Developer, None, "cc", None),
                row(&agents, Role::Validator, None, "cc", None),
            ],
            agents,
            sessions: Vec::new(),
            tokens: Vec::new(),
            children: Vec::new(),
            holds: Vec::new(),
        }
    }

    /// SHOP-40 "Wishlist", its lead started on Strong, with its children (`N6B9c`).
    pub fn shop_40_lead() -> TicketAgents {
        let agents = list();
        let strong = find(&agents, "strong");
        let light = find(&agents, "light");
        let child = |n: u32, status: Status, agent: &NamedAgent, came_from: CameFrom| ChildAgents {
            key: TicketKey::child(ProjectKey::new("SHOP").expect("project"), 40, n),
            title: String::new(),
            status,
            developer: Some(Resolved {
                agent: agent.clone(),
                came_from,
            }),
        };
        TicketAgents {
            key: Some("SHOP-40".parse().expect("key")),
            kind: Kind::Epic,
            status: Some(Status::Backlog),
            rows: vec![row(
                &agents,
                Role::EpicLead,
                None,
                "strong",
                Some(RowLock::Started),
            )],
            agents: agents.clone(),
            tokens: vec![AgentTokens {
                role: Role::EpicLead,
                agent: Some("Strong".into()),
                tool: Some(Tool::ALL[0]),
                tokens: Some(Tokens {
                    input: 48_000,
                    ..Tokens::default()
                }),
            }],
            sessions: vec![session(
                "lead40",
                Role::EpicLead,
                &strong,
                Some("opus"),
                Some("claude-opus-5-5"),
                48_000,
            )],
            children: vec![
                child(1, Status::Merged, &strong, CameFrom::ProjectDefault),
                child(
                    2,
                    Status::Working,
                    &light,
                    CameFrom::Row {
                        set_by: picked("Wishlist lead", ""),
                    },
                ),
            ],
            holds: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Several tools (*update Y*'s M23 samples, 05 `sFZiP`, `ZsIeY`)

    pub const CLAUDE: Tool = Tool::ALL[0];
    pub const CODEX: Tool = Tool::ALL[1];
    pub const COPILOT: Tool = Tool::ALL[2];

    pub fn agent_on(id: &str, name: &str, tool: Tool, model: Option<&str>) -> NamedAgent {
        NamedAgent {
            tool,
            ..agent(id, name, model)
        }
    }

    /// Claude Code and Codex starters, Strong (fallback Strong GPT), Strong GPT, Standard (fallback Sonnet),
    /// Sonnet, Verifier on Copilot, Old GPT on a model Codex no longer lists.
    pub fn several_list() -> Vec<NamedAgent> {
        let mut strong = agent_on("strong", "Strong", CLAUDE, Some("opus"));
        strong.fallback = Some("strong-gpt".into());
        let mut standard = agent_on("standard", "Standard", CODEX, None);
        standard.fallback = Some("sonnet".into());
        vec![
            starter(),
            agent_on("codex-starter", "Codex", CODEX, None),
            strong,
            agent_on("strong-gpt", "Strong GPT", CODEX, Some("gpt-5.5-pro")),
            standard,
            agent_on("sonnet", "Sonnet", CLAUDE, Some("sonnet")),
            agent_on("verifier", "Verifier", COPILOT, Some("gpt-5.5")),
            agent_on("old-gpt", "Old GPT", CODEX, Some("gpt-5.4")),
        ]
    }

    fn exact(id: &str) -> ModelEntry {
        ModelEntry {
            id: id.into(),
            display_name: id.into(),
            description: None,
            resolves_to: None,
            kind: ModelKind::Exact,
            family: None,
            released_at: None,
        }
    }

    /// Claude Code, Codex and Copilot connected, with their lists (Codex no longer lists gpt-5.4).
    pub fn several_tools() -> Tools {
        let mut tools = crate::view_models::tool_problems::fixtures::several();
        let tool_default = |resolves: Option<&str>| ModelEntry {
            id: "default".into(),
            display_name: "Default".into(),
            description: None,
            resolves_to: resolves.map(str::to_owned),
            kind: ModelKind::ToolDefault,
            family: None,
            released_at: None,
        };
        let auto = ModelEntry {
            id: "auto@balance".into(),
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

    /// SHOP-31 "Hotfix: checkout crash" started on Strong (Claude Code), the Validator on the project default
    /// Verifier (05 `sFZiP`).
    pub fn shop_31_several() -> TicketAgents {
        let agents = several_list();
        let strong = find(&agents, "strong");
        ticket(
            "SHOP-31",
            Status::Working,
            agents.clone(),
            vec![
                row(
                    &agents,
                    Role::Developer,
                    Some(("strong", RowSetBy::User)),
                    "strong",
                    Some(RowLock::Started),
                ),
                row(&agents, Role::Validator, None, "verifier", None),
            ],
            vec![session(
                "s31",
                Role::Developer,
                &strong,
                Some("opus"),
                Some("claude-opus-5-5"),
                120_000,
            )],
        )
    }

    /// SHOP-19 "Price table" on the Codex starter: the tool is the agent's name ("Codex" · "gpt-5.5 · started").
    pub fn shop_19_codex() -> TicketAgents {
        let agents = several_list();
        let codex = find(&agents, "codex-starter");
        ticket(
            "SHOP-19",
            Status::Working,
            agents.clone(),
            vec![
                row(
                    &agents,
                    Role::Developer,
                    None,
                    "codex-starter",
                    Some(RowLock::Started),
                ),
                row(&agents, Role::Validator, None, "verifier", None),
            ],
            vec![session(
                "s19",
                Role::Developer,
                &codex,
                None,
                Some("gpt-5.5"),
                182_000,
            )],
        )
    }

    /// SHOP-29 "Coupon field" validating on Verifier, which Copilot Free ran on auto (05 `sFZiP`, Q-J).
    pub fn shop_29_swapped() -> TicketAgents {
        let agents = several_list();
        let (strong, verifier) = (find(&agents, "strong"), find(&agents, "verifier"));
        ticket(
            "SHOP-29",
            Status::Checking,
            agents.clone(),
            vec![
                row(
                    &agents,
                    Role::Developer,
                    None,
                    "strong",
                    Some(RowLock::Started),
                ),
                row(
                    &agents,
                    Role::Validator,
                    None,
                    "verifier",
                    Some(RowLock::RoundRunning),
                ),
            ],
            vec![
                ended(
                    session(
                        "d29",
                        Role::Developer,
                        &strong,
                        Some("opus"),
                        Some("claude-opus-5-5"),
                        61_000,
                    ),
                    1,
                    5,
                ),
                session(
                    "v29",
                    Role::Validator,
                    &verifier,
                    Some("gpt-5.5"),
                    Some("auto"),
                    23_000,
                ),
            ],
        )
    }

    /// SHOP-31 moved by the automatic fallback from Strong (Claude Code limit) to Strong GPT (05 `ZsIeY`).
    pub fn shop_31_moved() -> TicketAgents {
        let agents = several_list();
        let (strong, gpt) = (find(&agents, "strong"), find(&agents, "strong-gpt"));
        let mut t = ticket(
            "SHOP-31",
            Status::Working,
            agents.clone(),
            vec![
                row(
                    &agents,
                    Role::Developer,
                    Some(("strong-gpt", RowSetBy::User)),
                    "strong",
                    Some(RowLock::Started),
                ),
                row(&agents, Role::Validator, None, "verifier", None),
            ],
            vec![
                ended(
                    session(
                        "s31a",
                        Role::Developer,
                        &strong,
                        Some("opus"),
                        Some("claude-opus-5-5"),
                        120_000,
                    ),
                    1,
                    5,
                ),
                session(
                    "s31b",
                    Role::Developer,
                    &gpt,
                    Some("gpt-5.5-pro"),
                    Some("gpt-5.5-pro"),
                    80_000,
                ),
            ],
        );
        t.rows[0].moved = Some(RowMove {
            from_id: Some("strong".into()),
            from_name: "Strong".into(),
            reason: MoveReason::ToolLimit,
            tool: Some(CLAUDE),
            at: 3,
        });
        t
    }

    /// SHOP-49 "Export orders as CSV" on Old GPT, held: Codex no longer lists gpt-5.4 (05 `sFZiP`, §7.2).
    pub fn shop_49_model_unavailable() -> TicketAgents {
        let agents = several_list();
        let mut t = ticket(
            "SHOP-49",
            Status::Queued,
            agents.clone(),
            vec![
                row(
                    &agents,
                    Role::Developer,
                    Some(("old-gpt", RowSetBy::User)),
                    "strong",
                    None,
                ),
                row(&agents, Role::Validator, None, "verifier", None),
            ],
            Vec::new(),
        );
        t.holds = vec![AgentHold {
            id: "h49".into(),
            ticket_id: Some("t49".into()),
            project_id: None,
            role: Role::Developer,
            reason: HoldReason::ModelUnavailable,
            tool: CODEX,
            model: Some("gpt-5.4".into()),
            named_agent_id: Some("old-gpt".into()),
            fallback_agent_id: None,
            fallback_held: false,
            since: 0,
        }];
        t
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use crate::view_models::named_agents::{self as settings, model_label};

    fn tools() -> Tools {
        Tools::one(Some(models()))
    }

    fn view(t: &TicketAgents) -> Vec<RowView> {
        rows(t, &tools())
    }

    #[test]
    fn the_model_reads_as_settings_shows_it() {
        let m = models();
        let label = |model: Option<&str>| model_label(model, Some(&m));
        assert_eq!(label(None), "tool default");
        assert_eq!(label(Some("default")), "tool default");
        assert_eq!(label(Some("opus")), "Opus \u{b7} always the newest");
        assert_eq!(label(Some("fable")), "Fable \u{b7} always the newest");
        assert_eq!(label(Some("claude-opus-5-5")), "claude-opus-5-5");
        assert_eq!(settings::agent_model(&starter(), Some(&m)), "tool default");
        // Never "Claude Code · opus" with one tool: no tool, and the alias as Settings words it.
        assert!(!agent_detail(&agent("s", "Strong", Some("opus")), &tools()).contains("Claude"));
    }

    /// *Column 1 · locked rows* (`CTAWe`): Developer once started, Validator while a round runs.
    #[test]
    fn locked_rows_show_the_exact_model_and_why() {
        let rows = view(&shop_19());
        let dev = &rows[0];
        assert_eq!(dev.label, "Developer");
        assert_eq!(dev.value, "Strong");
        assert_eq!(
            dev.detail.as_deref(),
            Some("claude-opus-5-5 \u{b7} started")
        );
        assert_eq!(dev.set_by.as_ref().unwrap().text, "set by you");
        assert_eq!(
            dev.lock,
            Some("Locked: Send back continues the same conversation")
        );
        assert_eq!(dev.picker, None, "no picker on a locked row");
        assert_eq!((dev.change, dev.let_agents_pick), (None, None));
        let validator = &rows[1];
        assert_eq!(validator.value, "Project default (Claude Code)");
        assert_eq!(validator.detail.as_deref(), Some("tool default"));
        assert_eq!(validator.change, Some("Change"));

        let rows = view(&shop_29_validating());
        assert_eq!(rows[0].value, "Strong");
        assert_eq!(rows[0].set_by.as_ref().unwrap().text, "project default");
        assert_eq!(rows[0].set_by.as_ref().unwrap().icon, None);
        assert_eq!(rows[1].value, "Light");
        assert_eq!(
            rows[1].detail.as_deref(),
            Some("claude-haiku-5-5 \u{b7} validating")
        );
        assert_eq!(rows[1].lock, Some("Locked until this round ends"));
    }

    /// The starter entry once started: the exact model Claude Code ran, and the tool name isn't repeated.
    #[test]
    fn a_tool_default_entry_locks_on_the_model_that_ran() {
        let mut t = shop_19();
        let cc = starter();
        t.rows[0] = row(
            &t.agents,
            Role::Developer,
            None,
            "cc",
            Some(RowLock::Started),
        );
        t.sessions = vec![session(
            "s",
            Role::Developer,
            &cc,
            None,
            Some("claude-opus-5-5"),
            1,
        )];
        let dev = &view(&t)[0];
        assert_eq!(dev.value, "Claude Code");
        assert_eq!(
            dev.detail.as_deref(),
            Some("claude-opus-5-5 \u{b7} started")
        );
        assert_eq!(dev.set_by.as_ref().unwrap().text, "project default");
        // Until the tool reports it: what the default resolves to in the list.
        t.sessions[0].model_ran = None;
        assert_eq!(
            view(&t)[0].detail.as_deref(),
            Some("claude-opus-5-5 \u{b7} started")
        );
    }

    /// "Light (started on claude-haiku-5-5)" once Light's model changed; "Light (deleted)" once it is gone.
    #[test]
    fn edited_and_deleted_agents_show_as_they_ran() {
        let mut t = shop_19();
        let light = find(&t.agents, "light");
        t.rows[0] = row(
            &t.agents,
            Role::Developer,
            Some(("light", picked("Wishlist lead", ""))),
            "cc",
            Some(RowLock::Started),
        );
        t.sessions = vec![session(
            "s",
            Role::Developer,
            &light,
            Some("haiku"),
            Some("claude-haiku-5-5"),
            1,
        )];
        t.agents.iter_mut().find(|a| a.id == "light").unwrap().model = Some("sonnet".into());
        let dev = &view(&t)[0];
        assert_eq!(dev.value, "Light (started on claude-haiku-5-5)");
        assert_eq!(dev.detail, None);
        let by = dev.set_by.as_ref().unwrap();
        assert_eq!(
            (by.text.as_str(), by.agent_made),
            ("picked by Wishlist lead", true)
        );
        t.agents.retain(|a| a.id != "light");
        assert_eq!(view(&t)[0].value, "Light (deleted)");
    }

    /// *08 → Ticket tab · edit (SHOP-48) · set by you* and *Picked by an agent (SHOP-24)*.
    #[test]
    fn unlocked_rows_say_who_set_them_and_offer_the_list() {
        let t = shop_48_set_by_you();
        let block = block(&t, &tools());
        let dev = &block.rows[0];
        assert_eq!(dev.value, "Fable test");
        assert_eq!(
            dev.detail.as_deref(),
            Some("Fable \u{b7} always the newest")
        );
        let by = dev.set_by.as_ref().unwrap();
        assert_eq!((by.text.as_str(), by.agent_made), ("set by you", false));
        assert_eq!(by.icon, Some(IconName::User));
        assert_eq!(dev.let_agents_pick, Some("Let agents pick again"));
        assert_eq!(block.note, Some("Not started: both rows can change."));
        assert_eq!(block.hint, "who writes the code and who checks it");
        let items = dev.picker.as_ref().unwrap();
        let labels: Vec<(&str, &str)> = items
            .iter()
            .map(|i| (i.label.as_str(), i.detail.as_str()))
            .collect();
        assert_eq!(
            labels,
            [
                ("Project default (Claude Code)", "tool default"),
                ("Claude Code", "tool default"),
                ("Strong", "Opus \u{b7} always the newest"),
                ("Light", "Haiku \u{b7} always the newest"),
                ("Sonnet", "Sonnet \u{b7} always the newest"),
                ("Fable test", "Fable \u{b7} always the newest"),
            ]
        );
        assert_eq!(
            items
                .iter()
                .filter(|i| i.selected)
                .map(|i| i.label.as_str())
                .collect::<Vec<_>>(),
            ["Fable test"]
        );
        assert_eq!(items[0].agent, None);
        assert!(block.rows[1].picker.as_ref().unwrap()[0].selected);

        let picked = &view(&shop_24_picked())[0];
        assert_eq!(picked.value, "Light");
        let by = picked.set_by.as_ref().unwrap();
        assert_eq!(
            (by.text.as_str(), by.agent_made),
            ("picked by Checkout slowness: footer text only", true)
        );
        assert_eq!(by.icon, Some(IconName::Bot));
        assert_eq!(picked.let_agents_pick, None);
    }

    /// *08 → ⌘N · fresh install, one agent*: read-only, the bare name, no chevron; locked rows still lock.
    #[test]
    fn one_named_agent_makes_the_rows_read_only() {
        let t = fresh_install();
        let block = block(&t, &tools());
        for row in &block.rows {
            assert_eq!(row.value, "Claude Code");
            assert_eq!(row.detail.as_deref(), Some("tool default"));
            assert_eq!(
                (row.picker.is_none(), row.change, row.set_by.is_none()),
                (true, None, true)
            );
        }
        assert_eq!(
            block.note,
            Some("Your only agent. Add another in Settings \u{2192} Agents to choose per ticket.")
        );
        let mut started = shop_19();
        started.agents = vec![starter()];
        let cc = starter();
        started.rows[0] = row(
            &started.agents,
            Role::Developer,
            None,
            "cc",
            Some(RowLock::Started),
        );
        started.rows[1] = row(&started.agents, Role::Validator, None, "cc", None);
        started.sessions = vec![session(
            "s",
            Role::Developer,
            &cc,
            None,
            Some("claude-opus-5-5"),
            1,
        )];
        let rows = view(&started);
        assert!(rows[0].lock.is_some());
        assert_eq!(rows[1].picker, None);
    }

    /// *Validator editable again after a failed round (SHOP-30)*: "set by you · applies to round 3".
    #[test]
    fn the_validator_changes_again_after_a_failed_round() {
        let agents = list();
        let (strong, light) = (find(&agents, "strong"), find(&agents, "light"));
        let t = ticket(
            "SHOP-30",
            Status::CheckFailed,
            agents.clone(),
            vec![
                row(
                    &agents,
                    Role::Developer,
                    None,
                    "strong",
                    Some(RowLock::Started),
                ),
                row(
                    &agents,
                    Role::Validator,
                    Some(("strong", RowSetBy::User)),
                    "cc",
                    None,
                ),
            ],
            vec![
                ended(
                    session("d", Role::Developer, &strong, Some("opus"), None, 141_000),
                    1,
                    9,
                ),
                round(
                    session("v1", Role::Validator, &light, Some("haiku"), None, 15_000),
                    "fail",
                ),
                round(
                    session("v2", Role::Validator, &light, Some("haiku"), None, 16_000),
                    "fail",
                ),
            ],
        );
        let validator = &view(&t)[1];
        assert_eq!(validator.lock, None);
        assert!(validator.picker.is_some());
        assert_eq!(
            validator.set_by.as_ref().unwrap().text,
            "set by you \u{b7} applies to round 3"
        );
        assert_eq!(
            token_line(&t, &tools()).as_deref(),
            Some("Strong 141k \u{b7} Light 31k (2 rounds)")
        );
    }

    /// The "moved on" line only while the session runs (*SHOP-22*); History keeps it after.
    #[test]
    fn the_moved_on_line_shows_while_the_session_runs() {
        let mut t = shop_22_moved_on();
        let dev = &view(&t)[0];
        assert_eq!(
            dev.detail.as_deref(),
            Some("claude-sonnet-5-6 \u{b7} started")
        );
        assert_eq!(
            dev.moved_on.as_deref(),
            Some(
                "Sonnet \u{2192} claude-sonnet-5-6 \u{b7} new since the last run (claude-sonnet-5-5)"
            )
        );
        t.sessions[1].running = false;
        t.sessions[1].ended_at = Some(20);
        assert_eq!(view(&t)[0].moved_on, None);
        // The same model as last time: nothing to say.
        let mut same = shop_22_moved_on();
        same.sessions[1].moved_on = None;
        assert_eq!(view(&same)[0].moved_on, None);
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
        // A validator that only had refused rounds (*SHOP-29 · No result*).
        let agents = list();
        let (cc, fable) = (starter(), find(&agents, "fable"));
        let t = ticket(
            "SHOP-29",
            Status::Checking,
            agents.clone(),
            vec![
                row(&agents, Role::Developer, None, "cc", Some(RowLock::Started)),
                row(
                    &agents,
                    Role::Validator,
                    Some(("fable", RowSetBy::User)),
                    "cc",
                    None,
                ),
            ],
            vec![
                ended(session("d", Role::Developer, &cc, None, None, 61_000), 1, 2),
                round(
                    session("v", Role::Validator, &fable, Some("fable"), None, 1_000),
                    "no_result",
                ),
            ],
        );
        assert_eq!(
            token_line(&t, &tools()).as_deref(),
            Some("Claude Code 61k \u{b7} Fable test 1k (no result)")
        );
        // A Start failed on Fable test, then Claude Code.
        let t = ticket(
            "SHOP-48",
            Status::Working,
            agents.clone(),
            vec![
                row(
                    &agents,
                    Role::Developer,
                    Some(("cc", RowSetBy::User)),
                    "cc",
                    Some(RowLock::Started),
                ),
                row(&agents, Role::Validator, None, "cc", None),
            ],
            vec![
                ended(
                    session("a", Role::Developer, &fable, Some("fable"), None, 2_000),
                    1,
                    2,
                ),
                session("b", Role::Developer, &cc, None, None, 50_000),
            ],
        );
        assert_eq!(
            token_line(&t, &tools()).as_deref(),
            Some("Fable test 2k \u{2192} Claude Code 50k \u{b7} no validation yet")
        );
        assert_eq!(
            token_line(&shop_48_set_by_you(), &tools()),
            None,
            "nothing ran yet"
        );
        assert_eq!(
            token_line(&shop_40_lead(), &tools()).as_deref(),
            Some("Strong 48k")
        );
    }

    /// The ⌘N form's rows picked before the ticket exists read *set by you* with Let agents pick again (08
    /// `Cnpry`); with one agent there is nothing to pick, and an existing ticket's rows say it themselves.
    #[test]
    fn a_pick_in_the_new_ticket_form_reads_set_by_you() {
        let mut t = fresh_install();
        let one = block(&t, &tools());
        assert_eq!(one.picked, None, "one agent: no pick");
        assert_eq!(
            (one.note, one.note_icon),
            (Some(ONLY_AGENT), Some(IconName::Info))
        );
        assert_eq!(
            block(&shop_48_set_by_you(), &Tools::default()).note_icon,
            None
        );
        t.agents = list();
        let picked = block(&t, &tools()).picked.expect("a choice");
        assert_eq!(picked.set_by.text, "set by you");
        assert!(!picked.set_by.agent_made);
        assert_eq!(picked.set_by.icon, Some(IconName::User));
        assert_eq!(picked.let_agents_pick, "Let agents pick again");
        assert_eq!(block(&shop_48_set_by_you(), &Tools::default()).picked, None);
    }

    #[test]
    fn the_epic_has_one_lead_row_and_lists_its_children() {
        let t = shop_40_lead();
        let block = block(&t, &tools());
        assert_eq!(block.hint, "who leads the epic's children");
        assert_eq!(block.children_note, Some(CHILDREN_NOTE));
        assert_eq!(block.rows.len(), 1);
        let lead = &block.rows[0];
        assert_eq!(lead.label, "Epic lead");
        assert_eq!(lead.value, "Strong");
        assert_eq!(
            lead.detail.as_deref(),
            Some("claude-opus-5-5 \u{b7} started")
        );
        assert_eq!(
            lead.lock,
            Some("Locked: Send back continues the same conversation")
        );
        assert_eq!(lead.next_lead, None);
        let children: Vec<(&str, &str, Option<&str>, Option<&str>)> = block
            .children
            .iter()
            .map(|c| {
                (
                    c.number.as_str(),
                    c.value.as_str(),
                    c.set_by.as_ref().map(|s| s.text.as_str()),
                    c.note,
                )
            })
            .collect();
        assert_eq!(
            children,
            [
                ("40.1", "Strong", None, Some("merged")),
                ("40.2", "Light", Some("picked by Wishlist lead"), None),
            ]
        );
        // The lead ended and its row was let go (T-0336's lock rule): it changes for the next lead.
        let mut ended_lead = shop_40_lead();
        ended_lead.rows[0].lock = None;
        ended_lead.sessions[0].running = false;
        let lead = &rows(&ended_lead, &tools())[0];
        assert_eq!(lead.next_lead, Some("Change for the next lead"));
        assert_eq!(lead.detail.as_deref(), Some("claude-opus-5-5 \u{b7} ended"));
        assert!(lead.picker.is_some());
    }

    /// At 1280 the locked row's secondary line fits the frame's `guMDH` ("Claude Code · claude-opus-5-5 · started").
    #[test]
    fn the_locked_line_fits_at_1280() {
        let frame = "Claude Code \u{b7} claude-opus-5-5 \u{b7} started"
            .chars()
            .count();
        for t in [
            shop_19(),
            shop_29_validating(),
            shop_22_moved_on(),
            shop_40_lead(),
        ] {
            for row in view(&t).iter().filter(|r| r.lock.is_some()) {
                let line = format!(
                    "{} \u{b7} {}",
                    row.value,
                    row.detail.clone().unwrap_or_default()
                );
                assert!(
                    line.chars().count() <= frame,
                    "{line} is longer than the frame's"
                );
            }
        }
    }

    #[test]
    fn history_names_the_agent_and_the_exact_model() {
        let lines = history_lines(&shop_22_moved_on(), &tools());
        assert_eq!(
            lines,
            [
                HistoryLine::Start {
                    session: "s22a".into(),
                    title: "Session started on Sonnet \u{b7} claude-sonnet-5-5".into(),
                    note: None,
                },
                HistoryLine::Line {
                    at: 5,
                    title: "Session ended \u{b7} Sonnet \u{b7} claude-sonnet-5-5 \u{b7} 54k".into(),
                    tokens: "\u{2014}".into()
                },
                HistoryLine::Start {
                    session: "s22b".into(),
                    title: "Session started on Sonnet \u{b7} claude-sonnet-5-6 (Sonnet \u{2192} claude-sonnet-5-6)"
                        .into(),
                    note: None,
                },
            ]
        );
        // A Done ticket keeps the deleted agent's name and the exact model (AUDIT-y C3).
        let agents = list();
        let light = find(&agents, "light");
        let mut done = ticket(
            "SHOP-17",
            Status::Merged,
            agents.iter().filter(|a| a.id != "light").cloned().collect(),
            vec![row(
                &agents,
                Role::Developer,
                None,
                "cc",
                Some(RowLock::Started),
            )],
            vec![ended(
                session(
                    "s17",
                    Role::Developer,
                    &light,
                    Some("haiku"),
                    Some("claude-haiku-5-5"),
                    48_000,
                ),
                1,
                9,
            )],
        );
        done.rows[0].row = None;
        let lines = history_lines(&done, &tools());
        assert!(lines.contains(&HistoryLine::Line {
            at: 9,
            title: "Developer: Light (deleted) \u{b7} claude-haiku-5-5 \u{b7} 48k".into(),
            tokens: "\u{2014}".into()
        }));
    }

    #[test]
    fn the_gallery_has_every_state() {
        let g = gallery();
        assert_eq!(g.samples.len(), 15);
        assert_eq!(g.samples[0].block.note, Some(ONLY_AGENT));
        assert_eq!(
            g.samples[6].block.rows[0].value,
            "Light (started on claude-haiku-5-5)"
        );
        assert_eq!(g.samples[7].block.rows[0].value, "Light (deleted)");
        assert!(
            g.samples[8].block.rows[1]
                .set_by
                .as_ref()
                .unwrap()
                .text
                .ends_with("round 3")
        );
    }

    #[test]
    fn row_changes_read_as_history_lines() {
        use otaq_core::named_agents::rows::row_event;
        let key: TicketKey = "SHOP-40.3".parse().unwrap();
        let line = |p| row_change_line(&p);
        assert_eq!(
            line(row_event(
                Role::Developer,
                Some("Strong"),
                None,
                &RowSetBy::User,
                None
            )),
            ("Developer: Strong (set by you)".into(), None)
        );
        assert_eq!(
            line(row_event(
                Role::Developer,
                None,
                Some("Strong"),
                &RowSetBy::User,
                None
            )),
            ("Developer: Let agents pick again (by you)".into(), None)
        );
        let by = picked("Wishlist lead", "only a config change after 40.1");
        assert_eq!(
            line(row_event(
                Role::Developer,
                Some("Light"),
                Some("Strong"),
                &by,
                Some(&key)
            )),
            (
                "Wishlist lead re-picked SHOP-40.3: Strong \u{2192} Light".into(),
                Some("only a config change after 40.1".into())
            )
        );
        let by = picked(
            "Wishlist lead",
            "its own pick: you approved the plan with \u{201c}go\u{201d}",
        );
        let key: TicketKey = "SHOP-40.4".parse().unwrap();
        assert_eq!(
            line(row_event(
                Role::Developer,
                Some("Standard"),
                None,
                &by,
                Some(&key)
            ))
            .0,
            "Wishlist lead picked Standard for SHOP-40.4"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // Several tools (M23)

    fn several_view(t: &TicketAgents) -> Vec<RowView> {
        rows(t, &several_tools())
    }

    /// 05 `sFZiP`: locked rows add the tool; with one tool connected the same rows read as M22.
    #[test]
    fn locked_rows_name_the_tool_with_several_tools() {
        let t = shop_31_several();
        let rows = several_view(&t);
        assert_eq!(rows[0].value, "Strong");
        assert_eq!(
            rows[0].detail.as_deref(),
            Some("Claude Code \u{b7} claude-opus-5-5 \u{b7} started")
        );
        assert_eq!(rows[0].set_by.as_ref().unwrap().text, "set by you");
        let avatar = rows[0]
            .avatar
            .as_ref()
            .expect("an avatar with several tools");
        assert_eq!(avatar.letters, "CC");
        assert_eq!(rows[1].value, "Project default (Verifier)");
        assert_eq!(
            rows[1].detail.as_deref(),
            Some("GitHub Copilot \u{b7} gpt-5.5")
        );
        assert_eq!(rows[1].avatar.as_ref().unwrap().letters, "CP");
        // One tool connected: no tool and no avatar.
        let one = rows_with_one(&t);
        assert_eq!(
            one[0].detail.as_deref(),
            Some("claude-opus-5-5 \u{b7} started")
        );
        assert_eq!(
            (one[0].avatar.is_none(), one[1].avatar.is_none()),
            (true, true)
        );
        assert_eq!(one[1].detail.as_deref(), Some("gpt-5.5"));
    }

    fn rows_with_one(t: &TicketAgents) -> Vec<RowView> {
        let mut tools = several_tools();
        tools.connected.truncate(1);
        rows(t, &tools)
    }

    /// AGT-04: "the tool is hidden … whenever it equals the agent's name".
    #[test]
    fn the_tool_drops_out_when_it_is_the_name() {
        let rows = several_view(&shop_19_codex());
        assert_eq!(rows[0].value, "Codex");
        assert_eq!(rows[0].detail.as_deref(), Some("gpt-5.5 \u{b7} started"));
        assert_eq!(rows[0].avatar.as_ref().unwrap().letters, "CX");
        assert_eq!(
            token_line(&shop_19_codex(), &several_tools()).as_deref(),
            Some("Codex 182k \u{b7} no validation yet")
        );
    }

    /// Update Y §3: "asked for gpt-5.5, ran on auto" for the whole session; never for Claude Code, a follow choice,
    /// Copilot's Auto, or the model asked for.
    #[test]
    fn the_swap_note_shows_only_for_an_exact_model_swapped() {
        let t = shop_29_swapped();
        let rows = several_view(&t);
        assert_eq!(rows[1].value, "Verifier");
        assert_eq!(
            rows[1].detail.as_deref(),
            Some("GitHub Copilot \u{b7} auto \u{b7} validating")
        );
        assert_eq!(
            rows[1].swap.as_deref(),
            Some("asked for gpt-5.5, ran on auto")
        );
        assert_eq!(rows[0].swap, None, "Claude Code never swaps");
        let tools = several_tools();
        let mut s = t.sessions[1].clone();
        s.running = false;
        assert!(
            swap_note(&s, COPILOT, &tools).is_some(),
            "after the round too"
        );
        s.model = Some("auto@balance".into());
        s.model_ran = Some("mai-code-1.1-flash".into());
        assert_eq!(
            swap_note(&s, COPILOT, &tools),
            None,
            "Auto choosing a model (Q-J)"
        );
        s.model = Some("gpt-5.5".into());
        s.model_ran = Some("gpt-5.5".into());
        assert_eq!(swap_note(&s, COPILOT, &tools), None);
        s.model = None;
        s.model_ran = Some("auto".into());
        assert_eq!(swap_note(&s, COPILOT, &tools), None, "the tool default");
        s.model = Some("gpt-5.5#high".into());
        s.model_ran = Some("gpt-5.5".into());
        assert_eq!(
            swap_note(&s, COPILOT, &tools),
            None,
            "a variant is the same model"
        );
        // OpenCode's variant (T-0357's `mark::is_swap`): asked opencode/x#high, ran opencode/x.
        let opencode = Tool::ALL[3];
        s.model = Some("opencode/x#high".into());
        s.model_ran = Some("opencode/x".into());
        assert_eq!(swap_note(&s, opencode, &tools), None, "the same model");
        s.model_ran = Some("opencode/y".into());
        assert_eq!(
            swap_note(&s, opencode, &tools).as_deref(),
            Some("asked for opencode/x#high, ran on opencode/y")
        );
        s.model = Some("claude-opus-5-5".into());
        s.model_ran = Some("claude-sonnet-5-6".into());
        assert_eq!(swap_note(&s, CLAUDE, &tools), None);
    }

    /// 05 `ZsIeY`: the moved row, *set by you* kept, and the tokens in both parts.
    #[test]
    fn a_moved_row_says_where_from_and_why() {
        let t = shop_31_moved();
        let rows = several_view(&t);
        assert_eq!(rows[0].value, "Strong GPT");
        assert_eq!(
            rows[0].detail.as_deref(),
            Some("Codex \u{b7} gpt-5.5-pro \u{b7} started")
        );
        assert_eq!(
            rows[0].moved.as_deref(),
            Some("moved from Strong: Claude Code limit")
        );
        assert_eq!(rows[0].set_by.as_ref().unwrap().text, "set by you");
        assert_eq!(
            token_line(&t, &several_tools()).as_deref(),
            Some(
                "Strong \u{b7} Claude Code 120k \u{2192} Strong GPT \u{b7} Codex 80k \u{b7} no validation yet"
            )
        );
        let mut unavailable = t.rows[0].moved.clone().unwrap();
        unavailable.reason = MoveReason::ToolUnavailable;
        unavailable.tool = Some(CODEX);
        assert_eq!(
            moved_text(&unavailable),
            "moved from Strong: Codex not available"
        );
        unavailable.reason = MoveReason::UserMove;
        unavailable.tool = None;
        assert_eq!(moved_text(&unavailable), "moved from Strong by you");
    }

    #[test]
    fn the_token_line_names_each_agent_and_tool() {
        assert_eq!(
            token_line(&shop_29_swapped(), &several_tools()).as_deref(),
            Some("Strong \u{b7} Claude Code 61k \u{b7} Verifier \u{b7} Copilot 23k (1 round)")
        );
        let mut one = several_tools();
        one.connected.truncate(1);
        assert_eq!(
            token_line(&shop_29_swapped(), &one).as_deref(),
            Some("Strong 61k \u{b7} Verifier 23k (1 round)")
        );
    }

    /// Picker items name the tool and carry its avatar (08 `rI6bn` · picking the Epic lead).
    #[test]
    fn picker_items_name_the_tool() {
        let t = shop_31_several();
        let validator = &several_view(&t)[1];
        let items: Vec<(&str, &str)> = validator
            .picker
            .as_ref()
            .unwrap()
            .iter()
            .map(|i| (i.label.as_str(), i.detail.as_str()))
            .collect();
        assert_eq!(
            items[0],
            (
                "Project default (Verifier)",
                "GitHub Copilot \u{b7} gpt-5.5"
            )
        );
        assert!(items.contains(&("Strong", "Claude Code \u{b7} Opus \u{b7} always the newest")));
        assert!(
            items.contains(&("Codex", "tool default")),
            "the tool is the name"
        );
        assert!(items.contains(&("Strong GPT", "Codex \u{b7} gpt-5.5-pro")));
        assert!(
            validator
                .picker
                .as_ref()
                .unwrap()
                .iter()
                .all(|i| i.avatar.is_some())
        );
        let one = rows_with_one(&t);
        assert!(
            one[1]
                .picker
                .as_ref()
                .unwrap()
                .iter()
                .all(|i| i.avatar.is_none())
        );
    }

    /// SHOP-49: the panel's callout for a held Developer.
    #[test]
    fn a_held_row_has_its_callout() {
        let t = shop_49_model_unavailable();
        let c = callouts(&t, &several_tools());
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].heading, "Model unavailable");
        assert!(
            c[0].text
                .as_deref()
                .unwrap()
                .starts_with("Codex no longer lists gpt-5.4, the model of Old GPT.")
        );
        let rows = several_view(&t);
        assert_eq!(rows[0].detail.as_deref(), Some("Codex \u{b7} gpt-5.4"));
        assert_eq!(rows[0].lock, None, "nothing locks");
        assert!(callouts(&shop_31_several(), &several_tools()).is_empty());
    }

    /// History with several tools: the start with the tool and the swap note, a Done ticket's line, and the moves.
    #[test]
    fn history_names_the_tool_and_the_moves() {
        let tools = several_tools();
        let lines = history_lines(&shop_29_swapped(), &tools);
        assert!(lines.contains(&HistoryLine::Start {
            session: "v29".into(),
            title: "Session started on Verifier \u{b7} Copilot \u{b7} auto".into(),
            note: Some("asked for gpt-5.5, ran on auto".into()),
        }));
        assert!(lines.contains(&HistoryLine::Start {
            session: "d29".into(),
            title: "Session started on Strong \u{b7} Claude Code \u{b7} claude-opus-5-5".into(),
            note: None,
        }));
        let lines = history_lines(&shop_19_codex(), &tools);
        assert_eq!(
            lines[0],
            HistoryLine::Start {
                session: "s19".into(),
                title: "Session started on Codex \u{b7} gpt-5.5".into(),
                note: None,
            }
        );
        // One tool: as M22.
        let lines = history_lines(&shop_29_swapped(), &Tools::one(Some(models())));
        assert!(
            matches!(&lines[0], HistoryLine::Start { title, .. } if title == "Session started on Strong \u{b7} claude-opus-5-5")
        );
        // A Done ticket: "Developer: Light (deleted) · Claude Code · claude-haiku-5-5 · 48k".
        let mut done = shop_31_several();
        done.status = Some(Status::Merged);
        done.sessions[0] = ended(done.sessions[0].clone(), 1, 9);
        done.agents.retain(|a| a.id != "strong");
        assert!(history_lines(&done, &tools).contains(&HistoryLine::Line {
            at: 9,
            title: "Developer: Strong (deleted) \u{b7} Claude Code \u{b7} claude-opus-5-5 \u{b7} 120k".into(),
            tokens: "\u{2014}".into()
        }));
    }

    /// A move's `agent_row_set` event ([`row_move_event`], written by the gate) as History's line.
    #[test]
    fn moves_read_as_history_lines() {
        use otaq_core::named_agents::rows::row_move_event;
        let moved = RowMove {
            from_id: Some("strong".into()),
            from_name: "Strong".into(),
            reason: MoveReason::ToolLimit,
            tool: Some(CLAUDE),
            at: 1,
        };
        let line = |m: &RowMove, to: Tool, by_user| {
            row_change_line(&row_move_event(
                Role::Developer,
                "Strong GPT",
                to,
                m,
                &RowSetBy::User,
                by_user,
            ))
            .0
        };
        assert_eq!(
            line(&moved, CODEX, false),
            "Moved to Strong GPT (Codex): Claude Code limit"
        );
        assert_eq!(
            line(&moved, CODEX, true),
            "Developer: Strong GPT (moved by you from Strong \u{b7} Claude Code limit)"
        );
        let unavailable = RowMove {
            reason: MoveReason::ToolUnavailable,
            tool: Some(CODEX),
            ..moved
        };
        assert_eq!(
            line(&unavailable, CLAUDE, false),
            "Moved to Strong GPT (Claude Code): Codex not available"
        );
        // A plain row change isn't a move.
        assert_eq!(
            move_line(&serde_json::json!({ "role": "developer", "agent": "Strong" })),
            None
        );
    }

    #[test]
    fn the_gallery_has_the_several_tools_samples() {
        let g = gallery();
        assert_eq!(g.samples.len(), 15);
        let swap = &g.samples[12];
        assert_eq!(
            swap.block.rows[1].swap.as_deref(),
            Some("asked for gpt-5.5, ran on auto")
        );
        assert_eq!(g.samples[14].callouts.len(), 1);
        assert!(
            g.samples[..10]
                .iter()
                .all(|s| s.block.rows.iter().all(|r| r.avatar.is_none()))
        );
    }
}
