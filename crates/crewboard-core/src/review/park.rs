//! Park (CLS-06; d0052, d0026): a Dev ticket dragged back to Backlog after a confirmation ("SHOP-25's agent will be
//! stopped. Its branch, files and conversation are kept."). The agent stops; the branch, the worktree (with
//! uncommitted files) and the Claude session ID stay; both ports return to the pool; the card shows "Parked · has
//! work" and holds no agent slot. Dragging it back to Dev resumes the same conversation (`claude --resume`) with
//! new ports: the start sequence sees the `parked` mark ([`crate::start::Starter`]).

use serde_json::json;

use super::Review;
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
        let mut tx = board.store().pool().begin().await?;
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
}
