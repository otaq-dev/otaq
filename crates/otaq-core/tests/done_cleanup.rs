//! Done cleanup (T-0099, REC-05): on Done the agent session ends, the ports return, the worktree is removed when
//! clean (the branch stays) and kept and flagged when it has uncommitted changes; the finished hook runs.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use otaq_core::domain::{Actor, Status};
use otaq_core::git::testing::{commit, git};
use otaq_core::ports::PortPool;
use otaq_core::review::LocalMergeOutcome;
use otaq_core::service::testing::TestService;
use otaq_core::session::SessionKind;

fn me() -> Actor {
    Actor::User("me".into())
}

#[tokio::test]
async fn a_local_merge_cleans_up_a_clean_worktree_and_keeps_the_branch() {
    let w = TestService::start().await;
    let project = w.add_project("shop-web").await;
    let t = w.create("SHOP", "Checkout API").await;
    let wt = w.start_worktree(&t).await;
    commit(&wt, "api.ts", "api\n");
    PortPool::with_range(w.board.store().clone(), 21_000..=21_999)
        .allocate(&t.id)
        .await
        .unwrap();
    w.force_status(&t, Status::ReadyForReview).await;
    let agent = w.agent_shell(&t).await;
    let finished = Arc::new(AtomicUsize::new(0));
    let seen = finished.clone();
    w.review.on_ticket_finished(Arc::new(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {})
    }));

    let outcome = w
        .review
        .local_merge("SHOP-1", "m", false, &me())
        .await
        .unwrap();
    assert!(
        matches!(outcome, LocalMergeOutcome::Merged { .. }),
        "{outcome:?}"
    );

    assert!(
        !w.sessions.info(&agent.session).unwrap().is_running(),
        "session ended"
    );
    assert!(w.sessions.running_of(&t.id, SessionKind::Agent).is_none());
    assert!(!wt.exists(), "worktree removed");
    let branch = otaq_core::naming::branch("SHOP-1");
    assert_eq!(
        git(
            std::path::Path::new(&project.repo_path),
            &["branch", "--list", &branch]
        )
        .trim_start_matches(['*', '+', ' ']),
        branch,
        "branch kept"
    );
    let row = w.board.store().ticket(&t.id).await.unwrap().unwrap();
    assert_eq!((row.agent_port, row.run_port), (None, None), "ports freed");
    assert_eq!(row.worktree_path, None);
    assert!(!row.worktree_kept);
    assert_eq!(finished.load(Ordering::SeqCst), 1);
    // The Done ticket's diff still reads from the branch.
    assert_eq!(w.board.diff_summary("SHOP-1").await.unwrap().files.len(), 1);
    w.stop().await;
}

#[tokio::test]
async fn a_dirty_worktree_is_kept_and_flagged() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let t = w.create("SHOP", "Price table").await;
    let wt = w.start_worktree(&t).await;
    std::fs::write(wt.join("notes.md"), "not committed\n").unwrap();
    let t = w.board.ticket("SHOP-1").await.unwrap();

    let cleanup = w.review.finish(&t).await.unwrap();
    assert!(!cleanup.worktree_removed);
    assert_eq!(cleanup.kept_dirty, ["notes.md"]);
    assert!(wt.join("notes.md").is_file(), "never removed");
    let row = w.board.store().ticket(&t.id).await.unwrap().unwrap();
    assert!(row.worktree_kept);
    assert!(row.worktree_path.is_some());
    w.stop().await;
}
