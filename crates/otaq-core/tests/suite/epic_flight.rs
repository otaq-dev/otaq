//! Epics in flight (EPC-11, EPC-13; d0080): the epic's place (card → track → card, delivery), the child groups,
//! and the stored queue mode and "joined the queue" mark. The example is shop-web's SHOP-45 Search (45.1 Search
//! index API, 45.2 Search results page).

use crate::common::{Fixture, agent, me};
use otaq_core::board::Refusal;
use otaq_core::board::epic_flight::{
    EpicPlace, EpicQueueMode, active, been_to_dev, epic_place, never_started, parked, running,
};
use otaq_core::board::moves::DropTarget;
use otaq_core::board::tickets::{Base, EventKind, Ticket, Work};
use otaq_core::domain::{Actor, Flags, Kind, Status, TicketKey};

fn t(key: &str, status: Status) -> Ticket {
    let key: TicketKey = key.parse().unwrap();
    let epic = key.child.map(|_| TicketKey {
        child: None,
        ..key.clone()
    });
    Ticket {
        id: key.to_string(),
        kind: if epic.is_some() {
            Kind::Ticket
        } else {
            Kind::Epic
        },
        title: key.to_string(),
        body: String::new(),
        priority: Default::default(),
        priority_inherited: false,
        base: match &epic {
            Some(e) => Base::Epic(e.clone()),
            None => Base::Branch("main".into()),
        },
        epic,
        key,
        column: status.column(),
        status,
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
        validation_round: 0,
        auto_merge_conflicts: 0,
        auto_merge_stop: None,
        work: None,
        start_problem: None,
        interrupted_from: None,
        signal: None,
        question: None,
        pushed_differs: None,
        epic_queue_mode: EpicQueueMode::None,
        joined_queue_by: None,
    }
}

/// A child that has worked: it has a branch and a worktree, and its first start is recorded.
fn worked(mut ticket: Ticket) -> Ticket {
    ticket.started_at = Some(1);
    ticket.work = Some(Work {
        branch: Some(format!("otaq/{}", ticket.key)),
        worktree: Some(format!("/tmp/{}", ticket.key)),
        ..Work::default()
    });
    ticket
}

/// A child the user queued: Ready for Dev, its first start recorded, nothing worked yet.
fn queued(mut ticket: Ticket) -> Ticket {
    ticket.started_at = Some(1);
    ticket.queued_at = Some(1);
    ticket
}

fn place(epic: Status, children: Vec<Ticket>) -> EpicPlace {
    let epic = t("SHOP-45", epic);
    epic_place(&epic, &children)
}

#[test]
fn an_epic_with_nothing_started_is_a_card() {
    assert_eq!(
        place(
            Status::Backlog,
            vec![
                t("SHOP-45.1", Status::Backlog),
                t("SHOP-45.2", Status::Backlog)
            ]
        ),
        EpicPlace::Card
    );
    assert_eq!(place(Status::Backlog, Vec::new()), EpicPlace::Card);
}

/// Every child never started, but one is queued: a track (its other child is a "not queued" chip).
#[test]
fn one_queued_child_makes_a_track() {
    let children = vec![
        queued(t("SHOP-45.1", Status::Queued)),
        t("SHOP-45.2", Status::Backlog),
    ];
    assert_eq!(place(Status::Backlog, children.clone()), EpicPlace::Track);
    let keys = |list: Vec<&Ticket>| list.iter().map(|c| c.key.to_string()).collect::<Vec<_>>();
    assert_eq!(keys(never_started(&children)), ["SHOP-45.1", "SHOP-45.2"]);
    assert_eq!(keys(active(&children)), ["SHOP-45.1"]);
    assert!(running(&children).is_empty());
}

/// Flow 3: "SHOP-45.1 has started, so the track stays" after 45.2 is taken back.
#[test]
fn a_merged_child_keeps_the_track_after_a_take_back() {
    let children = vec![
        worked(t("SHOP-45.1", Status::Merged)),
        t("SHOP-45.2", Status::Backlog),
    ];
    assert_eq!(place(Status::Backlog, children), EpicPlace::Track);
    // Merged counts as started even once its worktree is gone.
    let mut merged = t("SHOP-45.1", Status::Merged);
    merged.started_at = Some(1);
    assert!(been_to_dev(&merged));
    assert_eq!(
        place(
            Status::Backlog,
            vec![merged, t("SHOP-45.2", Status::Backlog)]
        ),
        EpicPlace::Track
    );
}

/// After Park all the epic is a card again, its rows "Parked · has work".
#[test]
fn park_all_makes_a_card() {
    let children = vec![
        worked(t("SHOP-45.1", Status::Parked)),
        worked(t("SHOP-45.2", Status::Parked)),
    ];
    assert_eq!(place(Status::Backlog, children.clone()), EpicPlace::Card);
    assert_eq!(parked(&children).len(), 2);
    assert!(never_started(&children).is_empty());
    // A single Park beside a merged child: still a card.
    let children = vec![
        worked(t("SHOP-45.1", Status::Merged)),
        worked(t("SHOP-45.2", Status::Parked)),
    ];
    assert_eq!(place(Status::Backlog, children), EpicPlace::Card);
    // A parked child resuming is in Ready for Dev with its work: active, so a track.
    let children = vec![
        worked(t("SHOP-45.1", Status::Queued)),
        worked(t("SHOP-45.2", Status::Parked)),
    ];
    assert_eq!(place(Status::Backlog, children.clone()), EpicPlace::Track);
    assert!(never_started(&children).is_empty());
}

#[test]
fn running_and_reviewed_children_make_a_track() {
    let children = vec![
        worked(t("SHOP-45.1", Status::Working)),
        t("SHOP-45.2", Status::Backlog),
    ];
    assert_eq!(place(Status::Backlog, children.clone()), EpicPlace::Track);
    assert_eq!(running(&children).len(), 1);
    let children = vec![
        worked(t("SHOP-45.1", Status::ReadyForReview)),
        worked(t("SHOP-45.2", Status::Merged)),
    ];
    assert_eq!(place(Status::Backlog, children), EpicPlace::Track);
}

/// Every child merged: the epic's delivery card once `reevaluate_epic` moved it to Ready for Review; every child
/// Closed: "Closed · not merged", also a delivery card.
#[test]
fn a_done_epic_is_a_delivery_card() {
    let merged = vec![
        worked(t("SHOP-45.1", Status::Merged)),
        worked(t("SHOP-45.2", Status::Merged)),
    ];
    assert_eq!(
        place(Status::Backlog, merged.clone()),
        EpicPlace::Track,
        "until reevaluate_epic moves it"
    );
    assert_eq!(
        place(Status::ReadyForReview, merged.clone()),
        EpicPlace::Delivery
    );
    assert_eq!(place(Status::PrOpen, merged.clone()), EpicPlace::Delivery);
    assert_eq!(place(Status::Merged, merged), EpicPlace::Delivery);
    let closed = vec![
        t("SHOP-45.1", Status::ClosedNotMerged),
        t("SHOP-45.2", Status::ClosedNotMerged),
    ];
    assert_eq!(
        place(Status::ClosedNotMerged, closed.clone()),
        EpicPlace::Delivery
    );
    // Closed children that never started don't count as started.
    assert_eq!(place(Status::Backlog, closed), EpicPlace::Card);
}

/// A child reopened after Review (or added after it) returns the epic to Backlog (EPC-05): a track again.
#[test]
fn a_reopened_child_makes_a_track_again() {
    let children = vec![
        worked(t("SHOP-45.1", Status::Merged)),
        worked(t("SHOP-45.2", Status::Backlog)),
    ];
    assert_eq!(place(Status::Backlog, children), EpicPlace::Track);
    let children = vec![
        worked(t("SHOP-45.1", Status::Merged)),
        t("SHOP-45.3", Status::Backlog),
    ];
    assert_eq!(place(Status::Backlog, children), EpicPlace::Track);
}

/// The whole project can be passed: other epics' children and standalone tickets don't count.
#[test]
fn only_the_epics_own_children_count() {
    let mut standalone = worked(t("SHOP-51", Status::Working));
    standalone.kind = Kind::Ticket;
    let project = vec![
        t("SHOP-45", Status::Backlog),
        t("SHOP-45.1", Status::Backlog),
        worked(t("SHOP-40.1", Status::Working)),
        standalone,
    ];
    assert_eq!(epic_place(&project[0], &project), EpicPlace::Card);
}

/// A queued ticket has a first-start time but hasn't been to Dev; a worked one has.
#[test]
fn been_to_dev_ignores_the_queue_mark() {
    assert!(!been_to_dev(&queued(t("SHOP-45.1", Status::Queued))));
    assert!(been_to_dev(&worked(t("SHOP-45.1", Status::Queued))));
    assert!(been_to_dev(&t("SHOP-45.1", Status::Working)));
    assert!(!been_to_dev(&t("SHOP-45.1", Status::Backlog)));
    assert!(been_to_dev(&queued(t(
        "SHOP-45.1",
        Status::ClosedNotMerged
    ))));
}

async fn search() -> (Fixture, Ticket) {
    let f = Fixture::shop().await;
    f.skip_to(45).await;
    let epic = f.epic("Search").await;
    f.child(&epic, "Search index API").await;
    f.child(&epic, "Search results page").await;
    (f, epic)
}

/// The queue mode is stored on the epic, read back, and logged in its History once per change.
#[tokio::test]
async fn the_queue_mode_is_stored_and_logged() {
    let (f, epic) = search().await;
    assert_eq!(epic.epic_queue_mode, EpicQueueMode::None);
    let json = serde_json::to_value(&epic).unwrap();
    assert!(
        json.get("epic_queue_mode").is_none(),
        "serialised only when set"
    );

    let whole = f
        .board
        .set_queue_mode("SHOP-45", EpicQueueMode::Whole, &me())
        .await
        .unwrap();
    assert_eq!(whole.epic_queue_mode, EpicQueueMode::Whole);
    assert_eq!(
        f.ticket("SHOP-45").await.epic_queue_mode,
        EpicQueueMode::Whole
    );
    let json = serde_json::to_value(&whole).unwrap();
    assert_eq!(json["epic_queue_mode"], "whole");
    // The same mode again changes nothing.
    f.board
        .set_queue_mode("SHOP-45", EpicQueueMode::Whole, &me())
        .await
        .unwrap();
    f.board
        .set_queue_mode("SHOP-45", EpicQueueMode::None, &me())
        .await
        .unwrap();
    let logged: Vec<_> = f
        .board
        .history("SHOP-45")
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EventKind::EpicQueueModeSet)
        .map(|e| (e.payload["from"].clone(), e.payload["to"].clone()))
        .collect();
    assert_eq!(
        logged,
        [
            ("whole".into(), "none".into()),
            ("none".into(), "whole".into())
        ]
    );

    let refused = f
        .board
        .set_queue_mode("SHOP-45.1", EpicQueueMode::Whole, &me())
        .await
        .unwrap_err();
    assert_eq!(refused.refusal(), Some(Refusal::NotAnEpic));
    let child = f.ticket("SHOP-45.1").await;
    assert_eq!(child.epic_queue_mode, EpicQueueMode::None);
    drop(epic);
}

/// "new · added by SHOP-40.2": the store writes who added a child to the queue, the ticket reads it, and taking
/// the child back clears it.
#[tokio::test]
async fn joined_the_queue_by_is_stored_and_cleared_when_taken_back() {
    let (f, _) = search().await;
    let child = f.ticket("SHOP-45.2").await;
    assert_eq!(child.joined_queue_by, None);
    f.board
        .move_ticket("SHOP-45.2", DropTarget::ReadyForDev, false, &me())
        .await
        .unwrap();
    let by = agent("SHOP-45.1");
    f.board
        .store()
        .set_joined_queue_by(&child.id, Some(&by.to_string()))
        .await
        .unwrap();
    let joined = f.ticket("SHOP-45.2").await;
    assert_eq!(joined.joined_queue_by, Some(by));
    let json = serde_json::to_value(&joined).unwrap();
    assert_eq!(json["joined_queue_by"], "agent:SHOP-45.1");

    f.board
        .move_ticket("SHOP-45.2", DropTarget::Backlog, false, &me())
        .await
        .unwrap();
    let back = f.ticket("SHOP-45.2").await;
    assert_eq!(back.status, Status::Backlog);
    assert_eq!(back.joined_queue_by, None);

    f.board
        .store()
        .set_joined_queue_by(&child.id, Some(&me().to_string()))
        .await
        .unwrap();
    assert_eq!(f.ticket("SHOP-45.2").await.joined_queue_by, Some(me()));
    f.board
        .store()
        .set_joined_queue_by(&child.id, None)
        .await
        .unwrap();
    assert_eq!(f.ticket("SHOP-45.2").await.joined_queue_by, None);
}
