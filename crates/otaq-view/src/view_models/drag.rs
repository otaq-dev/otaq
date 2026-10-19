//! Drag and drop on the board (UI-02, TKT-04; *03 · Board & columns → Drag · over an allowed column, Drag ·
//! over a forbidden column*). The rules are the core's [`check_move`]: a column
//! under the dragged card is drawn allowed (accent) or forbidden (red, with the reason). A drop does what the
//! rules allow, and a ticket with a short or empty description starts without a warning (d0067). Backlog → Ready
//! for Dev or Dev queues it (it lands in Ready for Dev until it can start), Ready for Dev → Backlog takes it out of
//! the queue without asking (d0066). A drop on Approved (since d0087 the Review column's Approved section) asks how
//! (Local merge or Open PR) and never approves by itself (T-0095); Dev → Backlog asks to park first (T-0107);
//! Approved → Ready for Review (the Review column outside its Approved section) is PR rework (the PR stays,
//! T-0150). The page's drop targets map onto these core columns (`board_page::DropTargetId`).
//!
//! Epics (EPC-12, EPC-14, d0080; *03 · Board & columns → epic flows 1 → 1*, *epic flows 3 → 8*): the Backlog epic
//! card, one of its rows, and a "not queued" chip on its track are dragged to Ready for Dev or Dev. The card queues
//! every child still in Backlog ("Queue 2 children"), a row or a chip only that child ("Queue SHOP-45.1", or
//! "Resume SHOP-45.1" when it is parked). Both drops go through `epic.queue`, whose outcome is the queue toast.
//!
//! Take back (EPC-14, d0080; *epic flows 1 → 3*, *epic flows 4 → 11*): an epic's track is dragged by its name.
//! Backlog accepts it with the core's take-back plan ("Unqueue 1 never-started child (45.2)" over "SHOP-45.1 keeps
//! running", or "Park 2 running children", which asks first); Ready for Dev and Dev queue the rest, as the card
//! does; the other columns are not allowed.

use otaq_core::board::epic_flight::{self, EpicPlace, TakeBackPlan, nothing_to_queue};
use otaq_core::board::moves::{
    DropTarget, MERGED_LOCKED, MoveAction, MoveCheck, ONLY_AGENT_SUBMITS, check_move,
};
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Column, Status, TicketKey};

use crate::icons::IconName;

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

/// What a drop of `ticket` on Ready for Dev or Dev queues, when it is an epic in Backlog or a child row (or "not
/// queued" chip) in Backlog: the epic's children still in Backlog, or that one child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpicQueue<'a> {
    /// The epic card (or its track, ⌘D): every child still in Backlog. Parked ones resume (d0052).
    Whole {
        queued: Vec<&'a Ticket>,
        resumed: Vec<&'a Ticket>,
    },
    /// One row or chip: only this child.
    One {
        child: &'a Ticket,
        epic: &'a TicketKey,
    },
}

/// The epic queue a drop of `ticket` makes, from the project's `tickets` (which may be just the epic's children);
/// `None` for anything else (a standalone ticket, a child already queued or started, an epic past Backlog).
pub fn epic_queue<'a>(ticket: &'a Ticket, tickets: &'a [Ticket]) -> Option<EpicQueue<'a>> {
    if ticket.column != Column::Backlog {
        return None;
    }
    if ticket.is_epic() {
        let (resumed, queued) = epic_flight::to_queue(ticket, tickets)
            .into_iter()
            .partition(|c| epic_flight::is_parked(c));
        return Some(EpicQueue::Whole { queued, resumed });
    }
    let epic = ticket.epic.as_ref()?;
    // A reopened child is a row too; only a merged one can't move (check_move refuses it).
    (ticket.status != Status::Merged).then_some(EpicQueue::One {
        child: ticket,
        epic,
    })
}

/// "Queue 2 children", "Resume 2 children", "Queue 1 child · resume 2", "Queue SHOP-45.1", "Resume SHOP-45.1";
/// `None` when the epic has nothing left in Backlog.
pub fn epic_queue_hint(queue: &EpicQueue) -> Option<String> {
    let children = |n: usize| if n == 1 { "child" } else { "children" };
    match queue {
        EpicQueue::Whole { queued, resumed } => match (queued.len(), resumed.len()) {
            (0, 0) => None,
            (q, 0) => Some(format!("Queue {q} {}", children(q))),
            (0, r) => Some(format!("Resume {r} {}", children(r))),
            (q, r) => Some(format!("Queue {q} {} · resume {r}", children(q))),
        },
        EpicQueue::One { child, .. } if child.status == Status::Parked => {
            Some(format!("Resume {}", child.key))
        }
        EpicQueue::One { child, .. } => Some(format!("Queue {}", child.key)),
    }
}

/// Whether `ticket` is an epic shown as its track (EPC-11): then it is dragged by the track's name.
pub fn is_track(ticket: &Ticket, tickets: &[Ticket]) -> bool {
    ticket.is_epic() && epic_flight::epic_place(ticket, tickets) == EpicPlace::Track
}

/// What dropping `ticket` on `column` takes back: the core's plan when it is an epic's track over Backlog
/// (EPC-14); `None` for anything else.
pub fn take_back(ticket: &Ticket, tickets: &[Ticket], column: Column) -> Option<TakeBackPlan> {
    (column == Column::Backlog && is_track(ticket, tickets))
        .then(|| epic_flight::take_back_plan(ticket, tickets))
}

/// How a column looks while `ticket` is dragged over it. `tickets` (the project's, or at least the epic's
/// children) only matter for an epic or a child row in Backlog.
pub fn feedback(ticket: &Ticket, tickets: &[Ticket], column: Column) -> DropFeedback {
    if let Some(plan) = take_back(ticket, tickets, column) {
        return match plan {
            TakeBackPlan::Refused { reason, .. } => DropFeedback::Forbidden {
                title: reason,
                detail: None,
            },
            _ => DropFeedback::Allowed,
        };
    }
    let check = check_move(ticket, target_for(column));
    // An epic with nothing left in Backlog has nothing to queue (its children all started or wait already).
    if check.action() == Some(MoveAction::QueueEpic)
        && epic_queue(ticket, tickets)
            .as_ref()
            .and_then(epic_queue_hint)
            .is_none()
    {
        return DropFeedback::Forbidden {
            title: nothing_to_queue(&ticket.key),
            detail: None,
        };
    }
    match check {
        MoveCheck::Unchanged => DropFeedback::Home,
        MoveCheck::Refused { reason } => {
            let detail = match reason.as_str() {
                // Only a Backlog card can be queued instead.
                ONLY_AGENT_SUBMITS if ticket.column == Column::Backlog => {
                    Some("Drag to Ready for Dev to queue it.")
                }
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

/// The accent label under the pointer over an allowed column (*Drag · over an allowed column*: "Drop to queue ·
/// starts in priority order"; *PR rework*: "Drop to rework · PR #481 stays"): what the drop will do. `None` over the
/// card's own column and over a forbidden one. An epic or a child row says what it queues ("Queue 2 children",
/// "Queue SHOP-45.1", "Resume SHOP-45.1").
pub fn drop_hint(
    ticket: &Ticket,
    tickets: &[Ticket],
    column: Column,
) -> Option<(IconName, String)> {
    match take_back(ticket, tickets, column) {
        Some(TakeBackPlan::Refused { .. }) => return None,
        Some(plan @ TakeBackPlan::ParkAll { .. }) => {
            return Some((IconName::SquareParking, plan.hint()));
        }
        Some(plan) => return Some((IconName::ArrowDownToLine, plan.hint())),
        None => {}
    }
    let action = check_move(ticket, target_for(column)).action()?;
    if matches!(action, MoveAction::Start | MoveAction::QueueEpic)
        && let Some(queue) = epic_queue(ticket, tickets)
    {
        return epic_queue_hint(&queue).map(|hint| (IconName::Play, hint));
    }
    Some(match action {
        MoveAction::Start if ticket.status == Status::Parked => (
            IconName::Play,
            "Drop to resume · same conversation".to_owned(),
        ),
        MoveAction::Start | MoveAction::QueueEpic => (
            IconName::Play,
            "Drop to queue · starts in priority order".to_owned(),
        ),
        MoveAction::Unqueue => (
            IconName::Undo2,
            "Drop to take it out of the queue".to_owned(),
        ),
        MoveAction::Park => (
            IconName::SquareParking,
            "Drop to park · asks first".to_owned(),
        ),
        // *Row · Review column (0086 · V)*: the Approved section as the target.
        MoveAction::Approve => (
            IconName::Check,
            format!(
                "Drop to approve {} \u{2192} choose Local merge or Open PR",
                ticket.key
            ),
        ),
        MoveAction::PrRework => (
            IconName::RotateCcw,
            match ticket.flags.pr_number {
                Some(n) => format!("Drop to rework · PR #{n} stays"),
                None => "Drop to rework".to_owned(),
            },
        ),
        MoveAction::Reopen => (IconName::RotateCcw, "Drop to reopen".to_owned()),
    })
}

/// The hint's second line, under it: "SHOP-45.1 keeps running" when a track's take back leaves children running.
pub fn drop_note(ticket: &Ticket, tickets: &[Ticket], column: Column) -> Option<String> {
    take_back(ticket, tickets, column)?.note()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropAction {
    /// Nothing happens (its own column, or refused: the card snaps back).
    Nothing,
    /// Ask the service to move it.
    Move,
    /// Queue the epic (`child: None`) or one of its children through `epic.queue`, which answers with what the
    /// queue toast reads (EPC-12, EPC-15).
    QueueEpic {
        epic: TicketKey,
        child: Option<TicketKey>,
    },
    /// Ready for Review → Approved: the approve choice opens; cancelling leaves the card where it was.
    ChooseApprove,
    /// Dev → Backlog: the Park confirmation; Cancel leaves the card in Dev.
    AskPark,
    /// A track on Backlog that unqueues its waiting children: `epic.take_back` at once, no dialog (EPC-14).
    TakeBack { epic: TicketKey },
    /// A track on Backlog whose children all started: the Park-all confirmation first (state 11); Cancel leaves
    /// everything running, its confirm sends `epic.take_back` confirmed.
    AskParkAll(TakeBackPlan),
    /// Allowed by the rules, carried out by a later ticket.
    NotYet(String),
}

/// What a drop of `ticket` on `column` does; `tickets` (the project's) only matter for an epic's track.
pub fn on_drop(ticket: &Ticket, tickets: &[Ticket], column: Column) -> DropAction {
    match take_back(ticket, tickets, column) {
        Some(TakeBackPlan::Refused { .. }) => return DropAction::Nothing,
        Some(plan @ TakeBackPlan::ParkAll { .. }) => return DropAction::AskParkAll(plan),
        Some(TakeBackPlan::Unqueue { epic, .. }) => return DropAction::TakeBack { epic },
        None => {}
    }
    let check = check_move(ticket, target_for(column));
    // A row of the epic card or a "not queued" chip: only that child (T-0263's single-child path).
    if let (
        MoveCheck::Allowed {
            action: MoveAction::Start,
        },
        Column::Backlog,
        Some(epic),
    ) = (&check, ticket.column, &ticket.epic)
    {
        return DropAction::QueueEpic {
            epic: epic.clone(),
            child: Some(ticket.key.clone()),
        };
    }
    match check {
        MoveCheck::Unchanged | MoveCheck::Refused { .. } => DropAction::Nothing,
        MoveCheck::Allowed {
            action: MoveAction::QueueEpic,
        } => DropAction::QueueEpic {
            epic: ticket.key.clone(),
            child: None,
        },
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
    use otaq_core::domain::{Kind, Status};

    fn epic() -> Ticket {
        let mut epic = ticket("SHOP-45", "Search", Status::Backlog);
        epic.kind = Kind::Epic;
        epic
    }

    fn child(key: &str, status: Status) -> Ticket {
        let mut child = ticket(key, "Search child", status);
        child.epic = Some(key.split('.').next().unwrap().parse().unwrap());
        child
    }

    /// *Epic flows 1 → 1*, *epic flows 3 → 8*: the epic card, a row and a chip say what they queue over Ready for
    /// Dev and Dev; Review, Approved and Done stay not allowed with the core's reasons.
    #[test]
    fn the_epic_card_a_row_and_a_chip() {
        let epic = epic();
        let children = vec![
            child("SHOP-45.1", Status::Backlog),
            child("SHOP-45.2", Status::Backlog),
        ];
        for column in [Column::ReadyForDev, Column::Dev] {
            assert_eq!(feedback(&epic, &children, column), DropFeedback::Allowed);
            assert_eq!(
                drop_hint(&epic, &children, column),
                Some((IconName::Play, "Queue 2 children".to_owned()))
            );
            assert_eq!(
                on_drop(&epic, &[], column),
                DropAction::QueueEpic {
                    epic: "SHOP-45".parse().unwrap(),
                    child: None,
                }
            );
        }
        for column in [Column::ReadyForReview, Column::Approved, Column::Done] {
            assert!(matches!(
                feedback(&epic, &children, column),
                DropFeedback::Forbidden { .. }
            ));
            assert_eq!(drop_hint(&epic, &children, column), None);
            assert_eq!(on_drop(&epic, &[], column), DropAction::Nothing);
        }
        assert_eq!(
            feedback(&epic, &children, Column::Backlog),
            DropFeedback::Home
        );

        // A row (or the "not queued" chip of the same child): only that child.
        let row = &children[0];
        assert_eq!(
            drop_hint(row, &children, Column::ReadyForDev),
            Some((IconName::Play, "Queue SHOP-45.1".to_owned()))
        );
        assert_eq!(
            on_drop(row, &[], Column::Dev),
            DropAction::QueueEpic {
                epic: "SHOP-45".parse().unwrap(),
                child: Some("SHOP-45.1".parse().unwrap()),
            }
        );
        for column in [Column::ReadyForReview, Column::Approved, Column::Done] {
            assert!(matches!(
                feedback(row, &children, column),
                DropFeedback::Forbidden { .. }
            ));
            assert_eq!(on_drop(row, &[], column), DropAction::Nothing);
        }
        assert_eq!(
            feedback(row, &children, Column::Backlog),
            DropFeedback::Home
        );

        // A parked row resumes (EPC-14).
        let parked = child("SHOP-45.1", Status::Parked);
        assert_eq!(parked.column, Column::Backlog);
        assert_eq!(
            drop_hint(&parked, &[], Column::ReadyForDev),
            Some((IconName::Play, "Resume SHOP-45.1".to_owned()))
        );
        assert!(matches!(
            on_drop(&parked, &[], Column::ReadyForDev),
            DropAction::QueueEpic { child: Some(_), .. }
        ));
    }

    #[test]
    fn the_epic_hint_counts_what_it_queues_and_resumes() {
        let epic = epic();
        let hint =
            |children: &[Ticket]| drop_hint(&epic, children, Column::ReadyForDev).map(|h| h.1);
        assert_eq!(
            hint(&[child("SHOP-45.1", Status::Backlog)]).as_deref(),
            Some("Queue 1 child")
        );
        // After Park all: the parked rows resume.
        let parked = [
            child("SHOP-45.1", Status::Parked),
            child("SHOP-45.2", Status::Parked),
        ];
        assert_eq!(hint(&parked).as_deref(), Some("Resume 2 children"));
        let mixed = [
            child("SHOP-45.1", Status::Parked),
            child("SHOP-45.2", Status::Parked),
            child("SHOP-45.3", Status::Backlog),
        ];
        assert_eq!(hint(&mixed).as_deref(), Some("Queue 1 child · resume 2"));
        // Active or done children aren't queued again; another epic's children don't count.
        let others = [
            child("SHOP-45.1", Status::Queued),
            child("SHOP-45.2", Status::Working),
            child("SHOP-45.3", Status::Merged),
            child("SHOP-40.1", Status::Backlog),
        ];
        assert_eq!(hint(&others), None);
        assert!(matches!(
            feedback(&epic, &others, Column::ReadyForDev),
            DropFeedback::Forbidden { .. }
        ));
        // A standalone ticket and a queued child keep their own hints.
        let queued = child("SHOP-45.1", Status::Queued);
        assert_eq!(
            drop_hint(&queued, &[], Column::Backlog).unwrap().1,
            "Drop to take it out of the queue"
        );
        assert_eq!(on_drop(&queued, &[], Column::Backlog), DropAction::Move);
    }

    /// *Epic flows 1 → 3*, *epic flows 4 → 11* (EPC-14): the track's name is dragged. Backlog takes it back with the
    /// core's plan; Ready for Dev and Dev queue the rest; the other columns are not allowed.
    #[test]
    fn a_track_answers_its_drag_and_takes_back_on_backlog() {
        let epic = epic();
        let key = |k: &str| -> TicketKey { k.parse().unwrap() };

        // Flow 3: SHOP-45.1 runs, SHOP-45.2 waits. Backlog unqueues 45.2, no dialog; 45.1 keeps running.
        let flow3 = [
            child("SHOP-45.1", Status::Working),
            child("SHOP-45.2", Status::Queued),
        ];
        assert!(is_track(&epic, &flow3));
        assert_eq!(
            feedback(&epic, &flow3, Column::Backlog),
            DropFeedback::Allowed
        );
        assert_eq!(
            drop_hint(&epic, &flow3, Column::Backlog),
            Some((
                IconName::ArrowDownToLine,
                "Unqueue 1 never-started child (45.2)".to_owned()
            ))
        );
        assert_eq!(
            drop_note(&epic, &flow3, Column::Backlog).as_deref(),
            Some("SHOP-45.1 keeps running")
        );
        assert_eq!(
            on_drop(&epic, &flow3, Column::Backlog),
            DropAction::TakeBack {
                epic: key("SHOP-45")
            }
        );
        // Nothing left in Backlog to queue: Ready for Dev and Dev say so.
        for column in [Column::ReadyForDev, Column::Dev] {
            assert!(matches!(
                feedback(&epic, &flow3, column),
                DropFeedback::Forbidden { .. }
            ));
        }
        for column in [Column::ReadyForReview, Column::Approved, Column::Done] {
            assert!(matches!(
                feedback(&epic, &flow3, column),
                DropFeedback::Forbidden { .. }
            ));
            assert_eq!(drop_hint(&epic, &flow3, column), None);
            assert_eq!(drop_note(&epic, &flow3, column), None);
            assert_eq!(on_drop(&epic, &flow3, column), DropAction::Nothing);
        }

        // After a single-child start: Ready for Dev and Dev queue the rest (mode Whole, T-0263).
        let single = [
            child("SHOP-45.1", Status::Working),
            child("SHOP-45.2", Status::Backlog),
        ];
        for column in [Column::ReadyForDev, Column::Dev] {
            assert_eq!(feedback(&epic, &single, column), DropFeedback::Allowed);
            assert_eq!(
                drop_hint(&epic, &single, column),
                Some((IconName::Play, "Queue 1 child".to_owned()))
            );
            assert_eq!(drop_note(&epic, &single, column), None);
            assert_eq!(
                on_drop(&epic, &single, column),
                DropAction::QueueEpic {
                    epic: key("SHOP-45"),
                    child: None,
                }
            );
        }

        // State 11: every child started and runs. Backlog parks them all, after the confirmation.
        let all_started = [
            child("SHOP-45.1", Status::Working),
            child("SHOP-45.2", Status::Working),
        ];
        assert_eq!(
            feedback(&epic, &all_started, Column::Backlog),
            DropFeedback::Allowed
        );
        assert_eq!(
            drop_hint(&epic, &all_started, Column::Backlog),
            Some((
                IconName::SquareParking,
                "Park 2 running children".to_owned()
            ))
        );
        assert_eq!(drop_note(&epic, &all_started, Column::Backlog), None);
        let DropAction::AskParkAll(plan) = on_drop(&epic, &all_started, Column::Backlog) else {
            panic!("Park all asks first");
        };
        let confirmation = plan.confirmation().unwrap();
        assert_eq!(confirmation.title, "Park 2 running tickets?");
        assert_eq!(confirmation.confirm, "Park 2 tickets");

        // Nothing queued or running (the last child merged, the epic not yet in Review): not allowed, with why.
        let merged = [
            child("SHOP-45.1", Status::Merged),
            child("SHOP-45.2", Status::ReadyForReview),
        ];
        assert!(is_track(&epic, &merged));
        assert_eq!(
            feedback(&epic, &merged, Column::Backlog),
            DropFeedback::Forbidden {
                title: "Nothing of SHOP-45 is queued or running".to_owned(),
                detail: None,
            }
        );
        assert_eq!(drop_hint(&epic, &merged, Column::Backlog), None);
        assert_eq!(
            on_drop(&epic, &merged, Column::Backlog),
            DropAction::Nothing
        );

        // The epic card (nothing started) stays home over Backlog.
        let card = [child("SHOP-45.1", Status::Backlog)];
        assert!(!is_track(&epic, &card));
        assert_eq!(feedback(&epic, &card, Column::Backlog), DropFeedback::Home);
        assert_eq!(on_drop(&epic, &card, Column::Backlog), DropAction::Nothing);
    }

    /// State 10: a single child card in Ready for Dev goes back to Backlog as today (d0066), no dialog.
    #[test]
    fn a_queued_child_card_goes_back_to_backlog() {
        let children = [
            child("SHOP-45.1", Status::Working),
            child("SHOP-45.2", Status::Queued),
        ];
        let queued = &children[1];
        assert_eq!(
            feedback(queued, &children, Column::Backlog),
            DropFeedback::Allowed
        );
        assert_eq!(
            drop_hint(queued, &children, Column::Backlog).unwrap().1,
            "Drop to take it out of the queue"
        );
        assert_eq!(drop_note(queued, &children, Column::Backlog), None);
        assert_eq!(
            on_drop(queued, &children, Column::Backlog),
            DropAction::Move
        );
    }

    /// The 0039 rule is gone (d0080): no column answers that epics stay in Backlog.
    #[test]
    fn no_epic_is_told_to_stay_in_backlog() {
        let epic = epic();
        let children = [child("SHOP-45.1", Status::Backlog)];
        for &column in Column::ALL {
            if let DropFeedback::Forbidden { title, .. } = feedback(&epic, &children, column) {
                assert!(!title.contains("Backlog"), "{title}");
            }
        }
    }

    #[test]
    fn allowed_and_forbidden_columns() {
        let backlog = ticket("SHOP-31", "Add order history page", Status::Backlog);
        assert_eq!(feedback(&backlog, &[], Column::Backlog), DropFeedback::Home);
        assert_eq!(feedback(&backlog, &[], Column::Dev), DropFeedback::Allowed);
        assert_eq!(
            feedback(&backlog, &[], Column::ReadyForDev),
            DropFeedback::Allowed
        );
        assert_eq!(
            feedback(&backlog, &[], Column::ReadyForReview),
            DropFeedback::Forbidden {
                title: ONLY_AGENTS_TO_REVIEW.into(),
                detail: Some("Drag to Ready for Dev to queue it."),
            }
        );
        let queued = ticket("SHOP-20", "Checkout page", Status::Queued);
        assert_eq!(
            feedback(&queued, &[], Column::Backlog),
            DropFeedback::Allowed
        );
        assert!(matches!(
            feedback(&queued, &[], Column::Dev),
            DropFeedback::Forbidden { .. }
        ));
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        // An epic drop queues its children (d0080); with none left in Backlog there is nothing to queue.
        assert_eq!(
            feedback(&epic, &[child("SHOP-40.1", Status::Backlog)], Column::Dev),
            DropFeedback::Allowed
        );
        assert_eq!(
            feedback(&epic, &[], Column::Dev),
            DropFeedback::Forbidden {
                title: "Every child of SHOP-40 is already queued or started".into(),
                detail: None,
            }
        );
        let merged = ticket("SHOP-17", "Add Stripe client", Status::Merged);
        assert!(matches!(
            feedback(&merged, &[], Column::Backlog),
            DropFeedback::Forbidden { .. }
        ));
        let working = ticket("SHOP-19", "Price table", Status::Working);
        assert_eq!(
            feedback(&working, &[], Column::Backlog),
            DropFeedback::Allowed,
            "Park"
        );
        assert_eq!(feedback(&working, &[], Column::Dev), DropFeedback::Home);
    }

    #[test]
    fn drop_hints_say_what_the_drop_does() {
        let backlog = ticket("SHOP-24", "Update footer links", Status::Backlog);
        assert_eq!(
            drop_hint(&backlog, &[], Column::ReadyForDev),
            Some((
                IconName::Play,
                "Drop to queue · starts in priority order".to_owned()
            )),
            "the frame's wording"
        );
        assert_eq!(drop_hint(&backlog, &[], Column::Backlog), None, "home");
        assert_eq!(drop_hint(&backlog, &[], Column::ReadyForReview), None);
        let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
        pr.flags.pr_number = Some(481);
        assert_eq!(
            drop_hint(&pr, &[], Column::ReadyForReview).unwrap().1,
            "Drop to rework · PR #481 stays"
        );
        let working = ticket("SHOP-25", "Upgrade to Next.js 15", Status::Working);
        assert_eq!(
            drop_hint(&working, &[], Column::Backlog).unwrap().1,
            "Drop to park · asks first"
        );
        let mut parked = ticket("SHOP-25", "Upgrade to Next.js 15", Status::Parked);
        parked.column = Column::Backlog;
        assert!(
            drop_hint(&parked, &[], Column::Dev)
                .unwrap()
                .1
                .contains("resume")
        );
    }

    #[test]
    fn drops() {
        // An empty description is a plain Start: no warning (d0067).
        let mut t24 = ticket("SHOP-24", "Update footer links", Status::Backlog);
        assert!(t24.body.is_empty());
        assert_eq!(on_drop(&t24, &[], Column::Dev), DropAction::Move);
        t24.body = "Fix the links.".into();
        assert_eq!(on_drop(&t24, &[], Column::Dev), DropAction::Move);
        assert_eq!(on_drop(&t24, &[], Column::Approved), DropAction::Nothing);
        assert_eq!(on_drop(&t24, &[], Column::ReadyForDev), DropAction::Move);
        // Ready for Dev → Backlog: out of the queue, no confirmation (d0066).
        let queued = ticket("SHOP-20", "Checkout page", Status::Queued);
        assert_eq!(on_drop(&queued, &[], Column::Backlog), DropAction::Move);
        let working = ticket("SHOP-19", "Price table", Status::Working);
        assert_eq!(on_drop(&working, &[], Column::Backlog), DropAction::AskPark);
        let review = ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
        assert_eq!(
            on_drop(&review, &[], Column::Approved),
            DropAction::ChooseApprove
        );
        let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
        pr.flags.pr_number = Some(481);
        assert_eq!(
            feedback(&pr, &[], Column::ReadyForReview),
            DropFeedback::Allowed
        );
        assert_eq!(
            on_drop(&pr, &[], Column::ReadyForReview),
            DropAction::Move,
            "PR rework: the PR stays"
        );
        assert!(matches!(
            feedback(&pr, &[], Column::Dev),
            DropFeedback::Forbidden { .. }
        ));
        assert!(matches!(
            feedback(&pr, &[], Column::Backlog),
            DropFeedback::Forbidden { .. }
        ));
        let closed = ticket("SHOP-26", "Add sitemap", Status::ClosedNotMerged);
        assert_eq!(
            on_drop(&closed, &[], Column::Backlog),
            DropAction::Move,
            "the service reopens or says why not"
        );
    }
}
