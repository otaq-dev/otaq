//! `crewboard prime` from inside an agent session (T-0071): the current ticket's Markdown, `--json`, and running it
//! counts as a `crewboard` command that clears an orange label (T-0067).

use crewboard_core::domain::Status;
use crewboard_core::service::testing::TestService;
use serde_json::Value;

const CB: &str = env!("CARGO_BIN_EXE_crewboard");

#[tokio::test(flavor = "multi_thread")]
async fn prime_prints_the_context_and_clears_orange() {
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
    assert_eq!(context["commit_skill"], "/crewboard:commit");
    w.stop().await;
}
