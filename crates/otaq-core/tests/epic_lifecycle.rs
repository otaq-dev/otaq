//! The epic as a delivery unit (T-0105, EPC-04, EPC-05): Review once every child is done with one merged, Closed ·
//! not merged when all closed, its own Local merge into the base, no Send back, and a new child sends it back
//! to Backlog.

use std::path::PathBuf;

use otaq_core::board::tickets::{NewTicketInput, Ticket};
use otaq_core::domain::{Actor, Kind, Status};
use otaq_core::git::testing::{commit, git};
use otaq_core::review::LocalMergeOutcome;
use otaq_core::service::testing::TestService;

fn me() -> Actor {
    Actor::User("me".into())
}

async fn epic(w: &TestService) -> Ticket {
    w.board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Wishlist".into(),
                kind: Kind::Epic,
                external_id: Some("PAY-123".into()),
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap()
}

async fn child(w: &TestService, epic: &Ticket, title: &str) -> Ticket {
    w.board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: title.into(),
                epic: Some(epic.key.to_string()),
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap()
}

/// Works on a child (one file) and merges it into the epic branch.
async fn merge_child(w: &TestService, child: &Ticket, file: &str) {
    let wt = w.start_worktree(child).await;
    commit(&wt, file, &format!("{file}\n"));
    w.force_status(child, Status::ReadyForReview).await;
    let outcome = w
        .review
        .local_merge(
            &child.key.to_string(),
            &format!("{}: {}", child.key, child.title),
            false,
            &me(),
        )
        .await
        .unwrap();
    assert!(
        matches!(outcome, LocalMergeOutcome::Merged { .. }),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn an_epic_reaches_review_then_local_merges_into_main() {
    let w = TestService::start().await;
    let project = w.add_project("shop-web").await;
    let repo = PathBuf::from(&project.repo_path);
    let epic = epic(&w).await;
    let api = child(&w, &epic, "Wishlist API").await;
    let button = child(&w, &epic, "Wishlist button").await;
    let page = child(&w, &epic, "Wishlist page").await;
    let main_before = git(&repo, &["rev-parse", "main"]);

    merge_child(&w, &api, "api.ts").await;
    assert_eq!(
        w.status("SHOP-1").await,
        Status::Backlog,
        "two children left"
    );
    merge_child(&w, &button, "button.ts").await;
    assert_eq!(w.status("SHOP-1").await, Status::Backlog);
    w.review
        .close(&page.key.to_string(), true, &me())
        .await
        .unwrap();
    assert_eq!(
        w.status("SHOP-1").await,
        Status::ReadyForReview,
        "every child done, two merged"
    );
    assert_eq!(
        git(&repo, &["rev-parse", "main"]),
        main_before,
        "children merged into the epic only"
    );

    // No Send back on an epic.
    let err = w
        .review
        .send_back("SHOP-1", "", false, &me())
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "Add or reopen a child ticket to change it");

    // The epic's own Local merge: the epic branch, squashed into main.
    let proposal = w.board.merge_proposal("SHOP-1").await.unwrap();
    assert_eq!(proposal.target, "main");
    assert!(
        proposal.message.contains("- SHOP-1.1 Wishlist API"),
        "{}",
        proposal.message
    );
    let outcome = w
        .review
        .local_merge("SHOP-1", &proposal.message, false, &me())
        .await
        .unwrap();
    assert!(
        matches!(outcome, LocalMergeOutcome::Merged { .. }),
        "{outcome:?}"
    );
    assert_eq!(w.status("SHOP-1").await, Status::Merged);
    assert_eq!(
        git(&repo, &["rev-parse", "main~1"]),
        main_before,
        "one squash commit"
    );
    assert!(repo.join("api.ts").is_file() && repo.join("button.ts").is_file());
    w.stop().await;
}

#[tokio::test]
async fn an_epic_whose_children_all_closed_is_closed_not_merged() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let epic = epic(&w).await;
    let a = child(&w, &epic, "A").await;
    let b = child(&w, &epic, "B").await;
    w.review
        .close(&a.key.to_string(), true, &me())
        .await
        .unwrap();
    assert_eq!(w.status("SHOP-1").await, Status::Backlog);
    w.review
        .close(&b.key.to_string(), true, &me())
        .await
        .unwrap();
    assert_eq!(w.status("SHOP-1").await, Status::ClosedNotMerged);
    w.stop().await;
}

#[tokio::test]
async fn a_new_child_returns_an_epic_in_review_to_backlog() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let epic = epic(&w).await;
    let api = child(&w, &epic, "Wishlist API").await;
    merge_child(&w, &api, "api.ts").await;
    assert_eq!(w.status("SHOP-1").await, Status::ReadyForReview);

    child(&w, &epic, "Wishlist share link").await;
    assert_eq!(
        w.status("SHOP-1").await,
        Status::Backlog,
        "until the new child is merged"
    );
    w.stop().await;
}
