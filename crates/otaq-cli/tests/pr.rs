//! `otaq pr propose --stdin`, `otaq pr opened <url>` and `otaq branch rename <name>` (T-0142, REV-10,
//! GIT-07) from inside an agent session, with stdin fixtures and the project's temporary repository.

use otaq_core::git::testing::git;
use otaq_core::service::testing::TestService;
use serde_json::Value;

const CB: &str = env!("CARGO_BIN_EXE_otaq");

const PROPOSAL: &str = r#"{
  "branch": "otaq/SHOP-19-price-table",
  "commits": [{"sha": "a1b2c3d", "message": "SHOP-19: add the price table"}],
  "title": "SHOP-19 Price table",
  "description": "Adds the table.\n\nTested by hand."
}"#;

#[tokio::test(flavor = "multi_thread")]
async fn propose_checks_the_shape_and_needs_a_flow() {
    let w = TestService::start().await;
    w.shop_sample().await;
    let shop19 = w.board.ticket("SHOP-19").await.unwrap();
    let agent = w.agent_shell(&shop19).await;

    // Without the user's Open PR click there is nothing to propose for (T-0143; open_pr.rs stores one).
    let ran = agent
        .run_with_stdin(&format!("'{CB}' pr propose --stdin --json"), PROPOSAL)
        .await;
    assert_ne!(ran.status, 0, "{ran:?}");
    let error: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("has no Open PR in progress"),
        "{error}"
    );
    assert!(w.board.pr_flow(&shop19.id).await.unwrap().is_none());

    // A malformed proposal is a JSON error.
    let ran = agent
        .run_with_stdin(
            &format!("'{CB}' pr propose --stdin --json"),
            r#"{"branch": "bad name", "commits": []}"#,
        )
        .await;
    assert_ne!(ran.status, 0);
    let error: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(error["error"]["data"]["refusal"], "bad_proposal", "{error}");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("isn't a valid branch name"),
        "{error}"
    );
    let ran = agent.run(&format!("'{CB}' pr propose")).await;
    assert_ne!(ran.status, 0);
    assert!(ran.stderr.contains("--stdin"), "{ran:?}");
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rename_works_until_the_pr_is_opened_then_the_branch_is_locked() {
    let w = TestService::start().await;
    let project = w.shop_sample().await;
    let shop19 = w.board.ticket("SHOP-19").await.unwrap();
    w.start_worktree(&shop19).await;
    let agent = w.agent_shell(&shop19).await;
    let repo = std::path::Path::new(&project.repo_path);

    let ran = agent
        .run(&format!(
            "'{CB}' branch rename bugfix/SHOP-19-price-table --json"
        ))
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    let ticket: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(ticket["work"]["branch"], "bugfix/SHOP-19-price-table");
    assert!(git(repo, &["branch", "--list", "bugfix/SHOP-19-price-table"]).contains("price-table"));
    let history = w.board.history("SHOP-19").await.unwrap();
    assert!(
        history.iter().any(|e| e.kind.as_str() == "branch_renamed"),
        "History logs the rename"
    );

    // An invalid name is refused.
    let ran = agent.run(&format!("'{CB}' branch rename 'a..b'")).await;
    assert_ne!(ran.status, 0);

    let ran = agent
        .run(&format!(
            "'{CB}' pr opened https://github.com/acme/shop-web/pull/481 --json"
        ))
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    let ticket: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(ticket["flags"]["pr_number"], 481);
    assert_eq!(
        ticket["work"]["pr_url"],
        "https://github.com/acme/shop-web/pull/481"
    );
    assert_eq!(ticket["work"]["branch_locked"], true);

    let ran = agent
        .run(&format!("'{CB}' branch rename feature/other --json"))
        .await;
    assert_ne!(ran.status, 0);
    let error: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(
        error["error"]["message"],
        "SHOP-19's branch was pushed as `bugfix/SHOP-19-price-table` and can't be renamed"
    );

    // Azure DevOps URLs give their number too; anything else is refused.
    let ran = agent
        .run(&format!(
            "'{CB}' pr opened https://dev.azure.com/acme/shop/_git/shop-web/pullrequest/90 --json"
        ))
        .await;
    let ticket: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(ticket["flags"]["pr_number"], 90);
    let ran = agent
        .run(&format!(
            "'{CB}' pr opened https://github.com/acme/shop-web --json"
        ))
        .await;
    assert_ne!(ran.status, 0);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_cant_use_them_on_another_ticket() {
    let w = TestService::start().await;
    w.shop_sample().await;
    let shop18 = w.board.ticket("SHOP-18").await.unwrap();
    let agent = w.agent_shell(&shop18).await;
    for command in [
        "pr opened https://github.com/acme/shop-web/pull/1".to_owned(),
        "branch rename x".to_owned(),
    ] {
        let ran = agent
            .run(&format!("OTAQ_TICKET=SHOP-12 '{CB}' {command} --json"))
            .await;
        assert_ne!(ran.status, 0, "{command}");
        let error: Value = serde_json::from_str(&ran.stdout).unwrap();
        assert_eq!(
            error["error"]["message"], "SHOP-18's agent can't change SHOP-12",
            "{command}"
        );
    }
    let ran = agent
        .run_with_stdin(
            &format!("OTAQ_TICKET=SHOP-12 '{CB}' pr propose --stdin --json"),
            PROPOSAL,
        )
        .await;
    assert_ne!(ran.status, 0);
    w.stop().await;
}
