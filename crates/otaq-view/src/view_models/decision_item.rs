//! Recorded decisions in the review (DEC-04, DEC-05; d0084, d0059, d0063; *07 → Row · Decision Item (0084)*, *Row ·
//! SHOP-23 review · Decisions (0084)*, *Row · SHOP-40 epic review · Decisions (0084)*): one Decision Item per
//! recorded decision (the question, Built, Other option, Why, the lead's mark, what the review did with it, and
//! Change… while it is open in review), and the Decisions section on the Diff tab, grouped by child for an epic.
//! No decisions → no section.

use crate::view_models::flow_columns::place_set;
use otaq_core::board::tickets::Ticket;
use otaq_core::decisions::{ChildDecisions, Decision, Outcome};
use otaq_core::domain::Status;
use otaq_core::domain::StatusSet;
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::clock::local_hhmm;

/// The field labels in front of each value (the page draws them in small capitals).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DecisionLabels")]
pub struct Labels {
    pub built: &'static str,
    pub other: &'static str,
    pub why: &'static str,
    pub change: &'static str,
}

pub const LABELS: Labels = Labels {
    built: "Built",
    other: "Other option",
    why: "Why",
    change: "Change\u{2026}",
};

/// The lead's mark: "Lead: contradicts PAY-123: “…”." with its source chip, violet, never red (History names the
/// lead: "Marked by Wishlist lead").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DecisionMarkView")]
pub struct MarkView {
    pub text: String,
    pub source: Option<String>,
}

/// What the review did with it: "Kept by you · Approve, 16:40", "Changed by you → sent back · “…”",
/// "Changed by you → SHOP-40.7".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DecisionStatusView")]
pub struct StatusView {
    pub text: String,
    pub icon: IconName,
    /// Kept: the icon takes the approved colour; a change stays quiet.
    pub kept: bool,
}

/// One Decision Item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DecisionItemView")]
pub struct DecisionItemView {
    /// `SHOP-23#1`, what Change… sends.
    pub id: String,
    pub question: String,
    pub built: String,
    /// One row per other option.
    pub other: Vec<String>,
    pub why: String,
    pub mark: Option<MarkView>,
    pub status: Option<StatusView>,
    /// Change… is offered: only in review, while the decision is open.
    pub change: bool,
    pub labels: Labels,
}

/// The head of an epic child's group: "SHOP-40.1 Wishlist API merged".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DecisionChildView")]
pub struct ChildView {
    pub key: String,
    pub title: String,
    /// "merged", "closed", or nothing.
    pub state: Option<&'static str>,
}

/// The items of one ticket: the ticket itself (`child` none) or one epic child.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DecisionGroupView")]
pub struct GroupView {
    pub child: Option<ChildView>,
    pub items: Vec<DecisionItemView>,
}

/// The Decisions section under the agent summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DecisionsSection")]
pub struct DecisionsSection {
    pub title: &'static str,
    pub count: u32,
    /// "Approve keeps them all · Change… sends it back" (or "… adds a fix child" for an epic); only in review.
    pub hint: Option<&'static str>,
    pub groups: Vec<GroupView>,
}

pub const HINT_TICKET: &str = "Approve keeps them all \u{b7} Change\u{2026} sends it back";
pub const HINT_EPIC: &str = "Approve keeps them all \u{b7} Change\u{2026} adds a fix child";

/// The review is open: a ticket in Ready for Review, or an epic in its Ready for Review column.
pub fn in_review(ticket: &Ticket) -> bool {
    if ticket.is_epic() {
        place_set(ticket) == StatusSet::HumanCheck
    } else {
        ticket.status == Status::WaitingForYou
    }
}

/// The section is shown once the ticket has reached its review: Ready for Review, Approved or Done.
fn reached_review(ticket: &Ticket) -> bool {
    matches!(
        place_set(ticket),
        StatusSet::HumanCheck | StatusSet::Delivery | StatusSet::Done
    )
}

/// The mark as the 07 frames draw it: "Lead: contradicts PAY-123: …" (the item sits in its epic's review).
pub fn mark_view(decision: &Decision) -> Option<MarkView> {
    decision.mark.as_ref().map(|m| MarkView {
        text: match &m.source {
            Some(source) => format!("Lead: contradicts {source}: {}", m.text),
            None => format!("Lead: {}", m.text),
        },
        source: m.source.clone(),
    })
}

pub fn status_view(decision: &Decision) -> Option<StatusView> {
    let at = decision.outcome_at.map(local_hhmm);
    match decision.outcome {
        Outcome::Open => None,
        Outcome::Kept => Some(StatusView {
            text: match at {
                Some(at) => format!("Kept by you \u{b7} Approve, {at}"),
                None => "Kept by you \u{b7} Approve".to_owned(),
            },
            icon: IconName::Check,
            kept: true,
        }),
        Outcome::ChangedSentBack => Some(StatusView {
            text: match decision.outcome_text.as_deref().map(str::trim) {
                Some(text) if !text.is_empty() => {
                    format!("Changed by you \u{2192} sent back \u{b7} \u{201c}{text}\u{201d}")
                }
                _ => "Changed by you \u{2192} sent back".to_owned(),
            },
            icon: IconName::CornerUpLeft,
            kept: false,
        }),
        Outcome::ChangedFixChild => Some(StatusView {
            text: match &decision.fix_child {
                Some(child) => format!("Changed by you \u{2192} {child}"),
                None => "Changed by you".to_owned(),
            },
            icon: IconName::Plus,
            kept: false,
        }),
    }
}

/// One item; `review` is whether the review is open now (Change… only then, and only on an open decision).
pub fn item(decision: &Decision, review: bool) -> DecisionItemView {
    DecisionItemView {
        id: decision.id.to_string(),
        question: decision.question.clone(),
        built: decision.built.clone(),
        other: decision.other_options.clone(),
        why: decision.why.clone(),
        mark: mark_view(decision),
        status: status_view(decision),
        change: review && decision.outcome == Outcome::Open,
        labels: LABELS,
    }
}

fn child_state(status: Option<Status>) -> Option<&'static str> {
    match status? {
        Status::Merged => Some("merged"),
        Status::ClosedNotMerged => Some("closed"),
        _ => None,
    }
}

/// The groups of `ticket`'s decisions: its own, and for an epic each child's with any (in child order, as
/// `decisions::list_for_epic` gives them; `tickets` names each child's status).
fn groups(
    ticket: &Ticket,
    own: &[Decision],
    children: &[ChildDecisions],
    tickets: &[Ticket],
    review: bool,
) -> Vec<GroupView> {
    let mut groups = Vec::new();
    if !own.is_empty() {
        groups.push(GroupView {
            child: None,
            items: own.iter().map(|d| item(d, review)).collect(),
        });
    }
    if ticket.is_epic() {
        for child in children.iter().filter(|c| !c.decisions.is_empty()) {
            let status = tickets
                .iter()
                .find(|t| t.key == child.key)
                .map(|t| t.status);
            groups.push(GroupView {
                child: Some(ChildView {
                    key: child.key.to_string(),
                    title: child.title.clone(),
                    state: child_state(status),
                }),
                items: child.decisions.iter().map(|d| item(d, review)).collect(),
            });
        }
    }
    groups
}

fn with_groups(groups: Vec<GroupView>, hint: Option<&'static str>) -> Option<DecisionsSection> {
    let count: usize = groups.iter().map(|g| g.items.len()).sum();
    (count > 0).then_some(DecisionsSection {
        title: "Decisions",
        count: count as u32,
        hint,
        groups,
    })
}

/// The section for `ticket` on the Diff tab: its own decisions, and for an epic every child's. `None` when there is
/// nothing to list or the ticket hasn't reached its review.
pub fn section(
    ticket: &Ticket,
    own: &[Decision],
    children: &[ChildDecisions],
    tickets: &[Ticket],
) -> Option<DecisionsSection> {
    if !reached_review(ticket) {
        return None;
    }
    let review = in_review(ticket);
    let hint = review.then_some(if ticket.is_epic() {
        HINT_EPIC
    } else {
        HINT_TICKET
    });
    with_groups(groups(ticket, own, children, tickets, review), hint)
}

/// The Ticket tab's hint until the review keeps or changes them (DEC-03).
pub const HINT_TICKET_TAB: &str = "recorded by the agent \u{b7} you keep or change them in review";

/// The read-only section under "What and why" in the Ticket tab (DEC-03; *05 → Row · Recorded decisions (0084)*):
/// while the ticket works and in review, with the hint and no Change… (that is the Diff tab's); after approval the
/// items with what the review did, no hint. `None` without decisions.
pub fn ticket_tab(
    ticket: &Ticket,
    own: &[Decision],
    children: &[ChildDecisions],
    tickets: &[Ticket],
) -> Option<DecisionsSection> {
    let decided = matches!(place_set(ticket), StatusSet::Delivery | StatusSet::Done);
    with_groups(
        groups(ticket, own, children, tickets, false),
        (!decided).then_some(HINT_TICKET_TAB),
    )
}

/// The open decisions Approve keeps: the ticket's own and, for an epic, every child's.
pub fn open_count(own: &[Decision], children: &[ChildDecisions]) -> u32 {
    let open = |d: &&Decision| d.outcome == Outcome::Open;
    let n = own.iter().filter(open).count()
        + children
            .iter()
            .flat_map(|c| c.decisions.iter())
            .filter(open)
            .count();
    n as u32
}

/// One Decision Item variant in the Gallery, named as in the master row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DecisionVariant")]
pub struct Variant {
    pub name: &'static str,
    pub item: DecisionItemView,
}

/// The Gallery's Decision Items: every variant of *Row · Decision Item (0084)*, and the SHOP-23 and SHOP-40 lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DecisionGallery")]
pub struct Gallery {
    pub variants: Vec<Variant>,
    pub ticket: DecisionsSection,
    pub epic: DecisionsSection,
}

/// The Gallery's fixtures (`gallery.decisions`), 16:40 as the frames' approval time.
pub fn gallery() -> Gallery {
    use crate::view_models::card_fixtures::ticket;
    use fixtures::*;
    use otaq_core::domain::Kind;
    let at = |outcome: Outcome, d: &mut Decision| {
        d.outcome = outcome;
        d.outcome_at = otaq_core::local_time::on_same_day_at(0, 16, 40);
    };
    let mut kept = full_name();
    at(Outcome::Kept, &mut kept);
    let mut sent = full_name();
    at(Outcome::ChangedSentBack, &mut sent);
    sent.outcome_text = Some("Split fullName everywhere".into());
    let mut fixed = order();
    at(Outcome::ChangedFixChild, &mut fixed);
    fixed.fix_child = Some("SHOP-40.7".parse().expect("key"));
    let variants = vec![
        Variant {
            name: "Decision Item",
            item: item(&full_name(), true),
        },
        Variant {
            name: "With the lead's mark (violet, never red)",
            item: item(&order(), true),
        },
        Variant {
            name: "Read-only while the ticket works (no Change\u{2026})",
            item: item(&full_name(), false),
        },
        Variant {
            name: "After Approve",
            item: item(&kept, false),
        },
        Variant {
            name: "After Change\u{2026} on a ticket",
            item: item(&sent, false),
        },
        Variant {
            name: "After Change\u{2026} in an epic review",
            item: item(&fixed, false),
        },
    ];
    let review = |key: &str, title: &str, kind: Kind| {
        let mut t = ticket(key, title, Status::WaitingForYou);
        t.kind = kind;
        t.place = otaq_core::domain::Place::stage_of("review");
        t.status = if kind == Kind::Epic {
            Status::Backlog
        } else {
            Status::WaitingForYou
        };
        t
    };
    let merged: Vec<Ticket> = ["SHOP-40.1", "SHOP-40.3", "SHOP-40.4"]
        .into_iter()
        .map(|k| ticket(k, "", Status::Merged))
        .collect();
    Gallery {
        variants,
        ticket: section(
            &review("SHOP-23", "Split user full name", Kind::Ticket),
            &[full_name()],
            &[],
            &[],
        )
        .expect("one decision"),
        epic: section(
            &review("SHOP-40", "Wishlist", Kind::Epic),
            &[],
            &wishlist(),
            &merged,
        )
        .expect("three decisions"),
    }
}

/// Hand-written decisions for tests and the Gallery: SHOP-23's fullName choice and SHOP-40's three children.
pub mod fixtures {
    use otaq_core::decisions::{ChildDecisions, Decision, DecisionId, Mark, Outcome};
    use otaq_core::domain::Actor;

    pub fn decision(id: &str, question: &str, built: &str, other: &str, why: &str) -> Decision {
        let id: DecisionId = id.parse().expect("a decision id");
        Decision {
            row_id: format!("row-{id}"),
            ticket_id: format!("ticket-{}", id.ticket),
            question: question.into(),
            built: built.into(),
            other_options: vec![other.into()],
            why: why.into(),
            recorded_by: Actor::Agent(id.ticket.clone()),
            recorded_at: 0,
            mark: None,
            outcome: Outcome::Open,
            outcome_text: None,
            fix_child: None,
            outcome_at: None,
            id,
        }
    }

    pub fn full_name() -> Decision {
        decision(
            "SHOP-23#1",
            "Split fullName everywhere, or only in the profile form?",
            "only in the profile form",
            "everywhere",
            "the description only mentions the profile form.",
        )
    }

    pub fn order() -> Decision {
        let mut d = decision(
            "SHOP-40.1#1",
            "Order of saved items?",
            "newest first",
            "in the order they were added",
            "matches the order history page.",
        );
        d.mark = Some(Mark {
            text: "\u{201c}items are listed in the order they were added\u{201d}.".into(),
            source: Some("PAY-123".into()),
            by: Actor::ProjectAgent("lead".into()),
            at: 0,
        });
        d
    }

    pub fn wishlist() -> Vec<ChildDecisions> {
        vec![
            ChildDecisions {
                key: "SHOP-40.1".parse().unwrap(),
                title: "Wishlist API".into(),
                decisions: vec![order()],
            },
            ChildDecisions {
                key: "SHOP-40.3".parse().unwrap(),
                title: "Wishlist page".into(),
                decisions: vec![decision(
                    "SHOP-40.3#1",
                    "What does an empty wishlist show?",
                    "\u{201c}Your wishlist is empty\u{201d} and a link to the shop",
                    "product suggestions",
                    "there is no suggestions API yet.",
                )],
            },
            ChildDecisions {
                key: "SHOP-40.4".parse().unwrap(),
                title: "Wishlist share link".into(),
                decisions: vec![decision(
                    "SHOP-40.4#1",
                    "Can anyone with the link see the list?",
                    "yes, read-only",
                    "only signed-in users",
                    "PAY-123 says \u{201c}share with friends\u{201d}, with no mention of signing in.",
                )],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use otaq_core::domain::Kind;

    fn review(key: &str, title: &str) -> Ticket {
        let mut t = ticket(key, title, Status::WaitingForYou);
        t.place = otaq_core::domain::Place::stage_of("review");
        t
    }

    fn epic_in_review() -> (Ticket, Vec<Ticket>) {
        let mut epic = review("SHOP-40", "Wishlist");
        epic.kind = Kind::Epic;
        epic.status = Status::Backlog;
        let children = ["SHOP-40.1", "SHOP-40.3", "SHOP-40.4"]
            .into_iter()
            .map(|k| {
                let mut c = ticket(k, "child", Status::Merged);
                c.place = otaq_core::domain::Place::Done;
                c
            })
            .collect();
        (epic, children)
    }

    #[test]
    fn no_decisions_means_no_section() {
        let shop18 = review("SHOP-18", "Checkout API");
        assert_eq!(section(&shop18, &[], &[], &[]), None);
        let (epic, children) = epic_in_review();
        let empty = vec![ChildDecisions {
            key: "SHOP-40.2".parse().unwrap(),
            title: "Wishlist button".into(),
            decisions: vec![],
        }];
        assert_eq!(section(&epic, &[], &empty, &children), None);
    }

    #[test]
    fn a_ticket_in_review_lists_its_decisions_with_change() {
        let shop23 = review("SHOP-23", "Split user full name");
        let s = section(&shop23, &[full_name()], &[], &[]).unwrap();
        assert_eq!(s.title, "Decisions");
        assert_eq!(s.count, 1);
        assert_eq!(s.hint, Some(HINT_TICKET));
        assert_eq!(
            s.hint,
            Some("Approve keeps them all \u{b7} Change\u{2026} sends it back")
        );
        assert_eq!(s.groups.len(), 1);
        assert_eq!(s.groups[0].child, None);
        let item = &s.groups[0].items[0];
        assert_eq!(item.id, "SHOP-23#1");
        assert_eq!(item.built, "only in the profile form");
        assert_eq!(item.other, ["everywhere"]);
        assert!(item.change);
        assert_eq!((item.mark.clone(), item.status.clone()), (None, None));
    }

    #[test]
    fn an_epic_groups_by_child_with_the_lead_mark() {
        let (epic, children) = epic_in_review();
        let s = section(&epic, &[], &wishlist(), &children).unwrap();
        assert_eq!(s.count, 3);
        assert_eq!(
            s.hint,
            Some("Approve keeps them all \u{b7} Change\u{2026} adds a fix child")
        );
        let heads: Vec<_> = s
            .groups
            .iter()
            .map(|g| g.child.clone().unwrap())
            .map(|c| (c.key, c.title, c.state))
            .collect();
        assert_eq!(
            heads,
            [
                ("SHOP-40.1".into(), "Wishlist API".into(), Some("merged")),
                ("SHOP-40.3".into(), "Wishlist page".into(), Some("merged")),
                (
                    "SHOP-40.4".into(),
                    "Wishlist share link".into(),
                    Some("merged")
                ),
            ]
        );
        let mark = s.groups[0].items[0].mark.clone().unwrap();
        assert_eq!(
            mark.text,
            "Lead: contradicts PAY-123: \u{201c}items are listed in the order they were added\u{201d}."
        );
        assert_eq!(mark.source.as_deref(), Some("PAY-123"));
        assert!(s.groups.iter().all(|g| g.items.iter().all(|i| i.change)));
        assert_eq!(open_count(&[], &wishlist()), 3);
    }

    #[test]
    fn change_only_in_review_and_while_open() {
        let mut shop23 = review("SHOP-23", "Split user full name");
        assert!(item(&full_name(), in_review(&shop23)).change);
        let mut kept = full_name();
        kept.outcome = Outcome::Kept;
        assert!(!item(&kept, in_review(&shop23)).change, "already kept");
        shop23.status = Status::Working;
        shop23.place = otaq_core::domain::Place::stage_of("dev");
        assert!(!in_review(&shop23));
        assert!(!item(&full_name(), in_review(&shop23)).change);
        assert_eq!(
            section(&shop23, &[full_name()], &[], &[]),
            None,
            "while it works the Ticket tab lists them (T-0288)"
        );
        shop23.status = Status::Merged;
        shop23.place = otaq_core::domain::Place::Done;
        let after = section(&shop23, &[kept], &[], &[]).unwrap();
        assert_eq!(after.hint, None, "no hint after approval");
        assert!(!after.groups[0].items[0].change);
    }

    #[test]
    fn each_outcome_has_its_status() {
        let mut d = full_name();
        assert_eq!(status_view(&d), None);
        d.outcome = Outcome::Kept;
        let kept = status_view(&d).unwrap();
        assert_eq!(kept.text, "Kept by you \u{b7} Approve");
        assert_eq!((kept.icon, kept.kept), (IconName::Check, true));
        d.outcome_at = Some(0);
        assert_eq!(
            status_view(&d).unwrap().text,
            format!("Kept by you \u{b7} Approve, {}", local_hhmm(0))
        );
        d.outcome = Outcome::ChangedSentBack;
        d.outcome_text = Some("Split fullName everywhere".into());
        let sent = status_view(&d).unwrap();
        assert_eq!(
            sent.text,
            "Changed by you \u{2192} sent back \u{b7} \u{201c}Split fullName everywhere\u{201d}"
        );
        assert_eq!((sent.icon, sent.kept), (IconName::CornerUpLeft, false));
        let mut o = order();
        o.outcome = Outcome::ChangedFixChild;
        o.fix_child = Some("SHOP-40.7".parse().unwrap());
        let fix = status_view(&o).unwrap();
        assert_eq!(fix.text, "Changed by you \u{2192} SHOP-40.7");
        assert_eq!(fix.icon, IconName::Plus);
    }

    #[test]
    fn the_gallery_has_every_variant_of_the_master_row() {
        let g = gallery();
        assert_eq!(g.variants.len(), 6);
        let changes: Vec<bool> = g.variants.iter().map(|v| v.item.change).collect();
        assert_eq!(changes, [true, true, false, false, false, false]);
        assert!(g.variants[1].item.mark.is_some());
        assert_eq!(
            g.variants[3].item.status.as_ref().unwrap().text,
            "Kept by you \u{b7} Approve, 16:40"
        );
        assert_eq!(
            g.variants[5].item.status.as_ref().unwrap().text,
            "Changed by you \u{2192} SHOP-40.7"
        );
        assert_eq!((g.ticket.count, g.epic.count), (1, 3));
        assert!(
            g.epic.groups[0].items[0]
                .mark
                .as_ref()
                .unwrap()
                .text
                .starts_with("Lead: contradicts PAY-123")
        );
        assert_eq!(g.epic.groups.len(), 3);
    }

    #[test]
    fn the_mark_reads_lead_as_the_07_frame_never_the_actor() {
        let mut child = review("SHOP-40.1", "Wishlist API");
        child.epic = Some("SHOP-40".parse().unwrap());
        let s = section(&child, &[order()], &[], &[]).unwrap();
        let text = &s.groups[0].items[0].mark.as_ref().unwrap().text;
        assert_eq!(
            text,
            "Lead: contradicts PAY-123: \u{201c}items are listed in the order they were added\u{201d}."
        );
        assert!(!text.contains("project-agent"));
    }

    #[test]
    fn the_ticket_tab_lists_them_read_only() {
        let mut shop23 = review("SHOP-23", "Split user full name");
        shop23.status = Status::Working;
        shop23.place = otaq_core::domain::Place::stage_of("dev");
        let s = ticket_tab(&shop23, &[full_name()], &[], &[]).unwrap();
        assert_eq!((s.title, s.count), ("Decisions", 1));
        assert_eq!(
            s.hint,
            Some("recorded by the agent \u{b7} you keep or change them in review")
        );
        assert!(
            !s.groups[0].items[0].change,
            "no Change\u{2026} while it works"
        );
        assert_eq!(s.groups[0].items[0].status, None);
        // In review the Ticket tab stays read-only: Change… is on the Diff tab.
        shop23.status = Status::WaitingForYou;
        shop23.place = otaq_core::domain::Place::stage_of("review");
        let s = ticket_tab(&shop23, &[full_name()], &[], &[]).unwrap();
        assert!(!s.groups[0].items[0].change);
        // After approval: the items with their status, no hint.
        shop23.status = Status::Merged;
        shop23.place = otaq_core::domain::Place::Done;
        let mut kept = full_name();
        kept.outcome = Outcome::Kept;
        let s = ticket_tab(&shop23, &[kept], &[], &[]).unwrap();
        assert_eq!(s.hint, None);
        assert!(s.groups[0].items[0].status.as_ref().unwrap().kept);
        assert_eq!(ticket_tab(&shop23, &[], &[], &[]), None, "none: no section");
    }

    #[test]
    fn a_mark_without_a_source_has_no_chip() {
        let mut d = order();
        d.mark.as_mut().unwrap().source = None;
        d.mark.as_mut().unwrap().text = "the page sorts the other way".into();
        let mark = mark_view(&d).unwrap();
        assert_eq!(mark.text, "Lead: the page sorts the other way");
        assert_eq!(mark.source, None);
    }
}
