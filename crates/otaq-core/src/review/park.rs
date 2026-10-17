//! Park (CLS-06; d0052, d0026): a Dev ticket dragged back to Backlog after a confirmation ("SHOP-25's agent will be
//! stopped. Its branch, files and conversation are kept."). The agent stops; the branch, the worktree (with
//! uncommitted files) and the session's tool id stay; both ports return to the pool; the card shows "Parked · has
//! work" and holds no agent slot. Dragging it back to Dev resumes the same conversation on its tool with
//! new ports: the start sequence sees the `parked` mark ([`crate::start::Starter`]).
//!
//! The ticket's Run stops too, before the ports are freed, so no program keeps the freed Run port (Q30); the
//! [`on_ticket_parked`](Review::on_ticket_parked) hooks do it. The Shell holds no port and stays open.
//!
//! **Park all** (EPC-14; d0080): an epic's track dropped on Backlog when nothing of it waits parks every running
//! child, one after the other with this same Park, after one confirmation ([`Review::take_back_epic`]).

use serde_json::json;

use super::Review;
use super::finish::{FinishedHook, FinishedTicket};
use crate::board::epic_flight::{EpicQueueMode, TakeBackPlan, take_back_plan};
use crate::board::tickets::{EventKind, Ticket, record_event};
use crate::board::{BoardError, Refusal};
use crate::domain::{Actor, Column, Status};
use crate::session::SessionKind;
use crate::store::now_ms;
use crate::store::tickets::set_state_since;

/// "SHOP-25's agent will be stopped. Its branch, files and conversation are kept."
pub fn park_message(key: &str) -> String {
    format!("{key}'s agent will be stopped. Its branch, files and conversation are kept.")
}

impl Review {
    /// Registers a hook run for every parked ticket, before its ports return to the pool (Run stops, Q30).
    pub fn on_ticket_parked(&self, hook: FinishedHook) {
        self.inner
            .parked_hooks
            .lock()
            .expect("parked hooks")
            .push(hook);
    }

    /// **Park**: `confirm` is the user's yes to the confirmation.
    pub async fn park(
        &self,
        key: &str,
        confirm: bool,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let inner = &self.inner;
        let board = &inner.board;
        let ticket = board.ticket(key).await?;
        if ticket.column != Column::Dev || ticket.is_epic() {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!("{} isn't in Dev; only Dev tickets are parked", ticket.key),
            ));
        }
        if !confirm {
            return Err(BoardError::refused(
                Refusal::NeedsConfirmation,
                park_message(&ticket.key.to_string()),
            ));
        }
        if let Some(session) = inner.sessions.running_of(&ticket.id, SessionKind::Agent) {
            inner
                .sessions
                .stop(&session.id)
                .await
                .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        }
        // Nothing may keep using the ports once they are freed (d0052, d0026): the Run stops first.
        let parked = FinishedTicket {
            id: ticket.id.clone(),
            key: ticket.key.clone(),
            status: Status::Parked,
        };
        let hooks = inner.parked_hooks.lock().expect("parked hooks").clone();
        for hook in hooks {
            hook(&parked).await;
        }
        inner
            .ports
            .free(&ticket.id)
            .await
            .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        // A ticket that never got to work (no branch yet) simply goes back to Backlog.
        let has_work = ticket.work.as_ref().is_some_and(|w| w.branch.is_some());
        let to = if has_work {
            Status::Parked
        } else {
            Status::Backlog
        };
        let now = now_ms();
        let mut tx = board.store().begin_write().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, parked = ?, start_problem_step = NULL,
                                start_problem_message = NULL, signal_text = NULL, signal_since = NULL,
                                wakeup_at = NULL, updated_by = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(to.column().as_str())
        .bind(to.as_str())
        .bind(has_work)
        .bind(actor.to_string())
        .bind(now)
        .bind(to.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::Parked,
            actor,
            json!({ "from": ticket.status, "to": to }),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(ticket = %ticket.key, has_work, "parked");
        board.ticket_by_id(&ticket.id).await
    }

    /// **Take back** an epic (EPC-14): its track dropped on Backlog. Carries out [`take_back_plan`]: waiting
    /// children are unqueued and running ones keep going; when nothing waits, every running child is parked
    /// (`confirm` is the user's yes to "Park 2 running tickets?"; without it, refused with
    /// [`Refusal::NeedsConfirmation`] and the confirmation's text). Either way the queue mode becomes None.
    /// Returns the plan it carried out.
    pub async fn take_back_epic(
        &self,
        epic: &str,
        confirm: bool,
        actor: &Actor,
    ) -> Result<TakeBackPlan, BoardError> {
        let board = &self.inner.board;
        let epic = board.ticket(epic).await?;
        if !epic.is_epic() {
            return Err(BoardError::refused(
                Refusal::NotAnEpic,
                format!("{} isn't an epic", epic.key),
            ));
        }
        let tickets = board.tickets(epic.key.project.as_str()).await?;
        let plan = take_back_plan(&epic, &tickets);
        match &plan {
            TakeBackPlan::Refused { reason, .. } => {
                return Err(BoardError::refused(Refusal::MoveRefused, reason.clone()));
            }
            TakeBackPlan::Unqueue { unqueue, .. } => {
                board
                    .unqueue_epic_children(&epic.key.to_string(), unqueue, actor)
                    .await?;
            }
            TakeBackPlan::ParkAll { park, .. } => {
                if !confirm {
                    let confirmation = plan.confirmation().expect("Park all asks first");
                    return Err(BoardError::refused(
                        Refusal::NeedsConfirmation,
                        confirmation.body,
                    ));
                }
                // Taken back first: a child created meanwhile no longer joins the queue (EPC-13).
                board
                    .set_queue_mode(&epic.key.to_string(), EpicQueueMode::None, actor)
                    .await?;
                for child in park {
                    self.park(&child.to_string(), true, actor).await?;
                }
            }
        }
        tracing::info!(epic = %epic.key, hint = %plan.hint(), "epic taken back");
        Ok(plan)
    }
}
