//! What the sessions say about a ticket's named agents (T-0336; AGT-04, ADP-04; d0086, d0088): the lock of each
//! row in each state, the "moved on" query (the panel's "Sonnet → claude-sonnet-5-6") and the tokens by role and
//! named agent ("Strong 61k · Light 23k (1 round)"). Sessions are stored rows; nothing runs.

use std::sync::Arc;

use otaq_core::agent_tools::{Adapters, Tool};
use otaq_core::board::Refusal;
use otaq_core::board::tickets::{NewTicketInput, Ticket};
use otaq_core::data_dir::DataDir;
use otaq_core::domain::Kind;
use otaq_core::named_agents::{FixedModels, NamedAgents, Role, RowLock, row_lock};
use otaq_core::service::EventBus;
use otaq_core::session::{MovedOn, moved_on};
use otaq_core::store::new_id;
use otaq_core::usage::{AgentTokens, Tokens, Usage};

use crate::common::{self, Fixture};

/// One stored session of `t`: its kind, the named agent it ran on (id, name), the exact model that ran, and when it
/// started.
async fn session(
    f: &Fixture,
    t: &Ticket,
    kind: &str,
    agent: Option<(&str, &str)>,
    model_ran: Option<&str>,
    started_at: i64,
) -> String {
    let project = f.board.project("SHOP").await.unwrap();
    let id = new_id();
    sqlx::query(
        "INSERT INTO sessions (id, project_id, ticket_id, kind, tool, tool_session_id, named_agent_id,
                               named_agent_name, model_ran, state, started_at, ended_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'ended', ?, ?)",
    )
    .bind(&id)
    .bind(&project.id)
    .bind(&t.id)
    .bind(kind)
    .bind(Tool::Claude.as_str())
    .bind(format!("conversation-{id}"))
    .bind(agent.map(|(id, _)| id))
    .bind(agent.map(|(_, name)| name))
    .bind(model_ran)
    .bind(started_at)
    .bind(started_at + 1)
    .execute(f.board.store().pool())
    .await
    .unwrap();
    id
}

async fn lock(rules: &NamedAgents, t: &Ticket, role: Role) -> Option<RowLock> {
    row_lock(role, rules.lock_facts(&t.id).await.unwrap())
}

/// `sql` with `values` bound in order.
async fn exec(f: &Fixture, sql: &'static str, values: &[&str]) {
    let mut query = sqlx::query(sql);
    for value in values {
        query = query.bind(*value);
    }
    query.execute(f.board.store().pool()).await.unwrap();
}

/// Developer: free until the ticket starts, locked once it has a developer session, free while its only session
/// ended refused on the model ("not started", T-0337).
/// Validator: locked only while a round's validator runs; a round waiting for the limit, or ended (pass, fail, no
/// result), leaves it free. Epic lead: locked once the lead has a session not refused on the model. A change to a
/// locked row is refused.
#[tokio::test]
async fn each_row_locks_in_its_own_state() {
    let f = Fixture::shop().await;
    let rules = NamedAgents::new(f.board.clone(), Arc::new(FixedModels::sample()));
    let t = f.create("Price table").await;
    for role in [Role::Developer, Role::Validator] {
        assert_eq!(
            lock(&rules, &t, role).await,
            None,
            "{role:?} before the start"
        );
    }

    exec(
        &f,
        "UPDATE tickets SET started_at = 1 WHERE id = ?",
        &[&t.id],
    )
    .await;
    let first = session(&f, &t, "agent", Some(("strong", "Strong")), None, 10).await;
    assert_eq!(
        lock(&rules, &t, Role::Developer).await,
        Some(RowLock::Started)
    );
    assert_eq!(lock(&rules, &t, Role::Validator).await, None);
    let refused = rules
        .set_row_by_user("SHOP-1", Role::Developer, None)
        .await
        .unwrap_err();
    assert_eq!(refused.refusal(), Some(Refusal::RowLocked));
    assert_eq!(
        refused.to_string(),
        "SHOP-1's Developer is locked: Send back continues the same conversation"
    );

    // The first turn was refused on the model: back to not started, the row can change again.
    exec(
        &f,
        "UPDATE sessions SET end_reason = 'model refused' WHERE id = ?",
        &[&first],
    )
    .await;
    assert_eq!(lock(&rules, &t, Role::Developer).await, None, "not started");
    // Retry start: a new session that got through counts as started.
    session(&f, &t, "agent", Some(("light", "Light")), None, 12).await;
    assert_eq!(
        lock(&rules, &t, Role::Developer).await,
        Some(RowLock::Started)
    );

    // A round waiting for the usage limit has no validator yet.
    let round = new_id();
    sqlx::query(
        "INSERT INTO validations (id, ticket_id, round, trigger, started_at) VALUES (?, ?, 1, 'agent', 20)",
    )
    .bind(&round)
    .bind(&t.id)
    .execute(f.board.store().pool())
    .await
    .unwrap();
    assert_eq!(lock(&rules, &t, Role::Validator).await, None, "waiting");
    exec(
        &f,
        "UPDATE validations SET session_id = ? WHERE id = ?",
        &[&first, &round],
    )
    .await;
    assert_eq!(
        lock(&rules, &t, Role::Validator).await,
        Some(RowLock::RoundRunning)
    );
    let refused = rules
        .set_row_by_user("SHOP-1", Role::Validator, None)
        .await
        .unwrap_err();
    assert_eq!(
        refused.to_string(),
        "SHOP-1's Validator is locked until this round ends"
    );
    for outcome in ["fail", "no_result", "pass"] {
        exec(
            &f,
            "UPDATE validations SET ended_at = NULL, outcome = NULL WHERE id = ?",
            &[&round],
        )
        .await;
        assert_eq!(
            lock(&rules, &t, Role::Validator).await,
            Some(RowLock::RoundRunning)
        );
        exec(
            &f,
            "UPDATE validations SET ended_at = 30, outcome = ? WHERE id = ?",
            &[outcome, &round],
        )
        .await;
        assert_eq!(
            lock(&rules, &t, Role::Validator).await,
            None,
            "after {outcome}"
        );
    }

    let epic = f
        .create_with(NewTicketInput {
            title: "Wishlist".into(),
            kind: Kind::Epic,
            ..common::shop_input()
        })
        .await;
    assert_eq!(lock(&rules, &epic, Role::EpicLead).await, None);
    let project = f.board.project("SHOP").await.unwrap();
    let lead = new_id();
    sqlx::query(
        "INSERT INTO project_agents (id, project_id, tool, state, created_at, epic_id)
         VALUES (?, ?, ?, 'running', 1, ?)",
    )
    .bind(&lead)
    .bind(&project.id)
    .bind(Tool::Claude.as_str())
    .bind(&epic.id)
    .execute(f.board.store().pool())
    .await
    .unwrap();
    let lead_session = new_id();
    sqlx::query(
        "INSERT INTO sessions (id, project_id, kind, tool, project_agent_id, state, started_at, end_reason)
         VALUES (?, ?, 'project_agent', ?, ?, 'ended', 1, 'model refused')",
    )
    .bind(&lead_session)
    .bind(&project.id)
    .bind(Tool::Claude.as_str())
    .bind(&lead)
    .execute(f.board.store().pool())
    .await
    .unwrap();
    assert_eq!(
        lock(&rules, &epic, Role::EpicLead).await,
        None,
        "a lead refused on the model hasn't started"
    );
    exec(
        &f,
        "UPDATE sessions SET end_reason = 'exit 0' WHERE id = ?",
        &[&lead_session],
    )
    .await;
    assert_eq!(
        lock(&rules, &epic, Role::EpicLead).await,
        Some(RowLock::Started)
    );
}

/// "Moved on": a session's exact model against the previous session of the same ticket, role and named agent.
#[tokio::test]
async fn moved_on_compares_the_previous_run_of_the_same_agent_and_role() {
    let f = Fixture::shop().await;
    let t = f.create("Fix flaky login redirect").await;
    let sonnet = Some(("sonnet", "Sonnet"));
    let pool = f.board.store().pool();
    let first = session(&f, &t, "agent", sonnet, Some("claude-sonnet-5-5"), 10).await;
    let validator = session(&f, &t, "validator", sonnet, Some("claude-sonnet-5-7"), 15).await;
    let other = session(
        &f,
        &t,
        "agent",
        Some(("light", "Light")),
        Some("claude-haiku-5-5"),
        18,
    )
    .await;
    let unknown = session(&f, &t, "agent", sonnet, None, 19).await;
    let moved = session(&f, &t, "agent", sonnet, Some("claude-sonnet-5-6"), 20).await;
    let same = session(&f, &t, "agent", sonnet, Some("claude-sonnet-5-6"), 30).await;

    assert_eq!(
        moved_on(pool, &first).await.unwrap(),
        None,
        "no earlier run"
    );
    assert_eq!(
        moved_on(pool, &moved).await.unwrap(),
        Some(MovedOn {
            before: "claude-sonnet-5-5".into(),
            now: "claude-sonnet-5-6".into(),
        }),
        "a different agent's or role's sessions and an unknown model in between don't count"
    );
    assert_eq!(moved_on(pool, &same).await.unwrap(), None, "same model");
    assert_eq!(
        moved_on(pool, &validator).await.unwrap(),
        None,
        "the validator's own role"
    );
    assert_eq!(moved_on(pool, &other).await.unwrap(), None);
    assert_eq!(
        moved_on(pool, &unknown).await.unwrap(),
        None,
        "not known yet"
    );
}

/// Tokens by role and named agent: the developer's sessions (resumed and answering ones too) on Strong, the
/// validator's on Light, an epic's lead on its own; a session that ran on no named agent is its own entry.
#[tokio::test]
async fn tokens_are_split_by_role_and_named_agent() {
    let f = Fixture::shop().await;
    let t = f.create("Price table").await;
    let strong = Some(("strong", "Strong"));
    let light = Some(("light", "Light"));
    let pool = f.board.store().pool();
    let mut used = Vec::new();
    for (kind, agent, at, input) in [
        ("validator", light, 15, 20_000),
        ("agent", strong, 10, 40_000),
        ("agent", strong, 20, 20_000),
        ("answer", strong, 30, 1_000),
        ("validator", light, 40, 3_000),
    ] {
        used.push((session(&f, &t, kind, agent, None, at).await, input));
    }
    for (id, input) in &used {
        sqlx::query("INSERT INTO usage (session_id, day, input, cached, output) VALUES (?, '2026-10-17', ?, 9, 0)")
            .bind(id)
            .bind(input)
            .execute(pool)
            .await
            .unwrap();
    }
    let home = tempfile::tempdir().unwrap();
    let data = DataDir::new(home.path());
    let usage = Usage::new(
        f.board.store().clone(),
        EventBus::new(8),
        Adapters::launching_only(None, data),
        std::time::Duration::from_secs(3600),
    );
    let tokens = |input: u64, sessions: u64| {
        Some(Tokens {
            input,
            cached: 9 * sessions,
            output: 0,
            reasoning: None,
        })
    };
    assert_eq!(
        usage.agents_of(&t.id).await.unwrap(),
        [
            AgentTokens {
                role: Role::Developer,
                agent: Some("Strong".into()),
                tool: Some(Tool::Claude),
                tokens: tokens(61_000, 3),
            },
            AgentTokens {
                role: Role::Validator,
                agent: Some("Light".into()),
                tool: Some(Tool::Claude),
                tokens: tokens(23_000, 2),
            },
        ]
    );

    let epic = f
        .create_with(NewTicketInput {
            title: "Wishlist".into(),
            kind: Kind::Epic,
            ..common::shop_input()
        })
        .await;
    let project = f.board.project("SHOP").await.unwrap();
    let lead = new_id();
    sqlx::query(
        "INSERT INTO project_agents (id, project_id, tool, state, created_at, epic_id)
         VALUES (?, ?, ?, 'running', 1, ?)",
    )
    .bind(&lead)
    .bind(&project.id)
    .bind(Tool::Claude.as_str())
    .bind(&epic.id)
    .execute(pool)
    .await
    .unwrap();
    let lead_session = new_id();
    sqlx::query(
        "INSERT INTO sessions (id, project_id, kind, tool, tool_session_id, named_agent_id, named_agent_name,
                               project_agent_id, state, started_at)
         VALUES (?, ?, 'project_agent', ?, 'lead-conversation', 'strong', 'Strong', ?, 'running', 5)",
    )
    .bind(&lead_session)
    .bind(&project.id)
    .bind(Tool::Claude.as_str())
    .bind(&lead)
    .execute(pool)
    .await
    .unwrap();
    let delivery = session(&f, &epic, "delivery", None, None, 50).await;
    for (id, input) in [(&lead_session, 7_000), (&delivery, 500)] {
        sqlx::query("INSERT INTO usage (session_id, day, input, cached, output) VALUES (?, '2026-10-17', ?, 0, 0)")
            .bind(id)
            .bind(input)
            .execute(pool)
            .await
            .unwrap();
    }
    let on_epic = usage.agents_of(&epic.id).await.unwrap();
    let parts: Vec<_> = on_epic
        .iter()
        .map(|a| (a.role, a.agent.as_deref(), a.tokens.map(|t| t.input)))
        .collect();
    assert_eq!(
        parts,
        [
            (Role::Developer, None, Some(500)),
            (Role::EpicLead, Some("Strong"), Some(7_000)),
        ]
    );
}
