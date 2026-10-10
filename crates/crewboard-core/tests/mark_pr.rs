//! Mark merged / Mark closed (T-0147; REV-13, CLS-02, SCH-05): the user says what happened to a PR. A bare
//! repository plays the remote and another clone pushes "the merged PR"; nothing goes over the network and nothing
//! asks GitHub or Azure DevOps.

use std::path::{Path, PathBuf};

use crewboard_core::board::moves::DropTarget;
use crewboard_core::board::projects::AddProject;
use crewboard_core::board::{BoardError, Refusal};
use crewboard_core::domain::{Actor, Status};
use crewboard_core::git::testing::{commit, git};
use crewboard_core::service::testing::TestService;

fn me() -> Actor {
    Actor::User("me".into())
}

async fn world() -> (TestService, PathBuf, PathBuf) {
    let w = TestService::start().await;
    let clone = w.repos.clone_with_origin("shop-web", "main", 1);
    let other = w.repos.root().join("shop-web-seed");
    w.board
        .add_project(&AddProject {
            path: clone.display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    (w, clone, other)
}

/// A ticket with a branch and worktree, in Approved · PR open with PR #`n`.
async fn with_pr(w: &TestService, title: &str, n: u32) -> (String, PathBuf) {
    let t = w.create("SHOP", title).await;
    let worktree = w.start_worktree(&t).await;
    commit(&worktree, &format!("{n}.rs"), "work\n");
    let key = t.key.to_string();
    w.board
        .pr_opened(
            &key,
            &format!("https://github.com/acme/shop-web/pull/{n}"),
            &me(),
        )
        .await
        .unwrap();
    w.force_status(&t, Status::PrOpen).await;
    (key, worktree)
}

fn tip(repo: &Path, reference: &str) -> String {
    git(repo, &["rev-parse", reference]).trim().to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn mark_merged_fetches_the_base_then_starts_the_waiting_ticket() {
    let (w, clone, other) = world().await;
    let (parent, worktree) = with_pr(&w, "Checkout API", 481).await;
    let child = w.create("SHOP", "Checkout page").await;
    w.board
        .add_dependency(&child.key.to_string(), &parent, &me())
        .await
        .unwrap();
    w.board
        .move_ticket(&child.key.to_string(), DropTarget::DevQueued, true, &me())
        .await
        .unwrap();
    // The PR is merged on the remote.
    commit(&other, "checkout.rs", "merged\n");
    git(&other, &["push", "--quiet", "origin", "main"]);
    let merged_tip = tip(&other, "HEAD");
    assert_ne!(tip(&clone, "main"), merged_tip, "the local base is behind");

    // Mark closed and Mark merged only work on an open PR.
    let refused = w
        .review
        .mark_merged(&child.key.to_string(), &me())
        .await
        .unwrap_err();
    assert_eq!(refused.refusal(), Some(Refusal::MoveRefused));

    let marked = w.review.mark_merged(&parent, &me()).await.unwrap();
    assert_eq!(marked.ticket.status, Status::Merged);
    assert_eq!(marked.ticket.flags.pr_number, Some(481));
    assert_eq!(marked.started, std::slice::from_ref(&child.key));
    assert!(
        !worktree.exists(),
        "Done cleanup removed the clean worktree"
    );
    assert_eq!(
        tip(&clone, "main"),
        merged_tip,
        "the base was fetched first"
    );
    // The start runs in the background: wait for its worktree.
    let mut child = w.board.ticket(&child.key.to_string()).await.unwrap();
    for _ in 0..100 {
        if child
            .work
            .as_ref()
            .and_then(|w| w.base_sha.as_ref())
            .is_some()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        child = w.board.ticket(&child.key.to_string()).await.unwrap();
    }
    assert_eq!(
        child.work.as_ref().and_then(|w| w.base_sha.clone()),
        Some(merged_tip),
        "the dependent starts from the fetched base"
    );
    let history = w.board.history(&parent).await.unwrap();
    assert!(history.iter().any(|e| e.kind.as_str() == "merged"));
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn mark_closed_asks_then_closes_like_close_ticket() {
    let (w, clone, _) = world().await;
    let (key, worktree) = with_pr(&w, "Coupon codes", 12).await;
    let branch = w
        .board
        .ticket(&key)
        .await
        .unwrap()
        .work
        .unwrap()
        .branch
        .unwrap();

    let err: BoardError = w.review.mark_closed(&key, false, &me()).await.unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NeedsConfirmation));
    assert_eq!(w.status(&key).await, Status::PrOpen);

    let closed = w.review.mark_closed(&key, true, &me()).await.unwrap();
    assert_eq!(closed.status, Status::ClosedNotMerged);
    assert!(!worktree.exists(), "worktree removed (it was clean)");
    assert!(
        git(&clone, &["branch", "--list", &branch]).contains(&branch),
        "the branch is kept"
    );
    w.stop().await;
}
