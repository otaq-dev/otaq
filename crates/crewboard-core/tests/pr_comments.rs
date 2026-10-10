//! Unresolved PR comments in `review feedback` (T-0148; REV-05, d0017): recorded `gh` / `az` answers are parsed, and
//! ordinary scripts named `gh` / `az` on a test PATH stand in for the CLIs. No real provider is called.

use std::ffi::OsString;
use std::path::Path;

use crewboard_core::domain::{Actor, Status};
use crewboard_core::git::testing::git;
use crewboard_core::pr::comments::{PrComment, parse_azure, parse_github};
use crewboard_core::service::testing::TestService;

const GITHUB: &str = r#"{"data": {"repository": {"pullRequest": {"reviewThreads": {"nodes": [
  {"isResolved": false, "path": "lib/totals.ts", "line": 12, "originalLine": 12,
   "comments": {"nodes": [{"author": {"login": "anna"}, "body": "Round per line, not per order."},
                          {"author": {"login": "nevzat"}, "body": "Agreed."}]}},
  {"isResolved": true, "path": "lib/cart.ts", "line": 3, "originalLine": 3,
   "comments": {"nodes": [{"author": {"login": "anna"}, "body": "Fixed already."}]}},
  {"isResolved": false, "path": "lib/old.ts", "line": null, "originalLine": 40,
   "comments": {"nodes": [{"author": {"login": "bo"}, "body": "Outdated but open."}]}}
]}}}}}"#;

const AZURE: &str = r#"{"count": 3, "value": [
  {"id": 1, "status": "active", "isDeleted": false,
   "threadContext": {"filePath": "/src/Billing/Invoice.cs", "rightFileStart": {"line": 88, "offset": 1}},
   "comments": [{"author": {"displayName": "Anna Berg"}, "content": "Use decimal here.", "commentType": "text"}]},
  {"id": 2, "status": "fixed", "isDeleted": false,
   "threadContext": {"filePath": "/src/Billing/Tax.cs", "rightFileStart": {"line": 4, "offset": 1}},
   "comments": [{"author": {"displayName": "Anna Berg"}, "content": "Done.", "commentType": "text"}]},
  {"id": 3, "status": "active", "isDeleted": false, "threadContext": null,
   "comments": [{"author": {"displayName": "Project Collection Build Service"}, "content": "Policy updated", "commentType": "system"},
                {"author": {"displayName": "Bo Lind"}, "content": "Please add a changelog line.", "commentType": "text"}]}
]}"#;

#[test]
fn github_threads_keep_only_unresolved_comments() {
    let comments = parse_github(GITHUB).unwrap();
    assert_eq!(
        comments,
        [
            PrComment {
                file: Some("lib/totals.ts".into()),
                line: Some(12),
                author: "anna".into(),
                text: "Round per line, not per order.".into()
            },
            PrComment {
                file: Some("lib/totals.ts".into()),
                line: Some(12),
                author: "nevzat".into(),
                text: "Agreed.".into()
            },
            PrComment {
                file: Some("lib/old.ts".into()),
                line: Some(40),
                author: "bo".into(),
                text: "Outdated but open.".into()
            },
        ]
    );
    assert!(parse_github("{}").is_err());
}

#[test]
fn azure_threads_keep_only_active_text_comments() {
    let comments = parse_azure(AZURE).unwrap();
    assert_eq!(
        comments,
        [
            PrComment {
                file: Some("src/Billing/Invoice.cs".into()),
                line: Some(88),
                author: "Anna Berg".into(),
                text: "Use decimal here.".into()
            },
            PrComment {
                file: None,
                line: None,
                author: "Bo Lind".into(),
                text: "Please add a changelog line.".into()
            },
        ]
    );
}

fn stub(dir: &Path, name: &str, script: &str) {
    crewboard_core::process::testing::stub_command(dir, name, script);
}

/// SHOP-26 with PR #481, in a project whose origin is `url`.
async fn with_pr(url: &str) -> TestService {
    let w = TestService::start().await;
    let project = w.add_project("shop-web").await;
    git(
        Path::new(&project.repo_path),
        &["remote", "add", "origin", url],
    );
    let t = w.create("SHOP", "Checkout totals").await;
    w.board
        .pr_opened(
            "SHOP-1",
            "https://github.com/acme/shop-web/pull/481",
            &Actor::User("me".into()),
        )
        .await
        .unwrap();
    w.force_status(&t, Status::ReadyForReview).await;
    w
}

#[tokio::test(flavor = "multi_thread")]
async fn fetched_on_demand_through_the_stand_in_cli() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = tmp.path().join("github.json");
    std::fs::write(&fixture, GITHUB).unwrap();
    let args = tmp.path().join("args");
    stub(
        tmp.path(),
        "gh",
        &format!(
            "echo \"$@\" > '{}'; /bin/cat '{}'",
            args.display(),
            fixture.display()
        ),
    );
    let path = OsString::from(tmp.path());
    let w = with_pr("git@github.com:acme/shop-web.git").await;
    let feedback = w.board.pr_feedback("SHOP-1", &path).await.unwrap().unwrap();
    assert_eq!(feedback.number, 481);
    assert_eq!(feedback.comments.len(), 3);
    assert_eq!(feedback.note, None);
    let called = std::fs::read_to_string(&args).unwrap();
    assert!(called.starts_with("api graphql"), "{called}");
    assert!(
        called.contains("owner=acme")
            && called.contains("name=shop-web")
            && called.contains("number=481")
    );

    // A logged-out gh adds a note instead of failing.
    stub(
        tmp.path(),
        "gh",
        "echo 'To get started with GitHub CLI, please run:  gh auth login' >&2; exit 4",
    );
    let feedback = w.board.pr_feedback("SHOP-1", &path).await.unwrap().unwrap();
    assert!(feedback.comments.is_empty());
    assert_eq!(
        feedback.note.as_deref(),
        Some("PR #481 comments weren't fetched: `gh` isn't logged in (`gh auth login`)")
    );
    // No PR, no fetch.
    let other = w.create("SHOP", "No PR").await;
    assert!(
        w.board
            .pr_feedback(&other.key.to_string(), &path)
            .await
            .unwrap()
            .is_none()
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn azure_devops_and_undetected_remotes() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = tmp.path().join("azure.json");
    std::fs::write(&fixture, AZURE).unwrap();
    stub(
        tmp.path(),
        "az",
        &format!("/bin/cat '{}'", fixture.display()),
    );
    let path = OsString::from(tmp.path());
    let w = with_pr("https://acme@dev.azure.com/acme/billing/_git/billing-api").await;
    let feedback = w.board.pr_feedback("SHOP-1", &path).await.unwrap().unwrap();
    assert_eq!(feedback.comments.len(), 2);
    w.stop().await;

    let w = with_pr("https://git.acme.internal/web/handbook.git").await;
    let feedback = w.board.pr_feedback("SHOP-1", &path).await.unwrap().unwrap();
    assert!(feedback.comments.is_empty());
    assert!(
        feedback
            .note
            .as_deref()
            .unwrap()
            .contains("the PR provider isn't detected"),
        "{feedback:?}"
    );
    w.stop().await;
}
