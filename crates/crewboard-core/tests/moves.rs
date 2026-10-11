//! The board state machine (SDD §7.3, §7.4; TKT-04): every column pair and the relevant sub-states, then the
//! moves this milestone carries out.

mod common;

use common::{Fixture, me, shop_input};
use crewboard_core::board::Refusal;
use crewboard_core::board::moves::{DropTarget, MoveAction, MoveCheck, check_move};
use crewboard_core::board::tickets::{Base, NewTicketInput, Ticket};
use crewboard_core::domain::{Actor, Column, Flags, Kind, Status, TicketKey};

fn ticket(kind: Kind, status: Status) -> Ticket {
    Ticket {
        id: "id".into(),
        key: "SHOP-18".parse::<TicketKey>().unwrap(),
        kind,
        title: "t".into(),
        body: String::new(),
        priority: Default::default(),
        priority_inherited: false,
        epic: None,
        column: status.column(),
        status,
        base: Base::Branch("main".into()),
        base_locked: false,
        external_id: None,
        depends_on: Vec::new(),
        aliases: Vec::new(),
        next_child_number: None,
        flags: Flags::default(),
        created_by: Actor::User("me".into()),
        updated_by: None,
        started_at: None,
        created_at: 0,
        updated_at: 0,
        status_since: None,
        queued_at: None,
        ready_since: None,
        work: None,
        start_problem: None,
        interrupted_from: None,
        signal: None,
        question: None,
        pushed_differs: None,
    }
}

fn allowed(action: MoveAction) -> MoveCheck {
    MoveCheck::Allowed { action }
}

fn reason(check: &MoveCheck) -> &str {
    match check {
        MoveCheck::Refused { reason } => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn allowed_drags() {
    use DropTarget as T;
    let rows = [
        (Status::Backlog, T::ReadyForDev, allowed(MoveAction::Start)),
        // On Dev it lands in Ready for Dev until it can start (d0066).
        (Status::Backlog, T::Dev, allowed(MoveAction::Start)),
        (Status::Parked, T::ReadyForDev, allowed(MoveAction::Start)),
        (Status::Parked, T::Dev, allowed(MoveAction::Start)),
        (Status::Queued, T::Backlog, allowed(MoveAction::Unqueue)),
        (
            Status::WaitingForSlot,
            T::Backlog,
            allowed(MoveAction::Unqueue),
        ),
        (
            Status::ParentClosed,
            T::Backlog,
            allowed(MoveAction::Unqueue),
        ),
        (
            Status::Working,
            T::Backlog,
            MoveCheck::Confirm {
                action: MoveAction::Park,
            },
        ),
        (
            Status::NeedsYou,
            T::Backlog,
            MoveCheck::Confirm {
                action: MoveAction::Park,
            },
        ),
        (
            Status::ReadyForReview,
            T::Approved,
            MoveCheck::Choose {
                action: MoveAction::Approve,
            },
        ),
        (
            Status::PrOpen,
            T::ReadyForReview,
            allowed(MoveAction::PrRework),
        ),
        (
            Status::ClosedNotMerged,
            T::Backlog,
            allowed(MoveAction::Reopen),
        ),
    ];
    for (status, target, expected) in rows {
        assert_eq!(
            check_move(&ticket(Kind::Ticket, status), target),
            expected,
            "{status} → {target}"
        );
    }
}

/// No "Done when" warning any more (TKT-04, d0067): a ticket with an empty description starts like any other.
#[test]
fn an_empty_description_never_warns() {
    let check = check_move(
        &ticket(Kind::Ticket, Status::Backlog),
        DropTarget::ReadyForDev,
    );
    assert_eq!(
        check,
        MoveCheck::Allowed {
            action: MoveAction::Start
        }
    );
}

#[test]
fn refused_drags_carry_a_reason() {
    use DropTarget as T;
    let t = |s| ticket(Kind::Ticket, s);
    for status in [Status::Backlog, Status::Working, Status::ClosedNotMerged] {
        assert_eq!(
            reason(&check_move(&t(status), T::ReadyForReview)),
            "Only the agent or Submit for review moves a ticket to Ready for Review"
        );
    }
    for target in [T::Backlog, T::ReadyForDev, T::ReadyForReview, T::Approved] {
        assert_eq!(
            reason(&check_move(&t(Status::Merged), target)),
            "Merged tickets are locked"
        );
    }
    let epic = ticket(Kind::Epic, Status::Backlog);
    assert_eq!(
        reason(&check_move(&epic, T::ReadyForDev)),
        "Epics don't go to Dev"
    );
    assert_eq!(reason(&check_move(&epic, T::Dev)), "Epics don't go to Dev");
    assert_eq!(
        reason(&check_move(&t(Status::Queued), T::Dev)),
        "It starts by itself when it can, in priority order",
        "not into Dev by hand"
    );
    assert_eq!(
        reason(&check_move(&t(Status::Working), T::ReadyForDev)),
        "Drag it to Backlog to park it"
    );
    for (status, target) in [
        (Status::Backlog, T::Approved),
        (Status::Backlog, T::Done),
        (Status::Working, T::Approved),
        (Status::Working, T::Done),
        (Status::ReadyForReview, T::ReadyForDev),
        (Status::ReadyForReview, T::Dev),
        (Status::ReadyForReview, T::Backlog),
        (Status::ReadyForReview, T::Done),
        (Status::PrOpen, T::ReadyForDev),
        (Status::PrOpen, T::Backlog),
        (Status::PrOpen, T::Done),
        (Status::ClosedNotMerged, T::ReadyForDev),
        (Status::ClosedNotMerged, T::Approved),
    ] {
        let check = check_move(&t(status), target);
        assert!(!reason(&check).is_empty(), "{status} → {target}");
    }
}

#[test]
fn same_column_is_unchanged_and_every_pair_is_decided() {
    for status in Status::ALL {
        for kind in Kind::ALL {
            for target in DropTarget::ALL {
                let check = check_move(&ticket(*kind, *status), *target);
                let epic_in_dev = *kind == Kind::Epic
                    && matches!(status.column(), Column::ReadyForDev | Column::Dev);
                if status.column() == target.column() && !epic_in_dev {
                    assert_eq!(check, MoveCheck::Unchanged, "{kind} {status} → {target}");
                }
                if *status == Status::Merged && target.column() != Column::Done {
                    assert!(check.is_refused());
                }
            }
        }
    }
}

#[tokio::test]
async fn start_queues_the_ticket_and_locks_its_base() {
    let f = Fixture::shop().await;
    let t = f
        .create_with(NewTicketInput {
            title: "Checkout".into(),
            ..shop_input()
        })
        .await;
    let moved = f
        .board
        .move_ticket(&t.key.to_string(), DropTarget::ReadyForDev, false, &me())
        .await
        .unwrap();
    assert_eq!(
        (moved.column, moved.status),
        (Column::ReadyForDev, Status::Queued)
    );
    assert!(moved.base_locked && moved.has_started());

    // No description at all: queued without a confirmation (d0067).
    let bare = f.create("Order history").await;
    let moved = f
        .board
        .move_ticket(&bare.key.to_string(), DropTarget::ReadyForDev, false, &me())
        .await
        .unwrap();
    assert_eq!(moved.status, Status::Queued);

    // A child locks its epic's base too; the epic itself can't go to Dev.
    let epic = f.epic("Wishlist").await;
    let child = f.child(&epic, "API").await;
    f.board
        .move_ticket(&child.key.to_string(), DropTarget::ReadyForDev, true, &me())
        .await
        .unwrap();
    assert!(f.ticket(&epic.key.to_string()).await.base_locked);
    let err = f
        .board
        .move_ticket(&epic.key.to_string(), DropTarget::ReadyForDev, true, &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::MoveRefused));
    assert_eq!(err.to_string(), "Epics don't go to Dev");
}

#[tokio::test]
async fn later_moves_are_allowed_but_not_available_yet() {
    let f = Fixture::shop().await;
    let t = f.create("Checkout").await;
    let key = t.key.to_string();
    // A working ticket: Park needs a confirmation.
    sqlx::query("UPDATE tickets SET state = 'working', board_column = 'dev' WHERE id = ?")
        .bind(&t.id)
        .execute(f.board.store().pool())
        .await
        .unwrap();
    let err = f
        .board
        .move_ticket(&key, DropTarget::Backlog, false, &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NeedsConfirmation));
    let err = f
        .board
        .move_ticket(&key, DropTarget::Backlog, true, &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NotAvailableYet));
    assert_eq!(err.to_string(), "Park isn't available yet");
    let err = f
        .board
        .move_ticket(&key, DropTarget::ReadyForReview, true, &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::MoveRefused));
}

#[tokio::test]
async fn reopen_of_a_never_started_closed_ticket_is_bookkeeping() {
    let f = Fixture::shop().await;
    let epic = f.epic("Search").await;
    f.board
        .close_epic(&epic.key.to_string(), &me())
        .await
        .unwrap();
    let reopened = f
        .board
        .move_ticket(&epic.key.to_string(), DropTarget::Backlog, false, &me())
        .await
        .unwrap();
    assert_eq!(
        (reopened.column, reopened.status),
        (Column::Backlog, Status::Backlog)
    );

    let t = f.create("Started then closed").await;
    f.board
        .move_ticket(&t.key.to_string(), DropTarget::ReadyForDev, true, &me())
        .await
        .unwrap();
    sqlx::query(
        "UPDATE tickets SET board_column = 'done', state = 'closed_not_merged' WHERE id = ?",
    )
    .bind(&t.id)
    .execute(f.board.store().pool())
    .await
    .unwrap();
    // Started work reopens too (T-0101): its next start rebases the old branch first.
    let reopened = f
        .board
        .move_ticket(&t.key.to_string(), DropTarget::Backlog, false, &me())
        .await
        .unwrap();
    assert_eq!(reopened.status, Status::Backlog);
    let row = f.board.store().ticket(&t.id).await.unwrap().unwrap();
    assert!(row.reopened);
}

/// Ready for Dev → Backlog (d0066): out of the queue; a ticket that never worked is "never started" again, so it
/// can still be deleted (d0060).
#[tokio::test]
async fn unqueue_takes_it_out_of_the_queue() {
    let f = Fixture::shop().await;
    let t = f.create("Checkout").await;
    let key = t.key.to_string();
    for target in [DropTarget::Dev, DropTarget::ReadyForDev] {
        let queued = f
            .board
            .move_ticket(&key, target, false, &me())
            .await
            .unwrap();
        assert_eq!(
            (queued.column, queued.status),
            (Column::ReadyForDev, Status::Queued),
            "{target}"
        );
        let back = f
            .board
            .move_ticket(&key, DropTarget::Backlog, false, &me())
            .await
            .unwrap();
        assert_eq!(
            (back.column, back.status),
            (Column::Backlog, Status::Backlog)
        );
        assert_eq!((back.queued_at, back.ready_since), (None, None));
        assert!(
            !back.has_started() && !back.base_locked,
            "nothing had started"
        );
    }
    f.board.delete_ticket(&key).await.unwrap();
}

/// "Ready to start" (d0066): a Backlog ticket whose parents are all merged; never an epic, and it never starts.
#[tokio::test]
async fn backlog_tickets_with_merged_parents_are_ready_to_start() {
    let f = Fixture::shop().await;
    let parent = f.create("Checkout API").await;
    let child = f.create("Checkout page").await;
    let epic = f.epic("Wishlist").await;
    f.board
        .add_dependency(&child.key.to_string(), &parent.key.to_string(), &me())
        .await
        .unwrap();
    let flags = |tickets: &[crewboard_core::board::tickets::Ticket], key: &TicketKey| {
        tickets
            .iter()
            .find(|t| &t.key == key)
            .unwrap()
            .flags
            .ready_to_start
    };
    let tickets = f.board.tickets("SHOP").await.unwrap();
    assert!(flags(&tickets, &parent.key), "no parents: unblocked");
    assert!(!flags(&tickets, &child.key), "waits for SHOP's parent");
    assert!(!flags(&tickets, &epic.key), "never an epic");
    sqlx::query("UPDATE tickets SET state = 'merged', board_column = 'done' WHERE id = ?")
        .bind(&parent.id)
        .execute(f.board.store().pool())
        .await
        .unwrap();
    let tickets = f.board.tickets("SHOP").await.unwrap();
    assert!(flags(&tickets, &child.key));
    assert_eq!(
        f.ticket(&child.key.to_string()).await.status,
        Status::Backlog
    );
}
