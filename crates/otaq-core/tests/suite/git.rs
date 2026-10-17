//! The git runner against temporary repositories (d0055, d0056).

use std::ffi::OsString;

use otaq_core::git::testing::{TempRepos, commit, git};
use otaq_core::git::{Git, GitError};

fn runner() -> Git {
    Git::find().expect("git on PATH")
}

#[tokio::test]
async fn run_returns_stdout_or_a_typed_error() {
    let repos = TempRepos::new();
    let repo = repos.repo("shop-web", 2);
    let git = runner();
    let log = git.run(&repo, &["log", "--format=%s"]).await.unwrap();
    assert_eq!(
        log.lines().collect::<Vec<_>>(),
        ["edit file2.txt", "edit file1.txt"]
    );

    let err = git
        .run(&repo, &["rev-parse", "--verify", "no-such-branch"])
        .await
        .unwrap_err();
    let GitError::Failed { code, stderr, args } = &err else {
        panic!("{err:?}")
    };
    assert_eq!(*code, Some(128));
    assert!(stderr.contains("fatal"), "{stderr}");
    assert_eq!(args, "rev-parse --verify no-such-branch");
    assert!(err.to_string().contains("exit 128"), "{err}");
}

#[tokio::test]
async fn is_repo_and_repo_root() {
    let repos = TempRepos::new();
    let repo = repos.repo("shop-web", 1);
    let nested = repo.join("src/deep");
    std::fs::create_dir_all(&nested).unwrap();
    let plain = repos.plain_folder("not-a-repo");
    let git = runner();

    assert!(git.is_repo(&repo).await);
    assert!(git.is_repo(&nested).await);
    assert!(!git.is_repo(&plain).await);
    assert_eq!(git.repo_root(&nested).await.unwrap(), repo);
    assert!(git.repo_root(&plain).await.is_err());
}

#[tokio::test]
async fn current_branch_and_detached_head() {
    let repos = TempRepos::new();
    let repo = repos.repo("shop-web", 2);
    let git = runner();
    assert_eq!(
        git.current_branch(&repo).await.unwrap().as_deref(),
        Some("main")
    );
    otaq_core::git::testing::git(&repo, &["checkout", "--quiet", "-b", "feature/x"]);
    assert_eq!(
        git.current_branch(&repo).await.unwrap().as_deref(),
        Some("feature/x")
    );
    otaq_core::git::testing::git(&repo, &["checkout", "--quiet", "--detach", "HEAD~1"]);
    assert_eq!(git.current_branch(&repo).await.unwrap(), None);
}

#[tokio::test]
async fn origin_head_follows_the_remote_default() {
    let repos = TempRepos::new();
    let clone = repos.clone_with_origin("shop-web", "develop", 2);
    let lone = repos.repo("billing-api", 1);
    let git = runner();
    assert_eq!(
        git.origin_head(&clone).await.unwrap().as_deref(),
        Some("develop")
    );
    assert_eq!(git.origin_head(&lone).await.unwrap(), None);
    assert_eq!(
        git.current_branch(&clone).await.unwrap().as_deref(),
        Some("develop")
    );
}

#[tokio::test]
async fn local_and_remote_branches() {
    let repos = TempRepos::new();
    let clone = repos.clone_with_origin("shop-web", "develop", 1);
    git(&clone, &["branch", "release/2.3"]);
    git(&clone, &["push", "--quiet", "origin", "release/2.3"]);
    git(&clone, &["branch", "local-only"]);
    commit(&clone, "a.txt", "a\n");
    let runner = runner();

    assert_eq!(
        runner.local_branches(&clone).await.unwrap(),
        ["develop", "local-only", "release/2.3"]
    );
    assert_eq!(
        runner.remote_branches(&clone).await.unwrap(),
        ["origin/develop", "origin/release/2.3"]
    );
    let lone = repos.repo("billing-api", 1);
    assert!(runner.remote_branches(&lone).await.unwrap().is_empty());
}

#[test]
fn missing_git_is_a_clear_error() {
    let empty = tempfile::TempDir::new().unwrap();
    let err = Git::find_on(&OsString::from(empty.path())).unwrap_err();
    assert!(matches!(err, GitError::NotFound));
    assert!(err.to_string().contains("git isn't installed"), "{err}");
}
