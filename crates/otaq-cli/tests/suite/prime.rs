//! `otaq prime` from inside an agent session (T-0071): the current ticket's Markdown, `--json`, and running it
//! counts as a `otaq` command that clears a needs-you label (T-0067).

use otaq_core::board::settings::WorkflowStep;
use otaq_core::domain::Status;
use otaq_core::service::testing::TestService;
use otaq_core::workflow_skills::testing::skill_folder;
use serde_json::Value;

const CB: &str = env!("CARGO_BIN_EXE_otaq");

#[tokio::test(flavor = "multi_thread")]
async fn prime_prints_the_context_and_clears_needs_you() {
    let w = TestService::start().await;
    w.shop_sample().await;
    assert_eq!(w.status("SHOP-12").await, Status::AsksYou);
    let shop12 = w.board.ticket("SHOP-12").await.unwrap();
    let agent = w.agent_shell(&shop12).await;

    let ran = agent.run(&format!("'{CB}' prime")).await;
    assert_eq!(ran.status, 0, "{ran:?}");
    assert!(
        ran.stdout.starts_with("# SHOP-12 · Add dark mode toggle\n"),
        "{}",
        ran.stdout
    );
    assert!(ran.stdout.contains("review submit --stdin"));
    assert_eq!(
        w.status("SHOP-12").await,
        Status::Working,
        "prime is a command"
    );

    let ran = agent.run(&format!("'{CB}' prime --json")).await;
    let context: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(context["ticket"]["key"], "SHOP-12");
    assert_eq!(context["commit_skill"], "/otaq:commit");
    w.stop().await;
}

/// REV-09, WSK-06: Developer's imported Commit skill is the skill prime names, by its copy in the ticket's worktree
/// once the launch wrote it there (d0092); before that, the built-in.
#[tokio::test(flavor = "multi_thread")]
async fn prime_names_developers_imported_commit_skill() {
    let w = TestService::start().await;
    w.shop_sample().await;
    let folder = skill_folder(w.data.root(), "my-commit");
    w.import_step_skill(WorkflowStep::Commit, &folder).await;
    let shop12 = w.board.ticket("SHOP-12").await.unwrap();
    let worktree = w.give_worktree(&shop12).await;
    let agent = w.agent_shell(&shop12).await;
    let ran = agent.run(&format!("'{CB}' prime --json")).await;
    let context: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(context["commit_skill"], "/otaq:commit", "not copied yet");

    w.board
        .copy_skills_into("SHOP", &worktree, &[WorkflowStep::Commit])
        .await
        .unwrap();
    let ran = agent.run(&format!("'{CB}' prime")).await;
    assert_eq!(ran.status, 0, "{ran:?}");
    let rule = ran
        .stdout
        .lines()
        .find(|l| l.contains("before you submit"))
        .expect("the commit rule");
    assert_eq!(
        rule,
        "2. Commit everything by following the skill in `.agents/skills/otaq-custom-commit/` (start with its \
         SKILL.md; paths in it are relative to that folder) before you submit. Never push and never open a PR: the \
         user does that."
    );
    let ran = agent.run(&format!("'{CB}' prime --json")).await;
    let context: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(
        context["commit_skill"],
        serde_json::json!({ "folder": ".agents/skills/otaq-custom-commit/" })
    );
    w.stop().await;
}
