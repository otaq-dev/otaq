//! Submit for review by the user (T-0103, CLS-05): allowed when the agent isn't mid-turn, with the agent's checks;
//! the summary reads "Submitted by you"; the session stays.

use otaq_core::board::Refusal;
use otaq_core::domain::{Actor, Status};
use otaq_core::git::testing::{commit, git};
use otaq_core::service::testing::TestService;

fn me() -> Actor {
    Actor::User("me".into())
}

#[tokio::test]
async fn allowed_from_idle_needs_you_and_interrupted_only() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    for (n, status) in [
        Status::Idle,
        Status::AsksYou,
        Status::NeedsYou,
        Status::Interrupted,
    ]
    .into_iter()
    .enumerate()
    {
        let t = w.create("SHOP", "t").await;
        let wt = w.start_worktree(&t).await;
        commit(&wt, "a.ts", "a\n");
        w.force_status(&t, status).await;
        let key = format!("SHOP-{}", n + 1);
        let after = w.board.submit_by_user(&key, "", &me()).await.unwrap();
        assert_eq!(after.status, Status::WaitingForYou, "from {status}");
    }
    for status in [Status::Working, Status::WorkingInBackground, Status::Queued] {
        let t = w.create("SHOP", "busy").await;
        w.start_worktree(&t).await;
        w.force_status(&t, status).await;
        let err = w
            .board
            .submit_by_user(&t.key.to_string(), "", &me())
            .await
            .unwrap_err();
        assert_eq!(err.refusal(), Some(Refusal::MoveRefused), "from {status}");
    }
    w.stop().await;
}

#[tokio::test]
async fn submitted_by_you_with_the_note_and_the_session_stays() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let t = w.create("SHOP", "Fix flaky login redirect").await;
    let wt = w.start_worktree(&t).await;
    commit(&wt, "login.ts", "fixed\n");
    std::fs::write(wt.join("debug.log"), "x\n").unwrap();
    std::fs::write(wt.join("login.ts"), "changed again\n").unwrap();
    w.force_status(&t, Status::NeedsYou).await;
    let agent = w.agent_shell(&t).await;

    let err = w
        .board
        .submit_by_user("SHOP-1", "", &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::Uncommitted));
    assert_eq!(
        err.to_string(),
        "2 files are not committed. Commit them, or discard them if they're scratch files"
    );

    std::fs::remove_file(wt.join("debug.log")).unwrap();
    git(&wt, &["checkout", "--", "login.ts"]);
    let after = w
        .board
        .submit_by_user("SHOP-1", "I finished the redirect by hand.", &me())
        .await
        .unwrap();
    assert_eq!(after.status, Status::WaitingForYou);
    assert_eq!(
        w.board.review_summary(&t.id).await.unwrap().as_deref(),
        Some("Submitted by you\n\nI finished the redirect by hand.")
    );
    let row = w.board.store().ticket(&t.id).await.unwrap().unwrap();
    assert_eq!(
        row.review_head_sha.as_deref(),
        Some(git(&wt, &["rev-parse", "HEAD"]).as_str())
    );
    assert!(
        w.sessions.info(&agent.session).unwrap().is_running(),
        "the session stays"
    );
    w.stop().await;
}
