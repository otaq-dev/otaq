//! The "Needs you" inbox (UI-06, ATT-06, ATT-09, ATT-10; d0016, d0024; *10 · Needs-you inbox*): the service's
//! needs-you items across all projects in three groups with counts, Resume all on the interrupted group, and items
//! that resolved elsewhere fading out instead of jumping away.
//!
//! An epic's lead adds its items (LEAD-06, LEAD-07, LEAD-08, LEAD-11; d0083; *10 → Row · Epic lead items (0083)*): a
//! question it passed on and a child's second stop are group 1 items with the lead's recommendation or note, its
//! suggestions sit in their own group after group 1, and its own question or permission prompt is a group 1 item
//! (from the Project agents list) that opens the epic panel on its Lead tab.
//!
//! An item that is gone from a new list stays as a struck-through "resolved" row for [`FADE`], so the rows below
//! don't jump while the user reads them. Rows keep the service's order.

use std::time::{Duration, Instant};

use otaq_core::attention::inbox::InboxList;
use otaq_core::board::needs_you::{NeedsYouGroup, NeedsYouItem, SuggestionItem};
use otaq_core::domain::{LeadNoteSent, Status, TicketKey, ValidationAction};
use otaq_core::epic_lead::suggest::SuggestionKind;
use otaq_core::named_agents::Role;
use otaq_core::project_agents::AgentNeedsYou;
use serde::Serialize;
use ts_rs::TS;

use crate::theme::Status as Legend;
use crate::view_models::answer::AnswerKind;
use crate::view_models::panel::Tab;
use crate::view_models::status::pill;
use crate::view_models::tool_avatar::AvatarTool;

/// The header's inbox button (ATT-10; *02 → Header states*): "Inbox", turquoise with the count while something
/// waits, quiet and outlined with 0 when nothing does (*Header · inbox clear*). d0093: the place is "Inbox"; "Needs
/// you" is the state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxButton")]
pub struct HeaderButton {
    pub label: &'static str,
    pub count: u32,
    pub active: bool,
}

/// The button for the inbox's total (the sum of the rail badges).
pub fn header_button(total: u32) -> HeaderButton {
    HeaderButton {
        label: "Inbox",
        count: total,
        active: total > 0,
    }
}

/// How long a resolved row stays before it goes.
pub const FADE: Duration = Duration::from_secs(4);

/// The group headings (frame *Inbox popover · mixed, 2 projects*).
pub fn group_title(group: NeedsYouGroup) -> &'static str {
    match group {
        // *10 → Row · Epic lead items (0083)*: Project agents and epic leads both wait here.
        NeedsYouGroup::Family => "ASKS YOU · NEEDS YOU · IDLE · VALIDATION · AGENTS",
        NeedsYouGroup::Suggestions => "SUGGESTIONS · NO OS NOTIFICATION",
        NeedsYouGroup::Stopped => "INTERRUPTED & PARENT CLOSED",
        NeedsYouGroup::Review => "READY FOR REVIEW",
    }
}

/// An item's age as the inbox shows it: "now", "6 min", "3 h", "2 d".
pub fn age(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    match minutes {
        0 => "now".to_owned(),
        m if m < 60 => format!("{m} min"),
        m if m < 24 * 60 => format!("{} h", m / 60),
        m => format!("{} d", m / (24 * 60)),
    }
}

/// A button in a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowAction {
    /// Start interrupted: its own Retry start (not part of Resume all).
    RetryStart,
    /// Parent closed: the two ways forward (d0044).
    ReopenParent(TicketKey),
    RemoveDependency(TicketKey),
    /// Idle: "This is the usage limit" (USG-07, the safety net when limit detection missed): every Idle agent
    /// folds into the usage-limit banner.
    UsageLimit,
    /// An agent's question sent to the user (MSG-05): "Resume SHOP-17" lets that finished ticket answer it.
    ResumeTarget {
        message: String,
        target: TicketKey,
    },
    /// The validation and Auto-merge items' own actions, inline (VAL-07, VAL-09, AMG-03, AMG-04; *10 → New item
    /// types · group 1 (M12)*): Move to Review anyway, Give it another round, Answer the agent…, Retry validation,
    /// Retry merge, Move to Review.
    Validation(ValidationAction),
    /// A lead's suggestion: Close… / Delete… opens the usual confirmation (d0044, d0060) with the lead's reason
    /// added; its confirm runs `suggestion.confirm` (LEAD-08).
    ConfirmSuggestion {
        id: String,
        kind: SuggestionKind,
        /// "Suggested by Wishlist lead: “PAY-123 doesn't ask for an export.”"
        line: String,
    },
    /// Dismiss: the item goes (`suggestion.dismiss`).
    DismissSuggestion {
        id: String,
    },
    /// The tool refused this role's model (AGT-05): Change agent (a Start failed ticket's Developer) or Change
    /// validator (No result), in place of the item's first action. The panel opens with that row's picker open.
    ChangeAgent(Role),
}

/// The line the close and delete confirmations add for a lead's suggestion.
pub fn suggested_line(s: &SuggestionItem) -> String {
    format!(
        "Suggested by {}: \u{201c}{}\u{201d}",
        s.lead,
        s.reason.trim()
    )
}

/// The inline actions of an M12 item, in the frame's order (the first one is the primary button).
pub fn validation_actions(status: Status) -> Vec<ValidationAction> {
    use ValidationAction as A;
    match status {
        Status::ValidationFailed => vec![A::MoveToReviewAnyway, A::AnotherRound, A::AnswerAgent],
        Status::ValidationNoResult => vec![A::Retry, A::MoveToReviewAnyway],
        Status::AutoMergeStopped => vec![A::RetryMerge, A::MoveToReview],
        _ => Vec::new(),
    }
}

/// One row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxRow {
    pub key: TicketKey,
    /// The project's key, for switching to it.
    pub project: String,
    pub project_name: String,
    pub title: String,
    pub reason: String,
    pub age: String,
    /// "blocks 2".
    pub blocks: Option<String>,
    /// The legend entry for the icon and its colours.
    pub legend: Legend,
    /// A needs-you ticket interrupted by a restart: "session ended by restart · answering resumes it".
    pub restart_note: Option<&'static str>,
    pub actions: Vec<RowAction>,
    /// Resolved elsewhere, fading out.
    pub resolved: bool,
    /// The panel tab the row opens: the Diff for a review, else the Terminal.
    pub tab: Tab,
    /// Asks you / Idle: answered in the row (ATT-08); an interrupted question resumes with it.
    pub answer: Option<AnswerKind>,
    /// The open question, with its answer options (ATT-12).
    pub question: Option<otaq_core::board::questions::HumanQuestion>,
    /// An agent's question sent to the user (MSG-05): its ID; the answer is free text (d0061 scope).
    pub message: Option<String>,
    /// "Sent to you: finished tickets don't answer in this project".
    pub sent_note: Option<&'static str>,
    /// Needs you · validator: opening it lands on the Terminal tab's Validator side (VAL-11).
    pub opens_validator: bool,
    /// A child's second stop after its lead's note (LEAD-07): the note and the lead's view.
    pub lead_note: Option<LeadNoteSent>,
    /// A lead's suggestion (LEAD-08): `title` is then the lead's headline and `reason` the lead's reason.
    pub suggestion: Option<SuggestionItem>,
}

/// One group with its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxGroupView {
    pub group: NeedsYouGroup,
    pub title: &'static str,
    /// Items still waiting (resolved rows don't count).
    pub count: usize,
    /// "Resume all (N)" on the interrupted group when N > 0.
    pub resume_all: Option<usize>,
    pub rows: Vec<InboxRow>,
}

/// The Resume all confirmation (frame *Resume all · confirm*).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeAllConfirm {
    /// "Resume 1 interrupted session?"
    pub title: String,
    pub text: &'static str,
    /// The tickets it resumes: key and title.
    pub included: Vec<(TicketKey, String)>,
    /// "Not included: SHOP-28 Cart badge count. …" when Start interrupted tickets wait too.
    pub not_included: Option<String>,
    /// "Resume 1".
    pub confirm: String,
}

impl ResumeAllConfirm {
    /// The whole body as one text, for a plain confirmation: what it does, the tickets it resumes, and the ones
    /// left out.
    pub fn body(&self) -> String {
        let mut body = self.text.to_owned();
        for (key, title) in &self.included {
            body.push_str(&format!("\n{key} {title}"));
        }
        if let Some(left_out) = &self.not_included {
            body.push_str("\n\n");
            body.push_str(left_out);
        }
        body
    }
}

/// The inbox as the app keeps it.
#[derive(Debug, Clone, Default)]
pub struct InboxModel {
    list: InboxList,
    /// Items that left the list, when, and their place in their group.
    fading: Vec<(NeedsYouItem, Instant, usize)>,
}

impl InboxModel {
    /// Takes a new list from the service; items that are gone start fading.
    pub fn update(&mut self, list: InboxList, now: Instant) {
        for (n, old) in self.list.items.iter().enumerate() {
            let still = list.items.iter().any(|i| i.id == old.id);
            if !still && !self.fading.iter().any(|(f, _, _)| f.id == old.id) {
                let place = self.list.items[..n]
                    .iter()
                    .filter(|i| i.group == old.group)
                    .count();
                self.fading.push((old.clone(), now, place));
            }
        }
        // An item that came back isn't fading any more.
        self.fading
            .retain(|(f, _, _)| !list.items.iter().any(|i| i.id == f.id));
        self.list = list;
    }

    /// Drops faded rows; `true` when something changed.
    pub fn expire(&mut self, now: Instant) -> bool {
        let before = self.fading.len();
        self.fading
            .retain(|(_, at, _)| now.duration_since(*at) < FADE);
        before != self.fading.len()
    }

    #[cfg(test)]
    pub fn is_fading(&self) -> bool {
        !self.fading.is_empty()
    }

    /// The header's count (= the sum of the rail badges, ATT-10).
    pub fn total(&self) -> usize {
        self.list.counts.total as usize
    }

    #[cfg(test)]
    pub fn list(&self) -> &InboxList {
        &self.list
    }

    /// The items still waiting, in inbox order.
    pub fn items(&self) -> &[NeedsYouItem] {
        &self.list.items
    }

    /// The groups to draw, in order; empty groups are left out. `parents` gives a Parent closed item its closed
    /// parent (for Reopen parent / Remove dependency).
    pub fn groups(&self, parents: &dyn Fn(&TicketKey) -> Option<TicketKey>) -> Vec<InboxGroupView> {
        let mut groups: Vec<InboxGroupView> = Vec::new();
        for group in [
            NeedsYouGroup::Family,
            NeedsYouGroup::Suggestions,
            NeedsYouGroup::Stopped,
            NeedsYouGroup::Review,
        ] {
            let mut rows: Vec<InboxRow> = self
                .list
                .items
                .iter()
                .filter(|i| i.group == group)
                .map(|i| row(i, false, parents))
                .collect();
            let count = rows.len();
            // A fading row keeps the place it had, as far as the list allows.
            let mut fading: Vec<&(NeedsYouItem, Instant, usize)> = self
                .fading
                .iter()
                .filter(|(i, _, _)| i.group == group)
                .collect();
            fading.sort_by_key(|(_, _, place)| *place);
            for (item, _, place) in fading {
                rows.insert((*place).min(rows.len()), row(item, true, parents));
            }
            if rows.is_empty() {
                continue;
            }
            let resumable = self.resume_all_keys().len();
            groups.push(InboxGroupView {
                group,
                title: group_title(group),
                count,
                resume_all: (group == NeedsYouGroup::Stopped && resumable > 0).then_some(resumable),
                rows,
            });
        }
        groups
    }

    /// What Resume all resumes: every Interrupted ticket (d0045), the ones that kept their label included.
    pub fn resume_all_keys(&self) -> Vec<TicketKey> {
        self.list
            .items
            .iter()
            .filter(|i| i.status == Status::Interrupted)
            .map(|i| i.key.clone())
            .collect()
    }

    /// The confirmation Resume all asks first.
    pub fn resume_all_confirm(&self) -> Option<ResumeAllConfirm> {
        let included: Vec<(TicketKey, String)> = self
            .list
            .items
            .iter()
            .filter(|i| i.status == Status::Interrupted)
            .map(|i| (i.key.clone(), i.title.clone()))
            .collect();
        if included.is_empty() {
            return None;
        }
        let n = included.len();
        let plural = if n == 1 { "session" } else { "sessions" };
        let left_out: Vec<String> = self
            .list
            .items
            .iter()
            .filter(|i| i.status == Status::StartInterrupted)
            .map(|i| format!("{} {}", i.key, i.title))
            .collect();
        let not_included = (!left_out.is_empty()).then(|| {
            let why = if left_out.len() == 1 {
                "Its start never finished, so there’s no session to resume. Use Retry start on its card."
            } else {
                "Their starts never finished, so there’s no session to resume. Use Retry start on their cards."
            };
            format!("Not included: {}. {why}", left_out.join(", "))
        });
        Some(ResumeAllConfirm {
            title: format!("Resume {n} interrupted {plural}?"),
            text: "Each session continues in its own worktree. This starts agents, so it uses tokens.",
            included,
            not_included,
            confirm: format!("Resume {n}"),
        })
    }
}

fn row(
    item: &NeedsYouItem,
    resolved: bool,
    parents: &dyn Fn(&TicketKey) -> Option<TicketKey>,
) -> InboxRow {
    // An agent's question to a finished ticket, sent to the user (MSG-05, *10 → Agent question for you*).
    if let Some(q) = &item.message {
        let (legend, _) = pill(Status::AsksYou);
        return InboxRow {
            key: item.key.clone(),
            project: item.project.clone(),
            project_name: item.project_name.clone(),
            title: item.title.clone(),
            reason: if resolved {
                "resolved elsewhere · just now".to_owned()
            } else {
                item.reason.clone()
            },
            age: if resolved {
                "now".to_owned()
            } else {
                age(item.age_ms)
            },
            blocks: None,
            legend,
            restart_note: None,
            actions: if resolved {
                Vec::new()
            } else {
                vec![RowAction::ResumeTarget {
                    message: q.id.clone(),
                    target: q.target.clone(),
                }]
            },
            resolved,
            tab: Tab::Terminal,
            answer: (!resolved).then_some(AnswerKind::Send),
            question: None,
            message: Some(q.id.clone()),
            sent_note: (!resolved)
                .then_some("Sent to you: finished tickets don\u{2019}t answer in this project"),
            opens_validator: false,
            lead_note: None,
            suggestion: None,
        };
    }
    // A lead's suggestion (LEAD-08, *10 → Row · Epic lead items (0083)*): the lead's headline, its reason, Close… or
    // Delete… and Dismiss. Opening it shows the never-started child's Ticket tab.
    if let Some(s) = &item.suggestion {
        let (legend, _) = pill(item.status);
        return InboxRow {
            key: item.key.clone(),
            project: item.project.clone(),
            project_name: item.project_name.clone(),
            title: item.reason.clone(),
            reason: if resolved {
                "resolved elsewhere · just now".to_owned()
            } else {
                s.reason.clone()
            },
            age: if resolved {
                "now".to_owned()
            } else {
                age(item.age_ms)
            },
            blocks: None,
            legend,
            restart_note: None,
            actions: if resolved {
                Vec::new()
            } else {
                vec![
                    RowAction::ConfirmSuggestion {
                        id: s.id.clone(),
                        kind: s.kind,
                        line: suggested_line(s),
                    },
                    RowAction::DismissSuggestion { id: s.id.clone() },
                ]
            },
            resolved,
            tab: Tab::Ticket,
            answer: None,
            question: None,
            message: None,
            sent_note: None,
            opens_validator: false,
            lead_note: None,
            suggestion: Some(s.clone()),
        };
    }
    let (legend, _) = pill(item.interrupted_from.unwrap_or(item.status));
    let actions = match item.status {
        Status::StartInterrupted => vec![RowAction::RetryStart],
        Status::ParentClosed => parents(&item.key)
            .map(|p| {
                vec![
                    RowAction::ReopenParent(p.clone()),
                    RowAction::RemoveDependency(p),
                ]
            })
            .unwrap_or_default(),
        // Only a live Idle agent; Asks you, Needs you and an Idle interrupted by a restart don't get it.
        Status::Idle => vec![RowAction::UsageLimit],
        other => validation_actions(other)
            .into_iter()
            .map(RowAction::Validation)
            .collect(),
    };
    // Start failed offers Change agent (Retry start only in the panel); No result Change validator first.
    let actions = match (item.change_agent, actions.split_first()) {
        (Some(role), Some((_, rest))) => std::iter::once(RowAction::ChangeAgent(role))
            .chain(rest.iter().cloned())
            .collect(),
        _ => actions,
    };
    InboxRow {
        key: item.key.clone(),
        project: item.project.clone(),
        project_name: item.project_name.clone(),
        title: item.title.clone(),
        reason: if resolved {
            "resolved elsewhere · just now".to_owned()
        } else {
            item.reason.clone()
        },
        age: if resolved {
            "now".to_owned()
        } else {
            age(item.age_ms)
        },
        blocks: (item.blocked > 0 && !resolved).then(|| format!("blocks {}", item.blocked)),
        legend,
        restart_note: (item.interrupted_from.is_some() && !resolved)
            .then_some("session ended by restart · answering resumes it"),
        actions: if resolved { Vec::new() } else { actions },
        resolved,
        tab: if item.status == Status::ReadyForReview {
            Tab::Diff
        } else {
            Tab::Terminal
        },
        answer: if resolved { None } else { AnswerKind::of(item) },
        question: item.question.clone(),
        message: None,
        sent_note: None,
        opens_validator: item.status == Status::ValidatorNeedsYou && !resolved,
        lead_note: item.lead_note.clone().filter(|_| !resolved),
        suggestion: None,
    }
}

/// With several tools connected, a refused model's row names the tool (M23; *04 → Start failed · several tools*):
/// "start failed · Codex couldn't run the model", "the validator couldn't run gpt-5.4 on Codex: …". `tool_of`
/// answers the tool of a ticket's role (its row's); with one tool connected the rows read as in M22. Holds add no
/// row at all (USG-10): a held ticket's status never puts it in a group.
pub fn name_tools(
    groups: &mut [InboxGroupView],
    tool_of: &dyn Fn(&TicketKey, Role) -> Option<AvatarTool>,
) {
    for row in groups.iter_mut().flat_map(|g| g.rows.iter_mut()) {
        if row.resolved {
            continue;
        }
        let Some(role) = row.actions.iter().find_map(|a| match a {
            RowAction::ChangeAgent(role) => Some(*role),
            _ => None,
        }) else {
            continue;
        };
        let Some(tool) = tool_of(&row.key, role) else {
            continue;
        };
        row.reason = tool_named(&row.reason, role, tool.name());
    }
}

/// [`name_tools`]'s wording of one reason.
fn tool_named(reason: &str, role: Role, tool: &str) -> String {
    match role {
        Role::Validator => match reason.split_once(':') {
            Some((head, rest)) => format!("{head} on {tool}:{rest}"),
            None => format!("{reason} on {tool}"),
        },
        _ => format!(
            "start failed \u{b7} {}",
            crate::view_models::card::couldnt_run_on(tool, None)
        ),
    }
}

/// The id prefix of an epic lead's own item in the ⌘J run.
const LEAD_ITEM: &str = "lead:";

/// What ⌘J walks: the inbox items, with each epic lead's own question or permission prompt after the tickets of
/// group 1 (LEAD-11). A lead's item is keyed by its epic, so the run opens the epic panel on its Lead tab.
pub fn run_items(items: &[NeedsYouItem], agents: &[AgentNeedsYou]) -> Vec<NeedsYouItem> {
    let leads = agents.iter().filter_map(|a| {
        let epic = a.epic.clone()?;
        Some(NeedsYouItem {
            group: NeedsYouGroup::Family,
            project: a.project.clone(),
            project_name: a.project_name.clone(),
            id: format!("{LEAD_ITEM}{}", a.agent),
            key: epic,
            title: a.name.clone(),
            status: a.status,
            interrupted_from: None,
            question: a.question.clone(),
            reason: a.reason.clone(),
            since: a.since,
            age_ms: 0,
            blocked: 0,
            message: None,
            suggestion: None,
            lead_note: None,
            change_agent: None,
        })
    });
    let at = items
        .iter()
        .position(|i| i.group != NeedsYouGroup::Family)
        .unwrap_or(items.len());
    let mut all = items[..at].to_vec();
    all.extend(leads);
    all.extend_from_slice(&items[at..]);
    all
}

/// The panel tab ⌘J opens an item on: an epic lead's on the Lead tab, a suggestion's never-started child on its
/// Ticket tab, the rest on the Terminal.
pub fn run_tab(item: &NeedsYouItem) -> Tab {
    if item.id.starts_with(LEAD_ITEM) {
        Tab::Lead
    } else if item.suggestion.is_some() {
        Tab::Ticket
    } else {
        Tab::Terminal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use otaq_core::attention::inbox::InboxCounts;

    const MIN: i64 = 60_000;

    /// *10 → New item types · group 1 (M12)*: each validation and Auto-merge item carries its own actions inline;
    /// the validator's permission prompt is answered in its terminal.
    #[test]
    fn the_m12_items_carry_their_actions() {
        use ValidationAction as A;
        let actions = |status| {
            row(
                &item(
                    "SHOP-32",
                    "Address autocomplete",
                    status,
                    NeedsYouGroup::Family,
                    MIN,
                ),
                false,
                &|_| None,
            )
            .actions
        };
        let v = |list: &[A]| {
            list.iter()
                .copied()
                .map(RowAction::Validation)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            actions(Status::ValidationFailed),
            v(&[A::MoveToReviewAnyway, A::AnotherRound, A::AnswerAgent])
        );
        assert_eq!(
            actions(Status::ValidationNoResult),
            v(&[A::Retry, A::MoveToReviewAnyway])
        );
        assert_eq!(
            actions(Status::AutoMergeStopped),
            v(&[A::RetryMerge, A::MoveToReview])
        );
        let validator = row(
            &item(
                "SHOP-33",
                "Shipping estimate",
                Status::ValidatorNeedsYou,
                NeedsYouGroup::Family,
                MIN,
            ),
            false,
            &|_| None,
        );
        assert!(validator.actions.is_empty());
        assert!(validator.opens_validator);
        assert_eq!(validator.tab, Tab::Terminal);
        assert_eq!(
            validator.answer, None,
            "answered in the validator's terminal"
        );
        // Resolved elsewhere: no buttons.
        let resolved = row(
            &item(
                "SHOP-34",
                "Tax display",
                Status::ValidationNoResult,
                NeedsYouGroup::Family,
                MIN,
            ),
            true,
            &|_| None,
        );
        assert!(resolved.actions.is_empty());
    }

    /// *10 → Agent question for you · new item type* (MSG-05): free-text Answer and "Resume SHOP-17".
    #[test]
    fn an_agent_question_sent_to_you() {
        let mut q = item(
            "SHOP-21",
            "Order confirmation email",
            Status::Working,
            NeedsYouGroup::Family,
            MIN,
        );
        q.id = "0199-q".into();
        q.reason = "asks SHOP-17 (Merged): which Stripe API version?".into();
        q.message = Some(otaq_core::board::needs_you::QuestionItem {
            id: "0199-q".into(),
            target: "SHOP-17".parse().unwrap(),
            target_status: Status::Merged,
            question: "which Stripe API version?".into(),
        });
        let r = row(&q, false, &|_| None);
        assert_eq!(r.answer, Some(AnswerKind::Send), "free text, no options");
        assert!(r.question.is_none());
        assert_eq!(r.message.as_deref(), Some("0199-q"));
        assert_eq!(
            r.actions,
            [RowAction::ResumeTarget {
                message: "0199-q".into(),
                target: "SHOP-17".parse().unwrap()
            }]
        );
        assert_eq!(
            r.sent_note,
            Some("Sent to you: finished tickets don\u{2019}t answer in this project")
        );
        assert_eq!(r.reason, "asks SHOP-17 (Merged): which Stripe API version?");
    }

    fn item(
        key: &str,
        title: &str,
        status: Status,
        group: NeedsYouGroup,
        age: i64,
    ) -> NeedsYouItem {
        let project = key.split('-').next().unwrap().to_owned();
        NeedsYouItem {
            group,
            project_name: if project == "SHOP" {
                "shop-web"
            } else {
                "billing-api"
            }
            .into(),
            project,
            id: key.into(),
            key: key.parse().unwrap(),
            title: title.into(),
            status,
            interrupted_from: None,
            question: None,
            reason: format!("reason of {key}"),
            since: 0,
            age_ms: age,
            blocked: if key == "SHOP-18" { 2 } else { 0 },
            message: None,
            suggestion: None,
            lead_note: None,
            change_agent: None,
        }
    }

    fn suggestion_item() -> NeedsYouItem {
        let mut s = item(
            "SHOP-40.6",
            "Wishlist export (CSV)",
            Status::Backlog,
            NeedsYouGroup::Suggestions,
            4 * MIN,
        );
        s.id = "sg1".into();
        s.reason = "Wishlist lead suggests closing SHOP-40.6 Wishlist export (CSV)".into();
        s.suggestion = Some(SuggestionItem {
            id: "sg1".into(),
            epic: "SHOP-40".parse().unwrap(),
            lead: "Wishlist lead".into(),
            kind: SuggestionKind::Close,
            reason: "PAY-123 doesn't ask for an export.".into(),
        });
        s
    }

    fn lead_agent() -> AgentNeedsYou {
        AgentNeedsYou {
            project: "SHOP".into(),
            project_name: "shop-web".into(),
            agent: "pa7".into(),
            name: "Wishlist lead".into(),
            epic: Some("SHOP-40".parse().unwrap()),
            status: Status::AsksYou,
            reason: "asks: Should the share link (40.4) expire? PAY-123 doesn't say.".into(),
            question: None,
            since: 0,
        }
    }

    /// *10 → Row · Epic lead items (0083)*: the groups in order (1, Suggestions, Interrupted, Review), the
    /// suggestion's headline, reason and buttons.
    #[test]
    fn suggestions_sit_between_group_1_and_interrupted() {
        let mut list = sample();
        list.items.insert(5, suggestion_item());
        let mut m = InboxModel::default();
        m.update(list, Instant::now());
        let groups = m.groups(&no_parents);
        let titles: Vec<&str> = groups.iter().map(|g| g.title).collect();
        assert_eq!(
            titles,
            [
                "ASKS YOU · NEEDS YOU · IDLE · VALIDATION · AGENTS",
                "SUGGESTIONS · NO OS NOTIFICATION",
                "INTERRUPTED & PARENT CLOSED",
                "READY FOR REVIEW"
            ]
        );
        let row = &groups[1].rows[0];
        assert_eq!(groups[1].count, 1);
        assert_eq!(
            row.title,
            "Wishlist lead suggests closing SHOP-40.6 Wishlist export (CSV)"
        );
        assert_eq!(row.reason, "PAY-123 doesn't ask for an export.");
        assert_eq!(row.tab, Tab::Ticket);
        assert_eq!(row.answer, None);
        assert_eq!(
            row.actions,
            [
                RowAction::ConfirmSuggestion {
                    id: "sg1".into(),
                    kind: SuggestionKind::Close,
                    line: "Suggested by Wishlist lead: “PAY-123 doesn't ask for an export.”".into(),
                },
                RowAction::DismissSuggestion { id: "sg1".into() }
            ]
        );
    }

    /// LEAD-07: the second stop keeps its own actions and carries the lead's note; a resolved one drops it.
    #[test]
    fn a_second_stop_carries_the_leads_note() {
        let mut stuck = item(
            "SHOP-40.3",
            "Wishlist page",
            Status::ValidationFailed,
            NeedsYouGroup::Family,
            6 * MIN,
        );
        stuck.lead_note = Some(LeadNoteSent {
            lead: "Wishlist lead".into(),
            text: "The API returns items[] (SHOP-40.1). Fix the test, don't change the API.".into(),
            view: Some("The agent keeps changing the API instead of the test.".into()),
            stop: otaq_core::domain::StuckStop::ValidationFailed,
            after_rounds: Some(3),
            at: 0,
        });
        let r = row(&stuck, false, &|_| None);
        assert_eq!(r.lead_note, stuck.lead_note);
        assert_eq!(
            r.actions,
            validation_actions(Status::ValidationFailed)
                .into_iter()
                .map(RowAction::Validation)
                .collect::<Vec<_>>()
        );
        assert_eq!(row(&stuck, true, &|_| None).lead_note, None);
    }

    /// LEAD-11: ⌘J reaches the lead's own item after the tickets of group 1 and opens its Lead tab; a suggestion
    /// opens its child's Ticket tab.
    #[test]
    fn the_run_walks_lead_items_and_suggestions() {
        let mut items = sample().items;
        items.insert(3, suggestion_item());
        let all = run_items(&items, &[lead_agent()]);
        let keys: Vec<String> = all.iter().map(|i| i.key.to_string()).collect();
        assert_eq!(
            keys,
            [
                "SHOP-22",
                "SHOP-23",
                "SHOP-12",
                "SHOP-40",
                "SHOP-40.6",
                "SHOP-25",
                "SHOP-28",
                "SHOP-18",
                "BILL-5"
            ]
        );
        assert_eq!(run_tab(&all[3]), Tab::Lead);
        assert_eq!(run_tab(&all[4]), Tab::Ticket);
        assert_eq!(run_tab(&all[0]), Tab::Terminal);
        // A Project agent without an epic isn't walked (it lives in the pane).
        let plain = AgentNeedsYou {
            epic: None,
            ..lead_agent()
        };
        assert_eq!(run_items(&items, &[plain]).len(), items.len());
    }

    /// The frame's sample: SHOP-22, 23, 12 · SHOP-25, 28 · SHOP-18, BILL-5.
    fn sample() -> InboxList {
        use NeedsYouGroup::*;
        let items = vec![
            item(
                "SHOP-22",
                "Fix flaky login redirect",
                Status::NeedsYou,
                Family,
                2 * MIN,
            ),
            item(
                "SHOP-23",
                "Split user full name",
                Status::AsksYou,
                Family,
                6 * MIN,
            ),
            item(
                "SHOP-12",
                "Add dark mode toggle",
                Status::AsksYou,
                Family,
                8 * MIN,
            ),
            item(
                "SHOP-25",
                "Upgrade to Next.js 15",
                Status::Interrupted,
                Stopped,
                180 * MIN,
            ),
            item(
                "SHOP-28",
                "Cart badge count",
                Status::StartInterrupted,
                Stopped,
                20 * MIN,
            ),
            item(
                "SHOP-18",
                "Checkout API",
                Status::ReadyForReview,
                Review,
                25 * MIN,
            ),
            item(
                "BILL-5",
                "VAT rounding fix",
                Status::ReadyForReview,
                Review,
                60 * MIN,
            ),
        ];
        InboxList {
            counts: InboxCounts::of(&items, &["SHOP".into(), "BILL".into()]),
            items,
            project_agents: Vec::new(),
        }
    }

    fn no_parents(_: &TicketKey) -> Option<TicketKey> {
        None
    }

    #[test]
    fn groups_counts_and_rows_follow_the_frame() {
        let mut m = InboxModel::default();
        m.update(sample(), Instant::now());
        assert_eq!(m.total(), 7);
        assert_eq!(
            (
                m.list().counts.of_project("SHOP"),
                m.list().counts.of_project("BILL")
            ),
            (6, 1),
            "header = sum of the rail badges"
        );
        let groups = m.groups(&no_parents);
        let shape: Vec<(&str, usize, Option<usize>, Vec<String>)> = groups
            .iter()
            .map(|g| {
                (
                    g.title,
                    g.count,
                    g.resume_all,
                    g.rows.iter().map(|r| r.key.to_string()).collect(),
                )
            })
            .collect();
        assert_eq!(
            shape,
            [
                (
                    "ASKS YOU · NEEDS YOU · IDLE · VALIDATION · AGENTS",
                    3,
                    None,
                    vec!["SHOP-22".into(), "SHOP-23".into(), "SHOP-12".into()]
                ),
                (
                    "INTERRUPTED & PARENT CLOSED",
                    2,
                    Some(1),
                    vec!["SHOP-25".into(), "SHOP-28".into()]
                ),
                (
                    "READY FOR REVIEW",
                    2,
                    None,
                    vec!["SHOP-18".into(), "BILL-5".into()]
                ),
            ]
        );
        let shop18 = &groups[2].rows[0];
        assert_eq!(
            (shop18.age.as_str(), shop18.blocks.as_deref(), shop18.tab),
            ("25 min", Some("blocks 2"), Tab::Diff)
        );
        assert_eq!(groups[1].rows[0].age, "3 h");
        assert_eq!(groups[1].rows[0].legend, Legend::Interrupted);
        assert_eq!(groups[1].rows[1].actions, [RowAction::RetryStart]);
        assert_eq!(groups[0].rows[0].legend, Legend::NeedsYou);
        assert_eq!(groups[0].rows[0].tab, Tab::Terminal);
        assert_eq!(groups[2].rows[1].project_name, "billing-api");
    }

    #[test]
    fn resume_all_lists_only_interrupted_sessions() {
        let mut m = InboxModel::default();
        m.update(sample(), Instant::now());
        let confirm = m.resume_all_confirm().unwrap();
        assert_eq!(confirm.title, "Resume 1 interrupted session?");
        assert_eq!(confirm.confirm, "Resume 1");
        assert_eq!(
            confirm.included,
            [(
                "SHOP-25".parse().unwrap(),
                "Upgrade to Next.js 15".to_owned()
            )]
        );
        assert_eq!(
            confirm.not_included.as_deref(),
            Some(
                "Not included: SHOP-28 Cart badge count. Its start never finished, so there’s no session to \
                 resume. Use Retry start on its card."
            )
        );
        assert_eq!(m.resume_all_keys().len(), 1);
        assert_eq!(
            confirm.body(),
            "Each session continues in its own worktree. This starts agents, so it uses tokens.\n\
             SHOP-25 Upgrade to Next.js 15\n\n\
             Not included: SHOP-28 Cart badge count. Its start never finished, so there’s no session to resume. \
             Use Retry start on its card."
        );
    }

    #[test]
    fn an_interrupted_question_keeps_its_place_and_label() {
        let mut list = sample();
        let shop23 = list
            .items
            .iter_mut()
            .find(|i| i.key.to_string() == "SHOP-23")
            .unwrap();
        shop23.status = Status::Interrupted;
        shop23.interrupted_from = Some(Status::AsksYou);
        let mut m = InboxModel::default();
        m.update(list, Instant::now());
        let groups = m.groups(&no_parents);
        let row = &groups[0].rows[1];
        assert_eq!(row.legend, Legend::AsksYou);
        assert_eq!(
            row.restart_note,
            Some("session ended by restart · answering resumes it")
        );
        assert_eq!(
            groups[1].resume_all,
            Some(2),
            "Resume all includes it (d0045, Q23)"
        );
    }

    #[test]
    fn parent_closed_offers_both_ways_forward() {
        let mut list = sample();
        list.items.push(item(
            "SHOP-30",
            "Sitemap for blog posts",
            Status::ParentClosed,
            NeedsYouGroup::Stopped,
            4 * MIN,
        ));
        let mut m = InboxModel::default();
        m.update(list, Instant::now());
        let parent: TicketKey = "SHOP-26".parse().unwrap();
        let p2 = parent.clone();
        let groups = m.groups(&move |k| (k.to_string() == "SHOP-30").then(|| p2.clone()));
        let row = groups[1]
            .rows
            .iter()
            .find(|r| r.key.to_string() == "SHOP-30")
            .unwrap();
        assert_eq!(
            row.actions,
            [
                RowAction::ReopenParent(parent.clone()),
                RowAction::RemoveDependency(parent)
            ]
        );
    }

    #[test]
    fn resolved_items_fade_out_without_a_jump() {
        let start = Instant::now();
        let mut m = InboxModel::default();
        m.update(sample(), start);
        let mut next = sample();
        next.items.retain(|i| i.key.to_string() != "SHOP-22");
        next.counts.total = 6;
        m.update(next, start + Duration::from_millis(100));
        assert_eq!(m.total(), 6, "counts update at once");
        let groups = m.groups(&no_parents);
        assert_eq!(groups[0].count, 2);
        assert_eq!(groups[0].rows.len(), 3, "the row stays while it fades");
        let gone = groups[0].rows.iter().find(|r| r.resolved).unwrap();
        assert_eq!(gone.key.to_string(), "SHOP-22");
        assert_eq!(gone.reason, "resolved elsewhere · just now");
        assert!(gone.actions.is_empty());
        assert!(!m.expire(start + Duration::from_secs(3)));
        assert!(m.expire(start + Duration::from_secs(5)));
        assert_eq!(m.groups(&no_parents)[0].rows.len(), 2);
        assert!(!m.is_fading());
    }

    #[test]
    fn ages() {
        assert_eq!(age(10_000), "now");
        assert_eq!(age(2 * MIN), "2 min");
        assert_eq!(age(60 * MIN), "1 h");
        assert_eq!(age(26 * 60 * MIN), "1 d");
    }

    #[test]
    fn empty_inbox_has_no_groups() {
        let m = InboxModel::default();
        assert!(m.groups(&no_parents).is_empty());
        assert_eq!(m.total(), 0);
        assert!(m.resume_all_confirm().is_none());
    }

    #[test]
    fn only_idle_items_offer_this_is_the_usage_limit() {
        let none = |_: &TicketKey| None;
        let actions = |status| {
            row(
                &item("SHOP-19", "Price table", status, NeedsYouGroup::Family, 0),
                false,
                &none,
            )
            .actions
        };
        assert_eq!(actions(Status::Idle), [RowAction::UsageLimit]);
        assert!(actions(Status::AsksYou).is_empty());
        assert!(actions(Status::NeedsYou).is_empty());
        let mut interrupted = item(
            "SHOP-19",
            "Price table",
            Status::Interrupted,
            NeedsYouGroup::Family,
            0,
        );
        interrupted.interrupted_from = Some(Status::Idle);
        assert!(row(&interrupted, false, &none).actions.is_empty());
    }

    #[test]
    fn the_header_button_is_quiet_when_nothing_waits() {
        assert_eq!(
            header_button(7),
            HeaderButton {
                label: "Inbox",
                count: 7,
                active: true
            }
        );
        assert_eq!(header_button(0).label, "Inbox");
        assert!(!header_button(0).active);
    }

    /// AGT-05, update Y §3: the Start failed item offers Change agent (not Retry start); a No result on the
    /// validator's model Change validator in place of Retry validation; other items as today.
    #[test]
    fn a_refused_model_offers_change_agent() {
        use ValidationAction as A;
        let actions = |status, group, change_agent| {
            let mut it = item("SHOP-48", "Add gift wrap option", status, group, MIN);
            it.change_agent = change_agent;
            row(&it, false, &|_| None).actions
        };
        assert_eq!(
            actions(
                Status::StartInterrupted,
                NeedsYouGroup::Stopped,
                Some(Role::Developer)
            ),
            [RowAction::ChangeAgent(Role::Developer)]
        );
        assert_eq!(
            actions(
                Status::ValidationNoResult,
                NeedsYouGroup::Family,
                Some(Role::Validator)
            ),
            [
                RowAction::ChangeAgent(Role::Validator),
                RowAction::Validation(A::MoveToReviewAnyway)
            ]
        );
        assert_eq!(
            actions(Status::StartInterrupted, NeedsYouGroup::Stopped, None),
            [RowAction::RetryStart]
        );
        assert_eq!(
            actions(Status::ValidationNoResult, NeedsYouGroup::Family, None),
            [
                RowAction::Validation(A::Retry),
                RowAction::Validation(A::MoveToReviewAnyway)
            ]
        );
    }

    /// USG-10: a held ticket adds no inbox item (the banner's tool line is the item, T-0364): the cards of every
    /// hold, held Developers in Ready for Dev and a held Validator in Dev, give no needs-you item.
    #[test]
    fn a_hold_adds_no_inbox_item() {
        use otaq_core::board::needs_you::{ProjectTickets, needs_you_items};
        let held: Vec<_> = crate::view_models::card_fixtures::tool_variants()
            .into_iter()
            .filter(|v| !v.extras.agents.holds.is_empty())
            .map(|v| v.ticket)
            .collect();
        assert_eq!(held.len(), 7);
        let items = needs_you_items(
            &[ProjectTickets {
                key: "SHOP",
                name: "shop-web",
                tickets: &held,
            }],
            0,
        );
        assert!(items.is_empty(), "{items:?}");
    }

    /// With several tools connected a refused model's row names its tool; other rows and resolved rows don't change.
    #[test]
    fn a_refused_models_row_names_its_tool() {
        let mut start = item(
            "SHOP-18",
            "Checkout API",
            Status::StartInterrupted,
            NeedsYouGroup::Stopped,
            MIN,
        );
        start.reason = "start failed \u{b7} couldn't run the model".into();
        start.change_agent = Some(Role::Developer);
        let mut validator = item(
            "SHOP-29",
            "Coupon field",
            Status::ValidationNoResult,
            NeedsYouGroup::Family,
            MIN,
        );
        validator.reason = "the validator couldn't run gpt-5.4: model not supported".into();
        validator.change_agent = Some(Role::Validator);
        let other = item(
            "SHOP-22",
            "Login",
            Status::NeedsYou,
            NeedsYouGroup::Family,
            MIN,
        );
        let mut groups = vec![InboxGroupView {
            group: NeedsYouGroup::Family,
            title: "",
            count: 3,
            resume_all: None,
            rows: [&start, &validator, &other]
                .iter()
                .map(|i| row(i, false, &|_| None))
                .collect(),
        }];
        name_tools(&mut groups, &|key, role| {
            Some(match (key.to_string().as_str(), role) {
                ("SHOP-18", Role::Developer) => AvatarTool::Codex,
                ("SHOP-29", Role::Validator) => AvatarTool::Copilot,
                _ => AvatarTool::ClaudeCode,
            })
        });
        let reasons: Vec<&str> = groups[0].rows.iter().map(|r| r.reason.as_str()).collect();
        assert_eq!(
            reasons,
            [
                "start failed \u{b7} Codex couldn\u{2019}t run the model",
                "the validator couldn't run gpt-5.4 on GitHub Copilot: model not supported",
                "reason of SHOP-22",
            ]
        );
    }
}
