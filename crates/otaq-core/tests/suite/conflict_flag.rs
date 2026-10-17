//! The early conflict flag (T-0096, GIT-04): after a merge into a branch, every ticket in Ready for Review that
//! merges into it is test-merged read-only and flagged "conflicts with `main`" or cleared.

use std::path::{Path, PathBuf};

use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Actor, Status};
use otaq_core::git::Git;
use otaq_core::git::testing::{commit, git, snapshot};
use otaq_core::review::LocalMergeOutcome;
use otaq_core::service::testing::TestService;

fn me() -> Actor {
    Actor::User("me".into())
}

async fn reviewed(w: &TestService, title: &str, file: &str, content: &str) -> (Ticket, PathBuf) {
    let t = w.create("SHOP", title).await;
    let wt = w.start_worktree(&t).await;
    std::fs::create_dir_all(wt.join("lib")).unwrap();
    commit(&wt, file, content);
    w.force_status(&t, Status::ReadyForReview).await;
    (w.board.ticket(&t.key.to_string()).await.unwrap(), wt)
}

/// Everything git keeps about the repository's state that a test merge must not change.
fn state(repo: &Path) -> (String, String, Vec<String>) {
    (
        git(repo, &["for-each-ref", "--format=%(refname) %(objectname)"]),
        git(repo, &["status", "--porcelain=v1", "--untracked-files=all"]),
        snapshot(repo)
            .into_iter()
            .filter(|p| !p.starts_with(".git/objects") && !p.starts_with(".git/logs"))
            .collect(),
    )
}

#[tokio::test]
async fn merging_one_ticket_flags_the_one_that_would_conflict() {
    let w = TestService::start().await;
    let project = w.add_project("shop-web").await;
    let repo = PathBuf::from(&project.repo_path);
    std::fs::create_dir_all(repo.join("lib")).unwrap();
    commit(&repo, "lib/pricing.ts", "base\n");
    let (shop1, _) = reviewed(&w, "Checkout API", "lib/pricing.ts", "from SHOP-1\n").await;
    let (shop2, _) = reviewed(&w, "Price table", "lib/pricing.ts", "from SHOP-2\n").await;
    let (shop3, _) = reviewed(&w, "Footer links", "lib/footer.ts", "footer\n").await;

    let outcome = w
        .review
        .local_merge("SHOP-1", "SHOP-1: Checkout API", false, &me())
        .await
        .unwrap();
    assert!(
        matches!(outcome, LocalMergeOutcome::Merged { .. }),
        "{outcome:?}"
    );

    let flagged = w.board.ticket(&shop2.key.to_string()).await.unwrap();
    assert_eq!(flagged.flags.conflicts_with.as_deref(), Some("main"));
    let conflict = w.board.conflict(&shop2.id).await.unwrap().unwrap();
    assert_eq!(conflict.files, ["lib/pricing.ts"]);
    let clean = w.board.ticket(&shop3.key.to_string()).await.unwrap();
    assert_eq!(clean.flags.conflicts_with, None);
    assert_eq!(shop1.flags.conflicts_with, None);
    w.stop().await;
}

#[tokio::test]
async fn the_flag_clears_when_the_next_test_merge_is_clean() {
    let w = TestService::start().await;
    let project = w.add_project("shop-web").await;
    let repo = PathBuf::from(&project.repo_path);
    std::fs::create_dir_all(repo.join("lib")).unwrap();
    commit(&repo, "lib/pricing.ts", "base\n");
    reviewed(&w, "Checkout API", "lib/pricing.ts", "from SHOP-1\n").await;
    let (shop2, wt2) = reviewed(&w, "Price table", "lib/pricing.ts", "from SHOP-2\n").await;
    w.review
        .local_merge("SHOP-1", "m", false, &me())
        .await
        .unwrap();
    assert!(w.board.conflict(&shop2.id).await.unwrap().is_some());

    // SHOP-2's branch changes (its agent rebased and resolved it); the next merge into main re-checks it.
    git(&wt2, &["rebase", "--quiet", "-X", "theirs", "main"]);
    reviewed(&w, "Other", "lib/other.ts", "x\n").await;
    w.review
        .local_merge("SHOP-3", "m", false, &me())
        .await
        .unwrap();
    assert_eq!(w.board.conflict(&shop2.id).await.unwrap(), None);
    w.stop().await;
}

#[tokio::test]
async fn the_test_merge_changes_no_worktree_index_branch_or_ref() {
    let w = TestService::start().await;
    let project = w.add_project("shop-web").await;
    let repo = PathBuf::from(&project.repo_path);
    std::fs::create_dir_all(repo.join("lib")).unwrap();
    commit(&repo, "lib/pricing.ts", "base\n");
    let (_, wt) = reviewed(&w, "Price table", "lib/pricing.ts", "ticket\n").await;
    commit(&repo, "lib/pricing.ts", "main moved\n");
    std::fs::write(repo.join("file1.txt"), "the user's edit\n").unwrap();
    let before = (state(&repo), state(&wt));

    let git_ = Git::find().unwrap();
    let files = git_
        .test_merge(&repo, "main", &otaq_core::naming::branch("SHOP-1"))
        .await
        .unwrap();
    assert_eq!(files, Some(vec!["lib/pricing.ts".to_owned()]));
    let clean = git_.test_merge(&repo, "main", "main").await.unwrap();
    assert_eq!(clean, None);
    assert_eq!((state(&repo), state(&wt)), before);
    w.stop().await;
}
