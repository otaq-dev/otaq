//! A ticket's agent rows as `ticket.agents` answers them (T-0341; AGT-04, AGT-05, VAL-14; d0086, d0088): each row
//! with what it resolves to and its lock, the sessions with the named agent and the exact model each ran on (the
//! columns T-0336 fills, seeded here), an epic's children, and the History event a row change writes.

use std::sync::Arc;

use otaq_core::agent_tools::Tool;
use otaq_core::board::tickets::EventKind;
use otaq_core::domain::Kind;
use otaq_core::named_agents::starter::{self, ConnectedTool};
use otaq_core::named_agents::{
    AgentFields, CameFrom, FixedModels, NamedAgent, NamedAgents, Role, RowLock, RowSetBy,
};
use serde_json::json;

use crate::common::Fixture;

async fn world() -> (Fixture, NamedAgents) {
    let f = Fixture::shop().await;
    let claude = ConnectedTool {
        tool: Tool::ALL[0].as_str().into(),
        name: "Claude Code".into(),
    };
    starter::ensure(f.board.store(), &[claude]).await.unwrap();
    let agents = NamedAgents::new(f.board.clone(), Arc::new(FixedModels::sample()));
    (f, agents)
}

async fn add(agents: &NamedAgents, name: &str, model: &str) -> NamedAgent {
    agents
        .add(&AgentFields {
            name: name.into(),
            tool: Tool::ALL[0],
            model: Some(model.into()),
            use_for: String::new(),
            fallback: None,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn the_rows_carry_their_resolution_locks_and_sessions() {
    let (f, agents) = world().await;
    let strong = add(&agents, "Strong", "opus").await;
    let light = add(&agents, "Light", "haiku").await;
    let ticket = f.create("Price table").await;
    let key = ticket.key.to_string();

    // A new ticket in the ⌘N form: both rows on the default, nothing locked.
    let new = agents
        .new_ticket_agents("SHOP", Kind::Ticket)
        .await
        .unwrap();
    assert_eq!((new.key, new.rows.len()), (None, 2));
    assert!(new.rows.iter().all(|r| r.lock.is_none() && r.row.is_none()));
    assert_eq!(new.rows[0].default.as_ref().unwrap().name, "Claude Code");
    let epic = agents.new_ticket_agents("SHOP", Kind::Epic).await.unwrap();
    assert_eq!(
        epic.rows.iter().map(|r| r.role).collect::<Vec<_>>(),
        [Role::EpicLead]
    );

    agents
        .set_row_by_user(&key, Role::Developer, Some(&strong.id))
        .await
        .unwrap();
    // Started on Strong; then a validation round on Light that runs.
    let store = f.board.store();
    let project = f.board.project("SHOP").await.unwrap();
    for (id, kind, state, started, agent, model, ran) in [
        (
            "d1",
            "agent",
            "running",
            1,
            &strong,
            "opus",
            "claude-opus-5-5",
        ),
        (
            "v1",
            "validator",
            "running",
            5,
            &light,
            "haiku",
            "claude-haiku-5-5",
        ),
    ] {
        sqlx::query(
            "INSERT INTO sessions (id, project_id, ticket_id, kind, tool, state, started_at, named_agent_id,
                                   named_agent_name, model, model_ran)
             VALUES (?, ?, ?, ?, 'claude', ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(&project.id)
        .bind(&ticket.id)
        .bind(kind)
        .bind(state)
        .bind(started)
        .bind(&agent.id)
        .bind(&agent.name)
        .bind(model)
        .bind(ran)
        .execute(store.pool())
        .await
        .unwrap();
    }
    sqlx::query("INSERT INTO usage (session_id, day, input, cached, output) VALUES ('d1', 'd', 60000, 5, 1000)")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO validations (id, ticket_id, round, trigger, session_id, started_at) VALUES ('r1', ?, 1, 'agent', 'v1', 5)",
    )
    .bind(&ticket.id)
    .execute(store.pool())
    .await
    .unwrap();

    let t = agents.ticket_agents(&key).await.unwrap();
    assert_eq!(t.key.as_ref().map(ToString::to_string), Some(key.clone()));
    let dev = &t.rows[0];
    assert_eq!(dev.lock, Some(RowLock::Started));
    assert_eq!(
        dev.resolved
            .as_ref()
            .map(|r| (r.agent.name.as_str(), &r.came_from)),
        Some((
            "Strong",
            &CameFrom::Row {
                set_by: RowSetBy::User
            }
        ))
    );
    assert_eq!(t.rows[1].lock, Some(RowLock::RoundRunning));
    assert_eq!(t.rows[1].default.as_ref().unwrap().name, "Claude Code");
    let sessions: Vec<_> = t
        .sessions
        .iter()
        .map(|s| {
            (
                s.id.as_str(),
                s.role,
                s.named_agent_name.as_deref(),
                s.model_ran.as_deref(),
                s.tokens,
            )
        })
        .collect();
    assert_eq!(
        sessions,
        [
            (
                "d1",
                Role::Developer,
                Some("Strong"),
                Some("claude-opus-5-5"),
                Some(61_000)
            ),
            (
                "v1",
                Role::Validator,
                Some("Light"),
                Some("claude-haiku-5-5"),
                None
            ),
        ]
    );
    assert!(
        t.sessions.iter().all(|s| s.moved_on.is_none()),
        "one run each"
    );
    // The round ended without a result: its outcome comes with the validator's session.
    sqlx::query("UPDATE validations SET ended_at = 6, outcome = 'no_result' WHERE id = 'r1'")
        .execute(store.pool())
        .await
        .unwrap();
    let t = agents.ticket_agents(&key).await.unwrap();
    assert_eq!(t.sessions[1].round_outcome.as_deref(), Some("no_result"));
    assert_eq!(t.rows[1].lock, None);
}

#[tokio::test]
async fn a_row_change_is_logged_for_history() {
    let (f, agents) = world().await;
    let strong = add(&agents, "Strong", "opus").await;
    let ticket = f.create("Footer").await;
    let key = ticket.key.to_string();
    agents
        .set_row_by_user(&key, Role::Developer, Some(&strong.id))
        .await
        .unwrap();
    agents
        .set_row_by_user(&key, Role::Developer, None)
        .await
        .unwrap();
    let events: Vec<_> = f
        .board
        .history(&key)
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EventKind::AgentRowSet)
        .map(|e| e.payload)
        .collect();
    // Newest first.
    assert_eq!(
        events,
        [
            json!({"role": "developer", "agent": null, "from": "Strong", "set_by": "user"}),
            json!({"role": "developer", "agent": "Strong", "from": null, "set_by": "user"}),
        ]
    );
}

#[tokio::test]
async fn an_epic_lists_its_childrens_developer_rows() {
    let (f, agents) = world().await;
    let light = add(&agents, "Light", "haiku").await;
    let epic = f.epic("Wishlist").await;
    let one = f.child(&epic, "Wishlist API").await;
    f.child(&epic, "Wishlist button").await;
    f.board
        .store()
        .set_ticket_agent_row(
            &one.id,
            Role::Developer,
            Some((
                &light.id,
                &RowSetBy::Agent {
                    picked_by: "Wishlist lead".into(),
                    reason: "small UI change".into(),
                },
            )),
        )
        .await
        .unwrap();
    let t = agents.ticket_agents(&epic.key.to_string()).await.unwrap();
    assert_eq!(
        t.rows.iter().map(|r| r.role).collect::<Vec<_>>(),
        [Role::EpicLead]
    );
    let children: Vec<_> = t
        .children
        .iter()
        .map(|c| {
            (
                c.title.as_str(),
                c.developer.as_ref().unwrap().agent.name.as_str(),
            )
        })
        .collect();
    assert_eq!(
        children,
        [
            ("Wishlist API", "Light"),
            ("Wishlist button", "Claude Code")
        ]
    );
}

/// `board.agents` (T-0345): a Developer row off the project default is named with who set it, one on the default
/// isn't, and the running sessions' named agents say who has the turn.
#[tokio::test]
async fn the_board_names_rows_off_the_default_and_who_runs() {
    let (f, agents) = world().await;
    let strong = add(&agents, "Strong", "opus").await;
    let light = add(&agents, "Light", "haiku").await;
    let fable = add(&agents, "Fable test", "fable").await;
    agents
        .set_project_default("SHOP", Role::Developer, Some(&strong.id))
        .await
        .unwrap();
    let mine = f.create("Add gift wrap option").await;
    let picked = f.create("Fix README typo").await;
    let same = f.create("Price table").await;
    let plain = f.create("Update footer links").await;
    agents
        .set_row_by_user(&mine.key.to_string(), Role::Developer, Some(&fable.id))
        .await
        .unwrap();
    agents
        .set_row_by_user(&same.key.to_string(), Role::Developer, Some(&strong.id))
        .await
        .unwrap();
    let store = f.board.store();
    let by = RowSetBy::Agent {
        picked_by: "Checkout slowness".into(),
        reason: "docs-only change".into(),
    };
    store
        .set_ticket_agent_row(&picked.id, Role::Developer, Some((&light.id, &by)))
        .await
        .unwrap();
    let project = f.board.project("SHOP").await.unwrap();
    for (id, kind, state, agent) in [
        ("v1", "validator", "running", &light),
        ("d0", "agent", "ended", &fable),
    ] {
        sqlx::query(
            "INSERT INTO sessions (id, project_id, ticket_id, kind, tool, state, started_at, named_agent_id,
                                   named_agent_name)
             VALUES (?, ?, ?, ?, 'claude', ?, 1, ?, ?)",
        )
        .bind(id)
        .bind(&project.id)
        .bind(&plain.id)
        .bind(kind)
        .bind(state)
        .bind(&agent.id)
        .bind(&agent.name)
        .execute(store.pool())
        .await
        .unwrap();
    }

    let board = agents.board_agents("SHOP").await.unwrap();
    assert_eq!(board.agent_count, 4);
    let card = |key: &otaq_core::domain::TicketKey| {
        board.cards.iter().find(|c| &c.key == key).unwrap().clone()
    };
    let m = card(&mine.key);
    assert!(m.developer_off_default);
    assert_eq!(m.developer.as_ref().unwrap().agent.name, "Fable test");
    let p = card(&picked.key);
    assert!(p.developer_off_default);
    assert_eq!(
        p.developer.unwrap().came_from,
        CameFrom::Row { set_by: by.clone() }
    );
    assert!(
        !card(&same.key).developer_off_default,
        "set by you, but on the project default"
    );
    let plain = card(&plain.key);
    assert!(!plain.developer_off_default);
    assert_eq!(plain.developer.unwrap().agent.name, "Strong");
    assert_eq!(plain.validator.unwrap().agent.name, "Claude Code");
    assert_eq!(
        (
            plain.running_developer.as_deref(),
            plain.running_validator.as_deref()
        ),
        (None, Some("Light")),
        "only running sessions"
    );
}

/// M23 (T-0363): the rows carry their move and the ticket its holds; each session names its tool.
#[tokio::test]
async fn the_rows_carry_their_move_the_holds_and_each_sessions_tool() {
    use otaq_core::named_agents::{AgentHold, HoldReason, MoveReason, RowMove};
    let (f, agents) = world().await;
    let strong = add(&agents, "Strong", "opus").await;
    let light = add(&agents, "Light", "haiku").await;
    let ticket = f.create("Checkout flow").await;
    let key = ticket.key.to_string();
    let store = f.board.store();
    let moved = RowMove {
        from_id: Some(strong.id.clone()),
        from_name: "Strong".into(),
        reason: MoveReason::ToolLimit,
        tool: Some(Tool::ALL[0]),
        at: 7,
    };
    store
        .move_ticket_agent_row(
            &ticket.id,
            Role::Developer,
            &light.id,
            &RowSetBy::User,
            &moved,
        )
        .await
        .unwrap();
    let hold = AgentHold {
        id: "h1".into(),
        ticket_id: Some(ticket.id.clone()),
        project_id: None,
        role: Role::Validator,
        reason: HoldReason::ToolUnavailable,
        tool: Tool::ALL[1],
        model: None,
        named_agent_id: Some(light.id.clone()),
        fallback_agent_id: None,
        fallback_held: false,
        since: 3,
    };
    store.set_agent_hold(&hold).await.unwrap();
    let project = f.board.project("SHOP").await.unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, project_id, ticket_id, kind, tool, state, started_at, named_agent_id,
                               named_agent_name)
         VALUES ('d1', ?, ?, 'agent', ?, 'ended', 1, ?, 'Light')",
    )
    .bind(&project.id)
    .bind(&ticket.id)
    .bind(Tool::ALL[1].as_str())
    .bind(&light.id)
    .execute(store.pool())
    .await
    .unwrap();

    let t = agents.ticket_agents(&key).await.unwrap();
    assert_eq!(t.rows[0].moved.as_ref(), Some(&moved));
    assert_eq!(t.rows[1].moved, None);
    assert_eq!(t.holds, [hold]);
    assert_eq!(t.sessions[0].tool, Some(Tool::ALL[1]));
    // A later change of the row clears the move.
    store
        .set_ticket_agent_row(
            &ticket.id,
            Role::Developer,
            Some((&strong.id, &RowSetBy::User)),
        )
        .await
        .unwrap();
    assert_eq!(
        agents.ticket_agents(&key).await.unwrap().rows[0].moved,
        None
    );
}
