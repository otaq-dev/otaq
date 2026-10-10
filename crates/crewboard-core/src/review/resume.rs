//! Resume, Resume all, Retry and Start from scratch (REC-01, REC-03; d0018, d0045): the user's clicks that bring
//! an Interrupted or Resume failed ticket's agent back. **Nothing here runs on its own.**
//!
//! - **Resume** continues the stored conversation, `claude --resume <id>`, in a new session in the same worktree.
//!   It goes through the start sequence like a parked ticket (worktree kept, no base sync or setup); with the
//!   agent limit full, the ticket waits in Queued for a slot like a Send back.
//! - **Resume all** resumes exactly the Interrupted Dev tickets ([`crate::recovery::resume_all_eligible`]).
//! - When `claude --resume` can't find the conversation the ticket becomes **Resume failed**
//!   ([`crate::recovery::Recovery`]); **Retry** runs the same resume again, **Start from scratch** (after a
//!   confirmation) starts a new conversation in the same worktree. Branch, commits and files are kept; every
//!   session and its Claude session ID stay in History.

use serde_json::json;

use super::Review;
use crate::board::tickets::{EventKind, Ticket, record_event};
use crate::board::{BoardError, Refusal};
use crate::domain::{Actor, Status, TicketKey};
use crate::recovery::{RETURN_RESUME, RETURN_SCRATCH, resume_all_eligible};
use crate::session::SessionKind;
use crate::start::StartOutcome;
use crate::store::now_ms;
use crate::store::tickets::set_state_since;

/// "Start SHOP-25 from scratch? …": what Start from scratch keeps and what it doesn't.
pub fn start_from_scratch_message(key: &str) -> String {
    format!(
        "Start {key} from scratch? A new Claude session starts in the same worktree: the branch, commits and \
         files are kept, but the old conversation isn't continued."
    )
}

impl Review {
    /// **Resume** an Interrupted ticket.
    pub async fn resume(&self, key: &str, actor: &Actor) -> Result<Ticket, BoardError> {
        let ticket = self.inner.board.ticket(key).await?;
        if ticket.status != Status::Interrupted {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!(
                    "{} is {}; only Interrupted tickets are resumed",
                    ticket.key,
                    ticket.status.label()
                ),
            ));
        }
        self.bring_back(&ticket, RETURN_RESUME, EventKind::Resumed, json!({}), actor)
            .await
    }

    /// **Retry** after Resume failed: the same resume again.
    pub async fn retry_resume(&self, key: &str, actor: &Actor) -> Result<Ticket, BoardError> {
        let ticket = self.resume_failed(key).await?;
        self.bring_back(
            &ticket,
            RETURN_RESUME,
            EventKind::Resumed,
            json!({ "retry": true }),
            actor,
        )
        .await
    }

    /// **Start from scratch** after Resume failed: `confirm` is the user's yes to
    /// [`start_from_scratch_message`].
    pub async fn start_from_scratch(
        &self,
        key: &str,
        confirm: bool,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let ticket = self.resume_failed(key).await?;
        if !confirm {
            return Err(BoardError::refused(
                Refusal::NeedsConfirmation,
                start_from_scratch_message(&ticket.key.to_string()),
            ));
        }
        let old: Option<String> = sqlx::query_scalar(
            "SELECT claude_session_id FROM sessions
             WHERE ticket_id = ? AND kind = 'agent' AND claude_session_id IS NOT NULL
             ORDER BY started_at DESC, id DESC LIMIT 1",
        )
        .bind(&ticket.id)
        .fetch_optional(self.inner.board.store().pool())
        .await?;
        self.bring_back(
            &ticket,
            RETURN_SCRATCH,
            EventKind::StartedFromScratch,
            json!({ "old_claude_session_id": old }),
            actor,
        )
        .await
    }

    /// **Resume all**: every Interrupted ticket, of one project or of all (the inbox is global). Returns the
    /// tickets resumed, in key order.
    pub async fn resume_all(
        &self,
        project: Option<&str>,
        actor: &Actor,
    ) -> Result<Vec<TicketKey>, BoardError> {
        let board = &self.inner.board;
        let projects = match project {
            Some(key) => vec![board.project(key).await?.key],
            None => board.projects().await?.into_iter().map(|p| p.key).collect(),
        };
        let mut resumed = Vec::new();
        for project in projects {
            let tickets = board.tickets(&project).await?;
            for key in resume_all_eligible(&tickets) {
                self.resume(&key.to_string(), actor).await?;
                resumed.push(key);
            }
        }
        resumed.sort();
        Ok(resumed)
    }

    async fn resume_failed(&self, key: &str) -> Result<Ticket, BoardError> {
        let ticket = self.inner.board.ticket(key).await?;
        if ticket.status != Status::ResumeFailed {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!(
                    "{} is {}; Retry and Start from scratch follow a failed resume",
                    ticket.key,
                    ticket.status.label()
                ),
            ));
        }
        Ok(ticket)
    }

    /// Back to Queued with how to continue (`mode`), then the start sequence right away, or a wait for a slot
    /// while the agent limit is full.
    async fn bring_back(
        &self,
        ticket: &Ticket,
        mode: &str,
        kind: EventKind,
        mut payload: serde_json::Value,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let inner = &self.inner;
        let board = &inner.board;
        if inner
            .sessions
            .running_of(&ticket.id, SessionKind::Agent)
            .is_some()
        {
            return Err(BoardError::refused(
                Refusal::AlreadyRunning,
                format!("{} already has a running agent session", ticket.key),
            ));
        }
        let full = self.limit_full().await?;
        let now = now_ms();
        let mut tx = board.store().pool().begin().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, return_mode = ?, queued_at = ?,
                                start_problem_step = NULL, start_problem_message = NULL, signal_text = NULL,
                                signal_since = NULL, wakeup_at = NULL, updated_by = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(Status::Queued.column().as_str())
        .bind(Status::Queued.as_str())
        .bind(mode)
        .bind(now)
        .bind(actor.to_string())
        .bind(now)
        .bind(Status::Queued.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        payload["from"] = json!(ticket.status);
        record_event(&mut tx, &ticket.id, kind, actor, payload).await?;
        tx.commit().await?;
        tracing::info!(ticket = %ticket.key, mode, full, "resuming");
        if full {
            inner.scheduler.wake();
        } else {
            match inner.starter.start(&ticket.key.to_string(), actor).await? {
                StartOutcome::Started(_) | StartOutcome::Skipped => {}
                StartOutcome::Held { message } | StartOutcome::Interrupted { message, .. } => {
                    tracing::warn!(ticket = %ticket.key, %message, "resuming the agent stopped");
                }
            }
        }
        board.ticket_by_id(&ticket.id).await
    }
}
