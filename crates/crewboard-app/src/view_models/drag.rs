//! Drag and drop on the board (UI-02, TKT-04; *03 · Board & columns → Drag · over an allowed column, Drag ·
//! over a forbidden column, Epic can't be dragged to Dev*). The rules are the core's [`check_move`]: a column
//! under the dragged card is drawn allowed (accent) or forbidden (red, with the reason). A drop does what the
//! rules allow, and a ticket with a short or empty description starts without a warning (d0067). Backlog → Ready
//! for Dev or Dev queues it (it lands in Ready for Dev until it can start), Ready for Dev → Backlog takes it out of
//! the queue without asking (d0066). A drop on Approved asks how (Local merge or Open PR) and never approves by
//! itself (T-0095); Dev → Backlog asks to park first (T-0107); Approved → Ready for Review is PR rework (the PR
//! stays, T-0150).

use crewboard_core::board::moves::{
    DropTarget, EPICS_NOT_TO_DEV, MERGED_LOCKED, MoveAction, MoveCheck, ONLY_AGENT_SUBMITS,
    check_move,
};
use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::Column;

/// Where a drop on a column lands. A drop on Dev from Backlog lands in Ready for Dev until it can start (d0066).
pub fn target_for(column: Column) -> DropTarget {
    DropTarget::of(column)
}

/// *03 · Drag · over a forbidden column*: the reason over Ready for Review.
pub const ONLY_AGENTS_TO_REVIEW: &str = "Only agents move tickets to Ready for Review";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropFeedback {
    /// The card's own column: no styling.
    Home,
    Allowed,
    Forbidden {
        title: String,
        detail: Option<&'static str>,
    },
}

/// How a column looks while `ticket` is dragged over it.
pub fn feedback(ticket: &Ticket, column: Column) -> DropFeedback {
    match check_move(ticket, target_for(column)) {
        MoveCheck::Unchanged => DropFeedback::Home,
        MoveCheck::Refused { reason } => {
            let detail = match reason.as_str() {
                ONLY_AGENT_SUBMITS => Some("Drag to Ready for Dev to queue it."),
                EPICS_NOT_TO_DEV => Some("Drag its tickets to Ready for Dev instead."),
                MERGED_LOCKED => Some("Fixing merged work means a new ticket."),
                _ => None,
            };
            // The frame's shorter wording for the board (the CLI keeps the core's).
            let title = match reason.as_str() {
                ONLY_AGENT_SUBMITS => ONLY_AGENTS_TO_REVIEW.to_owned(),
                _ => reason,
            };
            DropFeedback::Forbidden { title, detail }
        }
        _ => DropFeedback::Allowed,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropAction {
    /// Nothing happens (its own column, or refused: the card snaps back).
    Nothing,
    /// Ask the service to move it.
    Move,
    /// Ready for Review → Approved: the approve choice opens; cancelling leaves the card where it was.
    ChooseApprove,
    /// Dev → Backlog: the Park confirmation; Cancel leaves the card in Dev.
    AskPark,
    /// Allowed by the rules, carried out by a later ticket.
    NotYet(String),
}

pub fn on_drop(ticket: &Ticket, column: Column) -> DropAction {
    let check = check_move(ticket, target_for(column));
    match check {
        MoveCheck::Unchanged | MoveCheck::Refused { .. } => DropAction::Nothing,
        MoveCheck::Allowed {
            action:
                MoveAction::Start | MoveAction::Unqueue | MoveAction::Reopen | MoveAction::PrRework,
        } => DropAction::Move,
        MoveCheck::Choose {
            action: MoveAction::Approve,
        } => DropAction::ChooseApprove,
        MoveCheck::Confirm {
            action: MoveAction::Park,
        } => DropAction::AskPark,
        _ => DropAction::NotYet("This move isn’t available yet".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use crewboard_core::domain::{Kind, Status};

    #[test]
    fn allowed_and_forbidden_columns() {
        let backlog = ticket("SHOP-31", "Add order history page", Status::Backlog);
        assert_eq!(feedback(&backlog, Column::Backlog), DropFeedback::Home);
        assert_eq!(feedback(&backlog, Column::Dev), DropFeedback::Allowed);
        assert_eq!(
            feedback(&backlog, Column::ReadyForDev),
            DropFeedback::Allowed
        );
        assert_eq!(
            feedback(&backlog, Column::ReadyForReview),
            DropFeedback::Forbidden {
                title: ONLY_AGENTS_TO_REVIEW.into(),
                detail: Some("Drag to Ready for Dev to queue it."),
            }
        );
        let queued = ticket("SHOP-20", "Checkout page", Status::Queued);
        assert_eq!(feedback(&queued, Column::Backlog), DropFeedback::Allowed);
        assert!(matches!(
            feedback(&queued, Column::Dev),
            DropFeedback::Forbidden { .. }
        ));
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        assert_eq!(
            feedback(&epic, Column::Dev),
            DropFeedback::Forbidden {
                title: EPICS_NOT_TO_DEV.into(),
                detail: Some("Drag its tickets to Ready for Dev instead."),
            }
        );
        let merged = ticket("SHOP-17", "Add Stripe client", Status::Merged);
        assert!(matches!(
            feedback(&merged, Column::Backlog),
            DropFeedback::Forbidden { .. }
        ));
        let working = ticket("SHOP-19", "Price table", Status::Working);
        assert_eq!(
            feedback(&working, Column::Backlog),
            DropFeedback::Allowed,
            "Park"
        );
        assert_eq!(feedback(&working, Column::Dev), DropFeedback::Home);
    }

    #[test]
    fn drops() {
        // An empty description is a plain Start: no warning (d0067).
        let mut t24 = ticket("SHOP-24", "Update footer links", Status::Backlog);
        assert!(t24.body.is_empty());
        assert_eq!(on_drop(&t24, Column::Dev), DropAction::Move);
        t24.body = "Fix the links.".into();
        assert_eq!(on_drop(&t24, Column::Dev), DropAction::Move);
        assert_eq!(on_drop(&t24, Column::Approved), DropAction::Nothing);
        assert_eq!(on_drop(&t24, Column::ReadyForDev), DropAction::Move);
        // Ready for Dev → Backlog: out of the queue, no confirmation (d0066).
        let queued = ticket("SHOP-20", "Checkout page", Status::Queued);
        assert_eq!(on_drop(&queued, Column::Backlog), DropAction::Move);
        let working = ticket("SHOP-19", "Price table", Status::Working);
        assert_eq!(on_drop(&working, Column::Backlog), DropAction::AskPark);
        let review = ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
        assert_eq!(
            on_drop(&review, Column::Approved),
            DropAction::ChooseApprove
        );
        let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
        pr.flags.pr_number = Some(481);
        assert_eq!(feedback(&pr, Column::ReadyForReview), DropFeedback::Allowed);
        assert_eq!(
            on_drop(&pr, Column::ReadyForReview),
            DropAction::Move,
            "PR rework: the PR stays"
        );
        assert!(matches!(
            feedback(&pr, Column::Dev),
            DropFeedback::Forbidden { .. }
        ));
        assert!(matches!(
            feedback(&pr, Column::Backlog),
            DropFeedback::Forbidden { .. }
        ));
        let closed = ticket("SHOP-26", "Add sitemap", Status::ClosedNotMerged);
        assert_eq!(
            on_drop(&closed, Column::Backlog),
            DropAction::Move,
            "the service reopens or says why not"
        );
    }
}
