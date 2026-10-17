//! A ticket's or an epic's agent rows with what the screens need around them (AGT-04, AGT-05, VAL-14; d0086,
//! d0088): each row as stored, what it resolves to, what "Project default (…)" names, its lock, the sessions that
//! ran (with the named agent and the exact model of each), and for an epic its children's Developer rows. The
//! wording is the view models' (`otaq-view`'s `agent_rows.rs`); this is only the facts, answered by
//! `ticket.agents`.
//!
//! A row change is logged on the ticket as [`EventKind::AgentRowSet`] with [`row_event`]'s payload, so History can
//! say "Developer: Strong (set by you)".

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{
    AgentHold, AgentRow, HoldReason, NamedAgent, NamedAgents, Resolved, Role, RowLock, RowMove,
    RowSetBy, resolve, row_lock,
};
use crate::agent_tools::Tool;
use crate::board::BoardError;
use crate::board::tickets::EventKind;
use crate::domain::{Actor, Kind, Status, TicketKey};
pub use crate::store::named_agents::SessionAgentRun;
use crate::usage::AgentTokens;

/// One row of a ticket (Developer, Validator) or an epic (Epic lead).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowFacts {
    pub role: Role,
    /// As stored; `None` = empty.
    pub row: Option<AgentRow>,
    /// What the role runs on now (the row, else the project default, else the app default).
    pub resolved: Option<Resolved>,
    /// What an empty row runs on: "Project default (Claude Code)".
    pub default: Option<NamedAgent>,
    pub lock: Option<RowLock>,
    /// Where the row moved from, and why (AGT-08): "moved from Strong: Claude Code limit"; `None` when it didn't
    /// move or was changed since.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved: Option<RowMove>,
}

/// An epic's child with its Developer row, listed read-only in the epic panel (d0086).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildAgents {
    pub key: TicketKey,
    pub title: String,
    pub status: Status,
    pub developer: Option<Resolved>,
}

/// The answer to `ticket.agents`: a ticket's or an epic's rows, or (no key) a new one's in a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketAgents {
    /// `None` for a ticket not created yet (the ⌘N form).
    pub key: Option<TicketKey>,
    pub kind: Kind,
    pub status: Option<Status>,
    /// Every named agent (the pickers' items; one entry = read-only rows).
    pub agents: Vec<NamedAgent>,
    /// The kind's rows: Developer and Validator, or Epic lead.
    pub rows: Vec<RowFacts>,
    /// Oldest first.
    pub sessions: Vec<SessionAgentRun>,
    /// Tokens by role and named agent ([`crate::usage::Usage::agents_of`]); filled by the service.
    #[serde(default)]
    pub tokens: Vec<AgentTokens>,
    /// An epic's children, by key; empty for a ticket.
    pub children: Vec<ChildAgents>,
    /// The roles held before their start (SCH-04 `[M23]`, §7.2): a tool at its limit or not available, a model no
    /// longer listed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holds: Vec<AgentHold>,
}

/// What a project's board cards say about their agents (SHL-07's footer, AGT-04; T-0345), answered by
/// `board.agents`: how many named agents there are (with one, nothing names an agent) and each ticket's facts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardAgents {
    pub agent_count: usize,
    /// Every ticket of the project (not its epics).
    pub cards: Vec<CardAgents>,
    /// The agent tools connected now (`ConnectedTools`, filled by the service): avatars and tool names show once
    /// there are two (SHL-08, d0086; T-0362).
    #[serde(default)]
    pub connected: Vec<Tool>,
}

/// One ticket's agents for its card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardAgents {
    pub key: TicketKey,
    /// What the Developer and the Validator run on now (the row, else the defaults).
    pub developer: Option<Resolved>,
    pub validator: Option<Resolved>,
    /// The Developer's row points at another agent than an empty row would run on (the project default, else the
    /// app default): the card names it and who set it.
    pub developer_off_default: bool,
    /// The named agent of the running developer session and of the running validator.
    pub running_developer: Option<String>,
    pub running_validator: Option<String>,
    /// The tool (and model) the running developer session and the running validator run on (T-0362).
    #[serde(default)]
    pub running_developer_on: Option<RunsOnTool>,
    #[serde(default)]
    pub running_validator_on: Option<RunsOnTool>,
    /// The roles held before they start (AGT-08, SCH-04 `[M23]`; T-0357 writes them), with the names the card
    /// says.
    #[serde(default)]
    pub holds: Vec<CardHold>,
    /// Where the Developer and the Validator rows moved from, and why (AGT-08).
    #[serde(default)]
    pub developer_move: Option<RowMove>,
    #[serde(default)]
    pub validator_move: Option<RowMove>,
}

/// A tool and the model a session runs (the one the tool reported, else the one passed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunsOnTool {
    pub tool: Tool,
    pub model: Option<String>,
}

/// A role's hold ([`super::AgentHold`]) as a card says it: the named agent it would have started on and its
/// fallback by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardHold {
    pub role: Role,
    pub reason: HoldReason,
    pub tool: Tool,
    pub model: Option<String>,
    /// The named agent it would have started on ("Standard").
    pub agent: Option<String>,
    /// Its fallback ("Sonnet"), also when that one is blocked too (`fallback_held`).
    pub fallback: Option<String>,
    /// Use fallback is offered: the fallback can take it now.
    pub can_use_fallback: bool,
    /// The fallback is blocked as well: "Strong and its fallback Strong GPT are both at their limits".
    pub fallback_held: bool,
}

impl CardHold {
    pub fn of(hold: &AgentHold, agents: &[NamedAgent]) -> Self {
        let named = |id: Option<&String>| id.and_then(|id| agents.iter().find(|a| &a.id == id));
        let agent = named(hold.named_agent_id.as_ref());
        let fallback = named(hold.fallback_agent_id.as_ref())
            .or_else(|| named(agent.and_then(|a| a.fallback.as_ref())));
        Self {
            role: hold.role,
            reason: hold.reason,
            tool: hold.tool,
            model: hold.model.clone(),
            agent: agent.map(|a| a.name.clone()),
            can_use_fallback: hold.fallback_agent_id.is_some() && !hold.fallback_held,
            fallback: fallback.map(|a| a.name.clone()),
            fallback_held: hold.fallback_held,
        }
    }
}

/// The payload of an [`EventKind::AgentRowSet`] event: the role, the agent's name now (`None`: emptied, "Let
/// agents pick again") and before, who set it, and on an epic the child it was for (an epic lead's pick, T-0338).
pub fn row_event(
    role: Role,
    agent: Option<&str>,
    from: Option<&str>,
    set_by: &RowSetBy,
    child: Option<&TicketKey>,
) -> Value {
    let mut payload = json!({ "role": role.as_str(), "agent": agent, "from": from });
    match set_by {
        RowSetBy::User => payload["set_by"] = json!("user"),
        RowSetBy::Agent { picked_by, reason } => {
            payload["set_by"] = json!("agent");
            payload["picked_by"] = json!(picked_by);
            payload["reason"] = json!(reason);
        }
    }
    if let Some(child) = child {
        payload["child"] = json!(child.to_string());
    }
    payload
}

/// The payload of the [`EventKind::AgentRowSet`] event a move writes (AGT-08): [`row_event`]'s, with the tool the
/// row's agent runs on, where it moved from and why, and whether the user moved it (Use fallback, Move waiting to…)
/// or the automatic fallback did, so History can say "Moved to Strong GPT (Codex): Claude Code limit" or
/// "Developer: Strong GPT (moved by you from Strong · Claude Code limit)".
pub fn row_move_event(
    role: Role,
    agent: &str,
    agent_tool: Tool,
    moved: &RowMove,
    set_by: &RowSetBy,
    by_user: bool,
) -> Value {
    let mut payload = row_event(role, Some(agent), Some(&moved.from_name), set_by, None);
    payload["tool"] = json!(agent_tool.as_str());
    payload["moved_reason"] = json!(moved.reason.as_str());
    payload["moved_tool"] = json!(moved.tool.map(Tool::as_str));
    payload["moved_by_user"] = json!(by_user);
    payload
}

fn roles_of(kind: Kind) -> &'static [Role] {
    match kind {
        Kind::Ticket => &[Role::Developer, Role::Validator],
        Kind::Epic => &[Role::EpicLead],
    }
}

impl NamedAgents {
    /// The rows of a ticket of `kind` not created yet in `project`: empty, unlocked, nothing ran.
    pub async fn new_ticket_agents(
        &self,
        project: &str,
        kind: Kind,
    ) -> Result<TicketAgents, BoardError> {
        let project = self.board.project(project).await?;
        let store = self.board.store();
        let agents = store.named_agents().await?;
        let app = store.agent_defaults().await?;
        let defaults = project.agent_defaults();
        let rows = roles_of(kind)
            .iter()
            .map(|&role| {
                let resolved = resolve(role, None, &defaults, &app, &agents);
                RowFacts {
                    role,
                    default: resolved.as_ref().map(|r| r.agent.clone()),
                    resolved,
                    lock: None,
                    row: None,
                    moved: None,
                }
            })
            .collect();
        Ok(TicketAgents {
            key: None,
            kind,
            status: None,
            agents,
            rows,
            sessions: Vec::new(),
            tokens: Vec::new(),
            children: Vec::new(),
            holds: Vec::new(),
        })
    }

    /// Everything the rows of `key` show ([`TicketAgents`]).
    pub async fn ticket_agents(&self, key: &str) -> Result<TicketAgents, BoardError> {
        let ticket = self.board.ticket(key).await?;
        let project_key = ticket.key.project.to_string();
        let project = self.board.project(&project_key).await?;
        let store = self.board.store();
        let agents = store.named_agents().await?;
        let app = store.agent_defaults().await?;
        let defaults = project.agent_defaults();
        let stored = store.ticket_agent_rows(&ticket.id).await?;
        let facts = self.lock_facts(&ticket.id).await?;
        let mut rows = Vec::new();
        for &role in roles_of(ticket.kind) {
            let row = stored.iter().find(|r| r.role == role).cloned();
            rows.push(RowFacts {
                role,
                resolved: resolve(role, row.as_ref(), &defaults, &app, &agents),
                default: resolve(role, None, &defaults, &app, &agents).map(|r| r.agent),
                lock: row_lock(role, facts),
                moved: store.ticket_row_move(&ticket.id, role).await?,
                row,
            });
        }
        let holds = store.ticket_holds(&ticket.id).await?;
        let mut children = Vec::new();
        if ticket.kind == Kind::Epic {
            for child in self.board.tickets(&project_key).await? {
                if child.epic.as_ref() != Some(&ticket.key) {
                    continue;
                }
                let rows = store.ticket_agent_rows(&child.id).await?;
                children.push(ChildAgents {
                    developer: resolve(
                        Role::Developer,
                        rows.iter().find(|r| r.role == Role::Developer),
                        &defaults,
                        &app,
                        &agents,
                    ),
                    key: child.key,
                    title: child.title,
                    status: child.status,
                });
            }
        }
        let mut sessions = store.session_agents(&ticket.id).await?;
        for session in &mut sessions {
            session.moved_on = crate::session::moved_on(store.pool(), &session.id).await?;
        }
        Ok(TicketAgents {
            sessions,
            tokens: Vec::new(),
            key: Some(ticket.key),
            kind: ticket.kind,
            status: Some(ticket.status),
            agents,
            rows,
            children,
            holds,
        })
    }

    /// Every card's agent facts in `project` ([`BoardAgents`]): read in three queries, whatever the board's size.
    pub async fn board_agents(&self, project: &str) -> Result<BoardAgents, BoardError> {
        let project = self.board.project(project).await?;
        let store = self.board.store();
        let agents = store.named_agents().await?;
        let app = store.agent_defaults().await?;
        let defaults = project.agent_defaults();
        let rows = store.project_agent_rows(&project.id).await?;
        let running = store.running_agent_names(&project.id).await?;
        let holds = store.project_ticket_holds(&project.id).await?;
        let moves = store.project_row_moves(&project.id).await?;
        let mut cards = Vec::new();
        for ticket in self.board.tickets(&project.key).await? {
            if ticket.kind != Kind::Ticket {
                continue;
            }
            let row = |role| {
                rows.iter()
                    .find(|(id, r)| *id == ticket.id && r.role == role)
                    .map(|(_, r)| r)
            };
            let resolve_row = |role| resolve(role, row(role), &defaults, &app, &agents);
            let running_of = |role| {
                running
                    .iter()
                    .rev()
                    .find(|r| r.ticket_id == ticket.id && r.role == role)
            };
            let name_of = |role| running_of(role).map(|r| r.name.clone());
            let on_of = |role| {
                running_of(role).and_then(|r| {
                    Some(RunsOnTool {
                        tool: r.tool?,
                        model: r.model.clone(),
                    })
                })
            };
            let move_of = |role| {
                moves
                    .iter()
                    .find(|(id, r, _)| *id == ticket.id && *r == role)
                    .map(|(_, _, m)| m.clone())
            };
            let developer = resolve_row(Role::Developer);
            let default = resolve(Role::Developer, None, &defaults, &app, &agents);
            let card = CardAgents {
                developer_off_default: matches!(
                    (&developer, &default),
                    (Some(d), Some(e)) if matches!(d.came_from, super::CameFrom::Row { .. })
                        && d.agent.id != e.agent.id
                ),
                developer,
                validator: resolve_row(Role::Validator),
                running_developer: name_of(Role::Developer),
                running_validator: name_of(Role::Validator),
                running_developer_on: on_of(Role::Developer),
                running_validator_on: on_of(Role::Validator),
                holds: holds
                    .iter()
                    .filter(|h| h.ticket_id.as_deref() == Some(ticket.id.as_str()))
                    .map(|h| CardHold::of(h, &agents))
                    .collect(),
                developer_move: move_of(Role::Developer),
                validator_move: move_of(Role::Validator),
                key: ticket.key,
            };
            cards.push(card);
        }
        Ok(BoardAgents {
            agent_count: agents.len(),
            cards,
            connected: Vec::new(),
        })
    }

    /// Logs a row change on the ticket ([`row_event`]).
    pub(super) async fn record_row_change(
        &self,
        ticket_id: &str,
        role: Role,
        before: Option<&str>,
        after: Option<&str>,
        set_by: &RowSetBy,
    ) -> Result<(), BoardError> {
        let store = self.board.store();
        let name = async |id: Option<&str>| -> Result<Option<String>, BoardError> {
            Ok(match id {
                Some(id) => store.named_agent(id).await?.map(|a| a.name),
                None => None,
            })
        };
        let payload = row_event(
            role,
            name(after).await?.as_deref(),
            name(before).await?.as_deref(),
            set_by,
            None,
        );
        let mut tx = store.begin_write().await?;
        crate::board::tickets::record_event(
            &mut tx,
            ticket_id,
            EventKind::AgentRowSet,
            &Actor::current_user(),
            payload,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
