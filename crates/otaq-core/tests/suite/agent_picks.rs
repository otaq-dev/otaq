//! Agent rows from `ticket.create_batch` and `ticket.update` (T-0338; AGT-06, CLI-03, CLI-06, CLI-07; d0086,
//! d0087), through the socket: the user's rows are *set by you*; a Project agent picks on Backlog tickets and an
//! epic lead on its never-started children, each *picked by* its name with a reason; ticket agents and validators
//! never pick; a pick never overwrites a row the user set unless the agent says the user named it (`by_user`);
//! locked rows are refused for everyone; with one named agent agents make no picks.

use std::sync::Arc;

use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::agent_tools::{LaunchContext, Tool};
use otaq_core::board::tickets::{EventKind, NewTicketInput, Ticket};
use otaq_core::client::{Client, ClientConfig, ClientError};
use otaq_core::domain::{Actor, Kind};
use otaq_core::git::testing::TempRepos;
use otaq_core::named_agents::starter::{self, ConnectedTool};
use otaq_core::named_agents::{AgentFields, FixedModels, NamedAgents, Role, RowSetBy};
use otaq_core::policy::{BACKLOG_ONLY, NOTHING_TO_PICK, SAY_WHY, TICKET_AGENTS_CANT_PICK};
use otaq_core::protocol::Request;
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::{AgentShell, TestService, path_without_build_folders};
use otaq_core::session::SessionKind;
use serde_json::{Value, json};

/// The sample board (SHOP-40 Wishlist: 40.1 Merged, 40.2 Working, 40.3 Backlog), the epic SHOP-41 Search with
/// SHOP-41.1, and the starter entry "Claude Code" only; `claude` is an interactive `sh`.
async fn world() -> (TestService, NamedAgents) {
    let home = tempfile::TempDir::new().unwrap();
    let data = home.path().join("h");
    let script = format!(
        "export {}='{}'; export PS1=''; exec sh",
        otaq_core::data_dir::DataDir::env_var(),
        data.display()
    );
    let w = TestService::start_in(home, TempRepos::new(), |config| ServiceConfig {
        launch: Some(LaunchContext {
            bin_dir: "/nonexistent".into(),
            plugin_dir: "/nonexistent/plugin".into(),
            path: path_without_build_folders().into(),
            claude: ClaudeLaunch {
                program: vec!["sh".into(), "-c".into(), script, "sh".into()],
                config_dir: None,
            },
        }),
        ..config
    })
    .await;
    w.shop_sample().await;
    let me = Actor::User("me".into());
    let search = w
        .board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Search".into(),
                kind: Kind::Epic,
                ..Default::default()
            },
            &me,
        )
        .await
        .unwrap();
    w.board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Search index API".into(),
                epic: Some(search.key.to_string()),
                ..Default::default()
            },
            &me,
        )
        .await
        .unwrap();
    let claude_code = ConnectedTool {
        tool: Tool::ALL[0].as_str().into(),
        name: "Claude Code".into(),
    };
    starter::ensure(w.board.store(), &[claude_code])
        .await
        .unwrap();
    let agents = NamedAgents::new(w.board.clone(), Arc::new(FixedModels::sample()));
    (w, agents)
}

/// Adds Strong and Light: from now on there is something to pick.
async fn add_agents(agents: &NamedAgents) {
    for name in ["Strong", "Light"] {
        agents
            .add(&AgentFields {
                name: name.into(),
                tool: Tool::ALL[0],
                model: None,
                use_for: String::new(),
            })
            .await
            .unwrap();
    }
}

async fn send(w: &TestService, shell: &AgentShell, method: &str, params: Value) -> Value {
    let request =
        json!({"protocol_version": 1, "id": 1, "request": {"method": method, "params": params}});
    let ran = shell
        .send_raw(&w.data.socket_path(), &request.to_string())
        .await;
    serde_json::from_str(ran.stdout.lines().next().unwrap_or_default())
        .unwrap_or_else(|e| panic!("{e}: {ran:?}"))
}

async fn send_as_user(w: &TestService, method: &str, params: Value) -> Value {
    let request: Request =
        serde_json::from_value(json!({"method": method, "params": params})).unwrap();
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    match client.request(request).await {
        Ok(reply) => json!({"reply": serde_json::to_value(reply).unwrap()}),
        Err(ClientError::Service(e)) => json!({"error": serde_json::to_value(e).unwrap()}),
        Err(e) => panic!("{method}: {e}"),
    }
}

fn ok(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "{answer}");
    &answer["reply"]
}

fn refused(answer: &Value) -> &str {
    assert_eq!(answer["error"]["kind"], "refused", "{answer}");
    answer["error"]["message"].as_str().unwrap()
}

/// `ticket.update` params setting one row.
fn pick(key: &str, role: &str, agent: &str, reason: Option<&str>) -> Value {
    let mut pick = json!({"role": role, "agent": agent});
    if let Some(reason) = reason {
        pick["reason"] = json!(reason);
    }
    json!({"key": key, "agents": [pick]})
}

/// A row as (agent name, who set it); `None` when empty.
async fn row(
    w: &TestService,
    agents: &NamedAgents,
    key: &str,
    role: Role,
) -> Option<(String, RowSetBy)> {
    let ticket: Ticket = w.board.ticket(key).await.unwrap();
    let row = w
        .board
        .store()
        .ticket_agent_rows(&ticket.id)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.role == role)?;
    let name = agents.agent(row.agent.as_deref()?).await.unwrap().name;
    Some((name, row.set_by))
}

/// The payload of the newest `agent_row_set` event on `key`.
async fn last_row_event(w: &TestService, key: &str) -> Value {
    w.board
        .history(key)
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.kind == EventKind::AgentRowSet)
        .unwrap_or_else(|| panic!("no agent_row_set on {key}"))
        .payload
}

fn picked(by: &str, reason: &str) -> RowSetBy {
    RowSetBy::Agent {
        picked_by: by.into(),
        reason: reason.into(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_user_sets_rows_and_names_ignore_case() {
    let (w, agents) = world().await;
    add_agents(&agents).await;

    // `otaq create "Fix README typo" --developer light` from the user's terminal: set by you, no reason kept.
    let created = send_as_user(
        &w,
        "ticket.create_batch",
        json!({"project": "SHOP", "tickets": [{"title": "Fix README typo", "agents": [
            {"role": "developer", "agent": "light", "reason": "ignored"},
            {"role": "validator", "agent": "STRONG"}]}]}),
    )
    .await;
    let key = ok(&created)["tickets"][0]["key"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        row(&w, &agents, &key, Role::Developer).await,
        Some(("Light".into(), RowSetBy::User))
    );
    assert_eq!(
        row(&w, &agents, &key, Role::Validator).await,
        Some(("Strong".into(), RowSetBy::User))
    );

    // `default` empties the row; a field change and a pick in one update.
    let answer = send_as_user(
        &w,
        "ticket.update",
        json!({"key": key, "title": "Fix the README typo",
               "agents": [{"role": "developer", "agent": "Default"}]}),
    )
    .await;
    assert_eq!(ok(&answer)["ticket"]["title"], "Fix the README typo");
    assert_eq!(row(&w, &agents, &key, Role::Developer).await, None);

    // An unknown name lists the agents; nothing changes (the whole batch fails).
    let before = w.board.tickets("SHOP").await.unwrap().len();
    let answer = send_as_user(
        &w,
        "ticket.create_batch",
        json!({"project": "SHOP", "tickets": [
            {"title": "One"},
            {"title": "Two", "agents": [{"role": "developer", "agent": "Lite"}]}]}),
    )
    .await;
    assert_eq!(answer["error"]["kind"], "not_found", "{answer}");
    assert_eq!(
        answer["error"]["message"],
        "No agent named Lite. Agents: Claude Code, Strong, Light"
    );
    assert_eq!(w.board.tickets("SHOP").await.unwrap().len(), before);

    // The wrong row for the kind.
    assert_eq!(
        refused(
            &send_as_user(
                &w,
                "ticket.update",
                pick("SHOP-40", "developer", "Light", None)
            )
            .await
        ),
        "SHOP-40 is an epic: it has an Epic lead row, not a Developer row"
    );
    assert_eq!(
        refused(&send_as_user(&w, "ticket.update", pick("SHOP-13", "lead", "Light", None)).await),
        "SHOP-13 has no Epic lead row: tickets have Developer and Validator rows"
    );

    // A locked row is refused for the user too: SHOP-14's developer session has started.
    let shop14 = w.board.ticket("SHOP-14").await.unwrap();
    let _session = w.shell(&shop14, SessionKind::Agent).await;
    let answer = send_as_user(
        &w,
        "ticket.update",
        pick("SHOP-14", "developer", "Light", None),
    )
    .await;
    assert_eq!(
        refused(&answer),
        "SHOP-14's Developer is locked: Send back continues the same conversation"
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_project_agent_picks_on_backlog_tickets_with_a_reason_and_never_over_the_users_row() {
    let (w, agents) = world().await;
    let agent = w
        .project_agents
        .open("SHOP", "Look around the checkout code")
        .await
        .unwrap();
    let shell = w.shell_in(&agent.session.as_ref().unwrap().id);
    ok(&send(
        &w,
        &shell,
        "agent.name",
        json!({"name": "Checkout slowness"}),
    )
    .await);

    // One named agent: nothing to pick (Q-H), also on create.
    assert_eq!(
        refused(
            &send(
                &w,
                &shell,
                "ticket.update",
                pick("SHOP-13", "developer", "Claude Code", Some("x"))
            )
            .await
        ),
        NOTHING_TO_PICK
    );
    add_agents(&agents).await;

    // Created with Light picked: picked by its name, with the reason.
    let created = send(
        &w,
        &shell,
        "ticket.create_batch",
        json!({"project": "SHOP", "tickets": [{"title": "Fix README typo", "agents": [
            {"role": "developer", "agent": "Light", "reason": "docs-only change"}]}]}),
    )
    .await;
    let key = ok(&created)["tickets"][0]["key"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        row(&w, &agents, &key, Role::Developer).await,
        Some((
            "Light".into(),
            picked("Checkout slowness", "docs-only change")
        ))
    );

    // No reason: refused.
    assert_eq!(
        refused(
            &send(
                &w,
                &shell,
                "ticket.update",
                pick("SHOP-13", "developer", "Light", None)
            )
            .await
        ),
        SAY_WHY
    );
    assert_eq!(
        refused(
            &send(
                &w,
                &shell,
                "ticket.update",
                pick("SHOP-13", "developer", "Light", Some("  "))
            )
            .await
        ),
        SAY_WHY
    );
    // A Ready for Dev ticket (SHOP-20 is queued): CLI-06's refusal.
    assert_eq!(
        refused(
            &send(
                &w,
                &shell,
                "ticket.update",
                pick("SHOP-20", "developer", "Light", Some("small"))
            )
            .await
        ),
        BACKLOG_ONLY
    );

    // The user's row: refused, also with a field change, which then doesn't happen either.
    ok(&send_as_user(
        &w,
        "ticket.update",
        pick("SHOP-13", "developer", "Strong", None),
    )
    .await);
    let mut both = pick("SHOP-13", "developer", "Light", Some("small"));
    both["title"] = json!("Changed");
    assert_eq!(
        refused(&send(&w, &shell, "ticket.update", both).await),
        "SHOP-13's Developer was set by you; agents can't change it"
    );
    assert_ne!(w.board.ticket("SHOP-13").await.unwrap().title, "Changed");
    // ... unless the user named it: set by you.
    let mut by_user = pick("SHOP-13", "developer", "Light", None);
    by_user["by_user"] = json!(true);
    ok(&send(&w, &shell, "ticket.update", by_user).await);
    assert_eq!(
        row(&w, &agents, "SHOP-13", Role::Developer).await,
        Some(("Light".into(), RowSetBy::User))
    );
    // An agent's own earlier pick may be re-picked.
    ok(&send(
        &w,
        &shell,
        "ticket.update",
        pick(&key, "developer", "Strong", Some("needs more")),
    )
    .await);
    assert_eq!(
        row(&w, &agents, &key, Role::Developer).await,
        Some(("Strong".into(), picked("Checkout slowness", "needs more")))
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_epic_lead_picks_on_its_unstarted_children_only() {
    let (w, agents) = world().await;
    add_agents(&agents).await;
    let lead = w.project_agents.leads().open("SHOP-40").await.unwrap();
    let shell = w.shell_in(&lead.agent.session.as_ref().unwrap().id);
    let refusal = "Wishlist lead can only change SHOP-40's children that haven't started";

    ok(&send(
        &w,
        &shell,
        "ticket.update",
        pick(
            "SHOP-40.3",
            "developer",
            "Light",
            Some("only a config change"),
        ),
    )
    .await);
    assert_eq!(
        row(&w, &agents, "SHOP-40.3", Role::Developer).await,
        Some((
            "Light".into(),
            picked("Wishlist lead", "only a config change")
        ))
    );
    // History on the child and on the epic, which names the child.
    let (child_event, epic_event) = (
        last_row_event(&w, "SHOP-40.3").await,
        last_row_event(&w, "SHOP-40").await,
    );
    assert_eq!(
        child_event,
        json!({"role": "developer", "agent": "Light", "from": null, "set_by": "agent",
               "picked_by": "Wishlist lead", "reason": "only a config change"})
    );
    assert_eq!(epic_event["child"], "SHOP-40.3");
    assert_eq!(epic_event["picked_by"], "Wishlist lead");
    // A re-pick says what it was.
    ok(&send(
        &w,
        &shell,
        "ticket.update",
        pick(
            "SHOP-40.3",
            "developer",
            "Strong",
            Some("bigger than it looked"),
        ),
    )
    .await);
    assert_eq!(last_row_event(&w, "SHOP-40").await["from"], "Light");
    // A new child with its rows.
    let created = send(
        &w,
        &shell,
        "ticket.create_batch",
        json!({"project": "SHOP", "tickets": [{"title": "Wishlist email", "parent": "SHOP-40", "agents": [
            {"role": "validator", "agent": "Strong", "reason": "touches checkout"}]}]}),
    )
    .await;
    let key = ok(&created)["tickets"][0]["key"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        row(&w, &agents, &key, Role::Validator).await,
        Some(("Strong".into(), picked("Wishlist lead", "touches checkout")))
    );

    for (key, role) in [
        ("SHOP-40.2", "developer"), // started
        ("SHOP-41.1", "developer"), // another epic's child
        ("SHOP-40", "lead"),        // its own Epic lead row
        ("SHOP-13", "developer"),   // a standalone ticket
    ] {
        assert_eq!(
            refused(
                &send(
                    &w,
                    &shell,
                    "ticket.update",
                    pick(key, role, "Light", Some("x"))
                )
                .await
            ),
            refusal,
            "{key}"
        );
    }
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ticket_agents_and_validators_never_pick() {
    let (w, agents) = world().await;
    add_agents(&agents).await;
    let shop13 = w.board.ticket("SHOP-13").await.unwrap();
    let agent = w.agent_shell(&shop13).await;
    let validator = w.shell(&shop13, SessionKind::Validator).await;
    for shell in [&agent, &validator] {
        // Its own ticket, another, with `by_user`, and on create.
        let mut by_user = pick("SHOP-13", "validator", "Light", Some("x"));
        by_user["by_user"] = json!(true);
        let mut with_title = pick("SHOP-13", "developer", "Light", Some("x"));
        with_title["title"] = json!("t");
        for (method, params) in [
            (
                "ticket.update",
                pick("SHOP-13", "developer", "Light", Some("x")),
            ),
            (
                "ticket.update",
                pick("SHOP-14", "developer", "Light", Some("x")),
            ),
            ("ticket.update", by_user),
            ("ticket.update", with_title),
            (
                "ticket.create_batch",
                json!({"project": "SHOP", "tickets": [{"title": "New", "agents": [
                    {"role": "developer", "agent": "Light", "reason": "x"}]}]}),
            ),
        ] {
            assert_eq!(
                refused(&send(&w, shell, method, params).await),
                TICKET_AGENTS_CANT_PICK
            );
        }
    }
    assert_eq!(row(&w, &agents, "SHOP-13", Role::Developer).await, None);
    w.stop().await;
}
