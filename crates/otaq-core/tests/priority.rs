//! Priorities (T-0177, TKT-09; d0065): the default, an epic child following its epic, its own override, and
//! pinning leaving the field alone.

mod common;

use common::{Fixture, agent, me, shop_input};
use otaq_core::board::tickets::{NewTicketInput, TicketUpdate};
use otaq_core::domain::Priority;

fn set(priority: Priority) -> TicketUpdate {
    TicketUpdate {
        priority: Some(priority),
        ..Default::default()
    }
}

#[tokio::test]
async fn new_tickets_and_epics_are_normal() {
    let f = Fixture::shop().await;
    let ticket = f.create("Checkout").await;
    let epic = f.epic("Wishlist").await;
    for t in [&ticket, &epic] {
        assert_eq!(t.priority, Priority::Normal);
        assert!(!t.priority_inherited);
    }
    let urgent = f
        .create_with(NewTicketInput {
            title: "Payments down".into(),
            priority: Some(Priority::Urgent),
            ..shop_input()
        })
        .await;
    assert_eq!(urgent.priority, Priority::Urgent);
}

#[tokio::test]
async fn children_follow_their_epic_until_they_have_their_own() {
    let f = Fixture::shop().await;
    let epic = f.epic("Wishlist").await;
    let key = epic.key.to_string();
    f.board
        .update_ticket(&key, &set(Priority::High), &me())
        .await
        .unwrap();
    let child = f.child(&epic, "API").await;
    let other = f.child(&epic, "Button").await;
    assert_eq!(child.priority, Priority::High);
    assert!(child.priority_inherited);

    // The epic changes: children without their own follow.
    f.board
        .update_ticket(&key, &set(Priority::Urgent), &me())
        .await
        .unwrap();
    assert_eq!(
        f.ticket(&child.key.to_string()).await.priority,
        Priority::Urgent
    );

    // A child's own priority wins, also after the epic changes again.
    let own = f
        .board
        .update_ticket(
            &other.key.to_string(),
            &set(Priority::Low),
            &agent("SHOP-9"),
        )
        .await
        .unwrap();
    assert_eq!(own.priority, Priority::Low);
    assert!(!own.priority_inherited);
    f.board
        .update_ticket(&key, &set(Priority::Normal), &me())
        .await
        .unwrap();
    assert_eq!(
        f.ticket(&other.key.to_string()).await.priority,
        Priority::Low
    );
    assert_eq!(
        f.ticket(&child.key.to_string()).await.priority,
        Priority::Normal
    );
}

#[tokio::test]
async fn a_change_is_recorded_and_pinning_never_changes_it() {
    let f = Fixture::shop().await;
    let t = f.create("Checkout").await;
    let key = t.key.to_string();
    f.board
        .update_ticket(&key, &set(Priority::High), &agent("SHOP-9"))
        .await
        .unwrap();
    let edits = f.board.agent_edits(&key).await.unwrap();
    let change = &edits.changes[0].change;
    assert_eq!(change.before.as_deref(), Some("P3 · Normal"));
    assert_eq!(change.after.as_deref(), Some("P2 · High"));

    f.board.pin(&key, &me()).await.unwrap();
    let pinned = f.ticket(&key).await;
    assert!(pinned.flags.pinned_at.is_some());
    assert_eq!(pinned.priority, Priority::High);
    f.board.unpin(&key, &me()).await.unwrap();
    assert_eq!(f.ticket(&key).await.priority, Priority::High);

    // The same value again is no change at all.
    let before = f.board.history(&key).await.unwrap().len();
    f.board
        .update_ticket(&key, &set(Priority::High), &me())
        .await
        .unwrap();
    assert_eq!(f.board.history(&key).await.unwrap().len(), before);
}
