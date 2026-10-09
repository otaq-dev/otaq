//! The permission scope through the socket (T-0065, CLI-02, CLI-03): requests sent from inside SHOP-1's agent
//! session (an ordinary shell running `nc`) are judged as SHOP-1's agent; the same requests from the test process
//! are the user's.

use crewboard_core::domain::{Actor, Column};
use crewboard_core::service::testing::{AgentShell, TestService};
use serde_json::{Value, json};

/// Sends one raw request from inside the agent shell and returns the answer.
async fn send(world: &TestService, agent: &AgentShell, method: &str, params: Value) -> Value {
    let request =
        json!({"protocol_version": 1, "id": 1, "request": {"method": method, "params": params}});
    let ran = agent
        .run(&format!(
            "printf '%s\\n' '{request}' | nc -U -w 1 '{}'",
            world.data.socket_path().display()
        ))
        .await;
    serde_json::from_str(ran.stdout.lines().next().unwrap_or_default())
        .unwrap_or_else(|e| panic!("{e}: {ran:?}"))
}

fn refusal(answer: &Value) -> (&str, &str) {
    let error = &answer["error"];
    assert_eq!(error["kind"], "refused", "{answer}");
    (
        error["data"]["refusal"].as_str().unwrap(),
        error["message"].as_str().unwrap(),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_reads_anything_and_changes_only_its_own_ticket() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    w.create("SHOP", "Add wishlist").await;
    let agent = w.agent_shell(&shop1).await;

    // Reads across the project.
    let board = send(&w, &agent, "board.snapshot", json!({"project": "SHOP"})).await;
    assert!(board["reply"]["board"].is_object(), "{board}");
    let other = send(&w, &agent, "ticket.get", json!({"key": "SHOP-2"})).await;
    assert_eq!(other["reply"]["ticket"]["key"], "SHOP-2");

    // Its own ticket: allowed, recorded as the agent.
    let own = send(
        &w,
        &agent,
        "ticket.update",
        json!({"key": "SHOP-1", "title": "Split the full name"}),
    )
    .await;
    assert_eq!(own["reply"]["ticket"]["updated_by"], "agent:SHOP-1");

    // Another ticket: refused with the exact text.
    let answer = send(
        &w,
        &agent,
        "ticket.update",
        json!({"key": "SHOP-2", "title": "x"}),
    )
    .await;
    assert_eq!(
        refusal(&answer),
        ("not_allowed", "SHOP-1's agent can't change SHOP-2")
    );
    assert_eq!(
        w.board.ticket("SHOP-2").await.unwrap().title,
        "Add wishlist"
    );

    // Moves, deletes and settings are the user's.
    for (method, params) in [
        (
            "ticket.move",
            json!({"key": "SHOP-1", "target": "dev_queued"}),
        ),
        ("ticket.delete", json!({"key": "SHOP-2"})),
        ("ticket.pin", json!({"key": "SHOP-1"})),
        ("service.stop", Value::Null),
    ] {
        let answer = send(&w, &agent, method, params).await;
        let (code, message) = refusal(&answer);
        assert_eq!(code, "not_allowed", "{method}");
        assert!(message.starts_with("SHOP-1's agent can't "), "{message}");
    }

    // The same change from the user's own process passes.
    w.board
        .update_ticket(
            "SHOP-2",
            &crewboard_core::board::tickets::TicketUpdate {
                title: Some("Wishlist".into()),
                ..Default::default()
            },
            &Actor::User("me".into()),
        )
        .await
        .unwrap();
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_created_tickets_land_in_backlog_and_may_be_linked() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    w.create("SHOP", "Add wishlist").await;
    w.create("SHOP", "Fix checkout").await;
    let agent = w.agent_shell(&shop1).await;

    for title in ["Follow-up A", "Follow-up B"] {
        let created = send(
            &w,
            &agent,
            "ticket.create",
            json!({"project": "SHOP", "title": title}),
        )
        .await;
        let ticket = &created["reply"]["ticket"];
        assert_eq!(ticket["column"], Column::Backlog.as_str(), "{created}");
        assert_eq!(ticket["created_by"], "agent:SHOP-1");
    }
    // From its own ticket, and between the tickets it created: allowed.
    let from_own = send(
        &w,
        &agent,
        "dep.add",
        json!({"child": "SHOP-1", "parent": "SHOP-2"}),
    )
    .await;
    assert!(from_own.get("reply").is_some(), "{from_own}");
    let between = send(
        &w,
        &agent,
        "dep.add",
        json!({"child": "SHOP-5", "parent": "SHOP-4"}),
    )
    .await;
    assert!(between.get("reply").is_some(), "{between}");
    // Between two user-created tickets: refused.
    let answer = send(
        &w,
        &agent,
        "dep.add",
        json!({"child": "SHOP-3", "parent": "SHOP-2"}),
    )
    .await;
    assert_eq!(refusal(&answer).0, "not_allowed");
    assert!(
        w.board
            .ticket("SHOP-3")
            .await
            .unwrap()
            .depends_on
            .is_empty()
    );
    w.stop().await;
}
