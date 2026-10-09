//! Worktrees, temporary branches, base_sha and the epic integration branch (T-0048), against temporary repos.

mod common;

use std::path::{Path, PathBuf};

use common::Fixture;
use crewboard_core::data_dir::DataDir;
use crewboard_core::git::testing::{commit, git, snapshot};
use crewboard_core::start::prepare_worktree;

fn data(f: &Fixture) -> DataDir {
    DataDir::new(f.home.path().join("home"))
}

fn repo(f: &Fixture) -> PathBuf {
    f.repos.root().join("shop-web")
}

/// The user's repository outside `.git`.
fn user_files(repo: &Path) -> Vec<String> {
    snapshot(repo)
        .into_iter()
        .filter(|p| !p.starts_with(".git"))
        .collect()
}

#[tokio::test]
async fn fresh_start_creates_branch_and_worktree_outside_the_repo() {
    let f = Fixture::shop().await;
    let t = f.create("Checkout API").await;
    let before = user_files(&repo(&f));
    let main = git(&repo(&f), &["rev-parse", "main"]);

    let wt = prepare_worktree(&f.board, &data(&f), &t.key.to_string())
        .await
        .unwrap();
    let expected = data(&f).worktree_path("shop-web", "SHOP-1").unwrap();
    assert_eq!(wt.path, expected);
    assert_eq!(wt.branch, "crewboard/SHOP-1");
    assert_eq!(wt.base_branch, "main");
    assert_eq!(wt.base_sha, main);
    assert!(wt.path.join("file1.txt").is_file(), "checked out");
    assert_eq!(
        git(&wt.path, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "crewboard/SHOP-1"
    );
    // No upstream: the temporary branch tracks nothing.
    assert_eq!(
        git(
            &repo(&f),
            &[
                "for-each-ref",
                "--format=%(upstream)",
                "refs/heads/crewboard/SHOP-1"
            ]
        ),
        ""
    );

    let row = f.board.store().ticket(&t.id).await.unwrap().unwrap();
    assert_eq!(row.branch_name.as_deref(), Some("crewboard/SHOP-1"));
    assert_eq!(row.base_sha.as_deref(), Some(main.as_str()));
    assert_eq!(
        row.worktree_path.as_deref(),
        Some(expected.display().to_string().as_str())
    );
    assert!(row.branch_locked);
    assert_eq!(user_files(&repo(&f)), before, "nothing written in the repo");
}

#[tokio::test]
async fn rerunning_converges_from_any_partial_state() {
    let f = Fixture::shop().await;
    let t = f.create("t").await;
    let key = t.key.to_string();
    let first = prepare_worktree(&f.board, &data(&f), &key).await.unwrap();

    // Plain re-run.
    assert_eq!(
        prepare_worktree(&f.board, &data(&f), &key).await.unwrap(),
        first
    );

    // The worktree folder vanished but git still has it registered.
    std::fs::remove_dir_all(&first.path).unwrap();
    assert_eq!(
        prepare_worktree(&f.board, &data(&f), &key).await.unwrap(),
        first
    );
    assert!(first.path.join("file1.txt").is_file());

    // The worktree is gone entirely, only the branch exists; and base_sha was never stored.
    let path = first.path.display().to_string();
    git(&repo(&f), &["worktree", "remove", "--force", &path]);
    sqlx::query("UPDATE tickets SET base_sha = NULL, worktree_path = NULL WHERE id = ?")
        .bind(&t.id)
        .execute(f.board.store().pool())
        .await
        .unwrap();
    // The base moved on meanwhile: base_sha stays where the branch left it.
    commit(&repo(&f), "later.txt", "later\n");
    let again = prepare_worktree(&f.board, &data(&f), &key).await.unwrap();
    assert_eq!(again, first);

    // An empty folder at the path (left by a failed add) is fine too.
    git(&repo(&f), &["worktree", "remove", "--force", &path]);
    std::fs::create_dir_all(&first.path).unwrap();
    assert_eq!(
        prepare_worktree(&f.board, &data(&f), &key).await.unwrap(),
        first
    );
}

#[tokio::test]
async fn a_foreign_folder_in_the_way_is_reported_not_removed() {
    let f = Fixture::shop().await;
    let t = f.create("t").await;
    let path = data(&f).worktree_path("shop-web", "SHOP-1").unwrap();
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("mine.txt"), "keep me").unwrap();
    let err = prepare_worktree(&f.board, &data(&f), &t.key.to_string())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("isn't a worktree"), "{err}");
    assert_eq!(
        std::fs::read_to_string(path.join("mine.txt")).unwrap(),
        "keep me"
    );
}

#[tokio::test]
async fn epic_children_use_the_epic_branch() {
    let f = Fixture::shop().await;
    let epic = f.epic("Wishlist").await;
    let first = f.child(&epic, "Wishlist API").await;
    let second = f.child(&epic, "Wishlist page").await;
    assert_eq!(first.key.to_string(), "SHOP-1.1");
    let main = git(&repo(&f), &["rev-parse", "main"]);

    let wt1 = prepare_worktree(&f.board, &data(&f), "SHOP-1.1")
        .await
        .unwrap();
    assert_eq!(wt1.base_branch, "crewboard/SHOP-1");
    assert_eq!(wt1.branch, "crewboard/SHOP-1.1");
    assert_eq!(
        wt1.path,
        data(&f).worktree_path("shop-web", "SHOP-1.1").unwrap()
    );
    assert_eq!(git(&repo(&f), &["rev-parse", "crewboard/SHOP-1"]), main);
    let epic_row = f.board.store().ticket(&epic.id).await.unwrap().unwrap();
    assert_eq!(epic_row.branch_name.as_deref(), Some("crewboard/SHOP-1"));
    assert_eq!(epic_row.base_sha.as_deref(), Some(main.as_str()));
    assert_eq!(epic_row.worktree_path, None, "no worktree for the epic");
    assert!(
        !data(&f)
            .worktree_path("shop-web", "SHOP-1")
            .unwrap()
            .exists()
    );

    // A child merged into the epic branch meanwhile (simulated): the next child starts from there, and the
    // epic branch isn't recreated.
    let scratch = f.repos.root().join("scratch");
    let scratch_arg = scratch.display().to_string();
    git(
        &repo(&f),
        &[
            "worktree",
            "add",
            "--quiet",
            &scratch_arg,
            "crewboard/SHOP-1",
        ],
    );
    commit(&scratch, "merged.txt", "child 1\n");
    let epic_head = git(&repo(&f), &["rev-parse", "crewboard/SHOP-1"]);
    assert_ne!(epic_head, main);

    let wt2 = prepare_worktree(&f.board, &data(&f), &second.key.to_string())
        .await
        .unwrap();
    assert_eq!(wt2.base_branch, "crewboard/SHOP-1");
    assert_eq!(wt2.base_sha, epic_head);
    assert_eq!(
        git(&repo(&f), &["rev-parse", "crewboard/SHOP-1"]),
        epic_head
    );
    let epic_row = f.board.store().ticket(&epic.id).await.unwrap().unwrap();
    assert_eq!(
        epic_row.base_sha.as_deref(),
        Some(main.as_str()),
        "the epic keeps where it left main"
    );

    let err = prepare_worktree(&f.board, &data(&f), &epic.key.to_string())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("epic"), "{err}");
}
