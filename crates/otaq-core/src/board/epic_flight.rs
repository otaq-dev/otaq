//! Epics in flight (EPC-11, EPC-13; d0080): the epic's queue mode, its place on the board, and the child groups
//! the queue and take-back rules read.
//!
//! The epic moves as one thing: a **card** in Backlog before it starts (its children are rows inside it), a
//! **track** in the "Epics in flight" strip once a child is queued, and a **delivery card** in Ready for Review,
//! Approved or Done when every child is done (EPC-04). [`epic_place`] decides which, from the epic and its children
//! only, so the view models can call it without the database. The queue mode is stored on the epic: `Whole` lets
//! new children join the queue, `Single` keeps them "not queued" (EPC-13).

use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Sqlite, Transaction};

use super::tickets::{EventKind, Ticket, record_event};
use super::{Board, BoardError, Refusal};
use crate::domain::{Actor, Column, Status, stored_enum};
use crate::store::now_ms;

stored_enum! {
    /// How the user queued an epic (`tickets.epic_queue_mode`, migration 0030).
    #[derive(Default)]
    pub enum EpicQueueMode ("epic queue mode") {
        /// Never queued, or taken back.
        #[default]
        None = "none",
        /// The whole epic was queued (its card or track dropped on Ready for Dev / Dev, ⇧⌘D, the menu): new
        /// children join the queue on their own.
        Whole = "whole",
        /// One child row was queued: new children wait as "not queued".
        Single = "single",
    }
}

impl EpicQueueMode {
    pub fn is_none(&self) -> bool {
        *self == Self::None
    }
}

/// Where an epic shows on the board (EPC-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpicPlace {
    /// A card in Backlog with its children as rows: nothing is active, and either nothing ever started or a child
    /// is Parked (after Park all, or a single Park).
    Card,
    /// A track in the "Epics in flight" strip, until the epic reaches Ready for Review.
    Track,
    /// The epic itself is in Ready for Review, Approved or Done (EPC-04): its delivery card.
    Delivery,
}

/// Has this ticket been to Dev? Pure: from its column, its status and its work (branch or worktree). A queued
/// ticket that never reached Dev has **not** (unlike [`Ticket::has_started`], which a queue sets): it is still
/// "never started", a row or a "not queued" chip that queueing or taking back moves freely (d0080). A Closed
/// ticket counts if it was ever queued and kept that mark, or has work.
pub fn been_to_dev(ticket: &Ticket) -> bool {
    let worked = ticket
        .work
        .as_ref()
        .is_some_and(|w| w.branch.is_some() || w.worktree.is_some());
    match ticket.column {
        Column::Dev | Column::ReadyForReview | Column::Approved => true,
        Column::Backlog => ticket.status == Status::Parked || worked,
        Column::ReadyForDev => worked,
        Column::Done => ticket.status == Status::Merged || worked || ticket.has_started(),
    }
}

/// A never-started child: it has never been to Dev and is a row of the epic card, a card in Ready for Dev, or a
/// "not queued" chip on its track.
pub fn is_never_started(ticket: &Ticket) -> bool {
    matches!(ticket.column, Column::Backlog | Column::ReadyForDev) && !been_to_dev(ticket)
}

/// An active child: in Ready for Dev, Dev or Ready for Review.
pub fn is_active(ticket: &Ticket) -> bool {
    matches!(
        ticket.column,
        Column::ReadyForDev | Column::Dev | Column::ReadyForReview
    )
}

/// A running child: in Dev, where its agent session (or validator) works or waits for the user.
pub fn is_running(ticket: &Ticket) -> bool {
    ticket.column == Column::Dev
}

/// A parked child: "Parked · has work" in Backlog (d0052).
pub fn is_parked(ticket: &Ticket) -> bool {
    ticket.status == Status::Parked
}

/// The never-started children, in the given order.
pub fn never_started(children: &[Ticket]) -> Vec<&Ticket> {
    children.iter().filter(|c| is_never_started(c)).collect()
}

/// The active children (Ready for Dev, Dev, Ready for Review).
pub fn active(children: &[Ticket]) -> Vec<&Ticket> {
    children.iter().filter(|c| is_active(c)).collect()
}

/// The running children (Dev).
pub fn running(children: &[Ticket]) -> Vec<&Ticket> {
    children.iter().filter(|c| is_running(c)).collect()
}

/// The parked children.
pub fn parked(children: &[Ticket]) -> Vec<&Ticket> {
    children.iter().filter(|c| is_parked(c)).collect()
}

/// The epic's children among a project's tickets, in the given order.
pub fn children_of<'a>(epic: &Ticket, tickets: &'a [Ticket]) -> Vec<&'a Ticket> {
    tickets
        .iter()
        .filter(|t| t.epic.as_ref() == Some(&epic.key))
        .collect()
}

/// Where the epic shows (EPC-11). `tickets` may be just its children or the whole project: only the epic's own
/// children count.
///
/// SHOP-45 with nothing started is a card. SHOP-45.1 merged and SHOP-45.2 taken back is a track ("SHOP-45.1 has
/// started, so the track stays"). After Park all it is a card again, its rows "Parked · has work". A child reopened
/// after Review makes it a track again (the epic is back in Backlog, EPC-05).
pub fn epic_place(epic: &Ticket, tickets: &[Ticket]) -> EpicPlace {
    if matches!(
        epic.column,
        Column::ReadyForReview | Column::Approved | Column::Done
    ) {
        return EpicPlace::Delivery;
    }
    let children = children_of(epic, tickets);
    let any_active = children.iter().any(|c| is_active(c));
    let any_started = children.iter().any(|c| been_to_dev(c));
    let any_parked = children.iter().any(|c| is_parked(c));
    if !any_active && (!any_started || any_parked) {
        EpicPlace::Card
    } else {
        EpicPlace::Track
    }
}

impl Board {
    /// Sets an epic's queue mode and logs it in its History ("Queued the whole epic", "Taken back"). Setting the
    /// mode it already has changes nothing. Queueing and taking back (T-0263, T-0264) call it, or
    /// [`set_queue_mode_in`] inside their own transaction.
    pub async fn set_queue_mode(
        &self,
        epic: &str,
        mode: EpicQueueMode,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let epic = self.ticket(epic).await?;
        if !epic.is_epic() {
            return Err(BoardError::refused(
                Refusal::NotAnEpic,
                format!("{} isn't an epic", epic.key),
            ));
        }
        if epic.epic_queue_mode == mode {
            return Ok(epic);
        }
        let mut tx = self.store.pool().begin().await?;
        set_queue_mode_in(&mut tx, &epic.id, epic.epic_queue_mode, mode, actor).await?;
        tx.commit().await?;
        tracing::info!(epic = %epic.key, from = %epic.epic_queue_mode, to = %mode, "epic queue mode");
        self.ticket_by_id(&epic.id).await
    }
}

/// Writes an epic's queue mode and its History line inside a transaction. `from` is the mode it had (for the log).
pub(crate) async fn set_queue_mode_in(
    tx: &mut Transaction<'_, Sqlite>,
    epic_id: &str,
    from: EpicQueueMode,
    to: EpicQueueMode,
    actor: &Actor,
) -> Result<(), BoardError> {
    let now = now_ms();
    sqlx::query(
        "UPDATE tickets SET epic_queue_mode = ?, epic_queue_mode_at = ?, updated_by = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(to.as_str())
    .bind(now)
    .bind(actor.to_string())
    .bind(now)
    .bind(epic_id)
    .execute(&mut **tx)
    .await?;
    record_event(
        tx,
        epic_id,
        EventKind::EpicQueueModeSet,
        actor,
        json!({"from": from, "to": to}),
    )
    .await
}
