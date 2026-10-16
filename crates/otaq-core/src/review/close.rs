//! Close ticket and closing an epic (CLS-01, CLS-02, EPC-06; d0033, d0044).
//!
//! **Close ticket** works from any state that isn't finished: if an agent runs, the request needs `confirm`
//! ("its agent will be stopped first"). The ticket becomes Done · Closed · not merged; the Done cleanup follows
//! (T-0099): the agent stops, ports return, the worktree is removed unless it has uncommitted changes, the
//! **branch is kept**. Mark closed on a PR ticket (M10) takes the same path.
//!
//! **Closing an epic** runs its close plan as one request: unfinished children closed (their agents stopped),
//! merged children untouched, and the epic ends Closed · not merged (its branch never reached the base).

use super::Review;
use crate::board::epics::{ClosePlan, close_bookkeeping};
use crate::board::tickets::Ticket;
use crate::board::{BoardError, Refusal};
use crate::domain::{Actor, Status};
use crate::session::SessionKind;

impl Review {
    /// **Close ticket**: `confirm` is the user's yes to stopping its running agent.
    pub async fn close(
        &self,
        key: &str,
        confirm: bool,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let board = &self.inner.board;
        let ticket = board.ticket(key).await?;
        if ticket.is_epic() {
            return Err(BoardError::refused(
                Refusal::NotAnEpic,
                format!("{} is an epic: use Close epic", ticket.key),
            ));
        }
        match ticket.status {
            Status::Merged => {
                return Err(BoardError::refused(
                    Refusal::MoveRefused,
                    format!("{} is merged; merged tickets are locked", ticket.key),
                ));
            }
            Status::ClosedNotMerged => return Ok(ticket),
            _ => {}
        }
        if !confirm && self.agent_runs(&ticket) {
            return Err(BoardError::refused(
                Refusal::NeedsConfirmation,
                format!(
                    "{}'s agent is working. It will be stopped first.",
                    ticket.key
                ),
            ));
        }
        let closed = self.close_one(&ticket, actor).await?;
        // Only a child closed on its own wakes its lead; closing the whole epic doesn't (d0083).
        board
            .leads()
            .child_finished(&closed, crate::epic_lead::wake::Finished::Closed)
            .await;
        Ok(closed)
    }

    /// **Close epic** (EPC-06): the plan's unfinished children are closed and their agents stopped; merged
    /// children stay merged. Without `confirm`, refused when an agent would be stopped.
    pub async fn close_epic(
        &self,
        key: &str,
        confirm: bool,
        actor: &Actor,
    ) -> Result<ClosePlan, BoardError> {
        let board = &self.inner.board;
        let plan = board.epic_close_plan(key).await?;
        if !confirm && plan.close.iter().any(|c| c.agent_running) {
            return Err(BoardError::refused(
                Refusal::NeedsConfirmation,
                "Closing the epic stops the agents of its unfinished tickets.",
            ));
        }
        for child in &plan.close {
            let child = board.ticket(&child.key.to_string()).await?;
            self.close_one(&child, actor).await?;
        }
        let epic = board.ticket(key).await?;
        if epic.status != Status::ClosedNotMerged {
            let mut tx = board.store().begin_write().await?;
            close_bookkeeping(&mut tx, &epic.id, epic.status, actor).await?;
            tx.commit().await?;
        }
        tracing::info!(epic = %epic.key, closed = plan.close.len(), "epic closed");
        Ok(plan)
    }

    fn agent_runs(&self, ticket: &Ticket) -> bool {
        self.inner
            .sessions
            .running_of(&ticket.id, SessionKind::Agent)
            .is_some()
    }

    async fn close_one(&self, ticket: &Ticket, actor: &Actor) -> Result<Ticket, BoardError> {
        let board = &self.inner.board;
        let mut tx = board.store().begin_write().await?;
        close_bookkeeping(&mut tx, &ticket.id, ticket.status, actor).await?;
        sqlx::query(
            "UPDATE tickets SET parked = FALSE, sent_back_at = NULL, resume_skill = NULL,
                                signal_text = NULL, signal_since = NULL, wakeup_at = NULL
             WHERE id = ?",
        )
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::info!(ticket = %ticket.key, from = %ticket.status, "closed");
        let closed = board.ticket_by_id(&ticket.id).await?;
        // Branch kept; worktree removed unless dirty; agent stopped; ports freed (the Done cleanup).
        if let Err(e) = self.finish(&closed).await {
            tracing::warn!(ticket = %closed.key, error = %e, "cleanup after close failed");
        }
        if let Some(epic) = &closed.epic {
            board.reevaluate_epic(&epic.to_string(), actor).await?;
        }
        board.ticket_by_id(&ticket.id).await
    }
}
