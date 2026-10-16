//! The lead's changes in History (CLI-07; d0083): each change an epic lead makes to a child (an edit, a link, a new
//! child) is a line on the child **and** on the epic, with the lead's actor. The child's own event (`updated`,
//! `dependency_added`, `dependency_removed`, `created`) gets the lead's name, so it reads "Updated by Wishlist lead ·
//! description"; the epic gets a `lead_changed` event naming the child ("SHOP-40.3 waits for SHOP-40.4 · by Wishlist
//! lead"). The card's "Updated by agent" follows the child's event as for Project agents (d0068).

use serde_json::{Value, json};

use crate::board::tickets::{EventKind, Ticket, record_event};
use crate::board::{Board, BoardError};
use crate::domain::{Actor, ByLead, TicketKey};

/// What the lead changed on a child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeadChange {
    /// `otaq update`: the fields it set ("title", "description", "priority").
    Updated { fields: Vec<&'static str> },
    /// `otaq dep add`: the child now waits for `parent`.
    DepAdded { parent: TicketKey },
    /// `otaq dep remove`.
    DepRemoved { parent: TicketKey },
    /// `otaq create --parent`: a new child.
    Created,
}

impl LeadChange {
    fn child_event(&self) -> EventKind {
        match self {
            Self::Updated { .. } => EventKind::Updated,
            Self::DepAdded { .. } => EventKind::DependencyAdded,
            Self::DepRemoved { .. } => EventKind::DependencyRemoved,
            Self::Created => EventKind::Created,
        }
    }

    fn payload(&self, lead: &str, child: &Ticket) -> Value {
        let mut payload = json!({"lead": lead, "child": child.key});
        let extra = match self {
            Self::Updated { fields } => json!({"change": "updated", "fields": fields}),
            Self::DepAdded { parent } => json!({"change": "dep_added", "parent": parent}),
            Self::DepRemoved { parent } => json!({"change": "dep_removed", "parent": parent}),
            Self::Created => json!({"change": "created", "title": child.title}),
        };
        if let (Some(payload), Value::Object(extra)) = (payload.as_object_mut(), extra) {
            payload.extend(extra);
        }
        payload
    }
}

/// Writes the lead's change of `child` into History: names the lead (`lead`) on the child's latest event of that
/// kind by `actor`, and adds a `lead_changed` line to the epic `epic_id`.
pub async fn record(
    board: &Board,
    lead: &str,
    epic_id: &str,
    child: &Ticket,
    change: &LeadChange,
    actor: &Actor,
) -> Result<(), BoardError> {
    let mut tx = board.store().pool().begin().await?;
    sqlx::query(
        "UPDATE events SET payload_json = json_set(payload_json, '$.lead', ?)
         WHERE id = (SELECT id FROM events WHERE ticket_id = ? AND kind = ? AND actor = ?
                     ORDER BY id DESC LIMIT 1)",
    )
    .bind(lead)
    .bind(&child.id)
    .bind(change.child_event().as_str())
    .bind(actor.to_string())
    .execute(&mut *tx)
    .await?;
    record_event(
        &mut tx,
        epic_id,
        EventKind::LeadChanged,
        actor,
        change.payload(lead, child),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// The "by lead" flag of each child of the project (LEAD-08): the children a lead created, and the edits and links a
/// lead made since the user last looked at the child (the "Updated by agent" mark's `agent_edits_seen`, d0019).
pub(crate) async fn by_lead_flags(
    board: &Board,
    project_id: &str,
) -> Result<std::collections::HashMap<String, ByLead>, BoardError> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT e.ticket_id, e.kind, json_extract(e.payload_json, '$.lead'), e.payload_json FROM events e
         JOIN tickets t ON t.id = e.ticket_id
         WHERE t.project_id = ? AND e.kind IN (?, ?, ?, ?)
           AND json_extract(e.payload_json, '$.lead') IS NOT NULL
           AND (e.kind = ? OR e.id > COALESCE((SELECT MAX(s.id) FROM events s
                                               WHERE s.ticket_id = e.ticket_id AND s.kind = ?), ''))
         ORDER BY e.id",
    )
    .bind(project_id)
    .bind(EventKind::Created.as_str())
    .bind(EventKind::Updated.as_str())
    .bind(EventKind::DependencyAdded.as_str())
    .bind(EventKind::DependencyRemoved.as_str())
    .bind(EventKind::Created.as_str())
    .bind(EventKind::AgentEditsSeen.as_str())
    .fetch_all(board.store().pool())
    .await?;
    let mut flags: std::collections::HashMap<String, ByLead> = std::collections::HashMap::new();
    for (ticket, kind, lead, payload) in rows {
        let Ok(kind) = kind.parse::<EventKind>() else {
            continue;
        };
        let payload: Value = serde_json::from_str(&payload).unwrap_or_default();
        let flag = flags.entry(ticket).or_default();
        flag.lead = lead;
        match kind {
            EventKind::Created => flag.added = true,
            EventKind::Updated => {
                flag.updated = true;
                let names = payload["changes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c["field"].as_str())
                    .map(field_word);
                for name in names {
                    if !flag.fields.iter().any(|f| f == name) {
                        flag.fields.push(name.to_owned());
                    }
                }
            }
            EventKind::DependencyAdded | EventKind::DependencyRemoved => {
                flag.updated = true;
                let Some(parent) = payload["parent"].as_str().and_then(|k| k.parse().ok()) else {
                    continue;
                };
                flag.waits_for.retain(|k| k != &parent);
                if kind == EventKind::DependencyAdded {
                    flag.waits_for.push(parent);
                }
            }
            _ => {}
        }
    }
    Ok(flags)
}

/// A stored field's name in History's words ("body" is the description).
fn field_word(field: &str) -> &str {
    match field {
        "body" => "description",
        "base_branch" => "base branch",
        "external_id" => "External ID",
        other => other,
    }
}
