//! Close ticket, Reopen and Submit for review in the UI (CLS-01, CLS-03, CLS-05; d0033, d0044, d0052; *05 → Close
//! ticket · panel "…" menu, Close ticket · confirm, Submit for review · dialog, Closed · not merged · Reopen*):
//! which menu items show, and the dialogs' texts.

use crewboard_core::board::review::USER_SUBMITS_FROM;
use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Column, Status};

use crate::icons::IconName;
use crate::view_models::menu::MenuItem;

pub const CLOSE_TICKET: &str = "close-ticket";
pub const SUBMIT_FOR_REVIEW: &str = "submit-for-review";

/// Close ticket is offered on every ticket that isn't finished (epics have Close epic…).
pub fn can_close(ticket: &Ticket) -> bool {
    !ticket.is_epic() && !matches!(ticket.status, Status::Merged | Status::ClosedNotMerged)
}

/// "Close ticket…" in the panel's "…" menu and the card's right-click menu.
pub fn close_item(ticket: &Ticket) -> Option<MenuItem> {
    can_close(ticket).then(|| {
        MenuItem::new(CLOSE_TICKET, "Close ticket…")
            .icon(IconName::CircleSlash)
            .danger()
            .separated()
    })
}

/// "Submit for review…": on a ticket whose agent is at work or stopped; enabled only when it isn't mid-turn.
pub fn submit_item(ticket: &Ticket) -> Option<MenuItem> {
    let shown = !ticket.is_epic()
        && ticket.column == Column::Dev
        && (USER_SUBMITS_FROM.contains(&ticket.status)
            || matches!(ticket.status, Status::Working | Status::WorkingInBackground));
    if !shown {
        return None;
    }
    let item = MenuItem::new(SUBMIT_FOR_REVIEW, "Submit for review…").icon(IconName::Send);
    Some(if USER_SUBMITS_FROM.contains(&ticket.status) {
        item
    } else {
        item.disabled(true).hint("agent is working")
    })
}

/// The close confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseText {
    pub title: String,
    pub lines: Vec<String>,
    pub footer: String,
    pub confirm: String,
}

pub fn close_text(ticket: &Ticket, tickets: &[Ticket], agent_running: bool) -> CloseText {
    let mut lines = Vec::new();
    if agent_running {
        lines.push("Its agent is running. Closing stops it; the session is not kept.".to_owned());
    }
    let mut dependents: Vec<&Ticket> = tickets
        .iter()
        .filter(|t| t.depends_on.contains(&ticket.key) && t.column != Column::Done)
        .collect();
    dependents.sort_by(|a, b| a.key.cmp(&b.key));
    for d in dependents {
        lines.push(format!(
            "{} {} depends on it. It stays Queued with \u{201c}parent {} closed\u{201d}, and an inbox item offers \
             Reopen parent or Remove dependency.",
            d.key, d.title, ticket.key
        ));
    }
    CloseText {
        title: format!("Close {} {} without merging?", ticket.key, ticket.title),
        lines,
        footer: "The branch is kept. The worktree is removed unless it has uncommitted changes (then it\u{2019}s \
                 listed in Cleanup). You can reopen the ticket later."
            .to_owned(),
        confirm: if agent_running {
            "Stop agent & close".to_owned()
        } else {
            "Close ticket".to_owned()
        },
    }
}

/// The submit dialog's title and texts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitText {
    pub title: String,
    pub intro: String,
    pub footer: String,
}

pub fn submit_text(ticket: &Ticket, now: i64) -> SubmitText {
    let since = ticket
        .signal
        .as_ref()
        .and_then(|s| s.since)
        .or(ticket.status_since);
    let intro = match (ticket.status, since) {
        (Status::Interrupted, _) => {
            "Its session was interrupted. Send back resumes it later.".to_owned()
        }
        (_, Some(at)) => format!(
            "The agent ended its turn {} ago. Its session stays; Send back continues it.",
            crate::view_models::card::age_label(now - at)
        ),
        (_, None) => {
            "The agent isn't working right now. Its session stays; Send back continues it."
                .to_owned()
        }
    };
    SubmitText {
        title: format!("Submit {} {} for review?", ticket.key, ticket.title),
        intro,
        footer: format!(
            "{} runs no checks. The Review card shows \u{201c}Submitted by you\u{201d} with your note instead of the \
             agent\u{2019}s summary.",
            crewboard_core::naming::PRODUCT_NAME
        ),
    }
}

/// The card's line: "Submitted by you · “Paging works, tests pending”".
pub fn submitted_line(note: &str) -> String {
    let note = note.lines().next().unwrap_or_default().trim();
    if note.is_empty() {
        "Submitted by you".to_owned()
    } else {
        format!("Submitted by you \u{b7} \u{201c}{note}\u{201d}")
    }
}

/// The Park confirmation (*03 → Park · Dev → Backlog*): what stops and what is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParkText {
    pub title: String,
    pub message: String,
    pub kept: Vec<String>,
}

pub fn park_text(ticket: &Ticket) -> ParkText {
    let branch = ticket
        .work
        .as_ref()
        .and_then(|w| w.branch.clone())
        .unwrap_or_else(|| crewboard_core::naming::branch(&ticket.key.to_string()));
    ParkText {
        title: format!("Park {}?", ticket.key),
        message: crewboard_core::review::park::park_message(&ticket.key.to_string()),
        kept: vec![
            branch,
            "worktree \u{b7} kept with its files".to_owned(),
            "conversation \u{b7} resumes on Dev".to_owned(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;

    #[test]
    fn close_is_offered_until_the_ticket_is_finished() {
        for status in [
            Status::Backlog,
            Status::Working,
            Status::ReadyForReview,
            Status::PrOpen,
        ] {
            assert!(
                close_item(&ticket("SHOP-19", "t", status)).is_some(),
                "{status}"
            );
        }
        assert!(close_item(&ticket("SHOP-19", "t", Status::Merged)).is_none());
        assert!(close_item(&ticket("SHOP-19", "t", Status::ClosedNotMerged)).is_none());
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = crewboard_core::domain::Kind::Epic;
        assert!(close_item(&epic).is_none(), "epics use Close epic…");
    }

    #[test]
    fn submit_is_enabled_only_when_the_agent_isnt_mid_turn() {
        for status in [
            Status::Idle,
            Status::AsksYou,
            Status::NeedsYou,
            Status::Interrupted,
        ] {
            let item = submit_item(&ticket("SHOP-19", "t", status)).unwrap();
            assert!(!item.disabled, "{status}");
        }
        let working = submit_item(&ticket("SHOP-19", "t", Status::Working)).unwrap();
        assert!(working.disabled);
        assert_eq!(working.hint.as_deref(), Some("agent is working"));
        assert!(submit_item(&ticket("SHOP-19", "t", Status::Backlog)).is_none());
        assert!(submit_item(&ticket("SHOP-19", "t", Status::ReadyForReview)).is_none());
    }

    #[test]
    fn the_close_confirmation_names_the_agent_and_the_children() {
        let shop19 = ticket("SHOP-19", "Price table", Status::Working);
        let mut shop20 = ticket("SHOP-20", "Checkout page", Status::Queued);
        shop20.depends_on = vec![shop19.key.clone()];
        let text = close_text(&shop19, &[shop19.clone(), shop20], true);
        assert_eq!(text.title, "Close SHOP-19 Price table without merging?");
        assert_eq!(
            text.lines,
            [
                "Its agent is running. Closing stops it; the session is not kept.".to_owned(),
                "SHOP-20 Checkout page depends on it. It stays Queued with \u{201c}parent SHOP-19 closed\u{201d}, \
                 and an inbox item offers Reopen parent or Remove dependency."
                    .to_owned(),
            ]
        );
        assert_eq!(text.confirm, "Stop agent & close");
        assert_eq!(close_text(&shop19, &[], false).confirm, "Close ticket");
    }

    #[test]
    fn the_park_confirmation_says_what_is_kept() {
        let t = ticket("SHOP-25", "Upgrade to Next.js 15", Status::Working);
        let text = park_text(&t);
        assert_eq!(text.title, "Park SHOP-25?");
        assert_eq!(
            text.message,
            "SHOP-25's agent will be stopped. Its branch, files and conversation are kept."
        );
        assert_eq!(text.kept[2], "conversation \u{b7} resumes on Dev");
    }

    #[test]
    fn submit_texts_and_the_card_line() {
        let mut idle = ticket("SHOP-19", "Price table", Status::Idle);
        idle.status_since = Some(0);
        idle.signal = None;
        let text = submit_text(&idle, 120_000);
        assert_eq!(text.title, "Submit SHOP-19 Price table for review?");
        assert_eq!(
            text.intro,
            "The agent ended its turn 2m ago. Its session stays; Send back continues it."
        );
        assert_eq!(
            submitted_line("Paging works, tests pending"),
            "Submitted by you \u{b7} \u{201c}Paging works, tests pending\u{201d}"
        );
        assert_eq!(submitted_line(""), "Submitted by you");
    }
}
