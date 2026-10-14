//! Counts of agents for the "Service update ready · Restart now (3 agents working, 2 waiting)" banner (d0022).
//! Computed from stored state; zero until sessions exist (M3).

use super::{Store, StoreError};
use crate::domain::Status;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AgentCounts {
    pub working: u32,
    pub waiting: u32,
}

impl Store {
    /// Live agent sessions (not ended), split into waiting for the user and working.
    pub async fn agent_counts(&self) -> Result<AgentCounts, StoreError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT t.state FROM sessions s JOIN tickets t ON t.id = s.ticket_id
             WHERE s.kind = 'agent' AND s.ended_at IS NULL",
        )
        .fetch_all(self.pool())
        .await?;
        let waiting = rows
            .iter()
            .filter(|(state,)| {
                state
                    .parse::<Status>()
                    .is_ok_and(Status::is_needs_you_family)
            })
            .count();
        let to_u32 = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Ok(AgentCounts {
            working: to_u32(rows.len() - waiting),
            waiting: to_u32(waiting),
        })
    }
}
