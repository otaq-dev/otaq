//! "conflicts with `main`" (GIT-03, GIT-04, GIT-06; d0038, d0044, d0051): a fact about a ticket's branch and the
//! branch it merges into, shown on the card and in the panel, and named by `otaq prime`.
//!
//! It comes from a Delivery's free rebase (the ticket goes to its agent with `restack`, no click, FLW-17),
//! from the read-only test merge after another ticket merged (T-0096), or from the rebase of a reopened ticket on
//! its way to Dev (T-0101). It clears when the agent submits again, or when the next test merge is clean.

use serde::{Deserialize, Serialize};

use super::{Board, BoardError};
use crate::domain::stored_enum;

stored_enum! {
    /// Where a conflict was found.
    pub enum ConflictReason ("conflict reason") {
        /// A Delivery's rebase or the test merge: its agent fixes it with `restack`.
        Merge = "merge",
        /// The rebase of a reopened ticket on its way to Dev: the agent resolves it first.
        Reopen = "reopen",
    }
}

/// A ticket's conflict with the branch it merges into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    /// `main`, `develop`, or the epic's branch.
    pub target: String,
    pub files: Vec<String>,
    pub reason: ConflictReason,
}

impl Board {
    /// Records that the ticket's branch conflicts with `target`.
    pub async fn set_conflict(
        &self,
        ticket_id: &str,
        conflict: &Conflict,
    ) -> Result<(), BoardError> {
        sqlx::query(
            "UPDATE tickets SET conflicts_with = ?, conflict_files = ?, conflict_reason = ? WHERE id = ?",
        )
        .bind(&conflict.target)
        .bind(serde_json::to_string(&conflict.files).expect("paths serialize"))
        .bind(conflict.reason.as_str())
        .bind(ticket_id)
        .execute(self.store.pool())
        .await?;
        Ok(())
    }

    pub async fn clear_conflict(&self, ticket_id: &str) -> Result<(), BoardError> {
        sqlx::query(
            "UPDATE tickets SET conflicts_with = NULL, conflict_files = NULL, conflict_reason = NULL WHERE id = ?",
        )
        .bind(ticket_id)
        .execute(self.store.pool())
        .await?;
        Ok(())
    }

    /// The ticket's recorded conflict, if any.
    pub async fn conflict(&self, ticket_id: &str) -> Result<Option<Conflict>, BoardError> {
        let row = self.store.ticket(ticket_id).await?;
        Ok(row.and_then(|row| {
            Some(Conflict {
                target: row.conflicts_with?,
                files: row
                    .conflict_files
                    .and_then(|f| serde_json::from_str(&f).ok())
                    .unwrap_or_default(),
                reason: row
                    .conflict_reason
                    .and_then(|r| r.parse().ok())
                    .unwrap_or(ConflictReason::Merge),
            })
        }))
    }
}
