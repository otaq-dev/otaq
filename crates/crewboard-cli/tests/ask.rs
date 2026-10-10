//! Agent-to-agent questions (T-0151; MSG-01, MSG-02, MSG-04; d0030, d0048): `crewboard ask <KEY> --stdin`,
//! `crewboard answer <ID> --stdin` and `crewboard questions` from inside agent sessions (ordinary `sh`, never
//! `claude`). The sessions never report idle, so automatic deliveries wait until Deliver now.

use crewboard_core::messages::MessageState;
use crewboard_core::naming;
use crewboard_core::service::testing::TestService;
use serde_json::Value;

const CB: &str = env!("CARGO_BIN_EXE_crewboard");

#[tokio::test(flavor = "multi_thread")]
async fn ask_a_linked_live_ticket_then_answer() {
    let w = TestService::start().await;
    w.shop_sample().await;
    let shop20 = w.board.ticket("SHOP-20").await.unwrap();
    let shop19 = w.board.ticket("SHOP-19").await.unwrap();
    let asker = w.agent_shell(&shop20).await;

    // Only linked tickets (SHOP-20 depends on SHOP-18 and SHOP-19).
    let ran = asker
        .run_with_stdin(
            &format!("'{CB}' ask SHOP-12 --stdin --json"),
            "Which colors?",
        )
        .await;
    assert_ne!(ran.status, 0);
    let error: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("aren't linked"),
        "{error}"
    );
    // A target that never started is refused (MSG-04).
    let ran = asker
        .run_with_stdin(
            &format!("'{CB}' ask SHOP-18 --stdin --json"),
            "Which status code?",
        )
        .await;
    assert_ne!(ran.status, 0);
    let error: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(
        error["error"]["message"],
        format!(
            "SHOP-18 hasn't started yet, so there is no progress to ask about. Ask the human instead (`{} ask human`).",
            naming::COMMAND_NAME
        )
    );

    // A live target: the ID comes back at once; the question waits for an idle agent with an empty line.
    let target = w.agent_shell(&shop19).await;
    let ran = asker
        .run_with_stdin(
            &format!("'{CB}' ask SHOP-19 --stdin --json"),
            "Is the price table sorted by name or by price?",
        )
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    let message: Value = serde_json::from_str(&ran.stdout).unwrap();
    let id = message["id"].as_str().unwrap().to_owned();
    assert_eq!(message["to"], "SHOP-19");
    // "asked SHOP-19 · waiting" from the moment the ask is accepted (MSG-06).
    let card = w.board.ticket("SHOP-20").await.unwrap();
    assert_eq!(
        card.flags.asked.as_ref().map(|k| k.to_string()).as_deref(),
        Some("SHOP-19")
    );
    assert_eq!(card.flags.asked_more, 0);
    let waiting = w.deliveries.pending(&target.session).await.unwrap();
    assert_eq!(waiting.len(), 1, "1 message waiting · Deliver now");
    assert_eq!(
        w.board.message(&id).await.unwrap().state(),
        MessageState::Asked
    );

    // The target lists it, and prime shows it.
    let ran = target.run(&format!("'{CB}' questions --json")).await;
    let pending: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(pending[0]["id"], id.as_str());
    let ran = target.run(&format!("'{CB}' prime")).await;
    assert!(
        ran.stdout.contains("## Pending questions"),
        "{}",
        ran.stdout
    );
    assert!(ran.stdout.contains(&format!("{id} from SHOP-20")));

    // Deliver now types it.
    w.deliveries.deliver_now(&target.session).await.unwrap();
    w.wait_for_output(&target.session, "SHOP-20's agent asks you")
        .await;
    // The typed text went to `sh`, not an agent: Ctrl+C drops the half-read line before the next command.
    w.sessions.write(&target.session, b"\x03").unwrap();
    assert_eq!(
        w.board.message(&id).await.unwrap().state(),
        MessageState::Delivered
    );

    // Only the asked ticket's agent answers.
    let ran = asker
        .run_with_stdin(&format!("'{CB}' answer {id} --stdin --json"), "By price.")
        .await;
    assert_ne!(ran.status, 0);
    let error: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(
        error["error"]["message"],
        "SHOP-20's agent can't change SHOP-19"
    );

    let ran = target
        .run_with_stdin(
            &format!("'{CB}' answer {id} --stdin --json"),
            "By price, ascending.",
        )
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    let answered = w.board.message(&id).await.unwrap();
    assert_eq!(answered.state(), MessageState::Answered);
    // The answer waits for the asker the same way, then reaches it; the flag lasts until then.
    assert_eq!(w.deliveries.pending(&asker.session).await.unwrap().len(), 1);
    assert!(
        w.board
            .ticket("SHOP-20")
            .await
            .unwrap()
            .flags
            .asked
            .is_some(),
        "answered is not delivered"
    );
    w.deliveries.deliver_now(&asker.session).await.unwrap();
    w.wait_for_output(&asker.session, "SHOP-19 answered your question")
        .await;
    w.sessions.write(&asker.session, b"\x03").unwrap();
    assert_eq!(
        w.board.message(&id).await.unwrap().state(),
        MessageState::AnswerDelivered
    );
    assert!(
        w.board
            .ticket("SHOP-20")
            .await
            .unwrap()
            .flags
            .asked
            .is_none()
    );
    // Logged on both tickets.
    for key in ["SHOP-19", "SHOP-20"] {
        let kinds: Vec<String> = w
            .board
            .history(key)
            .await
            .unwrap()
            .iter()
            .map(|e| e.kind.as_str().to_owned())
            .collect();
        assert!(kinds.contains(&"question_asked".to_owned()), "{key}");
        assert!(kinds.contains(&"question_answered".to_owned()), "{key}");
    }
    // `show --long` prints the messages.
    let ran = asker.run(&format!("'{CB}' show SHOP-20 --long")).await;
    assert!(ran.stdout.contains("Messages"), "{}", ran.stdout);
    assert!(
        ran.stdout
            .contains("SHOP-20 → SHOP-19: Is the price table sorted")
    );
    assert!(ran.stdout.contains("answered: By price, ascending."));
    let ran = target.run(&format!("'{CB}' questions")).await;
    assert_eq!(ran.stdout.trim(), "No questions wait for SHOP-19.");
    w.stop().await;
}
