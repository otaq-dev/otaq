//! `crewboard validate pass|fail --stdin` and `crewboard prime --validate` (T-0184; VAL-03, CLI-05; d0063), from
//! a validator session (an ordinary `sh` started as one), from the ticket's agent and from the user.

use std::process::Command;

use crewboard_core::data_dir::DataDir;
use crewboard_core::domain::Status;
use crewboard_core::naming;
use crewboard_core::service::testing::TestService;
use crewboard_core::session::SessionKind;
use serde_json::{Value, json};

const CB: &str = env!("CARGO_BIN_EXE_crewboard");

#[tokio::test(flavor = "multi_thread")]
async fn only_the_validator_reports_a_verdict() {
    let w = TestService::start().await;
    w.shop_sample().await;
    let shop19 = w.board.ticket("SHOP-19").await.unwrap();
    let validator = w.shell(&shop19, SessionKind::Validator).await;
    let agent = w.agent_shell(&shop19).await;
    w.validating_with(&shop19, &validator.session).await;
    let only = "Only a validator can report a verdict";

    // The user, from their own terminal.
    let out = Command::new(CB)
        .args(["validate", "pass", "--stdin", "--json"])
        .env(DataDir::env_var(), w.data.root())
        .env(naming::ticket_env_var(), "SHOP-19")
        .current_dir(w.home.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let e: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(e["error"]["message"], only);
    // The ticket's agent.
    let ran = agent
        .run_with_stdin(
            &format!("'{CB}' validate pass --stdin --json"),
            "looks fine",
        )
        .await;
    assert_ne!(ran.status, 0);
    let e: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(e["error"]["message"], only);
    assert_eq!(w.status("SHOP-19").await, Status::Validating);

    // The validator: it reads its context, then fails the round; the findings go back to the agent.
    let ran = validator.run(&format!("'{CB}' prime --validate")).await;
    assert_eq!(ran.status, 0, "{ran:?}");
    assert!(
        ran.stdout.starts_with("# Validate SHOP-19 · Price table"),
        "{ran:?}"
    );
    let ran = validator
        .run_with_stdin(
            &format!("'{CB}' validate fail --stdin"),
            "src/prices.ts:3 not sorted",
        )
        .await;
    assert_eq!(
        (ran.status, ran.stdout.trim()),
        (0, "SHOP-19: validation failed. End your turn."),
        "{ran:?}"
    );
    assert_eq!(w.status("SHOP-19").await, Status::Working);
    let pending = w.deliveries.pending(&agent.session).await.unwrap();
    assert!(
        pending[0].text.ends_with("src/prices.ts:3 not sorted"),
        "{pending:?}"
    );

    // Its turn ends: the validator is done.
    // Typed, not awaited: the service ends the session the command runs in.
    let payload = json!({"hook_event_name": "Stop", "background_tasks": [], "session_crons": []});
    let line = format!("printf '%s' '{payload}' | '{CB}' hook Stop\r");
    w.sessions
        .input(&validator.session, line.as_bytes())
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while w
            .sessions
            .running_of(&shop19.id, SessionKind::Validator)
            .is_some()
        {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the validator ends after its verdict turn");
    w.stop().await;
}
