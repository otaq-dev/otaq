//! Needs-you items (ATT-06, SCH-03; d0016, d0042, d0024): everything that waits for the user, **across all
//! projects**, in the inbox order. Back-pressure (T-0080) counts them; the inbox (M7) lists them. Derived from
//! the tickets every time, never stored (SDD §8).
//!
//! Groups, in order:
//! 1. the orange family: Asks you, Needs you, Idle, also when a restart interrupted them and they kept their label
//!    (d0045; M10 adds agent-to-agent questions routed to the inbox);
//! 2. Interrupted, Start interrupted, Resume failed, Parent closed;
//! 3. Ready for Review, tickets and epics.
//!
//! Inside a group, tickets that block more tickets come first, then the oldest (the longest in that status),
//! then key order. A status belongs to a group by [`group_of`] alone, so the states that later milestones set
//! (Interrupted, Resume failed) slot in as their tickets land.
//!
//! A ticket whose terminal has focus raises no item (ATT-03, [`crate::domain::Flags::your_turn`]); the item
//! appears as soon as the user leaves the terminal. While the plan-limit banner is up, agents stopped by the
//! limit raise none either ([`crate::attention::limit_stops`]). Back-pressure follows the same list.

use serde::{Deserialize, Serialize};

use super::order::blocked_counts;
use super::readiness::{ReadyState, readiness};
use super::tickets::Ticket;
use super::{Board, BoardError};
use crate::attention::Suppression;
use crate::domain::{Status, TicketKey};
use crate::signals::hooks::HELPER_PROMPT_PREFIX;

/// The inbox groups of ATT-06, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedsYouGroup {
    /// Asks you, Needs you, Idle.
    Orange,
    /// Interrupted, Start interrupted, Resume failed, Parent closed.
    Stopped,
    /// Ready for Review.
    Review,
}

/// The group a status puts a ticket in; `None` when it doesn't need the user.
pub fn group_of(status: Status) -> Option<NeedsYouGroup> {
    match status {
        Status::AsksYou | Status::NeedsYou | Status::Idle => Some(NeedsYouGroup::Orange),
        Status::Interrupted
        | Status::StartInterrupted
        | Status::ResumeFailed
        | Status::ParentClosed => Some(NeedsYouGroup::Stopped),
        Status::ReadyForReview => Some(NeedsYouGroup::Review),
        _ => None,
    }
}

/// One thing that waits for the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeedsYouItem {
    pub group: NeedsYouGroup,
    /// The project's key and name ("shop-web").
    pub project: String,
    pub project_name: String,
    pub id: String,
    pub key: TicketKey,
    pub title: String,
    pub status: Status,
    /// Interrupted only: the orange status it kept (d0045); answering it resumes the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupted_from: Option<Status>,
    /// Asks you: the open question with its answer options (ATT-12, d0061).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<super::questions::HumanQuestion>,
    /// "asks: Should dark mode follow the system setting?", "parent SHOP-26 was closed without merging".
    pub reason: String,
    /// When the ticket entered this status.
    pub since: i64,
    /// How long it has waited, in milliseconds.
    pub age_ms: i64,
    /// How many tickets wait on it, directly or further down ("blocks 2"; the board's "blocks more" count).
    pub blocked: u32,
    /// An agent's question to a finished ticket, sent to the user (MSG-05): the item is the asker's, `id` is the
    /// question's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<QuestionItem>,
}

/// "SHOP-21 asks SHOP-17 (Done): which Stripe API version?" (MSG-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionItem {
    pub id: String,
    pub target: TicketKey,
    pub target_status: Status,
    pub question: String,
}

/// One project's tickets, as [`needs_you_items`] reads them.
#[derive(Debug, Clone, Copy)]
pub struct ProjectTickets<'a> {
    pub key: &'a str,
    pub name: &'a str,
    pub tickets: &'a [Ticket],
}

/// The needs-you items of every project, in inbox order.
pub fn needs_you_items(projects: &[ProjectTickets<'_>], now: i64) -> Vec<NeedsYouItem> {
    needs_you_items_with(projects, now, &Suppression::default())
}

/// [`needs_you_items`], leaving out the tickets `suppression` hides (agents stopped by the plan limit while its
/// banner is up, ATT-09).
pub fn needs_you_items_with(
    projects: &[ProjectTickets<'_>],
    now: i64,
    suppression: &Suppression,
) -> Vec<NeedsYouItem> {
    let mut items: Vec<NeedsYouItem> = projects
        .iter()
        .flat_map(|project| {
            let blocks = blocked_counts(project.tickets);
            project.tickets.iter().filter_map(move |ticket| {
                if ticket.flags.your_turn || suppression.hides(ticket) {
                    return None;
                }
                // An orange ticket interrupted by a restart keeps its label (d0045): it stays with the orange
                // items, where answering it resumes and delivers.
                let group = match ticket.interrupted_from {
                    Some(_) => NeedsYouGroup::Orange,
                    None => group_of(ticket.status)?,
                };
                let since = since(ticket);
                Some(NeedsYouItem {
                    group,
                    project: project.key.to_owned(),
                    project_name: project.name.to_owned(),
                    id: ticket.id.clone(),
                    key: ticket.key.clone(),
                    title: ticket.title.clone(),
                    status: ticket.status,
                    interrupted_from: ticket.interrupted_from,
                    question: ticket.question.clone(),
                    reason: reason(ticket, project.tickets),
                    since,
                    age_ms: (now - since).max(0),
                    blocked: u32::try_from(blocks.get(&ticket.key).copied().unwrap_or(0))
                        .unwrap_or(u32::MAX),
                    message: None,
                })
            })
        })
        .collect();
    items.sort_by(|a, b| {
        a.group
            .cmp(&b.group)
            .then(b.blocked.cmp(&a.blocked))
            .then(a.since.cmp(&b.since))
            .then(a.key.cmp(&b.key))
    });
    items
}

/// The number back-pressure compares with its threshold (SCH-03, T-0080): **every** needs-you item, all three
/// groups of ATT-06 across all projects, the same count the inbox shows.
pub fn needs_you_count(items: &[NeedsYouItem]) -> usize {
    items.len()
}

/// When the item began: the orange signal's start, else when the ticket entered its status.
fn since(ticket: &Ticket) -> i64 {
    let signal = ticket
        .signal
        .as_ref()
        .filter(|_| ticket.status.is_orange() || ticket.interrupted_from.is_some())
        .and_then(|s| s.since);
    signal.or(ticket.status_since).unwrap_or(ticket.updated_at)
}

/// The item's line in the inbox (brief 10).
fn reason(ticket: &Ticket, tickets: &[Ticket]) -> String {
    // Interrupted with its label kept: the label's own line (the inbox adds "session ended by restart").
    if let Some(label) = ticket.interrupted_from {
        let kept = Ticket {
            status: label,
            interrupted_from: None,
            ..ticket.clone()
        };
        return reason(&kept, tickets);
    }
    let signal = ticket
        .signal
        .as_ref()
        .and_then(|s| s.text.as_deref())
        .map(str::trim)
        .filter(|t| !t.is_empty());
    match ticket.status {
        Status::AsksYou => {
            let question = ticket.question.as_ref().map(|q| q.text.trim()).or(signal);
            match question {
                Some(q) => format!("asks: {q}"),
                None => "asks you".into(),
            }
        }
        Status::NeedsYou => match signal {
            Some(prompt) if prompt.starts_with(HELPER_PROMPT_PREFIX) => prompt.to_owned(),
            Some(prompt) => format!("permission: {prompt}"),
            None => "needs you".into(),
        },
        Status::Idle => match signal {
            Some(line) => format!("idle · ended its turn: “{line}”"),
            None => "idle · ended its turn without a message".into(),
        },
        Status::Interrupted => "session interrupted".into(),
        Status::ResumeFailed => "resume failed".into(),
        Status::StartInterrupted => match &ticket.start_problem {
            Some(problem) => format!("start interrupted · {}", problem.step.failure_label()),
            None => "start interrupted".into(),
        },
        Status::ParentClosed => match readiness(ticket, tickets) {
            ReadyState::ParentClosed { parents } => match parents.as_slice() {
                [one] => format!("parent {one} was closed without merging"),
                many => format!(
                    "parents {} were closed without merging",
                    many.iter()
                        .map(|k| k.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            },
            _ => "a parent was closed without merging".into(),
        },
        Status::ReadyForReview => "ready for review".into(),
        other => other.label().to_lowercase(),
    }
}

impl Board {
    /// The needs-you items of every project, in inbox order.
    pub async fn needs_you_items(&self, now: i64) -> Result<Vec<NeedsYouItem>, BoardError> {
        let projects = self.projects().await?;
        let mut loaded = Vec::with_capacity(projects.len());
        for project in &projects {
            loaded.push(self.tickets(&project.key).await?);
        }
        let views: Vec<ProjectTickets<'_>> = projects
            .iter()
            .zip(&loaded)
            .map(|(p, tickets)| ProjectTickets {
                key: &p.key,
                name: &p.name,
                tickets,
            })
            .collect();
        let mut items = needs_you_items_with(&views, now, &self.limit_stops().snapshot());
        // Agents' questions to finished tickets that this project sends to the user (MSG-05): with the orange items.
        let routed: Vec<(String, String, String, String, i64)> = sqlx::query_as(
            "SELECT id, from_ticket_id, to_ticket_id, question, asked_at FROM messages
             WHERE delivery = 'inbox' AND answer IS NULL ORDER BY asked_at, id",
        )
        .fetch_all(self.store().pool())
        .await?;
        for (id, from, to, question, asked_at) in routed {
            let (Some((project, asker)), Some((_, target))) =
                (find(&views, &from), find(&views, &to))
            else {
                continue;
            };
            items.push(NeedsYouItem {
                group: NeedsYouGroup::Orange,
                project: project.key.to_owned(),
                project_name: project.name.to_owned(),
                id,
                key: asker.key.clone(),
                title: asker.title.clone(),
                status: asker.status,
                interrupted_from: None,
                question: None,
                reason: format!(
                    "asks {} ({}): {}",
                    target.key,
                    target.status.label(),
                    question.lines().next().unwrap_or_default()
                ),
                since: asked_at,
                age_ms: (now - asked_at).max(0),
                blocked: 0,
                message: Some(QuestionItem {
                    id: String::new(),
                    target: target.key.clone(),
                    target_status: target.status,
                    question,
                }),
            });
        }
        for item in &mut items {
            if let Some(m) = &mut item.message {
                m.id = item.id.clone();
            }
        }
        items.sort_by(|a, b| {
            a.group
                .cmp(&b.group)
                .then(b.blocked.cmp(&a.blocked))
                .then(a.since.cmp(&b.since))
                .then(a.key.cmp(&b.key))
        });
        Ok(items)
    }
}

/// A ticket by id among the loaded projects.
fn find<'a>(
    views: &'a [ProjectTickets<'a>],
    id: &str,
) -> Option<(&'a ProjectTickets<'a>, &'a Ticket)> {
    views
        .iter()
        .find_map(|p| p.tickets.iter().find(|t| t.id == id).map(|t| (p, t)))
}
