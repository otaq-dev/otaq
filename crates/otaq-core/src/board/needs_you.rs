//! Needs-you items (ATT-06, SCH-03; d0016, d0042, d0024): everything that waits for the user, **across all
//! projects**, in the inbox order. Back-pressure (T-0080) counts them; the inbox (M7) lists them. Derived from
//! the tickets every time, never stored (SDD §8).
//!
//! Groups, in order:
//! 1. the needs-you family: Asks you, Needs you, Idle, also when a restart interrupted them and they kept their label
//!    (d0045; M10 adds agent-to-agent questions routed to the inbox). An epic child's question that went to its live
//!    lead is no item until the lead passes it on or hands it back (LEAD-06). Nor is a child's first stop that went
//!    to its live lead (LEAD-07); its second stop's item carries the lead's note and view;
//!
//! 1b. an epic lead's suggestions to close or delete a child (LEAD-08, d0083): no OS notification, they block no
//!    agent;
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
use super::validation::AUTO_MERGE_CONFLICT_LIMIT;
use super::{Board, BoardError};
use crate::attention::Suppression;
use crate::domain::validation::{NO_RESULT, failed_times};
use crate::domain::{AutoMergeStop, StartStep, Status, TicketKey};
use crate::named_agents::Role;
use crate::signals::hooks::HELPER_PROMPT_PREFIX;

/// The inbox groups of ATT-06, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedsYouGroup {
    /// Asks you, Needs you, Idle.
    Family,
    /// An epic lead's suggestions (LEAD-08): right after group 1, never notified.
    Suggestions,
    /// Interrupted, Start interrupted, Resume failed, Parent closed.
    Stopped,
    /// Ready for Review.
    Review,
}

/// The group a status puts a ticket in; `None` when it doesn't need the user.
pub fn group_of(status: Status) -> Option<NeedsYouGroup> {
    match status {
        Status::AsksYou
        | Status::NeedsYou
        | Status::Idle
        | Status::CheckerNeedsYou
        | Status::CheckFailed
        | Status::CheckNoResult
        | Status::DeliveryStopped => Some(NeedsYouGroup::Family),
        Status::Interrupted
        | Status::StartInterrupted
        | Status::ResumeFailed
        | Status::ParentClosed => Some(NeedsYouGroup::Stopped),
        Status::WaitingForYou => Some(NeedsYouGroup::Review),
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
    /// Interrupted only: the needs-you status it kept (d0045); answering it resumes the session.
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
    /// An epic lead's suggestion (LEAD-08): the item is the child's, `id` is the suggestion's and `reason` the
    /// headline "Wishlist lead suggests closing SHOP-40.6 Wishlist export (CSV)".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<SuggestionItem>,
    /// The second stop of a stuck epic child (LEAD-07): the note its lead sent at the first stop, with the lead's
    /// view ("LEAD'S NOTE · SENT AFTER THE FIRST 3 FAILURES", "LEAD'S VIEW"). The lead isn't woken a second time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_note: Option<crate::domain::LeadNoteSent>,
    /// The tool refused the model of this role's agent (AGT-05, SDD §7.2 `[M22]`): the item offers Change agent
    /// (Developer: a Start failed ticket) or Change validator (Validator: No result) in place of its first action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_agent: Option<Role>,
}

/// The lead's suggestion behind a [`NeedsYouGroup::Suggestions`] item: Close… / Delete… confirms it
/// (`suggestion.confirm`), Dismiss dismisses it (`suggestion.dismiss`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestionItem {
    pub id: String,
    pub epic: TicketKey,
    /// "Wishlist lead".
    pub lead: String,
    pub kind: crate::epic_lead::suggest::SuggestionKind,
    /// The lead's reason: "PAY-123 doesn't ask for an export."
    pub reason: String,
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
                // A needs-you ticket interrupted by a restart keeps its label (d0045): it stays with the turquoise
                // items, where answering it resumes and delivers.
                let group = match ticket.interrupted_from {
                    Some(_) => NeedsYouGroup::Family,
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
                    suggestion: None,
                    lead_note: ticket.flags.lead_note_sent.clone().filter(|_| {
                        use crate::epic_lead::stuck::is_stop;
                        is_stop(ticket.status) || ticket.interrupted_from.is_some_and(is_stop)
                    }),
                    change_agent: change_agent(ticket),
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

/// Whose agent to change when the tool refused its model: the Developer of a Start failed ticket (Start
/// interrupted at the `model` step), the Validator of a No result the validator's model caused.
pub fn change_agent(ticket: &Ticket) -> Option<Role> {
    match ticket.status {
        Status::StartInterrupted => ticket
            .start_problem
            .as_ref()
            .filter(|p| p.step == StartStep::Model)
            .map(|_| Role::Developer),
        Status::CheckNoResult => ticket
            .signal
            .as_ref()
            .and_then(|s| s.text.as_deref())
            .filter(|t| crate::validation::refused_its_model(t))
            .map(|_| Role::Validator),
        _ => None,
    }
}

/// When the item began: the needs-you signal's start, else when the ticket entered its status.
fn since(ticket: &Ticket) -> i64 {
    let signal = ticket
        .signal
        .as_ref()
        .filter(|_| ticket.status.is_needs_you_family() || ticket.interrupted_from.is_some())
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
            // A question its epic's lead passed on (LEAD-06): "asks · passed on by Wishlist lead: …".
            let asks = match ticket.question.as_ref().and_then(|q| q.via_lead.as_ref()) {
                Some(via) => format!("asks \u{b7} passed on by {}", via.lead),
                None => "asks".to_owned(),
            };
            match question {
                Some(q) => format!("{asks}: {q}"),
                None if asks == "asks" => "asks you".into(),
                None => asks,
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
            // Start failed on the model (AGT-05): shown as Start interrupted, with its own word.
            Some(problem) if problem.step == StartStep::Model => {
                format!("start failed · {}", problem.step.failure_label())
            }
            Some(problem) => format!("start failed · {}", problem.step.failure_label()),
            None => "start failed".into(),
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
        // The M12 items of frame *10 → New item types · group 1 (M12)* (d0063, d0064).
        Status::CheckerNeedsYou => match signal {
            Some(prompt) => format!("validator permission: {prompt}"),
            None => "validator permission".into(),
        },
        Status::CheckFailed => {
            let failed = failed_times(ticket.validation_round.max(1)).to_lowercase();
            match ticket.flags.findings.first() {
                Some(finding) => format!("{failed} · “{finding}”"),
                None => failed,
            }
        }
        Status::CheckNoResult => signal.map_or_else(
            || NO_RESULT.to_lowercase(),
            |text| lower_first(&text.replace('`', "")),
        ),
        Status::DeliveryStopped => {
            let text = signal.unwrap_or("stopped").replace('`', "");
            let tries = if ticket.auto_merge_stop == Some(AutoMergeStop::KeepsConflicting) {
                format!(" after {AUTO_MERGE_CONFLICT_LIMIT} tries in a row")
            } else {
                String::new()
            };
            format!("Auto-merge stopped · {}{tries}", lower_first(&text))
        }
        // "ready for review · validated · round 2": how it got there (d0063).
        Status::WaitingForYou => match ticket.flags.review_entry {
            Some(entry) => format!("ready for review · {}", lower_first(&entry.to_string())),
            None => "ready for review".into(),
        },
        other => other.label().to_lowercase(),
    }
}

/// "Keeps conflicting" → "keeps conflicting"; the inbox line reads on after a "·".
fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
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
        // Agents' questions to finished tickets that this project sends to the user (MSG-05): with the needs-you items.
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
                group: NeedsYouGroup::Family,
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
                suggestion: None,
                lead_note: None,
                change_agent: None,
            });
        }
        // The epic leads' suggestions (LEAD-08), while their child hasn't started or closed.
        for suggestion in crate::epic_lead::suggest::open(self).await? {
            let (Some((project, child)), Some((_, epic))) = (
                find(&views, &suggestion.child_id),
                find(&views, &suggestion.epic_id),
            ) else {
                continue;
            };
            if !crate::epic_lead::suggest::shown(&suggestion, child) {
                continue;
            }
            let lead = crate::project_agents::lead::lead_name(&epic.title);
            let blocks = blocked_counts(project.tickets);
            items.push(NeedsYouItem {
                group: NeedsYouGroup::Suggestions,
                project: project.key.to_owned(),
                project_name: project.name.to_owned(),
                id: suggestion.id.clone(),
                key: child.key.clone(),
                title: child.title.clone(),
                status: child.status,
                interrupted_from: None,
                question: None,
                reason: crate::epic_lead::suggest::headline(
                    &lead,
                    suggestion.kind,
                    &child.key,
                    &child.title,
                ),
                since: suggestion.created_at,
                age_ms: (now - suggestion.created_at).max(0),
                blocked: u32::try_from(blocks.get(&child.key).copied().unwrap_or(0))
                    .unwrap_or(u32::MAX),
                message: None,
                suggestion: Some(SuggestionItem {
                    id: suggestion.id,
                    epic: epic.key.clone(),
                    lead,
                    kind: suggestion.kind,
                    reason: suggestion.reason,
                }),
                lead_note: None,
                change_agent: None,
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
