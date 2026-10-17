//! Dependencies (DEP-01, DEP-02, DEP-04, PRJ-05) with the shop-web sample: SHOP-17 → SHOP-18 → SHOP-20 ← SHOP-19,
//! SHOP-21 depending on SHOP-20, the hotfix SHOP-31 on `release/2.3`, and SHOP-50 depending on epic SHOP-40.

use crate::common::{Fixture, me, shop_input};
use otaq_core::board::Refusal;
use otaq_core::board::deps::DepNode;
use otaq_core::board::tickets::{NewTicketInput, TicketUpdate};
use otaq_core::domain::Status;

/// Creates SHOP-17…SHOP-21 with the sample links.
async fn sample() -> Fixture {
    let f = Fixture::shop().await;
    f.skip_to(17).await;
    for title in [
        "Cart totals",
        "Checkout API",
        "Payment form",
        "Checkout page",
        "Order confirmation email",
    ] {
        f.create(title).await;
    }
    for (child, parent) in [
        ("SHOP-18", "SHOP-17"),
        ("SHOP-20", "SHOP-18"),
        ("SHOP-20", "SHOP-19"),
        ("SHOP-21", "SHOP-20"),
    ] {
        f.board.add_dependency(child, parent, &me()).await.unwrap();
    }
    f
}

fn keys(nodes: &[DepNode]) -> Vec<String> {
    nodes.iter().map(|n| n.key.to_string()).collect()
}

#[tokio::test]
async fn links_and_tree() {
    let f = sample().await;
    let t20 = f.ticket("SHOP-20").await;
    assert_eq!(
        t20.depends_on
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["SHOP-18", "SHOP-19"]
    );

    let tree = f.board.dependency_tree("SHOP-20").await.unwrap();
    assert_eq!(keys(&tree.parents), ["SHOP-18", "SHOP-19"]);
    assert_eq!(keys(&tree.parents[0].nodes), ["SHOP-17"]);
    assert_eq!(tree.parents[0].status, Status::Backlog);
    assert_eq!(keys(&tree.children), ["SHOP-21"]);

    let tree = f.board.dependency_tree("SHOP-17").await.unwrap();
    assert!(tree.parents.is_empty());
    assert_eq!(keys(&tree.children), ["SHOP-18"]);
    assert_eq!(keys(&tree.children[0].nodes), ["SHOP-20"]);
    assert_eq!(keys(&tree.children[0].nodes[0].nodes), ["SHOP-21"]);

    assert!(f.board.dependency_cycles("SHOP").await.unwrap().is_empty());

    f.board
        .remove_dependency("SHOP-20", "SHOP-19", &me())
        .await
        .unwrap();
    assert_eq!(
        keys(&f.board.dependency_tree("SHOP-20").await.unwrap().parents),
        ["SHOP-18"]
    );
    assert!(
        f.board
            .remove_dependency("SHOP-20", "SHOP-19", &me())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn refusals() {
    let f = sample().await;
    let err = f
        .board
        .add_dependency("SHOP-20", "SHOP-21", &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::Cycle));
    assert!(
        err.to_string().ends_with("SHOP-21 → SHOP-20 → SHOP-21"),
        "{err}"
    );

    let err = f
        .board
        .add_dependency("SHOP-17", "SHOP-21", &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::Cycle));
    assert!(
        err.to_string()
            .ends_with("SHOP-21 → SHOP-17 → SHOP-18 → SHOP-20 → SHOP-21"),
        "{err}"
    );

    let err = f
        .board
        .add_dependency("SHOP-18", "SHOP-18", &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::SelfDependency));
    let err = f
        .board
        .add_dependency("SHOP-20", "SHOP-18", &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::DuplicateDependency));

    f.add_project("billing-api").await;
    f.create_with(NewTicketInput {
        project: "BILL".into(),
        title: "Invoices".into(),
        ..Default::default()
    })
    .await;
    let err = f
        .board
        .add_dependency("SHOP-20", "BILL-1", &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::OtherProject));

    assert!(f.board.dependency_cycles("SHOP").await.unwrap().is_empty());
}

#[tokio::test]
async fn linked_tickets_share_a_base() {
    let f = sample().await;
    f.skip_to(31).await;
    let hotfix = f
        .create_with(NewTicketInput {
            title: "Hotfix".into(),
            base_branch: Some("release/2.3".into()),
            ..shop_input()
        })
        .await;
    assert_eq!(hotfix.key.to_string(), "SHOP-31");
    let err = f
        .board
        .add_dependency("SHOP-31", "SHOP-20", &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::CrossBase));
    assert_eq!(
        err.to_string(),
        "SHOP-31 merges into `release/2.3`, SHOP-20 into `main`. Linked tickets must use the same base branch."
    );

    // Changing a linked ticket's base is refused with the same message.
    let err = f
        .board
        .update_ticket(
            "SHOP-21",
            &TicketUpdate {
                base_branch: Some("release/2.3".into()),
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::CrossBase));
    assert!(
        err.to_string()
            .starts_with("SHOP-21 merges into `release/2.3`, SHOP-20 into `main`."),
        "{err}"
    );
    // An unlinked ticket can change its base.
    f.board
        .update_ticket(
            "SHOP-31",
            &TicketUpdate {
                base_branch: Some("main".into()),
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    f.board
        .add_dependency("SHOP-31", "SHOP-20", &me())
        .await
        .unwrap();
}

#[tokio::test]
async fn depending_on_an_epic() {
    let f = sample().await;
    f.skip_to(40).await;
    let epic = f.epic("Wishlist").await;
    assert_eq!(epic.key.to_string(), "SHOP-40");
    let c1 = f.child(&epic, "Wishlist API").await;
    let c2 = f.child(&epic, "Wishlist UI").await;
    f.skip_to(50).await;
    let dependent = f.create("Share wishlist").await;
    assert_eq!(dependent.key.to_string(), "SHOP-50");

    f.board
        .add_dependency("SHOP-50", "SHOP-40", &me())
        .await
        .unwrap();
    // Siblings share the epic's branch.
    f.board
        .add_dependency(&c2.key.to_string(), &c1.key.to_string(), &me())
        .await
        .unwrap();

    let err = f
        .board
        .add_dependency(&c1.key.to_string(), "SHOP-40", &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::DependsOnOwnEpic));
    let err = f
        .board
        .add_dependency(&c1.key.to_string(), "SHOP-20", &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::CrossBase));
    assert_eq!(
        err.to_string(),
        "SHOP-40.1 merges into the epic's branch (SHOP-40), SHOP-20 into `main`. Linked tickets must use the same base branch."
    );

    let hotfix = f
        .create_with(NewTicketInput {
            title: "Hotfix".into(),
            base_branch: Some("release/2.3".into()),
            ..shop_input()
        })
        .await;
    let err = f
        .board
        .add_dependency(&hotfix.key.to_string(), "SHOP-40", &me())
        .await
        .unwrap_err();
    assert_eq!(
        err.refusal(),
        Some(Refusal::CrossBase),
        "a dependent needs the epic's base"
    );

    // The epic's base can't move away from its dependents' base.
    let err = f
        .board
        .update_ticket(
            "SHOP-40",
            &TicketUpdate {
                base_branch: Some("develop".into()),
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::CrossBase));
}

#[tokio::test]
async fn cycles_finds_loops_written_around_the_rules() {
    let f = sample().await;
    let (t17, t21) = (f.ticket("SHOP-17").await, f.ticket("SHOP-21").await);
    sqlx::query("INSERT INTO ticket_deps (ticket_id, depends_on_id) VALUES (?, ?)")
        .bind(&t17.id)
        .bind(&t21.id)
        .execute(f.board.store().pool())
        .await
        .unwrap();
    let cycles = f.board.dependency_cycles("SHOP").await.unwrap();
    assert_eq!(cycles.len(), 1);
    let lap: Vec<_> = cycles[0].iter().map(ToString::to_string).collect();
    assert_eq!(lap.first(), lap.last());
    assert_eq!(lap.len(), 5, "{lap:?}");
    // The tree still terminates.
    f.board.dependency_tree("SHOP-20").await.unwrap();
}
