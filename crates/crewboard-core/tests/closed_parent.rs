//! Closed parent (T-0102, CLS-04): a Closed parent never counts; the child names every closed parent, sits in the
//! needs-you group "stopped", and the user picks Reopen parent or Remove dependency.

mod common;

use common::{Fixture, me};
use crewboard_core::board::needs_you::{NeedsYouGroup, group_of};
use crewboard_core::board::readiness::{ReadyState, readiness};
use crewboard_core::domain::Status;
use crewboard_core::scheduler::{Gate, plan};

#[tokio::test]
async fn closed_parents_hold_the_child_until_the_user_chooses() {
    let f = Fixture::shop().await;
    let a = f.create("Checkout API").await;
    let b = f.create("Price table").await;
    let child = f.create("Checkout page").await;
    f.board
        .add_dependency("SHOP-3", "SHOP-1", &me())
        .await
        .unwrap();
    f.board
        .add_dependency("SHOP-3", "SHOP-2", &me())
        .await
        .unwrap();
    f.force_status(&a, Status::ClosedNotMerged).await;
    f.force_status(&b, Status::ClosedNotMerged).await;
    f.force_status(&child, Status::Queued).await;

    let tickets = f.board.tickets("SHOP").await.unwrap();
    let child_now = tickets.iter().find(|t| t.key == child.key).unwrap();
    let state = readiness(child_now, &tickets);
    assert_eq!(
        state,
        ReadyState::ParentClosed {
            parents: vec![a.key.clone(), b.key.clone()]
        }
    );
    assert_eq!(state.reason().unwrap(), "parents SHOP-1, SHOP-2 closed");
    // The scheduler moves it to Parent closed; that status is a needs-you item ("stopped" group).
    let plan = plan(&tickets, &Default::default(), &Gate::default());
    assert!(
        plan.changes
            .iter()
            .any(|c| c.key == child.key && c.to == Status::ParentClosed)
    );
    assert_eq!(group_of(Status::ParentClosed), Some(NeedsYouGroup::Stopped));
    f.force_status(&child, Status::ParentClosed).await;
    let items = f.board.needs_you_items(0).await.unwrap();
    let item = items.iter().find(|i| i.key == child.key).unwrap();
    assert_eq!(item.group, NeedsYouGroup::Stopped);

    // Reopen parent: SHOP-1 is back in Backlog, so the child now waits for it.
    f.board.reopen("SHOP-1", &me()).await.unwrap();
    let tickets = f.board.tickets("SHOP").await.unwrap();
    let child_now = tickets.iter().find(|t| t.key == child.key).unwrap();
    assert_eq!(
        readiness(child_now, &tickets),
        ReadyState::ParentClosed {
            parents: vec![b.key.clone()]
        }
    );
    assert_eq!(
        readiness(child_now, &tickets).reason().unwrap(),
        "parent SHOP-2 closed"
    );

    // Remove dependency: the link goes and readiness is evaluated again.
    f.board
        .remove_dependency("SHOP-3", "SHOP-2", &me())
        .await
        .unwrap();
    let tickets = f.board.tickets("SHOP").await.unwrap();
    let child_now = tickets.iter().find(|t| t.key == child.key).unwrap();
    assert!(matches!(
        readiness(child_now, &tickets),
        ReadyState::Waiting { .. }
    ));
}
