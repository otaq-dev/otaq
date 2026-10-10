//! The "Needs you" inbox (UI-06, ATT-06, ATT-09, ATT-10; d0016, d0024; *10 · Needs-you inbox*): the service's
//! needs-you items across all projects in three groups with counts, Resume all on the interrupted group, and items
//! that resolved elsewhere fading out instead of jumping away.
//!
//! An item that is gone from a new list stays as a struck-through "resolved" row for [`FADE`], so the rows below
//! don't jump while the user reads them. Rows keep the service's order.

use std::time::{Duration, Instant};

use crewboard_core::attention::inbox::InboxList;
use crewboard_core::board::needs_you::{NeedsYouGroup, NeedsYouItem};
use crewboard_core::domain::{Status, TicketKey};

use crate::theme::Status as Legend;
use crate::view_models::answer::AnswerKind;
use crate::view_models::panel::Tab;
use crate::view_models::status::pill;

/// How long a resolved row stays before it goes.
pub const FADE: Duration = Duration::from_secs(4);

/// The group headings (frame *Inbox popover · mixed, 2 projects*).
pub fn group_title(group: NeedsYouGroup) -> &'static str {
    match group {
        NeedsYouGroup::Orange => "ASKS YOU · NEEDS YOU · IDLE",
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
    /// An orange ticket interrupted by a restart: "session ended by restart · answering resumes it".
    pub restart_note: Option<&'static str>,
    pub actions: Vec<RowAction>,
    /// Resolved elsewhere, fading out.
    pub resolved: bool,
    /// The panel tab the row opens: the Diff for a review, else the Terminal.
    pub tab: Tab,
    /// Asks you / Idle: answered in the row (ATT-08); an interrupted question resumes with it.
    pub answer: Option<AnswerKind>,
    /// The open question, with its answer options (ATT-12).
    pub question: Option<crewboard_core::board::questions::HumanQuestion>,
    /// An agent's question sent to the user (MSG-05): its ID; the answer is free text (d0061 scope).
    pub message: Option<String>,
    /// "Sent to you: finished tickets don't answer in this project".
    pub sent_note: Option<&'static str>,
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
            NeedsYouGroup::Orange,
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
        _ => Vec::new(),
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crewboard_core::attention::inbox::InboxCounts;

    const MIN: i64 = 60_000;

    /// *10 → Agent question for you · new item type* (MSG-05): free-text Answer and "Resume SHOP-17".
    #[test]
    fn an_agent_question_sent_to_you() {
        let mut q = item(
            "SHOP-21",
            "Order confirmation email",
            Status::Working,
            NeedsYouGroup::Orange,
            MIN,
        );
        q.id = "0199-q".into();
        q.reason = "asks SHOP-17 (Merged): which Stripe API version?".into();
        q.message = Some(crewboard_core::board::needs_you::QuestionItem {
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
        }
    }

    /// The frame's sample: SHOP-22, 23, 12 · SHOP-25, 28 · SHOP-18, BILL-5.
    fn sample() -> InboxList {
        use NeedsYouGroup::*;
        let items = vec![
            item(
                "SHOP-22",
                "Fix flaky login redirect",
                Status::NeedsYou,
                Orange,
                2 * MIN,
            ),
            item(
                "SHOP-23",
                "Split user full name",
                Status::AsksYou,
                Orange,
                6 * MIN,
            ),
            item(
                "SHOP-12",
                "Add dark mode toggle",
                Status::AsksYou,
                Orange,
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
                    "ASKS YOU · NEEDS YOU · IDLE",
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
                &item("SHOP-19", "Price table", status, NeedsYouGroup::Orange, 0),
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
            NeedsYouGroup::Orange,
            0,
        );
        interrupted.interrupted_from = Some(Status::Idle);
        assert!(row(&interrupted, false, &none).actions.is_empty());
    }
}
