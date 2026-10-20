//! The bookkeeping of a ticket returning from a human check to the nearest earlier Agent stage (REV-04, GIT-03,
//! FLW-14; d0034, d0038, d0108 rule 6): on the default flow, Ready for Review to Dev.
//!
//! **Fail** (Send back on the default flow) bundles the round's pending comments and the optional note: the comments get `sent_at`, a
//! `reviews` row records the action, the note and the round, `sent_back_round` becomes that round and the card
//! shows "Sent back · N comments · round K" until the ticket next reaches Ready for Review. The ticket goes to
//! Dev: Agent Working when its live session gets the skill at once, otherwise Queued with the skill to run once
//! the agent is live again (`resume_skill`). Typing and resuming are the caller's job
//! ([`crate::review::Review`]).

use serde_json::json;

use super::stage_runs;
use super::tickets::{EventKind, Ticket, record_event};
use super::{Board, BoardError};
use crate::domain::{Actor, Status};
use crate::flow::engine::{Move, RunResult};
use crate::store::tickets::set_state_since;
use crate::store::{Store, new_id, now_ms};

/// What a Send back sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SentRound {
    pub comments: u32,
    pub round: u32,
}

/// What `tickets.stage_id` holds for a ticket queued to come back to the Agent stage `agent_stage` (Resume, a Send
/// back with the limit full): that stage, which it waits before, or `None` (Ready for Dev) when it is the flow's
/// first Agent stage, so the default flow queues as today. The start continues its conversation there (STG-07,
/// STG-09).
pub(crate) fn waits_before(flow: &crate::flow::Flow, agent_stage: &str) -> Option<String> {
    let first = flow.first_agent_stage().map(|s| s.id.as_str());
    (Some(agent_stage) != first).then(|| agent_stage.to_owned())
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
        let stage = self.back_stage(ticket, to).await?;
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
        // A human check's Fail (STG-06).
        stage_runs::end_open(&mut tx, &ticket.id, RunResult::Failed, Some(note)).await?;
        return_to_dev(&mut tx, ticket, to, stage.as_deref(), resume_skill, actor).await?;
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

    /// Where a Send back takes the ticket: the nearest earlier Agent stage, as the flow engine says (a human
    /// check's Fail, STG-06). Queued (`to`, the limit is full), it waits before that stage instead
    /// ([`waits_before`]): Ready for Dev before the flow's first.
    async fn back_stage(&self, ticket: &Ticket, to: Status) -> Result<Option<String>, BoardError> {
        let flow = self.effective_flow(ticket.key.project.as_str()).await?;
        let stage = match self
            .moved(ticket, Status::WaitingForYou, Move::HumanFail)
            .await
        {
            Ok(moved) => moved.place.stage().map(str::to_owned),
            Err(_) => flow.first_agent_stage().map(|s| s.id.clone()),
        };
        Ok(match to.is_queued() {
            true => stage.and_then(|id| waits_before(&flow, &id)),
            false => stage,
        })
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

/// Ready for Review → Dev: Agent Working in `stage` (the live session got the skill) or Queued (`queued_at` now)
/// with the skill still to run, waiting before `stage` (`None`: Ready for Dev, as today). The next submit validates again from round 1 (d0063,
/// VAL-10).
pub(crate) async fn return_to_dev(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ticket: &Ticket,
    to: Status,
    stage: Option<&str>,
    resume_skill: Option<&str>,
    actor: &Actor,
) -> Result<(), BoardError> {
    let now = now_ms();
    sqlx::query(concat!(
        "UPDATE tickets SET stage_id = ?, state = ?, resume_skill = ?,
                            queued_at = CASE WHEN ? THEN ? ELSE queued_at END,
                            last_signal_at = CASE WHEN ? THEN ? ELSE last_signal_at END,
                            signal_text = NULL, signal_since = NULL, wakeup_at = NULL,
                            review_entry = NULL,
                            updated_by = ?, updated_at = ?, ",
        set_state_since!(),
        " WHERE id = ?"
    ))
    .bind(stage)
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
    // A human Fail: every agent check's counter restarts at 0 (FLW-13).
    super::validation::reset_check_counters(tx, &ticket.id).await?;
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
