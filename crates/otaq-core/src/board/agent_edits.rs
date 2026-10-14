//! Agent edits made visible (TKT-05; d0019, d0036): every field an agent changes is in the `events` log with its
//! before and after value (written by `update_ticket`). A ticket shows "Updated by agent" while there are agent
//! edits the user hasn't looked at; looking at them writes an `agent_edits_seen` event, which clears the marker
//! and keeps the history.
//!
//! Event ids are UUIDv7, ordered by creation within the service, so "after the last seen mark" compares ids.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::tickets::{EventKind, FieldChange, record_event};
use super::{Board, BoardError};
use crate::domain::Actor;

/// One field change made by an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentChange {
    pub actor: Actor,
    /// Unix milliseconds.
    pub at: i64,
    pub change: FieldChange,
}

/// The "Updated by agent" marker and the changes behind it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentEdits {
    pub updated_by_agent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_agent: Option<Actor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_at: Option<i64>,
    /// Changes since the user last looked, newest first.
    pub changes: Vec<AgentChange>,
}

/// Agent "updated" events after the ticket's last seen mark (SQL shared by the per-ticket and per-project
/// queries). Binds: none; the caller adds its own filter on `e.ticket_id`.
const UNSEEN_AGENT_UPDATES: &str = "
    SELECT e.ticket_id, e.actor, e.payload_json, e.at FROM events e
    WHERE e.kind = 'updated' AND (e.actor LIKE 'agent:%' OR e.actor LIKE 'project-agent:%')
      AND e.id > COALESCE((SELECT MAX(s.id) FROM events s
                           WHERE s.ticket_id = e.ticket_id AND s.kind = 'agent_edits_seen'), '')";

impl Board {
    /// The agent edits the user hasn't seen yet on a ticket.
    pub async fn agent_edits(&self, key: &str) -> Result<AgentEdits, BoardError> {
        let id = self.ticket_id(key).await?;
        let sql = format!("{UNSEEN_AGENT_UPDATES} AND e.ticket_id = ? ORDER BY e.id DESC");
        let rows: Vec<(String, String, String, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(&id)
            .fetch_all(self.store.pool())
            .await?;
        let mut edits = AgentEdits::default();
        for (_, actor, payload, at) in rows {
            let actor: Actor = actor.parse().expect("infallible");
            if edits.last_agent.is_none() {
                edits.updated_by_agent = true;
                edits.last_agent = Some(actor.clone());
                edits.last_at = Some(at);
            }
            let payload: Value = serde_json::from_str(&payload).unwrap_or_default();
            // One by one: changes of fields that no longer exist ("Done when", d0067) are skipped.
            let changes: Vec<FieldChange> = payload["changes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|change| serde_json::from_value(change.clone()).ok())
                .collect();
            // Newest first, also within one update.
            edits
                .changes
                .extend(changes.into_iter().rev().map(|change| AgentChange {
                    actor: actor.clone(),
                    at,
                    change,
                }));
        }
        Ok(edits)
    }

    /// Clears the "Updated by agent" marker; the history stays.
    pub async fn mark_agent_edits_seen(&self, key: &str, actor: &Actor) -> Result<(), BoardError> {
        let id = self.ticket_id(key).await?;
        let mut tx = self.store.pool().begin().await?;
        record_event(&mut tx, &id, EventKind::AgentEditsSeen, actor, json!({})).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Ids of a project's tickets with unseen agent edits (the card flag).
    pub(crate) async fn tickets_updated_by_agent(
        &self,
        project_id: &str,
    ) -> Result<Vec<String>, BoardError> {
        let sql = format!(
            "SELECT DISTINCT u.ticket_id FROM ({UNSEEN_AGENT_UPDATES}) u
             JOIN tickets t ON t.id = u.ticket_id WHERE t.project_id = ?"
        );
        Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(project_id)
            .fetch_all(self.store.pool())
            .await?)
    }
}
