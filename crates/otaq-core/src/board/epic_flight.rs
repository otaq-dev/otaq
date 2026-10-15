//! Epics in flight (EPC-11, EPC-13; d0080): the epic's queue mode, its place on the board, and the child groups
//! the queue and take-back rules read.
//!
//! The epic moves as one thing: a **card** in Backlog before it starts (its children are rows inside it), a
//! **track** in the "Epics in flight" strip once a child is queued, and a **delivery card** in Ready for Review,
//! Approved or Done when every child is done (EPC-04). [`epic_place`] decides which, from the epic and its children
//! only, so the view models can call it without the database. The queue mode is stored on the epic: `Whole` lets
//! new children join the queue, `Single` keeps them "not queued" (EPC-13).
//!
//! **Queueing** (EPC-12, EPC-14; d0080): [`Board::queue_epic`] (the epic card or track dropped on Ready for Dev or
//! Dev, ⇧⌘D) queues every child still in Backlog in one transaction; a parked one resumes its conversation through
//! the same Parked → Dev path as a dragged ticket (d0052). [`Board::queue_epic_child`] (one row of the card, or a
//! "not queued" chip) queues only that child. Nothing starts here: the scheduler then applies the dependencies
//! (d0049), the optional limit (d0016) and the start order (d0065), and the user's drag is the go-ahead (d0066).

use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Sqlite, Transaction};

use super::moves::{DropTarget, MoveAction, MoveCheck, check_move, queue_in};
use super::settings::AutoMerge;
use super::tickets::{EventKind, Ticket, record_event};
use super::{Board, BoardError, Refusal};
use crate::domain::{Actor, Column, Status, TicketKey, stored_enum};
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

/// What a queue did, for the queue toast (EPC-15): "Queued 2 children of SHOP-45 · Undo", and with Auto-merge Off
/// "Queued 3 tickets. Auto-merge is off for shop-web, so each one waits for your review." Undo takes back
/// `queued` and `resumed` (T-0264).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpicQueueOutcome {
    /// The epic, e.g. SHOP-45.
    pub epic: TicketKey,
    /// Never-started (or reopened) children that went to Ready for Dev, in key order.
    pub queued: Vec<TicketKey>,
    /// Parked children that go back to Dev with their conversations (d0052), in key order.
    pub resumed: Vec<TicketKey>,
    /// The project's effective Auto-merge is Off (d0064): each child will wait for the user's review.
    pub auto_merge_off: bool,
    /// The project's name, e.g. shop-web.
    pub project: String,
}

impl EpicQueueOutcome {
    /// Every child the queue moved: queued, then resumed.
    pub fn keys(&self) -> impl Iterator<Item = &TicketKey> {
        self.queued.iter().chain(&self.resumed)
    }
}

/// The refusal when an epic has nothing left to queue or resume.
pub fn nothing_to_queue(epic: &TicketKey) -> String {
    format!("Every child of {epic} is already queued or started")
}

/// The children a whole-epic queue moves: every child still in Backlog (never started, parked, or reopened). Active
/// and done children stay where they are.
pub fn to_queue<'a>(epic: &Ticket, tickets: &'a [Ticket]) -> Vec<&'a Ticket> {
    children_of(epic, tickets)
        .into_iter()
        .filter(|c| c.column == Column::Backlog)
        .collect()
}

impl Board {
    /// **Queue an epic** (EPC-12): the epic card or track dropped on Ready for Dev or Dev, or ⇧⌘D on it. Every
    /// child in Backlog goes to Ready for Dev in one transaction, as the user's drag ("Queued with the epic"); a
    /// parked one resumes (d0052). The queue mode becomes Whole. Refused for an epic in Ready for Review, Approved
    /// or Done (its delivery card), and when nothing is left to queue.
    pub async fn queue_epic(
        &self,
        epic: &str,
        actor: &Actor,
    ) -> Result<EpicQueueOutcome, BoardError> {
        let epic = self.ticket(epic).await?;
        if !epic.is_epic() {
            return Err(BoardError::refused(
                Refusal::NotAnEpic,
                format!("{} isn't an epic", epic.key),
            ));
        }
        match check_move(&epic, DropTarget::ReadyForDev) {
            MoveCheck::Allowed {
                action: MoveAction::QueueEpic,
            } => {}
            MoveCheck::Refused { reason } => {
                return Err(BoardError::refused(Refusal::MoveRefused, reason));
            }
            _ => {
                return Err(BoardError::refused(
                    Refusal::MoveRefused,
                    nothing_to_queue(&epic.key),
                ));
            }
        }
        let tickets = self.tickets(&epic.key.project.to_string()).await?;
        let children = to_queue(&epic, &tickets);
        self.queue_children(&epic, &children, EpicQueueMode::Whole, actor)
            .await
    }

    /// **Queue one child** (EPC-12): a row of the epic card, or a "not queued" chip on its track, dropped on Ready
    /// for Dev or Dev. Only that child moves (a parked one resumes). The queue mode None becomes Single; Whole and
    /// Single stay. The outcome has one key.
    pub async fn queue_epic_child(
        &self,
        child: &str,
        actor: &Actor,
    ) -> Result<EpicQueueOutcome, BoardError> {
        let child = self.ticket(child).await?;
        let Some(epic_key) = &child.epic else {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!("{} isn't an epic child", child.key),
            ));
        };
        match check_move(&child, DropTarget::ReadyForDev) {
            MoveCheck::Allowed {
                action: MoveAction::Start,
            } => {}
            MoveCheck::Refused { reason } => {
                return Err(BoardError::refused(Refusal::MoveRefused, reason));
            }
            _ => {
                return Err(BoardError::refused(
                    Refusal::MoveRefused,
                    format!("{} is already queued or started", child.key),
                ));
            }
        }
        let epic = self.ticket(&epic_key.to_string()).await?;
        let mode = match epic.epic_queue_mode {
            EpicQueueMode::None => EpicQueueMode::Single,
            kept => kept,
        };
        self.queue_children(&epic, &[&child], mode, actor).await
    }

    /// Moves `children` to Ready for Dev and sets the epic's queue mode, in one transaction.
    async fn queue_children(
        &self,
        epic: &Ticket,
        children: &[&Ticket],
        mode: EpicQueueMode,
        actor: &Actor,
    ) -> Result<EpicQueueOutcome, BoardError> {
        if children.is_empty() {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                nothing_to_queue(&epic.key),
            ));
        }
        let project_key = epic.key.project.to_string();
        let project = self.project(&project_key).await?;
        let auto_merge_off =
            self.effective_settings(&project_key).await?.auto_merge == AutoMerge::Off;
        let with_epic = json!({"with_epic": epic.key});
        let mut tx = self.store.pool().begin().await?;
        for child in children {
            queue_in(&mut tx, child, Some(&epic.id), actor, with_epic.clone()).await?;
        }
        if epic.epic_queue_mode != mode {
            set_queue_mode_in(&mut tx, &epic.id, epic.epic_queue_mode, mode, actor).await?;
        }
        tx.commit().await?;
        let keys = |parked: bool| {
            children
                .iter()
                .filter(|c| is_parked(c) == parked)
                .map(|c| c.key.clone())
                .collect()
        };
        let outcome = EpicQueueOutcome {
            epic: epic.key.clone(),
            queued: keys(false),
            resumed: keys(true),
            auto_merge_off,
            project: project.name,
        };
        tracing::info!(
            epic = %epic.key, mode = %mode, queued = outcome.queued.len(), resumed = outcome.resumed.len(),
            "epic queued"
        );
        Ok(outcome)
    }

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
