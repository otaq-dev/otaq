//! The bookkeeping of a ticket returning from Ready for Review to Dev (REV-04, GIT-03; d0034, d0038).
//!
//! **Send back** bundles the round's pending comments and the optional note: the comments get `sent_at`, a
//! `reviews` row records the action, the note and the round, `sent_back_round` becomes that round and the card
//! shows "Sent back · N comments · round K" until the ticket next reaches Ready for Review. The ticket goes to
//! Dev: Agent Working when its live session gets the skill at once, otherwise Queued with the skill to run once
//! the agent is live again (`resume_skill`). Typing and resuming are the caller's job
//! ([`crate::review::Review`]).

use serde_json::json;

use super::tickets::{EventKind, Ticket, record_event};
use super::{Board, BoardError};
use crate::domain::{Actor, Status};
use crate::store::tickets::set_state_since;
use crate::store::{Store, new_id, now_ms};

/// What a Send back sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SentRound {
    pub comments: u32,
    pub round: u32,
}

impl Board {
    /// Marks the pending comments sent, records the review, and moves the ticket to `to` (Working or Queued)
    /// with `resume_skill` (the skill still to run, when it couldn't be typed now).
    pub async fn record_send_back(
        &self,
        ticket: &Ticket,
        note: &str,
        to: Status,
        resume_skill: Option<&str>,
        actor: &Actor,
    ) -> Result<SentRound, BoardError> {
        let now = now_ms();
        let round = self.current_round(&ticket.id).await?;
        let mut tx = self.store.begin_write().await?;
        let sent = sqlx::query(
            "UPDATE review_comments SET sent_at = ?, round = ? WHERE ticket_id = ? AND sent_at IS NULL",
        )
        .bind(now)
        .bind(i64::from(round))
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let note = note.trim();
        sqlx::query(
            "INSERT INTO reviews (id, ticket_id, round, action, note, at) VALUES (?, ?, ?, 'send_back', ?, ?)",
        )
        .bind(new_id())
        .bind(&ticket.id)
        .bind(i64::from(round))
        .bind((!note.is_empty()).then_some(note))
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE tickets SET sent_back_round = ?, sent_back_at = ? WHERE id = ?")
            .bind(i64::from(round))
            .bind(now)
            .bind(&ticket.id)
            .execute(&mut *tx)
            .await?;
        return_to_dev(&mut tx, ticket, to, resume_skill, actor).await?;
        let comments = u32::try_from(sent).unwrap_or(u32::MAX);
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::SentBack,
            actor,
            json!({ "round": round, "comments": comments, "note": note, "to": to }),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(key = %ticket.key, round, comments, %to, "sent back");
        Ok(SentRound { comments, round })
    }

    /// **Send to agent to fix** (GIT-03): back to Dev to resolve the conflict with `target`; the conflict stays
    /// on the ticket until the agent submits again.
    pub async fn record_send_to_fix(
        &self,
        ticket: &Ticket,
        target: &str,
        to: Status,
        resume_skill: Option<&str>,
        actor: &Actor,
    ) -> Result<(), BoardError> {
        let mut tx = self.store.begin_write().await?;
        return_to_dev(&mut tx, ticket, to, resume_skill, actor).await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::SentToFix,
            actor,
            json!({ "target": target, "to": to }),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(key = %ticket.key, target, %to, "sent to the agent to fix");
        Ok(())
    }

    /// The note of the latest Send back, if it had one.
    pub async fn latest_send_back_note(
        &self,
        ticket_id: &str,
    ) -> Result<Option<(u32, Option<String>)>, BoardError> {
        latest_review(&self.store, ticket_id).await
    }
}

async fn latest_review(
    store: &Store,
    ticket_id: &str,
) -> Result<Option<(u32, Option<String>)>, BoardError> {
    let row: Option<(i64, Option<String>)> = sqlx::query_as(
        "SELECT round, note FROM reviews WHERE ticket_id = ? AND action = 'send_back'
         ORDER BY at DESC, round DESC LIMIT 1",
    )
    .bind(ticket_id)
    .fetch_optional(store.pool())
    .await?;
    Ok(row.map(|(round, note)| (u32::try_from(round).unwrap_or(0), note)))
}

/// Ready for Review → Dev: Agent Working (the live session got the skill) or Queued (`queued_at` now) with the
/// skill still to run. The next submit validates again from round 1 (d0063, VAL-10).
pub(crate) async fn return_to_dev(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ticket: &Ticket,
    to: Status,
    resume_skill: Option<&str>,
    actor: &Actor,
) -> Result<(), BoardError> {
    let now = now_ms();
    sqlx::query(concat!(
        "UPDATE tickets SET board_column = ?, state = ?, resume_skill = ?,
                            queued_at = CASE WHEN ? THEN ? ELSE queued_at END,
                            last_signal_at = CASE WHEN ? THEN ? ELSE last_signal_at END,
                            signal_text = NULL, signal_since = NULL, wakeup_at = NULL,
                            validation_round = 0, validation_rounds_extra = 0, review_entry = NULL,
                            updated_by = ?, updated_at = ?, ",
        set_state_since!(),
        " WHERE id = ?"
    ))
    .bind(to.column().as_str())
    .bind(to.as_str())
    .bind(resume_skill)
    .bind(to.is_queued())
    .bind(now)
    .bind(to == Status::Working)
    .bind(now)
    .bind(actor.to_string())
    .bind(now)
    .bind(to.as_str())
    .bind(now)
    .bind(&ticket.id)
    .execute(&mut **tx)
    .await?;
    record_event(
        tx,
        &ticket.id,
        EventKind::Moved,
        actor,
        json!({ "from": ticket.status, "to": to }),
    )
    .await
}

impl Board {
    /// The agent of `ticket_id` is alive in its conversation (a hook or a `otaq` command came from it): the
    /// first prompt it was started or resumed with (a Send back's skill, an inbox answer) has reached it, so it is
    /// not sent again by a later Retry or Resume (T-0086, T-0112).
    pub async fn first_prompt_delivered(&self, ticket_id: &str) -> Result<(), BoardError> {
        sqlx::query(
            "UPDATE tickets SET resume_skill = NULL, resume_prompt = NULL
             WHERE id = ? AND (resume_skill IS NOT NULL OR resume_prompt IS NOT NULL)",
        )
        .bind(ticket_id)
        .execute(self.store().pool())
        .await?;
        Ok(())
    }
}
