//! Named agents (T-0334; AGT-01…AGT-05, ADP-04; d0086, d0088): the starter entry and the role defaults, resolution
//! (row → project default → app default), the rules of add, edit and delete, the agent rows and their locks, and
//! the requests over the socket (the user's only).

use std::sync::Arc;

use otaq_core::agent_tools::Tool;
use otaq_core::agent_tools::models::{ModelCopy, ModelEntry, ModelKind, ModelList, settings_key};
use otaq_core::board::Refusal;
use otaq_core::board::tickets::Ticket;
use otaq_core::client::{Client, ClientConfig, ClientError};
use otaq_core::data_dir::DataDir;
use otaq_core::domain::Status;
use otaq_core::git::testing::TempRepos;
use otaq_core::named_agents::starter::{self, ConnectedTool, STARTER_USE_FOR};
use otaq_core::named_agents::{
    AgentFields, AgentModel, CameFrom, DeleteOutcome, FixedModels, NamedAgent, NamedAgents, Role,
    RoleDefaults, RowSetBy,
};
use otaq_core::protocol::{ErrorKind, Event, Reply, Request};
use otaq_core::service::testing::TestService;
use otaq_core::store::Store;
use serde_json::{Value, json};

use crate::common::Fixture;

fn claude_code() -> ConnectedTool {
    ConnectedTool {
        tool: Tool::ALL[0].as_str().into(),
        name: "Claude Code".into(),
    }
}

fn test_tool(tool: &str, name: &str) -> ConnectedTool {
    ConnectedTool {
        tool: tool.into(),
        name: name.into(),
    }
}

/// The shop fixture with the starter entry ("Claude Code", every default on it) and the rules over it.
async fn world() -> (Fixture, NamedAgents) {
    let f = Fixture::shop().await;
    assert!(
        starter::ensure(f.board.store(), &[claude_code()])
            .await
            .unwrap()
    );
    let agents = NamedAgents::new(f.board.clone(), Arc::new(FixedModels::sample()));
    (f, agents)
}

fn fields(name: &str, model: Option<&str>) -> AgentFields {
    AgentFields {
        name: name.into(),
        tool: Tool::ALL[0],
        model: model.map(str::to_owned),
        use_for: String::new(),
    }
}

async fn add(agents: &NamedAgents, name: &str, model: Option<&str>) -> NamedAgent {
    agents.add(&fields(name, model)).await.unwrap()
}

async fn starter_entry(agents: &NamedAgents) -> NamedAgent {
    agents.list().await.unwrap().agents.remove(0)
}

fn refusal(
    result: Result<impl std::fmt::Debug, otaq_core::board::BoardError>,
) -> (Refusal, String) {
    let e = result.expect_err("refused");
    (e.refusal().expect("a refusal"), e.to_string())
}

async fn raw_tools(store: &Store) -> Vec<(String, String)> {
    store
        .named_agent_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|(_, name, tool)| (name, tool))
        .collect()
}

#[tokio::test]
async fn the_starter_entry_is_made_once_with_every_default_on_it() {
    let f = Fixture::shop().await;
    let store = f.board.store();
    assert!(
        !starter::ensure(store, &[]).await.unwrap(),
        "no tool connected"
    );
    assert_eq!(store.named_agent_count().await.unwrap(), 0);

    assert!(starter::ensure(store, &[claude_code()]).await.unwrap());
    let agents = store.named_agents().await.unwrap();
    assert_eq!(agents.len(), 1);
    let cc = &agents[0];
    assert_eq!(cc.name, "Claude Code");
    assert_eq!(cc.tool, Tool::ALL[0]);
    assert_eq!(cc.model, None, "tool default: no model option");
    assert_eq!(cc.use_for, STARTER_USE_FOR);
    assert_eq!(cc.use_for, "Everything, until you add more agents");
    assert_eq!(cc.mark, None);
    assert_eq!(
        store.agent_defaults().await.unwrap(),
        RoleDefaults::all_on(&cc.id)
    );

    // Not made again while one exists, nor after deleting down to one.
    assert!(!starter::ensure(store, &[claude_code()]).await.unwrap());
    let rules = NamedAgents::new(f.board.clone(), Arc::new(FixedModels::sample()));
    let strong = add(&rules, "Strong", Some("opus")).await;
    assert_eq!(
        rules.delete(&strong.id).await.unwrap(),
        DeleteOutcome::Deleted
    );
    assert!(!starter::ensure(store, &[claude_code()]).await.unwrap());
    assert_eq!(store.named_agent_count().await.unwrap(), 1);
}

#[tokio::test]
async fn one_entry_per_connected_tool_and_a_tool_connected_later_changes_nothing_else() {
    let f = Fixture::shop().await;
    let store = f.board.store();
    let (a, b) = (test_tool("tool-a", "Tool A"), test_tool("tool-b", "Tool B"));
    assert!(
        starter::ensure(store, &[a.clone(), b.clone()])
            .await
            .unwrap()
    );
    assert_eq!(
        raw_tools(store).await,
        [
            ("Tool A".into(), "tool-a".into()),
            ("Tool B".into(), "tool-b".into())
        ]
    );
    let first = store.named_agent_tools().await.unwrap()[0].0.clone();
    let defaults = store.agent_defaults().await.unwrap();
    assert_eq!(
        defaults,
        RoleDefaults::all_on(&first),
        "all four on the first"
    );

    // A tool with an entry gets none; a new one gets its own; no default moves.
    assert_eq!(
        starter::add_for_connected_tool(store, &a).await.unwrap(),
        None
    );
    let c = test_tool("tool-c", "Tool B");
    let added = starter::add_for_connected_tool(store, &c).await.unwrap();
    assert!(added.is_some());
    assert_eq!(
        raw_tools(store).await.last().unwrap(),
        &("Tool B 2".to_owned(), "tool-c".to_owned()),
        "the tool's name is taken: the next free number"
    );
    assert_eq!(store.agent_defaults().await.unwrap(), defaults);
    assert_eq!(
        starter::add_for_connected_tool(store, &c).await.unwrap(),
        None
    );
    assert_eq!(store.named_agent_count().await.unwrap(), 3);
}

#[tokio::test]
async fn each_role_resolves_row_then_project_default_then_app_default() {
    let (f, agents) = world().await;
    let cc = starter_entry(&agents).await;
    let strong = add(&agents, "Strong", Some("opus")).await;
    let light = add(&agents, "Light", Some("haiku")).await;
    let ticket = f.create("Price table").await;
    let epic = f.epic("Wishlist").await;
    let key = ticket.key.to_string();
    let epic_key = epic.key.to_string();

    let on = |resolved: Option<otaq_core::named_agents::Resolved>| {
        let r = resolved.expect("resolved");
        (r.agent.name, r.came_from)
    };
    // The app default for every role.
    for (k, role) in [
        (&key, Role::Developer),
        (&key, Role::Validator),
        (&epic_key, Role::EpicLead),
    ] {
        assert_eq!(
            on(agents.resolve_for_ticket(k, role).await.unwrap()),
            (cc.name.clone(), CameFrom::AppDefault)
        );
    }
    assert_eq!(
        on(agents
            .resolve_for_project("SHOP", Role::ProjectAgent)
            .await
            .unwrap()),
        (cc.name.clone(), CameFrom::AppDefault)
    );

    // The project's overrides.
    for role in Role::ALL.iter().copied() {
        agents
            .set_project_default("SHOP", role, Some(&strong.id))
            .await
            .unwrap();
    }
    assert_eq!(
        on(agents
            .resolve_for_ticket(&key, Role::Validator)
            .await
            .unwrap()),
        ("Strong".into(), CameFrom::ProjectDefault)
    );
    assert_eq!(
        on(agents
            .resolve_for_ticket(&epic_key, Role::EpicLead)
            .await
            .unwrap()),
        ("Strong".into(), CameFrom::ProjectDefault)
    );
    assert_eq!(
        on(agents
            .resolve_for_project("SHOP", Role::ProjectAgent)
            .await
            .unwrap()),
        ("Strong".into(), CameFrom::ProjectDefault)
    );

    // The rows.
    agents
        .set_row_by_user(&key, Role::Developer, Some(&light.id))
        .await
        .unwrap();
    agents
        .set_row_by_user(&epic_key, Role::EpicLead, Some(&light.id))
        .await
        .unwrap();
    let by_you = (
        "Light".to_owned(),
        CameFrom::Row {
            set_by: RowSetBy::User,
        },
    );
    assert_eq!(
        on(agents
            .resolve_for_ticket(&key, Role::Developer)
            .await
            .unwrap()),
        by_you
    );
    assert_eq!(
        on(agents
            .resolve_for_ticket(&epic_key, Role::EpicLead)
            .await
            .unwrap()),
        by_you
    );
    assert_eq!(
        on(agents
            .resolve_for_ticket(&key, Role::Validator)
            .await
            .unwrap()),
        ("Strong".into(), CameFrom::ProjectDefault),
        "the Validator row is still empty"
    );

    // An agent's pick reads "picked by".
    let pick = RowSetBy::Agent {
        picked_by: "Checkout slowness".into(),
        reason: "docs-only change".into(),
    };
    f.board
        .store()
        .set_ticket_agent_row(&ticket.id, Role::Validator, Some((&light.id, &pick)))
        .await
        .unwrap();
    assert_eq!(
        on(agents
            .resolve_for_ticket(&key, Role::Validator)
            .await
            .unwrap()),
        ("Light".into(), CameFrom::Row { set_by: pick })
    );

    // "Use app default" again.
    agents
        .set_project_default("SHOP", Role::Validator, None)
        .await
        .unwrap();
    let other = f.create("Coupon field").await;
    assert_eq!(
        on(agents
            .resolve_for_ticket(&other.key.to_string(), Role::Validator)
            .await
            .unwrap()),
        (cc.name.clone(), CameFrom::AppDefault)
    );
}

#[tokio::test]
async fn names_are_unique_ignoring_case_and_models_come_from_the_list() {
    let (_f, agents) = world().await;
    let strong = add(&agents, "Strong", Some("opus")).await;
    assert_eq!(strong.model.as_deref(), Some("opus"));
    assert_eq!(
        strong.model_kind(agents.models()),
        AgentModel::Follow("opus".into())
    );

    for taken in ["strong", " STRONG ", "claude code"] {
        let (code, message) = refusal(agents.add(&fields(taken, None)).await);
        assert_eq!(code, Refusal::NameTaken, "{taken}");
        assert_eq!(
            message,
            format!("Another agent is already called {}", taken.trim())
        );
    }
    add(&agents, "Éclair", None).await;
    let (code, _) = refusal(agents.add(&fields("éclair", None)).await);
    assert_eq!(code, Refusal::NameTaken, "beyond ASCII too");
    let (code, _) = refusal(agents.add(&fields("  ", None)).await);
    assert_eq!(code, Refusal::EmptyField);

    // Only what the tool's list has; never typed text.
    let (code, message) = refusal(agents.add(&fields("Typed", Some("claude-opus-9"))).await);
    assert_eq!(code, Refusal::ModelNotListed);
    assert_eq!(message, "claude-opus-9 isn't in the tool's model list");
    let (code, _) = refusal(
        agents
            .edit(&strong.id, &fields("Strong", Some("gpt-5")))
            .await,
    );
    assert_eq!(code, Refusal::ModelNotListed);
    // The list's "default" is the tool default: nothing is passed.
    assert_eq!(add(&agents, "Plain", Some("default")).await.model, None);
    assert_eq!(add(&agents, "Plain 2", Some("")).await.model, None);

    // A rename of the entry itself, in another case, is fine.
    let renamed = agents
        .edit(&strong.id, &fields("STRONG", Some("opus")))
        .await
        .unwrap();
    assert_eq!(renamed.name, "STRONG");
    assert_eq!(renamed.id, strong.id, "the id is stable");
}

#[tokio::test]
async fn a_model_change_clears_the_mark_and_a_rename_does_not() {
    let (f, agents) = world().await;
    let fable = add(&agents, "Fable test", Some("fable")).await;
    let mark = async || {
        sqlx::query(
            "UPDATE named_agents SET mark_text = ?, mark_model = 'claude-fable-5-1', mark_ticket = 'SHOP-48',
             marked_at = 1 WHERE id = ?",
        )
        .bind("Claude Code couldn't run `claude-fable-5-1` on SHOP-48")
        .bind(&fable.id)
        .execute(f.board.store().pool())
        .await
        .unwrap();
    };
    mark().await;
    let listed = agents.agent(&fable.id).await.unwrap();
    let shown = listed.mark.expect("marked");
    assert_eq!(
        shown.text,
        "Claude Code couldn't run `claude-fable-5-1` on SHOP-48"
    );
    assert_eq!(shown.ticket.as_deref(), Some("SHOP-48"));

    let mut renamed = fields("Fable trial", Some("fable"));
    renamed.use_for = "Trying Fable on a few tickets".into();
    let after = agents.edit(&fable.id, &renamed).await.unwrap();
    assert!(
        after.mark.is_some(),
        "a rename and a new note keep the mark"
    );
    assert_eq!(after.use_for, "Trying Fable on a few tickets");

    let after = agents
        .edit(&fable.id, &fields("Fable trial", Some("sonnet")))
        .await
        .unwrap();
    assert_eq!(after.mark, None, "a saved model change clears it");
    mark().await;
    let after = agents
        .edit(&fable.id, &fields("Fable trial", None))
        .await
        .unwrap();
    assert_eq!(after.mark, None, "so does going back to the tool default");
}

#[tokio::test]
async fn duplicate_is_a_draft_named_copy() {
    let (_f, agents) = world().await;
    let mut strong = fields("Strong", Some("opus"));
    strong.use_for = "Complex features, tricky bugs".into();
    let strong = agents.add(&strong).await.unwrap();
    let draft = agents.duplicate(&strong.id).await.unwrap();
    assert_eq!(
        draft,
        AgentFields {
            name: "Strong copy".into(),
            tool: strong.tool,
            model: Some("opus".into()),
            use_for: "Complex features, tricky bugs".into(),
        }
    );
    assert_eq!(agents.list().await.unwrap().agents.len(), 2, "not saved");
}

#[tokio::test]
async fn delete_is_refused_while_in_use_and_names_what_uses_it() {
    let (f, agents) = world().await;
    f.add_project("docs-site").await;
    let light = add(&agents, "Light", Some("haiku")).await;
    let mut tickets: Vec<Ticket> = Vec::new();
    for n in 0..12 {
        let t = f.create(&format!("Small change {n}")).await;
        agents
            .set_row_by_user(&t.key.to_string(), Role::Developer, Some(&light.id))
            .await
            .unwrap();
        tickets.push(t);
    }
    agents
        .set_default(Role::Validator, &light.id)
        .await
        .unwrap();
    agents
        .set_project_default("DOCS", Role::Developer, Some(&light.id))
        .await
        .unwrap();

    let refused = |outcome: DeleteOutcome| match outcome {
        DeleteOutcome::InUse(uses) => uses,
        DeleteOutcome::Deleted => panic!("deleted"),
    };
    let uses = refused(agents.delete(&light.id).await.unwrap());
    assert_eq!(
        uses.message(),
        "Light is used by 12 tickets, the Validator default and project docs-site"
    );
    assert_eq!(uses.tickets.len(), 12);
    assert_eq!(uses.tickets[0], tickets[0].key);
    assert_eq!(uses.defaults, [Role::Validator]);
    assert_eq!(uses.projects.len(), 1);
    assert_eq!(uses.projects[0].key, "DOCS");
    assert_eq!(uses.projects[0].roles, [Role::Developer]);

    // Done, Closed and Approved tickets don't count; Ready for Review does.
    f.force_status(&tickets[0], Status::Merged).await;
    f.force_status(&tickets[1], Status::ClosedNotMerged).await;
    f.force_status(&tickets[2], Status::PrOpen).await;
    f.force_status(&tickets[3], Status::ReadyForReview).await;
    f.force_status(&tickets[4], Status::Working).await;
    f.force_status(&tickets[5], Status::Queued).await;
    let uses = refused(agents.delete(&light.id).await.unwrap());
    assert_eq!(uses.tickets.len(), 9);
    assert!(!uses.tickets.contains(&tickets[0].key));
    assert!(!uses.tickets.contains(&tickets[2].key));
    assert!(uses.tickets.contains(&tickets[3].key));

    // Free it: only the defaults and the project remain, then nothing.
    for t in &tickets[3..] {
        agents
            .set_row_by_user(&t.key.to_string(), Role::Developer, None)
            .await
            .unwrap();
    }
    assert_eq!(
        refused(agents.delete(&light.id).await.unwrap()).message(),
        "Light is used by the Validator default and project docs-site"
    );
    let cc = starter_entry(&agents).await;
    agents.set_default(Role::Validator, &cc.id).await.unwrap();
    agents
        .set_project_default("DOCS", Role::Developer, None)
        .await
        .unwrap();

    // The Done ticket's session ran on Light: it keeps the name after the delete.
    let store = f.board.store();
    let project = f.board.project("SHOP").await.unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, project_id, ticket_id, kind, tool, state, started_at, ended_at, named_agent_id,
                               named_agent_name, model, model_ran)
         VALUES ('s1', ?, ?, 'agent', 'claude', 'ended', 1, 2, ?, 'Light', 'haiku', 'claude-haiku-5-5')",
    )
    .bind(&project.id)
    .bind(&tickets[0].id)
    .bind(&light.id)
    .execute(store.pool())
    .await
    .unwrap();
    assert_eq!(
        agents.delete(&light.id).await.unwrap(),
        DeleteOutcome::Deleted
    );
    let rows = store.ticket_agent_rows(&tickets[0].id).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].agent, None, "the Done ticket's row is empty");
    let kept: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT named_agent_id, named_agent_name, model_ran FROM sessions WHERE id = 's1'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        kept,
        (
            Some(light.id.clone()),
            Some("Light".into()),
            Some("claude-haiku-5-5".into())
        )
    );

    // The last entry can't go: every role needs a default.
    let (code, message) = refusal(agents.delete(&cc.id).await);
    assert_eq!(code, Refusal::LastAgent);
    assert_eq!(message, "Otaq needs at least one agent");
}

#[tokio::test]
async fn being_a_fallback_counts_as_in_use() {
    let (f, agents) = world().await;
    let strong = add(&agents, "Strong", Some("opus")).await;
    let sonnet = add(&agents, "Sonnet", Some("sonnet")).await;
    sqlx::query("UPDATE named_agents SET fallback_id = ? WHERE id = ?")
        .bind(&sonnet.id)
        .bind(&strong.id)
        .execute(f.board.store().pool())
        .await
        .unwrap();
    match agents.delete(&sonnet.id).await.unwrap() {
        DeleteOutcome::InUse(uses) => {
            assert_eq!(uses.message(), "Sonnet is used by the fallback of Strong")
        }
        DeleteOutcome::Deleted => panic!("deleted"),
    }
}

#[tokio::test]
async fn rows_are_set_by_you_emptied_and_refused_where_they_do_not_exist() {
    let (f, agents) = world().await;
    let light = add(&agents, "Light", Some("haiku")).await;
    let ticket = f.create("Fix README typo").await;
    let epic = f.epic("Wishlist").await;
    let key = ticket.key.to_string();
    let store = f.board.store();

    agents
        .set_row_by_user(&key, Role::Developer, Some(&light.id))
        .await
        .unwrap();
    let rows = store.ticket_agent_rows(&ticket.id).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].role, Role::Developer);
    assert_eq!(rows[0].agent.as_deref(), Some(light.id.as_str()));
    assert_eq!(rows[0].set_by, RowSetBy::User);

    // "Let agents pick again": the row is empty (the project default).
    agents
        .set_row_by_user(&key, Role::Developer, None)
        .await
        .unwrap();
    assert!(
        store
            .ticket_agent_rows(&ticket.id)
            .await
            .unwrap()
            .is_empty()
    );

    let (code, message) = refusal(
        agents
            .set_row_by_user(&key, Role::EpicLead, Some(&light.id))
            .await,
    );
    assert_eq!(code, Refusal::NoSuchRow);
    assert_eq!(
        message,
        format!("{key} has no Epic lead row: tickets have Developer and Validator rows")
    );
    let epic_key = epic.key.to_string();
    let (code, message) = refusal(
        agents
            .set_row_by_user(&epic_key, Role::Developer, Some(&light.id))
            .await,
    );
    assert_eq!(code, Refusal::NoSuchRow);
    assert_eq!(
        message,
        format!("{epic_key} is an epic: it has an Epic lead row, not a Developer row")
    );
    let (code, _) = refusal(
        agents
            .set_row_by_user(&key, Role::ProjectAgent, Some(&light.id))
            .await,
    );
    assert_eq!(code, Refusal::NoSuchRow);
    assert!(
        agents
            .set_row_by_user(&key, Role::Developer, Some("nope"))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn locked_rows_are_refused_with_their_reason() {
    let (f, agents) = world().await;
    let light = add(&agents, "Light", Some("haiku")).await;
    let ticket = f.create("Price table").await;
    let epic = f.epic("Wishlist").await;
    let (key, epic_key) = (ticket.key.to_string(), epic.key.to_string());
    let project = f.board.project("SHOP").await.unwrap();
    let pool = f.board.store().pool();

    // A validation round runs (its validator started): the Validator locks, the Developer doesn't yet.
    sqlx::query(
        "INSERT INTO validations (id, ticket_id, round, trigger, session_id, started_at)
         VALUES ('v1', ?, 1, 'agent', 'validator-1', 1)",
    )
    .bind(&ticket.id)
    .execute(pool)
    .await
    .unwrap();
    let (code, message) = refusal(
        agents
            .set_row_by_user(&key, Role::Validator, Some(&light.id))
            .await,
    );
    assert_eq!(code, Refusal::RowLocked);
    assert_eq!(
        message,
        format!("{key}'s Validator is locked until this round ends")
    );
    // The round ended: it can change again, for the next round.
    sqlx::query("UPDATE validations SET ended_at = 2, outcome = 'fail' WHERE id = 'v1'")
        .execute(pool)
        .await
        .unwrap();
    agents
        .set_row_by_user(&key, Role::Validator, Some(&light.id))
        .await
        .unwrap();

    // The ticket started: the Developer locks.
    agents
        .set_row_by_user(&key, Role::Developer, Some(&light.id))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, project_id, ticket_id, kind, tool, state, started_at)
         VALUES ('s1', ?, ?, 'agent', 'claude', 'running', 1)",
    )
    .bind(&project.id)
    .bind(&ticket.id)
    .execute(pool)
    .await
    .unwrap();
    for agent in [None, Some(light.id.as_str())] {
        let (code, message) = refusal(agents.set_row_by_user(&key, Role::Developer, agent).await);
        assert_eq!(code, Refusal::RowLocked);
        assert_eq!(
            message,
            format!("{key}'s Developer is locked: Send back continues the same conversation")
        );
    }

    // The lead started: the Epic lead locks.
    agents
        .set_row_by_user(&epic_key, Role::EpicLead, Some(&light.id))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO project_agents (id, project_id, tool, state, created_at, epic_id)
         VALUES ('lead1', ?, 'claude', 'running', 1, ?)",
    )
    .bind(&project.id)
    .bind(&epic.id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, project_id, kind, tool, project_agent_id, state, started_at)
         VALUES ('lead-s1', ?, 'project_agent', 'claude', 'lead1', 'running', 1)",
    )
    .bind(&project.id)
    .execute(pool)
    .await
    .unwrap();
    let (code, message) = refusal(
        agents
            .set_row_by_user(&epic_key, Role::EpicLead, None)
            .await,
    );
    assert_eq!(code, Refusal::RowLocked);
    assert_eq!(
        message,
        format!("{epic_key}'s Epic lead is locked: Send back continues the same conversation")
    );
}

#[tokio::test]
async fn a_project_settings_save_keeps_the_agent_overrides() {
    let (f, agents) = world().await;
    let strong = add(&agents, "Strong", Some("opus")).await;
    agents
        .set_project_default("SHOP", Role::Developer, Some(&strong.id))
        .await
        .unwrap();
    let mut settings = f.board.project_settings("SHOP").await.unwrap();
    settings.run_command = "npm run dev".into();
    f.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    let project = f.board.project("SHOP").await.unwrap();
    assert_eq!(
        project.agent_defaults().developer.as_deref(),
        Some(strong.id.as_str())
    );
    assert_eq!(
        f.board.project_settings("SHOP").await.unwrap().run_command,
        "npm run dev"
    );
}

/// A service whose copy of the tool's model list was read earlier (tests never read a tool): `default` and the four
/// aliases, as Claude Code's `initialize` answers.
async fn with_model_list() -> TestService {
    let home = tempfile::TempDir::new().unwrap();
    let store = Store::open_in(&DataDir::new(home.path().join("h")))
        .await
        .unwrap();
    let tool = Tool::ALL[0];
    let entry = |id: &str, kind| ModelEntry {
        id: id.into(),
        display_name: id.into(),
        description: None,
        resolves_to: None,
        kind,
    };
    let mut entries = vec![entry("default", ModelKind::ToolDefault)];
    entries.extend(["opus", "sonnet", "haiku", "fable"].map(|id| entry(id, ModelKind::Follow)));
    let copy = ModelCopy {
        tool,
        list: ModelList { entries },
        read_at: 1,
    };
    store
        .set_setting(&settings_key(tool), &serde_json::to_string(&copy).unwrap())
        .await
        .unwrap();
    store.close().await;
    TestService::start_in(home, TempRepos::new(), |config| config).await
}

/// One request over the socket, as the app sends it.
async fn ask(client: &Client, method: &str, params: Value) -> Result<Reply, ClientError> {
    let request: Request = serde_json::from_value(json!({"method": method, "params": params}))
        .unwrap_or_else(|e| panic!("{method}: {e}"));
    client.request(request).await
}

#[tokio::test(flavor = "multi_thread")]
async fn the_requests_are_the_users_and_a_refused_delete_carries_its_parts() {
    let w = with_model_list().await;
    w.add_project("shop-web").await;
    let ticket = w.create("SHOP", "Price table").await;
    let key = ticket.key.to_string();
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let mut events = client.subscribe().await.unwrap();

    // No tool is detected in tests: the list starts empty, and the user adds entries.
    let Reply::Agents(list) = client.request(Request::AgentsList).await.unwrap() else {
        panic!("agents.list")
    };
    assert!(list.agents.is_empty());
    let tool = Tool::ALL[0].as_str();
    let Reply::Agent(strong) = ask(
        &client,
        "agents.add",
        json!({"name": "Strong", "tool": tool, "model": "opus", "use_for": "Complex features, tricky bugs"}),
    )
    .await
    .unwrap() else {
        panic!("agents.add")
    };
    assert_eq!(strong.model.as_deref(), Some("opus"));
    let Err(ClientError::Service(e)) = ask(
        &client,
        "agents.add",
        json!({"name": "Typed", "tool": tool, "model": "claude-opus-9"}),
    )
    .await
    else {
        panic!("a model outside the list was saved")
    };
    assert_eq!(e.data.unwrap()["refusal"], "model_not_listed");
    let changed = async { while events.recv().await.unwrap() != Event::AgentsChanged {} };
    tokio::time::timeout(std::time::Duration::from_secs(5), changed)
        .await
        .expect("agents_changed");
    let Reply::Agent(light) = ask(
        &client,
        "agents.add",
        json!({"name": "Light", "tool": tool, "model": "haiku"}),
    )
    .await
    .unwrap() else {
        panic!("agents.add")
    };
    let Reply::AgentDraft(draft) = ask(&client, "agents.duplicate", json!({"id": strong.id}))
        .await
        .unwrap()
    else {
        panic!("agents.duplicate")
    };
    assert_eq!(draft.name, "Strong copy");
    let Reply::Agent(edited) = ask(
        &client,
        "agents.edit",
        json!({"id": light.id, "name": "Light", "tool": tool, "model": "sonnet", "use_for": "Typos"}),
    )
    .await
    .unwrap() else {
        panic!("agents.edit")
    };
    assert_eq!(edited.model.as_deref(), Some("sonnet"));
    let Reply::AgentDefaults(defaults) = ask(
        &client,
        "agents.set_default",
        json!({"role": "validator", "agent": light.id}),
    )
    .await
    .unwrap() else {
        panic!("agents.set_default")
    };
    assert_eq!(defaults.validator.as_deref(), Some(light.id.as_str()));
    let Reply::AgentDefaults(project) = ask(
        &client,
        "project.set_agent_default",
        json!({"project": "SHOP", "role": "developer", "agent": strong.id}),
    )
    .await
    .unwrap() else {
        panic!("project.set_agent_default")
    };
    assert_eq!(project.developer.as_deref(), Some(strong.id.as_str()));
    assert_eq!(project.validator, None, "the others use the app default");
    // Read back for the project's Agents block (T-0340).
    let Reply::AgentDefaults(read) = ask(
        &client,
        "project.agent_defaults",
        json!({"project": "SHOP"}),
    )
    .await
    .unwrap() else {
        panic!("project.agent_defaults")
    };
    assert_eq!(read, project);

    // The user's row.
    let row = ask(
        &client,
        "ticket.agent_row",
        json!({"key": key, "role": "validator", "agent": light.id}),
    )
    .await
    .unwrap();
    assert_eq!(row, Reply::Ok);
    let rows = w.board.store().ticket_agent_rows(&ticket.id).await.unwrap();
    assert_eq!(rows[0].set_by, RowSetBy::User);

    // A refused delete: the sentence, and its parts for the dialog.
    let Err(ClientError::Service(e)) = ask(&client, "agents.delete", json!({"id": light.id})).await
    else {
        panic!("agents.delete was not refused")
    };
    assert_eq!(e.kind, ErrorKind::Refused);
    assert_eq!(
        e.message,
        "Light is used by 1 ticket and the Validator default"
    );
    let data = e.data.unwrap();
    assert_eq!(data["refusal"], "agent_in_use");
    assert_eq!(data["in_use"]["tickets"], json!([key]));
    assert_eq!(data["in_use"]["defaults"], json!(["validator"]));
    // The same parts before Delete asks (T-0340).
    let Reply::AgentInUse(uses) = ask(&client, "agents.in_use", json!({"id": light.id}))
        .await
        .unwrap()
    else {
        panic!("agents.in_use")
    };
    assert_eq!(uses.message(), e.message);
    let Reply::AgentInUse(strong_uses) = ask(&client, "agents.in_use", json!({"id": strong.id}))
        .await
        .unwrap()
    else {
        panic!("agents.in_use")
    };
    assert_eq!(strong_uses.message(), "Strong is used by project shop-web");
    let row = ask(
        &client,
        "ticket.agent_row",
        json!({"key": key, "role": "validator", "agent": null}),
    )
    .await
    .unwrap();
    assert_eq!(row, Reply::Ok);
    assert!(
        w.board
            .store()
            .ticket_agent_rows(&ticket.id)
            .await
            .unwrap()
            .is_empty()
    );

    // An agent may not call any of them (T-0338 is its way in).
    let agent = w.agent_shell(&ticket).await;
    for (method, params) in [
        ("agents.list", Value::Null),
        ("agents.add", json!({"name": "Mine", "tool": tool})),
        ("agents.delete", json!({"id": strong.id})),
        ("agents.in_use", json!({"id": strong.id})),
        ("project.agent_defaults", json!({"project": "SHOP"})),
        (
            "agents.set_default",
            json!({"role": "developer", "agent": light.id}),
        ),
        (
            "project.set_agent_default",
            json!({"project": "SHOP", "role": "developer", "agent": null}),
        ),
        (
            "ticket.agent_row",
            json!({"key": key, "role": "validator", "agent": light.id}),
        ),
    ] {
        let mut request = json!({"method": method});
        if !params.is_null() {
            request["params"] = params;
        }
        let line = json!({"protocol_version": 1, "id": 1, "request": request}).to_string();
        let ran = agent.send_raw(&w.data.socket_path(), &line).await;
        let answer: Value = serde_json::from_str(ran.stdout.lines().next().unwrap_or_default())
            .unwrap_or_else(|e| panic!("{e}: {ran:?}"));
        assert_eq!(
            answer["error"]["data"]["refusal"], "not_allowed",
            "{method}: {answer}"
        );
    }
    let expected = format!("{key}'s agent can't set agent rows; ask the user");
    let line = json!({"protocol_version": 1, "id": 1, "request": {"method": "ticket.agent_row",
        "params": {"key": key, "role": "validator", "agent": null}}})
    .to_string();
    let ran = agent.send_raw(&w.data.socket_path(), &line).await;
    let answer: Value = serde_json::from_str(ran.stdout.lines().next().unwrap()).unwrap();
    assert_eq!(answer["error"]["message"], expected);
    w.stop().await;
}

#[tokio::test]
async fn a_deleted_ticket_takes_its_rows_with_it() {
    let (f, agents) = world().await;
    let light = add(&agents, "Light", Some("haiku")).await;
    let ticket = f.create("Update footer links").await;
    let key = ticket.key.to_string();
    for role in [Role::Developer, Role::Validator] {
        agents
            .set_row_by_user(&key, role, Some(&light.id))
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM tickets WHERE id = ?")
        .bind(&ticket.id)
        .execute(f.board.store().pool())
        .await
        .unwrap();
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ticket_agent_rows")
        .fetch_one(f.board.store().pool())
        .await
        .unwrap();
    assert_eq!(left, 0);
    assert_eq!(
        agents.delete(&light.id).await.unwrap(),
        DeleteOutcome::Deleted
    );
}
