//! Close ticket and Close epic (T-0100, CLS-01, EPC-06): from any state, the agent stopped after a
//! confirmation, the branch kept, the worktree removed unless it has uncommitted changes.

use otaq_core::board::Refusal;
use otaq_core::board::tickets::{EventKind, NewTicketInput, Ticket};
use otaq_core::domain::{Actor, Kind, Status};
use otaq_core::git::testing::{commit, git};
use otaq_core::service::testing::TestService;
use otaq_core::session::SessionKind;

fn me() -> Actor {
    Actor::User("me".into())
}

fn branch_exists(w: &TestService, key: &str) -> bool {
    let repo = w.repos.root().join("shop-web");
    !git(
        &repo,
        &["branch", "--list", &otaq_core::naming::branch(key)],
    )
    .is_empty()
}

#[tokio::test]
async fn close_from_backlog_dev_and_review() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    // Backlog: nothing to stop or remove.
    w.create("SHOP", "Won't do").await;
    let closed = w.review.close("SHOP-1", false, &me()).await.unwrap();
    assert_eq!(closed.status, Status::ClosedNotMerged);
    let history = w.board.history("SHOP-1").await.unwrap();
    assert!(history.iter().any(|e| e.kind == EventKind::Closed));

    // Dev with a running agent: asks first, then stops it.
    let dev = w.create("SHOP", "Working").await;
    let wt = w.start_worktree(&dev).await;
    commit(&wt, "a.ts", "a\n");
    w.force_status(&dev, Status::Working).await;
    let agent = w.agent_shell(&dev).await;
    let err = w.review.close("SHOP-2", false, &me()).await.unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NeedsConfirmation));
    assert_eq!(
        err.to_string(),
        "SHOP-2's agent is working. It will be stopped first."
    );
    assert_eq!(w.status("SHOP-2").await, Status::Working);
    let closed = w.review.close("SHOP-2", true, &me()).await.unwrap();
    assert_eq!(closed.status, Status::ClosedNotMerged);
    assert!(!w.sessions.info(&agent.session).unwrap().is_running());
    assert!(!wt.exists(), "clean worktree removed");
    assert!(branch_exists(&w, "SHOP-2"), "branch kept");
    let row = w.board.store().ticket(&dev.id).await.unwrap().unwrap();
    assert_eq!(row.outcome.as_deref(), Some("closed"));

    // Ready for Review with uncommitted files: the worktree is kept and flagged.
    let review = w.create("SHOP", "In review").await;
    let wt = w.start_worktree(&review).await;
    std::fs::write(wt.join("scratch.md"), "x\n").unwrap();
    w.force_status(&review, Status::ReadyForReview).await;
    w.review.close("SHOP-3", false, &me()).await.unwrap();
    assert!(wt.join("scratch.md").is_file());
    let row = w.board.store().ticket(&review.id).await.unwrap().unwrap();
    assert!(row.worktree_kept);

    // Merged tickets are locked.
    let merged = w.create("SHOP", "Merged").await;
    w.force_status(&merged, Status::Merged).await;
    let err = w.review.close("SHOP-4", true, &me()).await.unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::MoveRefused));
    w.stop().await;
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

#[tokio::test]
async fn closing_an_epic_closes_its_unfinished_children_and_leaves_merged_ones() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let epic = w
        .board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Wishlist".into(),
                kind: Kind::Epic,
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    let merged = child(&w, &epic, "API").await;
    w.force_status(&merged, Status::Merged).await;
    let working = child(&w, &epic, "Button").await;
    w.start_worktree(&working).await;
    w.force_status(&working, Status::Working).await;
    w.agent_shell(&working).await;
    child(&w, &epic, "Page").await;

    let err = w
        .review
        .close_epic("SHOP-1", false, &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NeedsConfirmation));
    let plan = w.review.close_epic("SHOP-1", true, &me()).await.unwrap();
    let closed: Vec<_> = plan.close.iter().map(|c| c.key.to_string()).collect();
    assert_eq!(closed, ["SHOP-1.2", "SHOP-1.3"]);
    assert_eq!(w.status("SHOP-1.1").await, Status::Merged);
    assert_eq!(w.status("SHOP-1.2").await, Status::ClosedNotMerged);
    assert_eq!(w.status("SHOP-1.3").await, Status::ClosedNotMerged);
    assert_eq!(w.status("SHOP-1").await, Status::ClosedNotMerged);
    assert!(
        w.sessions
            .running_of(&working.id, SessionKind::Agent)
            .is_none()
    );
    w.stop().await;
}
