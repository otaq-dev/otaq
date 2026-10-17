//! Agent rows set from `otaq create` / `otaq update` (`--developer`, `--validator`, `--lead`, `-f` lines; AGT-06;
//! d0086, d0087).
//!
//! A pick names a named agent ignoring case (Q-E), or `default` to empty the row. The service decides who set it
//! (CLI-03): the user's own terminals and an agent's `--by-user` (the agent judged that the user named it) write
//! *set by you*; an agent's own pick writes *picked by <its name>* with its reason. Who may pick at all is
//! [`crate::policy`]'s [`Action::PickAgents`](crate::policy::Action::PickAgents); each row is then judged by
//! [`pick_rule`] before anything is written.

use serde::{Deserialize, Serialize};

use super::tickets::{EventKind, Ticket, record_event};
use super::{Board, BoardError, Refusal};
use crate::domain::{Actor, Kind};
use crate::named_agents::rows::row_event;
use crate::named_agents::{NamedAgent, NamedAgents, Role, RowSetBy, row_lock};
use crate::policy::{RowPick, pick_rule};

/// What `--developer default` names: the empty row (the project default).
pub const DEFAULT_ROW: &str = "default";

/// One row a request sets: `{"role": "developer", "agent": "Light", "reason": "docs-only change"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPick {
    pub role: Role,
    /// A named agent's name, ignoring case; [`DEFAULT_ROW`] empties the row.
    pub agent: String,
    /// An agent's reason ("docs-only change"); needed for an agent's own pick, unused for the user's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Who picks: the user, or an agent by the name its picks carry ("Checkout slowness", "Wishlist lead").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picker<'a> {
    User,
    Agent { name: &'a str, by_user: bool },
}

impl Picker<'_> {
    /// An agent's own pick ("picked by …"), not the user's words.
    fn agent_pick(self) -> bool {
        matches!(self, Self::Agent { by_user: false, .. })
    }
}

/// The ticket the picks are for: one that exists, or one a batch is about to create.
#[derive(Debug, Clone, Copy)]
pub enum PickTarget<'a> {
    Existing(&'a Ticket),
    New { kind: Kind, label: &'a str },
}

/// A checked pick, ready to write: `agent` `None` empties the row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowWrite {
    pub role: Role,
    pub agent: Option<NamedAgent>,
    pub set_by: RowSetBy,
}

/// The named agent `name` names (ignoring case), `None` for [`DEFAULT_ROW`]; an unknown name lists the agents:
/// "No agent named Lite. Agents: Claude Code, Strong, Light, Sonnet".
pub fn find_agent<'a>(
    agents: &'a [NamedAgent],
    name: &str,
) -> Result<Option<&'a NamedAgent>, BoardError> {
    let name = name.trim();
    if name.eq_ignore_ascii_case(DEFAULT_ROW) {
        return Ok(None);
    }
    let folded = name.to_lowercase();
    match agents.iter().find(|a| a.name.to_lowercase() == folded) {
        Some(agent) => Ok(Some(agent)),
        None => {
            let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
            Err(BoardError::NotFound(format!(
                "No agent named {name}. Agents: {}",
                names.join(", ")
            )))
        }
    }
}

/// The refusal of a row the ticket doesn't have: "SHOP-40 is an epic: it has an Epic lead row, not a Developer
/// row".
fn no_such_row(label: &str, kind: Kind, role: Role) -> BoardError {
    let message = match kind {
        Kind::Epic => format!(
            "{label} is an epic: it has an Epic lead row, not a {} row",
            role.label()
        ),
        Kind::Ticket => format!(
            "{label} has no {} row: tickets have Developer and Validator rows",
            role.label()
        ),
    };
    BoardError::refused(Refusal::NoSuchRow, message)
}

/// Checks every pick for `target` and returns what to write; nothing is written, so a refused pick leaves the
/// ticket (or the whole batch) as it was.
pub async fn check_picks(
    board: &Board,
    agents: &NamedAgents,
    target: PickTarget<'_>,
    picks: &[AgentPick],
    picker: Picker<'_>,
) -> Result<Vec<RowWrite>, BoardError> {
    if picks.is_empty() {
        return Ok(Vec::new());
    }
    let list = agents.list().await?.agents;
    let (kind, label, rows, lock_facts) = match target {
        PickTarget::Existing(ticket) => (
            ticket.kind,
            ticket.key.to_string(),
            board.store().ticket_agent_rows(&ticket.id).await?,
            agents.lock_facts(&ticket.id).await?,
        ),
        PickTarget::New { kind, label } => (kind, label.to_owned(), Vec::new(), Default::default()),
    };
    let mut writes = Vec::with_capacity(picks.len());
    for pick in picks {
        if !pick.role.is_row_of(kind) {
            return Err(no_such_row(&label, kind, pick.role));
        }
        let agent = find_agent(&list, &pick.agent)?;
        let row = rows
            .iter()
            .find(|r| r.role == pick.role && r.agent.is_some());
        pick_rule(
            picker.agent_pick(),
            list.len(),
            pick.reason.as_deref(),
            RowPick {
                key: &label,
                role: pick.role,
                set_by: row.map(|r| &r.set_by),
                lock: row_lock(pick.role, lock_facts),
            },
        )?;
        let set_by = match picker {
            Picker::Agent {
                name,
                by_user: false,
            } => RowSetBy::Agent {
                picked_by: name.to_owned(),
                reason: pick.reason.as_deref().unwrap_or_default().trim().to_owned(),
            },
            _ => RowSetBy::User,
        };
        writes.push(RowWrite {
            role: pick.role,
            agent: agent.cloned(),
            set_by,
        });
    }
    Ok(writes)
}

impl Board {
    /// Writes checked picks on `ticket`, skipping rows that already read so. Returns whether a row changed. An
    /// epic lead's pick for a child is logged on the child and on the epic (`epic_id`).
    pub async fn write_picks(
        &self,
        ticket: &Ticket,
        writes: &[RowWrite],
        actor: &Actor,
        epic_id: Option<&str>,
    ) -> Result<bool, BoardError> {
        let store = self.store();
        let mut changed = false;
        for write in writes {
            let before = store
                .ticket_agent_rows(&ticket.id)
                .await?
                .into_iter()
                .find(|r| r.role == write.role && r.agent.is_some());
            let after_id = write.agent.as_ref().map(|a| a.id.as_str());
            let same = match &before {
                Some(row) => row.agent.as_deref() == after_id && row.set_by == write.set_by,
                None => after_id.is_none(),
            };
            if same {
                continue;
            }
            store
                .set_ticket_agent_row(
                    &ticket.id,
                    write.role,
                    after_id.map(|id| (id, &write.set_by)),
                )
                .await?;
            let from = match before.as_ref().and_then(|r| r.agent.as_deref()) {
                Some(id) => store.named_agent(id).await?.map(|a| a.name),
                None => None,
            };
            self.log_pick(ticket, write, from.as_deref(), actor, epic_id)
                .await?;
            changed = true;
        }
        Ok(changed)
    }

    /// History of one row change ([`row_event`]): on the ticket, and for an epic lead's pick on its epic too, with
    /// the child's key ("Wishlist lead re-picked SHOP-40.3: Strong → Light").
    async fn log_pick(
        &self,
        ticket: &Ticket,
        write: &RowWrite,
        from: Option<&str>,
        actor: &Actor,
        epic_id: Option<&str>,
    ) -> Result<(), BoardError> {
        let agent = write.agent.as_ref().map(|a| a.name.as_str());
        let mut tx = self.store().begin_write().await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::AgentRowSet,
            actor,
            row_event(write.role, agent, from, &write.set_by, None),
        )
        .await?;
        if let Some(epic_id) = epic_id.filter(|epic| *epic != ticket.id) {
            record_event(
                &mut tx,
                epic_id,
                EventKind::AgentRowSet,
                actor,
                row_event(write.role, agent, from, &write.set_by, Some(&ticket.key)),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
