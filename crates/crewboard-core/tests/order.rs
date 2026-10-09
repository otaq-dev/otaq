//! Pinning and column order (TKT-06, SDD §7.5) with the shop-web sample.

mod common;

use common::{Fixture, me};
use crewboard_core::board::order::{blocked_counts, fold_dev, sort_column};
use crewboard_core::board::tickets::{Base, Ticket};
use crewboard_core::domain::{Actor, Column, Flags, Kind, Status, TicketKey};

fn t(key: &str, status: Status, created_at: i64) -> Ticket {
    Ticket {
        id: key.into(),
        key: key.parse::<TicketKey>().unwrap(),
        kind: Kind::Ticket,
        title: key.into(),
        body: String::new(),
        done_when: Vec::new(),
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
        created_at,
        updated_at: created_at,
        work: None,
        start_problem: None,
    }
}

fn deps(mut ticket: Ticket, parents: &[&str]) -> Ticket {
    ticket.depends_on = parents.iter().map(|p| p.parse().unwrap()).collect();
    ticket
}

fn pinned(mut ticket: Ticket, at: i64) -> Ticket {
    ticket.flags.pinned_at = Some(at);
    ticket
}

fn keys(list: &[TicketKey]) -> Vec<String> {
    list.iter().map(ToString::to_string).collect()
}

/// The Dev column of the sample board.
fn sample() -> Vec<Ticket> {
    vec![
        t("SHOP-17", Status::Merged, 1),
        t("SHOP-18", Status::Working, 2),
        pinned(t("SHOP-19", Status::Working, 3), 100),
        deps(t("SHOP-20", Status::Queued, 4), &["SHOP-18", "SHOP-19"]),
        deps(t("SHOP-21", Status::Queued, 5), &["SHOP-20"]),
        t("SHOP-22", Status::NeedsYou, 6),
        t("SHOP-23", Status::AsksYou, 7),
        t("SHOP-24", Status::Backlog, 8),
        t("SHOP-25", Status::WorkingInBackground, 9),
        t("SHOP-26", Status::Interrupted, 10),
        t("SHOP-27", Status::WaitingForLimitReset, 11),
        t("SHOP-28", Status::StartInterrupted, 12),
        t("SHOP-29", Status::Held, 13),
    ]
}

#[test]
fn dev_order() {
    let tickets = sample();
    let order = sort_column(Column::Dev, &tickets);
    assert_eq!(
        keys(&order.cards),
        [
            "SHOP-19", // pinned, above orange
            "SHOP-22", "SHOP-23", // orange
            "SHOP-26", "SHOP-28", // Interrupted, Start interrupted (floats up from Queued)
            "SHOP-18", // working, blocks SHOP-20 and SHOP-21
            "SHOP-25", "SHOP-27", // working, oldest first
        ]
    );
    assert_eq!(keys(&order.queued), ["SHOP-20", "SHOP-21", "SHOP-29"]);
    assert!(
        sort_column(Column::Dev, &tickets)
            .queued
            .iter()
            .all(|k| k.to_string() != "SHOP-28")
    );
}

#[test]
fn blocks_more_counts_direct_and_indirect_dependents() {
    let counts = blocked_counts(&sample());
    let of = |k: &str| counts[&k.parse::<TicketKey>().unwrap()];
    assert_eq!(
        (of("SHOP-18"), of("SHOP-19"), of("SHOP-20"), of("SHOP-21")),
        (2, 2, 1, 0)
    );
}

#[test]
fn pins_in_every_column_most_recent_first() {
    let tickets = vec![
        t("SHOP-30", Status::ReadyForReview, 1),
        pinned(t("SHOP-31", Status::ReadyForReview, 2), 10),
        pinned(t("SHOP-32", Status::ReadyForReview, 3), 20),
        pinned(t("SHOP-33", Status::Queued, 4), 30),
        t("SHOP-34", Status::Queued, 5),
    ];
    assert_eq!(
        keys(&sort_column(Column::ReadyForReview, &tickets).cards),
        ["SHOP-32", "SHOP-31", "SHOP-30"]
    );
    let dev = sort_column(Column::Dev, &tickets);
    assert!(dev.cards.is_empty());
    assert_eq!(
        keys(&dev.queued),
        ["SHOP-33", "SHOP-34"],
        "pins are first in Queued too"
    );
}

#[test]
fn fold_never_hides_needs_you_cards() {
    let tickets = sample();
    let order = sort_column(Column::Dev, &tickets);
    let fold = fold_dev(&order, &tickets, 6);
    // 4 needs-you cards always shown, 2 more slots: SHOP-19 (pinned) and SHOP-18.
    assert_eq!(
        keys(&fold.shown_cards),
        [
            "SHOP-19", "SHOP-22", "SHOP-23", "SHOP-26", "SHOP-28", "SHOP-18"
        ]
    );
    assert!(fold.shown_queued.is_empty());
    assert_eq!((fold.hidden_working, fold.hidden_queued), (2, 3));
    assert_eq!(
        fold.label().as_deref(),
        Some("5 more · 2 working · 3 queued")
    );

    let fold = fold_dev(&order, &tickets, 1);
    assert_eq!(
        fold.shown_cards.len(),
        4,
        "needs-you cards stay even past the limit"
    );
    assert!(fold_dev(&order, &tickets, 50).label().is_none());
}

#[tokio::test]
async fn pin_and_unpin() {
    let f = Fixture::shop().await;
    let a = f.create("A").await;
    let b = f.create("B").await;
    let c = f.create("C").await;
    let order = |tickets: &[Ticket]| keys(&sort_column(Column::Backlog, tickets).cards);

    f.board.pin(&b.key.to_string(), &me()).await.unwrap();
    f.board.pin(&c.key.to_string(), &me()).await.unwrap();
    assert_eq!(
        order(&f.board.tickets("SHOP").await.unwrap()),
        ["SHOP-3", "SHOP-2", "SHOP-1"]
    );
    f.board.pin(&b.key.to_string(), &me()).await.unwrap();
    assert_eq!(
        order(&f.board.tickets("SHOP").await.unwrap()),
        ["SHOP-2", "SHOP-3", "SHOP-1"],
        "re-pin bumps"
    );
    let unpinned = f.board.unpin(&b.key.to_string(), &me()).await.unwrap();
    assert!(!unpinned.flags.is_pinned());
    assert_eq!(
        (unpinned.column, unpinned.status),
        (Column::Backlog, Status::Backlog),
        "pinning changes no state"
    );
    assert_eq!(
        order(&f.board.tickets("SHOP").await.unwrap()),
        ["SHOP-3", "SHOP-1", "SHOP-2"]
    );
    let _ = a;

    // The pin follows the ticket across columns.
    f.board
        .move_ticket(
            "SHOP-3",
            crewboard_core::board::moves::DropTarget::DevQueued,
            true,
            &me(),
        )
        .await
        .unwrap();
    let tickets = f.board.tickets("SHOP").await.unwrap();
    assert_eq!(keys(&sort_column(Column::Dev, &tickets).queued), ["SHOP-3"]);
    assert!(f.ticket("SHOP-3").await.flags.is_pinned());
}
