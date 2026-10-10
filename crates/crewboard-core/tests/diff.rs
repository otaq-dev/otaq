//! The diff engine (T-0084): a ticket's branch against its `base_sha`, epics grouped by child, against
//! temporary repositories.

mod common;

use std::path::Path;

use common::Fixture;
use crewboard_core::board::Refusal;
use crewboard_core::domain::Status;
use crewboard_core::git::diff::{FileStatus, LineKind};
use crewboard_core::git::testing::{commit, git};

/// Squash-merges `branch` into `target` the way Local merge leaves it (one commit with the branch's tree on top
/// of the target), without touching any checkout. Returns the commit.
fn squash_into(repo: &Path, target: &str, branch: &str, message: &str) -> String {
    let tree = git(repo, &["rev-parse", &format!("{branch}^{{tree}}")]);
    let parent = git(repo, &["rev-parse", target]);
    let sha = git(repo, &["commit-tree", &tree, "-p", &parent, "-m", message]);
    git(
        repo,
        &["update-ref", &format!("refs/heads/{target}"), &sha, &parent],
    );
    sha
}

#[tokio::test]
async fn a_branch_shows_every_kind_of_change_against_its_base_sha() {
    let f = Fixture::shop().await;
    let t = f.create("Checkout API").await;
    let wt = f.start_worktree(&t).await;
    // On main already: file1.txt. Add files the branch then changes.
    std::fs::create_dir(wt.join("lib")).unwrap();
    commit(&wt, "lib/pricing.ts", "a\nb\nc\n");
    commit(&wt, "old_name.ts", "same\ncontent\nhere\nfor\nrename\n");
    commit(&wt, "gone.ts", "bye\n");
    let base_before = git(&f.repo(), &["rev-parse", "main"]);
    // Pretend those three were already on the base: move base_sha to the branch's current head.
    let head = git(&wt, &["rev-parse", "HEAD"]);
    sqlx::query("UPDATE tickets SET base_sha = ? WHERE id = ?")
        .bind(&head)
        .bind(&t.id)
        .execute(f.board.store().pool())
        .await
        .unwrap();

    commit(&wt, "lib/pricing.ts", "a\nB\nc\nd\n");
    commit(&wt, "new_file.ts", "one\ntwo\n");
    git(&wt, &["rm", "--quiet", "gone.ts"]);
    git(&wt, &["mv", "old_name.ts", "new_name.ts"]);
    std::fs::write(wt.join("logo.png"), [0u8, 159, 146, 150, 0, 1, 2, 3]).unwrap();
    git(&wt, &["add", "logo.png"]);
    git(&wt, &["commit", "--quiet", "-m", "more"]);

    let diff = f.board.diff_summary("SHOP-1").await.unwrap();
    assert_eq!(diff.base_branch, "main");
    assert_eq!(diff.base_sha, head);
    assert_eq!(diff.base_short, &head[..7]);
    assert_eq!(diff.branch, "crewboard/SHOP-1");
    let by_path = |p: &str| diff.files.iter().find(|f| f.path == p).unwrap().clone();
    assert_eq!(by_path("lib/pricing.ts").status, FileStatus::Modified);
    assert_eq!(
        (
            by_path("lib/pricing.ts").added,
            by_path("lib/pricing.ts").removed
        ),
        (2, 1)
    );
    assert_eq!(by_path("new_file.ts").status, FileStatus::Added);
    assert_eq!(by_path("gone.ts").status, FileStatus::Deleted);
    let renamed = by_path("new_name.ts");
    assert_eq!(renamed.status, FileStatus::Renamed);
    assert_eq!(renamed.old_path.as_deref(), Some("old_name.ts"));
    let logo = by_path("logo.png");
    assert!(logo.binary);
    assert_eq!(logo.size, Some(8));
    assert_eq!(diff.files.len(), 5);
    assert_eq!((diff.added, diff.removed), (4, 2));
    assert_ne!(base_before, head);

    let file = f
        .board
        .diff_file("SHOP-1", "lib/pricing.ts", None)
        .await
        .unwrap();
    let lines: Vec<_> = file.hunks[0]
        .lines
        .iter()
        .map(|l| (l.kind, l.old, l.new, l.text.as_str()))
        .collect();
    assert_eq!(
        lines,
        [
            (LineKind::Context, Some(1), Some(1), "a"),
            (LineKind::Removed, Some(2), None, "b"),
            (LineKind::Added, None, Some(2), "B"),
            (LineKind::Context, Some(3), Some(3), "c"),
            (LineKind::Added, None, Some(4), "d"),
        ]
    );
    let logo = f.board.diff_file("SHOP-1", "logo.png", None).await.unwrap();
    assert!(logo.file.binary && logo.hunks.is_empty());
    let renamed = f
        .board
        .diff_file("SHOP-1", "new_name.ts", None)
        .await
        .unwrap();
    assert_eq!(renamed.file.old_path.as_deref(), Some("old_name.ts"));
}

#[tokio::test]
async fn a_base_that_moves_on_doesnt_change_the_diff() {
    let f = Fixture::shop().await;
    let t = f.create("Price table").await;
    let wt = f.start_worktree(&t).await;
    commit(&wt, "price.ts", "1\n");
    let before = f.board.diff_summary("SHOP-1").await.unwrap();

    // Someone else's work lands on main after the ticket left it.
    commit(&f.repo(), "other.ts", "theirs\n");
    let after = f.board.diff_summary("SHOP-1").await.unwrap();
    assert_eq!(after, before);
    assert_eq!(after.files.len(), 1);
    assert_eq!(after.files[0].path, "price.ts");
}

#[tokio::test]
async fn an_empty_diff_is_an_empty_list() {
    let f = Fixture::shop().await;
    let t = f.create("Nothing yet").await;
    f.start_worktree(&t).await;
    let diff = f.board.diff_summary("SHOP-1").await.unwrap();
    assert!(diff.files.is_empty());
    assert_eq!((diff.added, diff.removed), (0, 0));
}

#[tokio::test]
async fn a_ticket_that_never_started_has_no_diff() {
    let f = Fixture::shop().await;
    f.create("Backlog").await;
    let err = f.board.diff_summary("SHOP-1").await.unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NoBranch));
}

#[tokio::test]
async fn the_diff_works_from_the_branch_once_the_worktree_is_gone() {
    let f = Fixture::shop().await;
    let t = f.create("Done").await;
    let wt = f.start_worktree(&t).await;
    commit(&wt, "done.ts", "x\n");
    git(&f.repo(), &["worktree", "remove", wt.to_str().unwrap()]);
    assert!(!wt.exists());
    f.force_status(&t, Status::Merged).await;
    let diff = f.board.diff_summary("SHOP-1").await.unwrap();
    assert_eq!(diff.files.len(), 1);
    let file = f.board.diff_file("SHOP-1", "done.ts", None).await.unwrap();
    assert_eq!(file.hunks[0].lines[0].text, "x");
}

#[tokio::test]
async fn an_epic_diff_is_its_branch_grouped_by_merged_child() {
    let f = Fixture::shop().await;
    let epic = f.epic("Wishlist").await;
    let api = f.child(&epic, "Wishlist API").await;
    let button = f.child(&epic, "Wishlist button").await;
    let page = f.child(&epic, "Wishlist page").await;
    let repo = f.repo();
    let epic_branch = "crewboard/SHOP-1";

    let wt = f.start_worktree(&api).await;
    commit(&wt, "api.ts", "api\n");
    let api_sha = squash_into(
        &repo,
        epic_branch,
        "crewboard/SHOP-1.1",
        "SHOP-1.1 Wishlist API",
    );
    let wt = f.start_worktree(&button).await;
    commit(&wt, "button.ts", "b\nb\n");
    commit(&wt, "api.ts", "api\nmore\n");
    let button_sha = squash_into(
        &repo,
        epic_branch,
        "crewboard/SHOP-1.2",
        "SHOP-1.2 Wishlist button",
    );
    for (child, sha) in [(&api, &api_sha), (&button, &button_sha)] {
        f.force_status(child, Status::Merged).await;
        sqlx::query("UPDATE tickets SET merge_commit_sha = ? WHERE id = ?")
            .bind(sha)
            .bind(&child.id)
            .execute(f.board.store().pool())
            .await
            .unwrap();
    }
    // Started but nothing merged: not listed.
    f.start_worktree(&page).await;
    // main moves on: the epic's diff stays its own.
    commit(&repo, "elsewhere.ts", "x\n");

    let diff = f.board.diff_summary("SHOP-1").await.unwrap();
    assert_eq!(diff.base_branch, "main");
    assert_eq!(diff.branch, epic_branch);
    let paths: Vec<_> = diff.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["api.ts", "button.ts"]);
    let children: Vec<_> = diff
        .children
        .iter()
        .map(|c| {
            (
                c.key.to_string(),
                c.commit.clone(),
                c.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        children,
        [
            ("SHOP-1.1".to_owned(), api_sha, vec!["api.ts".to_owned()]),
            (
                "SHOP-1.2".to_owned(),
                button_sha,
                vec!["api.ts".to_owned(), "button.ts".to_owned()]
            ),
        ]
    );
    // A child's own change to a file.
    let file = f
        .board
        .diff_file("SHOP-1", "api.ts", Some("SHOP-1.2"))
        .await
        .unwrap();
    let added: Vec<_> = file.hunks[0]
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::Added)
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(added, ["more"]);

    // A child's diff is against the epic branch.
    let child = f.board.diff_summary("SHOP-1.3").await.unwrap();
    assert_eq!(child.base_branch, epic_branch);
    assert!(child.files.is_empty());
}
