//! The lead's changes in History (CLI-07; d0083): each change an epic lead makes to a child (an edit, a link, a new
//! child) is a line on the child **and** on the epic, with the lead's actor. The child's own event (`updated`,
//! `dependency_added`, `dependency_removed`, `created`) gets the lead's name, so it reads "Updated by Wishlist lead ·
//! description"; the epic gets a `lead_changed` event naming the child ("SHOP-40.3 waits for SHOP-40.4 · by Wishlist
//! lead"). The card's "Updated by agent" follows the child's event as for Project agents (d0068).

use serde_json::{Value, json};

use crate::board::tickets::{EventKind, Ticket, record_event};
use crate::board::{Board, BoardError};
use crate::domain::{Actor, TicketKey};

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
