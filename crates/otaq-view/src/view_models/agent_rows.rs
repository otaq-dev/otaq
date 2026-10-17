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
//!   tool is connected (M22: always).
//! - **One named agent:** read-only rows with the bare name and "Your only agent…" under them.
//! - **"Moved on":** "Sonnet → claude-sonnet-5-6 · new since the last run (claude-sonnet-5-5)" under a locked row
//!   while its session runs.
//! - **The token line** in the panel's header (VAL-14, Q-G): "Strong 61k · Light 23k (1 round)".
//! - **History lines**: session starts and ends with the named agent and the exact model, row changes, and a Done
//!   ticket's "Developer: Light (deleted) · claude-haiku-5-5 · 48k".

use otaq_core::agent_tools::models::{ModelKind, ModelList};
use otaq_core::domain::{Column, Kind, Status, TicketKey};
use otaq_core::named_agents::rows::{RowFacts, SessionAgentRun, TicketAgents};
use otaq_core::named_agents::{CameFrom, Role, RowLock, RowSetBy};
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::card::tokens_label;
use crate::view_models::named_agents::{RoleId, TOOL_DEFAULT, agent_model};

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
    /// The model as Settings shows it.
    pub detail: String,
    pub selected: bool,
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

fn picker(t: &TicketAgents, facts: &RowFacts, models: Option<&ModelList>) -> Vec<PickerItem> {
    let chosen = facts.row.as_ref().and_then(|r| r.agent.as_deref());
    let chosen = chosen.filter(|id| t.agents.iter().any(|a| a.id == *id));
    let default = facts.default.as_ref();
    std::iter::once(PickerItem {
        agent: None,
        label: format!(
            "Project default ({})",
            default.map(|a| a.name.as_str()).unwrap_or_default()
        ),
        detail: default.map(|a| agent_model(a, models)).unwrap_or_default(),
        selected: chosen.is_none(),
    })
    .chain(t.agents.iter().map(|a| PickerItem {
        agent: Some(a.id.clone()),
        label: a.name.clone(),
        detail: agent_model(a, models),
        selected: chosen == Some(a.id.as_str()),
    }))
    .collect()
}

fn row_view(t: &TicketAgents, facts: &RowFacts, models: Option<&ModelList>) -> Option<RowView> {
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
                let exact = exact_model(session, models);
                let (value, changed) = ran_on(t, session, facts, &exact);
                view.value = value;
                let state = match lock {
                    RowLock::RoundRunning => "validating",
                    RowLock::Started => "started",
                };
                view.detail = (!changed).then(|| format!("{exact} \u{b7} {state}"));
                if session.running {
                    view.moved_on = moved_on(session, models);
                }
            }
            None => {
                // Locked before a session is listed (the lead's row exists before its first session).
                let agent = facts.resolved.as_ref().map(|r| &r.agent);
                view.value = agent.map(|a| a.name.clone()).unwrap_or_default();
                view.detail = agent.map(|a| agent_model(a, models));
            }
        }
        return Some(view);
    }

    // One named agent: read-only, the bare name (AGT-04, 08 `ehjFV`).
    if t.agents.len() <= 1 {
        let agent = facts.resolved.as_ref().map(|r| &r.agent);
        view.value = agent.map(|a| a.name.clone()).unwrap_or_default();
        view.detail = agent.map(|a| agent_model(a, models));
        return Some(view);
    }

    match (came_from, facts.resolved.as_ref()) {
        (Some(CameFrom::Row { set_by }), Some(resolved)) => {
            view.value = resolved.agent.name.clone();
            view.detail = Some(agent_model(&resolved.agent, models));
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
            view.detail = default.map(|a| agent_model(a, models));
        }
    }
    // An ended lead: the next lead runs on the new pick (*Agent row · Epic lead*).
    if facts.role == Role::EpicLead
        && let Some(last) = sessions_of(t, Role::EpicLead).next_back()
        && !last.running
    {
        view.next_lead = Some(NEXT_LEAD);
        view.detail = Some(format!("{} \u{b7} ended", exact_model(last, models)));
    }
    view.picker = Some(picker(t, facts, models));
    view.change = Some(CHANGE);
    Some(view)
}

fn finished(t: &TicketAgents) -> bool {
    t.status.is_some_and(|s| s.column() == Column::Done)
}

/// Every row of `t`, in the kind's order.
pub fn rows(t: &TicketAgents, models: Option<&ModelList>) -> Vec<RowView> {
    t.rows
        .iter()
        .filter_map(|facts| row_view(t, facts, models))
        .collect()
}

fn child_number(key: &TicketKey) -> String {
    match key.child {
        Some(child) => format!("{}.{child}", key.number),
        None => key.number.to_string(),
    }
}

/// The "Agents" block: the rows, the note under them, and an epic's children.
pub fn block(t: &TicketAgents, models: Option<&ModelList>) -> Block {
    let rows = rows(t, models);
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
fn agent_part(t: &TicketAgents, role: Role) -> Option<String> {
    let row_name = t
        .rows
        .iter()
        .find(|r| r.role == role)
        .and_then(|r| r.resolved.as_ref())
        .map(|r| r.agent.name.clone());
    let parts: Vec<String> = t
        .tokens
        .iter()
        .filter(|p| p.role == role)
        .map(|p| {
            let name = p
                .agent
                .clone()
                .or_else(|| row_name.clone())
                .unwrap_or_default();
            format!("{name} {}", tokens_text(p.tokens.map(|t| t.shown())))
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(" \u{2192} "))
}

/// The panel header's tokens by agent (VAL-14, d0086, Q-G): "Strong 182k · validation —", "Strong 61k · Light 23k
/// (1 round)", "Claude Code 61k · Fable test 1k (no result)"; an epic's lead "Strong 48k". `None` before any agent
/// session. The tool is never named while one tool is connected.
pub fn token_line(t: &TicketAgents) -> Option<String> {
    if t.kind == Kind::Epic {
        return agent_part(t, Role::EpicLead);
    }
    let developer = agent_part(t, Role::Developer)?;
    let Some(validator) = agent_part(t, Role::Validator) else {
        return Some(format!("{developer} \u{b7} validation \u{2014}"));
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
    /// claude-sonnet-5-6)".
    Start { session: String, title: String },
    /// A line of its own: "Session ended · Sonnet · claude-sonnet-5-5 · 54k", "Developer: Light (deleted) ·
    /// claude-haiku-5-5 · 48k".
    Line {
        at: i64,
        title: String,
        tokens: String,
    },
}

/// One line per session start and end with the exact model, and a Done or Closed ticket's agents as they ran.
pub fn history_lines(t: &TicketAgents, models: Option<&ModelList>) -> Vec<HistoryLine> {
    let mut lines = Vec::new();
    for session in &t.sessions {
        let Some(facts) = t.rows.iter().find(|r| r.role == session.role) else {
            continue;
        };
        let name = session_name(session, facts);
        if name.is_empty() {
            continue;
        }
        let exact = exact_model(session, models);
        let mut title = format!("Session started on {name} \u{b7} {exact}");
        if let Some(moved) = &session.moved_on {
            let follow = follow_name(session.model.as_deref(), models).unwrap_or(name.clone());
            title.push_str(&format!(" ({follow} \u{2192} {})", moved.now));
        }
        lines.push(HistoryLine::Start {
            session: session.id.clone(),
            title,
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
            let exact = exact_model(last, models);
            let deleted = last
                .named_agent_id
                .as_deref()
                .is_some_and(|id| !t.agents.iter().any(|a| a.id == id));
            let name = session_name(last, facts);
            let tokens: Option<u64> = ran.iter().filter_map(|s| s.tokens).reduce(|a, b| a + b);
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
pub fn row_change_line(p: &serde_json::Value) -> (String, Option<String>) {
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
}

/// Every row state of the frames: one agent, set by you, picked by an agent, locked while started and while a
/// round runs, moved on, edited and deleted after the start, the Validator after a failed round, the epic lead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentRowsGallery")]
pub struct Gallery {
    pub samples: Vec<GallerySample>,
}

pub fn gallery() -> Gallery {
    use fixtures::*;
    let models = models();
    let sample = |caption, t: TicketAgents| GallerySample {
        caption,
        block: block(&t, Some(&models)),
        token_line: token_line(&t),
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
    failed.status = Some(Status::ValidationFailed);
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
        AgentRow, CameFrom, NamedAgent, Resolved, Role, RowLock, RowSetBy,
    };
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
                .find(|p| p.role == s.role && p.agent == s.named_agent_name)
            {
                Some(p) => {
                    if let Some(t) = &mut p.tokens {
                        t.input += tokens.input;
                    }
                }
                None => out.push(AgentTokens {
                    role: s.role,
                    agent: s.named_agent_name.clone(),
                    tool: Some(Tool::ALL[0]),
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
            Status::Validating,
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
            status: Some(Status::EpicInProgress),
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use crate::view_models::named_agents::model_label;

    fn models_list() -> ModelList {
        models()
    }

    fn view(t: &TicketAgents) -> Vec<RowView> {
        rows(t, Some(&models_list()))
    }

    #[test]
    fn the_model_reads_as_settings_shows_it() {
        let m = models_list();
        let label = |model: Option<&str>| model_label(model, Some(&m));
        assert_eq!(label(None), "tool default");
        assert_eq!(label(Some("default")), "tool default");
        assert_eq!(label(Some("opus")), "Opus \u{b7} always the newest");
        assert_eq!(label(Some("fable")), "Fable \u{b7} always the newest");
        assert_eq!(label(Some("claude-opus-5-5")), "claude-opus-5-5");
        assert_eq!(agent_model(&starter(), Some(&m)), "tool default");
        // Never "Claude Code · opus": no tool, and the alias as Settings words it.
        assert!(!agent_model(&agent("s", "Strong", Some("opus")), Some(&m)).contains("Claude"));
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
        let block = block(&t, Some(&models_list()));
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
        let block = block(&t, Some(&models_list()));
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
            Status::ValidationFailed,
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
            token_line(&t).as_deref(),
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
            token_line(&shop_19()).as_deref(),
            Some("Strong 182k \u{b7} validation \u{2014}")
        );
        assert_eq!(
            token_line(&shop_29_validating()).as_deref(),
            Some("Strong 61k \u{b7} Light 23k (1 round)")
        );
        // A validator that only had refused rounds (*SHOP-29 · No result*).
        let agents = list();
        let (cc, fable) = (starter(), find(&agents, "fable"));
        let t = ticket(
            "SHOP-29",
            Status::Validating,
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
            token_line(&t).as_deref(),
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
            token_line(&t).as_deref(),
            Some("Fable test 2k \u{2192} Claude Code 50k \u{b7} validation \u{2014}")
        );
        assert_eq!(token_line(&shop_48_set_by_you()), None, "nothing ran yet");
        assert_eq!(token_line(&shop_40_lead()).as_deref(), Some("Strong 48k"));
    }

    /// The ⌘N form's rows picked before the ticket exists read *set by you* with Let agents pick again (08
    /// `Cnpry`); with one agent there is nothing to pick, and an existing ticket's rows say it themselves.
    #[test]
    fn a_pick_in_the_new_ticket_form_reads_set_by_you() {
        let mut t = fresh_install();
        let one = block(&t, Some(&models_list()));
        assert_eq!(one.picked, None, "one agent: no pick");
        assert_eq!(
            (one.note, one.note_icon),
            (Some(ONLY_AGENT), Some(IconName::Info))
        );
        assert_eq!(block(&shop_48_set_by_you(), None).note_icon, None);
        t.agents = list();
        let picked = block(&t, Some(&models_list())).picked.expect("a choice");
        assert_eq!(picked.set_by.text, "set by you");
        assert!(!picked.set_by.agent_made);
        assert_eq!(picked.set_by.icon, Some(IconName::User));
        assert_eq!(picked.let_agents_pick, "Let agents pick again");
        assert_eq!(block(&shop_48_set_by_you(), None).picked, None);
    }

    #[test]
    fn the_epic_has_one_lead_row_and_lists_its_children() {
        let t = shop_40_lead();
        let block = block(&t, Some(&models_list()));
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
        let lead = &rows(&ended_lead, Some(&models_list()))[0];
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
        let lines = history_lines(&shop_22_moved_on(), Some(&models_list()));
        assert_eq!(
            lines,
            [
                HistoryLine::Start {
                    session: "s22a".into(),
                    title: "Session started on Sonnet \u{b7} claude-sonnet-5-5".into()
                },
                HistoryLine::Line {
                    at: 5,
                    title: "Session ended \u{b7} Sonnet \u{b7} claude-sonnet-5-5 \u{b7} 54k".into(),
                    tokens: "\u{2014}".into()
                },
                HistoryLine::Start {
                    session: "s22b".into(),
                    title: "Session started on Sonnet \u{b7} claude-sonnet-5-6 (Sonnet \u{2192} claude-sonnet-5-6)"
                        .into()
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
        let lines = history_lines(&done, Some(&models_list()));
        assert!(lines.contains(&HistoryLine::Line {
            at: 9,
            title: "Developer: Light (deleted) \u{b7} claude-haiku-5-5 \u{b7} 48k".into(),
            tokens: "\u{2014}".into()
        }));
    }

    #[test]
    fn the_gallery_has_every_state() {
        let g = gallery();
        assert_eq!(g.samples.len(), 10);
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
}
