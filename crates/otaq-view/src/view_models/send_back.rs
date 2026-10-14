//! The Send back dialog (REV-04; d0034, d0048, d0058, *07 → Send back · review & confirm*): what the agent will
//! receive (every pending comment and the optional note), which tickets keep waiting, and the request it sends.
//! Sending with no comments and no note is allowed: the note is optional.

use otaq_core::board::comments::{ReviewComment, Side};
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Status, TicketKey};
use otaq_core::protocol::{Request, SendBackParams};

/// Whether Send back is offered: a ticket (never an epic) in Ready for Review.
pub fn available(ticket: &Ticket) -> bool {
    !ticket.is_epic() && ticket.status == Status::ReadyForReview
}

/// What the dialog lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendBackPlan {
    /// "Send SHOP-18 Checkout API back".
    pub title: String,
    pub intro: String,
    /// "api/checkout/route.ts · L9" and the comment.
    pub comments: Vec<(String, String)>,
    /// Tickets that keep waiting until this one is merged: key, "Checkout page · Queued".
    pub waiting: Vec<(TicketKey, String)>,
}

pub fn plan(ticket: &Ticket, pending: &[ReviewComment], tickets: &[Ticket]) -> SendBackPlan {
    let comments = pending
        .iter()
        .filter(|c| c.is_pending())
        .map(|c| {
            let side = if c.side == Side::Old {
                " (removed line)"
            } else {
                ""
            };
            (
                format!("{} \u{b7} L{}{side}", c.file, c.line),
                c.body.clone(),
            )
        })
        .collect();
    let mut waiting: Vec<(TicketKey, String)> = tickets
        .iter()
        .filter(|t| t.depends_on.contains(&ticket.key) && t.status != Status::Merged)
        .map(|t| {
            (
                t.key.clone(),
                format!("{} \u{b7} {}", t.title, t.status.label()),
            )
        })
        .collect();
    waiting.sort();
    SendBackPlan {
        title: format!("Send {} {} back", ticket.key, ticket.title),
        intro: format!(
            "{} goes straight back to its agent in Dev (Agent Working) and continues in the same session.",
            ticket.key
        ),
        comments,
        waiting,
    }
}

/// "These keep waiting until SHOP-18 is merged".
pub fn waiting_title(key: &TicketKey) -> String {
    format!("These keep waiting until {key} is merged")
}

/// The request; `clear_first` is the unsent-text prompt's "Clear it and send".
pub fn request(key: &TicketKey, note: &str, clear_first: bool) -> Request {
    Request::TicketSendBack(SendBackParams {
        key: key.to_string(),
        note: note.trim().to_owned(),
        clear_first,
    })
}

/// What the unsent-text prompt says Send back would type.
pub fn sending(comments: usize) -> String {
    match comments {
        0 => "Your review note (the review feedback skill)".to_owned(),
        1 => "Your review: 1 comment (the review feedback skill)".to_owned(),
        n => format!("Your review: {n} comments (the review feedback skill)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;

    fn comment(file: &str, line: u32, side: Side, body: &str) -> ReviewComment {
        ReviewComment {
            id: body.into(),
            round: 1,
            file: file.into(),
            line,
            side,
            body: body.into(),
            created_at: 0,
            sent_at: None,
        }
    }

    #[test]
    fn the_dialog_lists_comments_and_waiting_tickets() {
        let shop18 = ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
        let mut shop20 = ticket("SHOP-20", "Checkout page", Status::Queued);
        shop20.depends_on = vec![shop18.key.clone()];
        let mut shop21 = ticket("SHOP-21", "Order confirmation email", Status::Queued);
        shop21.depends_on = vec![shop18.key.clone()];
        let other = ticket("SHOP-22", "Other", Status::Queued);
        let p = plan(
            &shop18,
            &[
                comment(
                    "api/checkout/route.ts",
                    9,
                    Side::New,
                    "Return { code, message }",
                ),
                comment("api/checkout/route.ts", 4, Side::Old, "Why removed?"),
            ],
            &[shop21.clone(), shop20.clone(), other],
        );
        assert_eq!(p.title, "Send SHOP-18 Checkout API back");
        assert_eq!(
            p.intro,
            "SHOP-18 goes straight back to its agent in Dev (Agent Working) and continues in the same session."
        );
        assert_eq!(
            p.comments,
            [
                (
                    "api/checkout/route.ts \u{b7} L9".to_owned(),
                    "Return { code, message }".to_owned()
                ),
                (
                    "api/checkout/route.ts \u{b7} L4 (removed line)".to_owned(),
                    "Why removed?".to_owned()
                ),
            ]
        );
        assert_eq!(
            p.waiting,
            [
                (shop20.key.clone(), "Checkout page \u{b7} Queued".to_owned()),
                (
                    shop21.key.clone(),
                    "Order confirmation email \u{b7} Queued".to_owned()
                ),
            ]
        );
        assert_eq!(
            waiting_title(&shop18.key),
            "These keep waiting until SHOP-18 is merged"
        );
    }

    #[test]
    fn only_tickets_in_review_and_never_epics() {
        assert!(available(&ticket("SHOP-18", "t", Status::ReadyForReview)));
        assert!(!available(&ticket("SHOP-18", "t", Status::Working)));
        let mut epic = ticket("SHOP-40", "Wishlist", Status::ReadyForReview);
        epic.kind = otaq_core::domain::Kind::Epic;
        assert!(!available(&epic));
    }

    #[test]
    fn an_empty_send_back_is_allowed() {
        let key: TicketKey = "SHOP-18".parse().unwrap();
        assert_eq!(
            request(&key, "  ", false),
            Request::TicketSendBack(SendBackParams {
                key: "SHOP-18".into(),
                note: String::new(),
                clear_first: false
            })
        );
        assert!(matches!(
            request(&key, "x", true),
            Request::TicketSendBack(SendBackParams {
                clear_first: true,
                ..
            })
        ));
        assert_eq!(sending(0), "Your review note (the review feedback skill)");
        assert_eq!(
            sending(2),
            "Your review: 2 comments (the review feedback skill)"
        );
    }
}
