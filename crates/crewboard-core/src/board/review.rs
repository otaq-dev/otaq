//! `crewboard review submit` (REV-07; d0050, d0004): the agent's work goes to **Ready for Review** with its
//! summary.
//!
//! It is **refused while the worktree has uncommitted files**, tracked or untracked: "2 files are not committed.
//! Commit them, or discard them if they're scratch files". The agent commits through the Commit skill; Crewboard
//! never commits by itself (GIT-08). Crewboard runs no tests or checks of its own (P5).
//!
//! On submit the service records the review head (`review_head_sha`, the worktree's `HEAD`) and when the ticket
//! entered review (`review_entered_at`), for "changed since you reviewed it" later (T-0097), and the summary in the
//! ticket's history (`review_submitted`), where `crewboard prime` finds it for the tickets that depend on it.

use std::path::Path;

use serde_json::json;

use super::tickets::{EventKind, Ticket, record_event};
use super::{Board, BoardError, Refusal};
use crate::domain::{Actor, Status};
use crate::signals::LIVE;
use crate::store::now_ms;
use crate::store::tickets::set_state_since;

/// "2 files are not committed. Commit them, or discard them if they're scratch files".
pub fn uncommitted_message(count: usize) -> String {
    if count == 1 {
        "1 file is not committed. Commit it, or discard it if it's a scratch file".to_owned()
    } else {
        format!(
            "{count} files are not committed. Commit them, or discard them if they're scratch files"
        )
    }
}

impl Board {
    /// Moves a working ticket to Ready for Review with `summary`, unless its worktree has uncommitted files.
    pub async fn submit_for_review(
        &self,
        key: &str,
        summary: &str,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let ticket = self.ticket(key).await?;
        if !LIVE.contains(&ticket.status) {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!(
                    "{} is {}; only a ticket whose agent is working can be submitted",
                    ticket.key,
                    ticket.status.label()
                ),
            ));
        }
        let worktree = ticket
            .work
            .as_ref()
            .and_then(|w| w.worktree.clone())
            .ok_or_else(|| {
                BoardError::refused(
                    Refusal::MoveRefused,
                    format!("{} has no worktree to submit", ticket.key),
                )
            })?;
        let git = self.git()?;
        let uncommitted = git.uncommitted_files(Path::new(&worktree)).await?;
        if !uncommitted.is_empty() {
            return Err(BoardError::refused(
                Refusal::Uncommitted,
                uncommitted_message(uncommitted.len()),
            ));
        }
        let head = git
            .run(Path::new(&worktree), &["rev-parse", "HEAD"])
            .await?
            .trim()
            .to_owned();
        let now = now_ms();
        let status = Status::ReadyForReview;
        let mut tx = self.store.pool().begin().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, review_head_sha = ?, review_entered_at = ?,
                                signal_text = NULL, signal_since = NULL, wakeup_at = NULL, updated_by = ?,
                                updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(status.column().as_str())
        .bind(status.as_str())
        .bind(&head)
        .bind(now)
        .bind(actor.to_string())
        .bind(now)
        .bind(status.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::ReviewSubmitted,
            actor,
            json!({ "summary": summary.trim(), "head": head }),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(key = %ticket.key, %actor, "submitted for review");
        self.ticket_by_id(&ticket.id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_refusal_names_the_count() {
        assert_eq!(
            uncommitted_message(2),
            "2 files are not committed. Commit them, or discard them if they're scratch files"
        );
        assert_eq!(
            uncommitted_message(1),
            "1 file is not committed. Commit it, or discard it if it's a scratch file"
        );
    }
}
