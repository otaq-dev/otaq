//! Change… on a recorded decision (DEC-06; d0084, d0034, d0080; *07 → Change… on SHOP-23*, *Change… on 40.1's
//! decision: add a fix child*). On a ticket in review, "Change this decision" sends it back to its agent with the
//! other option (editable), pending line comments included. In an epic's review the child is already merged, so
//! "Add a fix child" creates a new child of the epic, prefilled from the decision and the lead's mark. Which dialog
//! opens, and every word in it, is decided here.

use otaq_core::board::tickets::Ticket;
use otaq_core::decisions::Decision;
use otaq_core::decisions::outcome::{fix_child_description, fix_child_title};
use otaq_core::domain::{Status, TicketKey};
use serde::Serialize;
use ts_rs::TS;

use crate::view_models::decision_item::{MarkView, mark_view};

/// "Change this decision": Send back with the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DecisionSendBackView")]
pub struct SendBackChange {
    /// `SHOP-23#1`, what `decision.change` takes.
    pub id: String,
    /// The ticket that goes back to Dev.
    pub key: String,
    pub title: &'static str,
    pub question: String,
    /// "Built: only in the profile form · Other option: everywhere".
    pub summary: String,
    pub label: &'static str,
    /// The first other option, editable.
    pub text: String,
    pub help: &'static str,
    /// "This is a Send back (0034): SHOP-23 goes back to Dev with …".
    pub note: String,
    /// The pending line comments that go with it, for the unsent-text question's wording.
    pub comments: u32,
    pub confirm: &'static str,
    pub cancel: &'static str,
    pub hint: &'static str,
}

/// "Add a fix child": a new child of the epic, added by the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DecisionFixChildView")]
pub struct FixChildChange {
    pub id: String,
    pub epic: String,
    pub title: &'static str,
    pub question: String,
    pub summary: String,
    /// The lead's mark, violet as on the Decision Item.
    pub mark: Option<MarkView>,
    /// "NEW CHILD OF SHOP-40 · TITLE".
    pub title_label: String,
    pub child_title: String,
    pub child_description: String,
    pub description_label: &'static str,
    pub help: &'static str,
    /// "Creates SHOP-40.7, added by you. The epic becomes a track again …".
    pub note: String,
    pub confirm: &'static str,
    pub cancel: &'static str,
    pub hint: &'static str,
}

/// The dialog Change… opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "DecisionChangeView")]
pub enum ChangeView {
    SendBack(SendBackChange),
    FixChild(FixChildChange),
}

pub const SEND_BACK_TITLE: &str = "Change this decision";
pub const FIX_CHILD_TITLE: &str = "Add a fix child";

/// "Built: newest first · Other option: in the order they were added" ("Other options: a · b" for several).
pub fn summary(decision: &Decision) -> String {
    let mut out = format!("Built: {}", decision.built.trim());
    match decision.other_options.as_slice() {
        [] => {}
        [one] => out.push_str(&format!(" \u{b7} Other option: {}", one.trim())),
        many => out.push_str(&format!(
            " \u{b7} Other options: {}",
            many.iter()
                .map(|o| o.trim())
                .collect::<Vec<_>>()
                .join(" \u{b7} ")
        )),
    }
    out
}

/// The key the fix child will take: the epic's next child number.
fn next_child(epic: &Ticket) -> Option<TicketKey> {
    let child = epic.next_child_number?;
    Some(TicketKey {
        child: Some(child),
        ..epic.key.clone()
    })
}

/// A merged epic child's decision gets a fix child; any other ticket's goes back to its agent.
fn fix_child_epic<'a>(ticket: &Ticket, tickets: &'a [Ticket]) -> Option<&'a Ticket> {
    let epic = ticket.epic.as_ref()?;
    (ticket.status == Status::Merged)
        .then(|| tickets.iter().find(|t| &t.key == epic))
        .flatten()
}

/// The dialog for `decision` of `ticket`. `tickets` is the project (the epic, for a merged child); `comments` the
/// pending line comments that go with a Send back; `has_lead` whether the epic has a lead to resume.
pub fn view(
    decision: &Decision,
    ticket: &Ticket,
    tickets: &[Ticket],
    comments: u32,
    has_lead: bool,
) -> ChangeView {
    if let Some(epic) = fix_child_epic(ticket, tickets) {
        return ChangeView::FixChild(fix_child(decision, epic, has_lead));
    }
    ChangeView::SendBack(SendBackChange {
        id: decision.id.to_string(),
        key: ticket.key.to_string(),
        title: SEND_BACK_TITLE,
        question: decision.question.clone(),
        summary: summary(decision),
        label: "SEND BACK TO THE AGENT WITH",
        text: decision
            .other_options
            .first()
            .map(|o| o.trim().to_owned())
            .unwrap_or_default(),
        help: "Prefilled with the other option. Edit it or write your own.",
        note: format!(
            "This is a Send back (0034): {} goes back to Dev with this message and any line comments you\u{2019}ve \
             written, and its agent continues in the same session. Its next submit is validated again.",
            ticket.key
        ),
        comments,
        confirm: "Send back to the agent",
        cancel: "Cancel",
        hint: "cancel",
    })
}

fn fix_child(decision: &Decision, epic: &Ticket, has_lead: bool) -> FixChildChange {
    let created = match next_child(epic) {
        Some(key) => {
            let short = key
                .child
                .map(|c| format!("{}.{c}", key.number))
                .unwrap_or_default();
            format!(
                "Creates {key}, added by you. The epic becomes a track again and {short} joins the queue (0080)"
            )
        }
        None => "Creates a new child, added by you. The epic becomes a track again and it joins the queue (0080)"
            .to_owned(),
    };
    let lead = if has_lead {
        "; its lead resumes the same conversation."
    } else {
        "."
    };
    FixChildChange {
        id: decision.id.to_string(),
        epic: epic.key.to_string(),
        title: FIX_CHILD_TITLE,
        question: decision.question.clone(),
        summary: summary(decision),
        mark: mark_view(decision),
        title_label: format!("NEW CHILD OF {} \u{b7} TITLE", epic.key),
        child_title: fix_child_title(decision),
        child_description: fix_child_description(decision),
        description_label: "Description",
        help: "Prefilled from the decision; edit both before adding.",
        note: format!("{created}{lead}"),
        confirm: "Add fix child",
        cancel: "Cancel",
        hint: "cancel",
    }
}

/// The Gallery's two dialogs (`gallery.decision_change`): SHOP-23's Send back and SHOP-40's fix child with the
/// lead's mark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DecisionChangeGallery")]
pub struct Gallery {
    pub send_back: ChangeView,
    pub fix_child: ChangeView,
}

pub fn gallery() -> Gallery {
    let (shop23, _) = fixtures::shop23();
    let (child, tickets) = fixtures::shop40();
    Gallery {
        send_back: view(
            &crate::view_models::decision_item::fixtures::full_name(),
            &shop23,
            &[],
            0,
            false,
        ),
        fix_child: view(
            &crate::view_models::decision_item::fixtures::order(),
            &child,
            &tickets,
            0,
            true,
        ),
    }
}

/// SHOP-23 in review, and SHOP-40 in review with its merged child 40.1 (next child: 40.7).
pub mod fixtures {
    use otaq_core::board::tickets::Ticket;
    use otaq_core::domain::{Column, Kind, Status};

    use crate::view_models::card_fixtures::ticket;

    pub fn shop23() -> (Ticket, Vec<Ticket>) {
        let mut t = ticket("SHOP-23", "Split user full name", Status::ReadyForReview);
        t.column = Column::ReadyForReview;
        (t.clone(), vec![t])
    }

    /// (SHOP-40.1, the project).
    pub fn shop40() -> (Ticket, Vec<Ticket>) {
        let mut epic = ticket("SHOP-40", "Wishlist", Status::ReadyForReview);
        epic.kind = Kind::Epic;
        epic.column = Column::ReadyForReview;
        epic.next_child_number = Some(7);
        let mut child = ticket("SHOP-40.1", "Wishlist API", Status::Merged);
        child.column = Column::Done;
        child.epic = Some(epic.key.clone());
        (child.clone(), vec![epic, child])
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use crate::view_models::decision_item::fixtures::{full_name, order};

    fn send_back(v: ChangeView) -> SendBackChange {
        match v {
            ChangeView::SendBack(s) => s,
            other => panic!("{other:?}"),
        }
    }

    fn fix(v: ChangeView) -> FixChildChange {
        match v {
            ChangeView::FixChild(f) => f,
            other => panic!("{other:?}"),
        }
    }

    /// *07 → Change… on SHOP-23*.
    #[test]
    fn change_on_a_ticket_is_prefilled_with_the_other_option() {
        let (shop23, tickets) = shop23();
        let v = send_back(view(&full_name(), &shop23, &tickets, 2, false));
        assert_eq!(v.id, "SHOP-23#1");
        assert_eq!(v.key, "SHOP-23");
        assert_eq!(v.title, "Change this decision");
        assert_eq!(
            v.question,
            "Split fullName everywhere, or only in the profile form?"
        );
        assert_eq!(
            v.summary,
            "Built: only in the profile form \u{b7} Other option: everywhere"
        );
        assert_eq!(v.label, "SEND BACK TO THE AGENT WITH");
        assert_eq!(v.text, "everywhere");
        assert_eq!(
            v.help,
            "Prefilled with the other option. Edit it or write your own."
        );
        assert!(
            v.note
                .starts_with("This is a Send back (0034): SHOP-23 goes back to Dev"),
            "{}",
            v.note
        );
        assert_eq!(v.comments, 2);
        assert_eq!(v.confirm, "Send back to the agent");
    }

    /// *07 → Change… on 40.1's decision: add a fix child*, with the lead's mark.
    #[test]
    fn change_in_an_epic_review_adds_a_fix_child_with_the_mark() {
        let (child, tickets) = shop40();
        let v = fix(view(&order(), &child, &tickets, 0, true));
        assert_eq!(v.id, "SHOP-40.1#1");
        assert_eq!(v.epic, "SHOP-40");
        assert_eq!(v.title, "Add a fix child");
        assert_eq!(v.question, "Order of saved items?");
        assert_eq!(
            v.summary,
            "Built: newest first \u{b7} Other option: in the order they were added"
        );
        let mark = v.mark.unwrap();
        assert!(mark.text.starts_with("Lead: contradicts PAY-123: "));
        assert_eq!(mark.source.as_deref(), Some("PAY-123"));
        assert_eq!(v.title_label, "NEW CHILD OF SHOP-40 \u{b7} TITLE");
        assert_eq!(v.child_title, "Change: in the order they were added");
        assert!(
            v.child_description.starts_with(
                "SHOP-40.1 built newest first for \u{201c}Order of saved items?\u{201d}"
            ),
            "{}",
            v.child_description
        );
        assert!(v.child_description.contains("Lead: "), "the mark goes in");
        assert_eq!(
            v.note,
            "Creates SHOP-40.7, added by you. The epic becomes a track again and 40.7 joins the queue (0080); its \
             lead resumes the same conversation."
        );
        assert_eq!(v.confirm, "Add fix child");
    }

    #[test]
    fn a_fix_child_without_a_mark_or_a_lead() {
        let (child, tickets) = shop40();
        let mut d = order();
        d.mark = None;
        let v = fix(view(&d, &child, &tickets, 0, false));
        assert_eq!(v.mark, None);
        assert!(!v.child_description.contains("Lead"));
        assert!(
            v.child_description
                .ends_with("Change it to: in the order they were added."),
            "{}",
            v.child_description
        );
        assert!(v.note.ends_with("joins the queue (0080)."), "{}", v.note);
    }

    #[test]
    fn a_child_still_in_review_goes_back_to_its_agent() {
        let (mut child, tickets) = shop40();
        child.status = Status::ReadyForReview;
        assert!(matches!(
            view(&order(), &child, &tickets, 0, true),
            ChangeView::SendBack(_)
        ));
    }

    #[test]
    fn several_other_options_are_listed() {
        let mut d = full_name();
        d.other_options = vec!["everywhere".into(), "nowhere".into()];
        assert_eq!(
            summary(&d),
            "Built: only in the profile form \u{b7} Other options: everywhere \u{b7} nowhere"
        );
    }

    #[test]
    fn the_gallery_has_both_dialogs() {
        let g = gallery();
        assert!(matches!(g.send_back, ChangeView::SendBack(_)));
        assert!(matches!(g.fix_child, ChangeView::FixChild(_)));
    }
}
