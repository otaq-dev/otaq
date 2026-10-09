//! Pinning and the order of cards in each column (TKT-06, SDD §7.5; d0016, d0037, d0042, d0047).
//!
//! Every column: pinned first, most recently pinned first. Dev has two sections, Agent Working above Queued.
//! A card that needs the user floats to the top of Dev from either section (d0047 changelog), so Dev reads:
//! pinned → orange (Asks you / Needs you / Idle) → Interrupted / Start interrupted / Resume failed / Parent closed →
//! working (incl. in background, waiting for limit reset); then the Queued section with its own pins first.
//! Inside a group, tickets that block more tickets come first (direct and indirect dependents, as in the inbox,
//! d0016); ties go to the oldest ticket (created first, then the lower key), so the order is stable.
//! Pinning changes board order only: never the column, the inbox or the agent (d0047).

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::tickets::{EventKind, Ticket, record_event};
use super::{Board, BoardError};
use crate::domain::{Actor, Column, Status, TicketKey};
use crate::store::now_ms;

/// A column's cards in board order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnOrder {
    pub column: Column,
    /// The column's cards; for Dev, the Agent Working section (with every card that needs the user).
    pub cards: Vec<TicketKey>,
    /// Dev only: the Queued section.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued: Vec<TicketKey>,
}

/// Does this status need the user (orange family or the inbox's second group)? Such cards float to the top
/// of Dev and are never folded.
pub fn needs_user(status: Status) -> bool {
    status.is_orange()
        || matches!(
            status,
            Status::Interrupted
                | Status::StartInterrupted
                | Status::ResumeFailed
                | Status::ParentClosed
        )
}

/// The group a card sorts in, lowest first.
fn group(ticket: &Ticket) -> u8 {
    if ticket.flags.is_pinned() {
        0
    } else if ticket.status.is_orange() {
        1
    } else if needs_user(ticket.status) {
        2
    } else {
        3
    }
}

/// How many tickets of the project wait on each ticket, directly or indirectly.
pub fn blocked_counts(tickets: &[Ticket]) -> HashMap<TicketKey, usize> {
    let mut children: HashMap<&TicketKey, Vec<&TicketKey>> = HashMap::new();
    for t in tickets {
        for parent in &t.depends_on {
            children.entry(parent).or_default().push(&t.key);
        }
    }
    tickets
        .iter()
        .map(|t| {
            let mut seen: HashSet<&TicketKey> = HashSet::new();
            let mut stack = vec![&t.key];
            while let Some(at) = stack.pop() {
                for child in children.get(at).into_iter().flatten() {
                    if seen.insert(child) {
                        stack.push(child);
                    }
                }
            }
            seen.remove(&t.key);
            (t.key.clone(), seen.len())
        })
        .collect()
}

/// Orders one column. `tickets` is the whole project (needed for the "blocks more" counts).
pub fn sort_column(column: Column, tickets: &[Ticket]) -> ColumnOrder {
    let blocks = blocked_counts(tickets);
    let mut in_column: Vec<&Ticket> = tickets.iter().filter(|t| t.column == column).collect();
    in_column.sort_by_key(|t| {
        (
            group(t),
            Reverse(t.flags.pinned_at.unwrap_or(0)),
            Reverse(blocks.get(&t.key).copied().unwrap_or(0)),
            t.created_at,
            t.key.clone(),
        )
    });
    let (cards, queued): (Vec<&Ticket>, Vec<&Ticket>) = if column == Column::Dev {
        in_column
            .into_iter()
            .partition(|t| !t.status.is_queued() || needs_user(t.status))
    } else {
        (in_column, Vec::new())
    };
    let keys = |list: Vec<&Ticket>| list.into_iter().map(|t| t.key.clone()).collect();
    ColumnOrder {
        column,
        cards: keys(cards),
        queued: keys(queued),
    }
}

/// The Dev column with its fold applied: at most `max_cards` cards are shown, but cards that need the user are
/// never folded; the fold covers only working and queued cards ("4 more · 2 working · 2 queued").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevFold {
    pub shown_cards: Vec<TicketKey>,
    pub shown_queued: Vec<TicketKey>,
    pub hidden_working: usize,
    pub hidden_queued: usize,
}

impl DevFold {
    pub fn hidden(&self) -> usize {
        self.hidden_working + self.hidden_queued
    }

    /// The fold row's text, e.g. "4 more · 2 working · 2 queued"; `None` when nothing is folded.
    pub fn label(&self) -> Option<String> {
        if self.hidden() == 0 {
            return None;
        }
        let mut parts = vec![format!("{} more", self.hidden())];
        if self.hidden_working > 0 {
            parts.push(format!("{} working", self.hidden_working));
        }
        if self.hidden_queued > 0 {
            parts.push(format!("{} queued", self.hidden_queued));
        }
        Some(parts.join(" · "))
    }
}

/// Folds a sorted Dev column to `max_cards`. Cards that need the user are always shown.
pub fn fold_dev(order: &ColumnOrder, tickets: &[Ticket], max_cards: usize) -> DevFold {
    let status: HashMap<&TicketKey, Status> = tickets.iter().map(|t| (&t.key, t.status)).collect();
    let needs = |k: &TicketKey| status.get(k).copied().is_some_and(needs_user);
    let always = order.cards.iter().filter(|k| needs(k)).count();
    let mut room = max_cards.saturating_sub(always);
    let mut fold = DevFold {
        shown_cards: Vec::new(),
        shown_queued: Vec::new(),
        hidden_working: 0,
        hidden_queued: 0,
    };
    for key in &order.cards {
        if needs(key) {
            fold.shown_cards.push(key.clone());
        } else if room > 0 {
            room -= 1;
            fold.shown_cards.push(key.clone());
        } else {
            fold.hidden_working += 1;
        }
    }
    for key in &order.queued {
        if room > 0 {
            room -= 1;
            fold.shown_queued.push(key.clone());
        } else {
            fold.hidden_queued += 1;
        }
    }
    fold
}

impl Board {
    /// Pins a ticket or epic to the top of its column; pinning again bumps it above the other pins.
    pub async fn pin(&self, key: &str, actor: &Actor) -> Result<Ticket, BoardError> {
        self.set_pin(key, Some(now_ms()), actor).await
    }

    pub async fn unpin(&self, key: &str, actor: &Actor) -> Result<Ticket, BoardError> {
        self.set_pin(key, None, actor).await
    }

    async fn set_pin(
        &self,
        key: &str,
        pinned_at: Option<i64>,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let ticket = self.ticket(key).await?;
        // Pins made within the same millisecond still order by when they were made.
        let latest: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(pinned_at) FROM tickets
             WHERE project_id = (SELECT project_id FROM tickets WHERE id = ?)",
        )
        .bind(&ticket.id)
        .fetch_one(self.store.pool())
        .await?;
        let pinned_at = pinned_at.map(|at| at.max(latest.unwrap_or(0) + 1));
        let mut tx = self.store.pool().begin().await?;
        sqlx::query("UPDATE tickets SET pinned_at = ? WHERE id = ?")
            .bind(pinned_at)
            .bind(&ticket.id)
            .execute(&mut *tx)
            .await?;
        let kind = if pinned_at.is_some() {
            EventKind::Pinned
        } else {
            EventKind::Unpinned
        };
        record_event(&mut tx, &ticket.id, kind, actor, json!({})).await?;
        tx.commit().await?;
        self.ticket_by_id(&ticket.id).await
    }
}
