//! The check before every launch (T-0357; SCH-04 `[M23]`, AGT-05, AGT-08, USG-10; d0086, d0088, d0018): a tool that
//! isn't available or is at its limit holds only the agents that would use it, a model no longer listed holds with
//! "Model unavailable", the fallback moves (automatically) or is offered (Use fallback), resumes are never checked,
//! and a hold lifts once when its cause goes, never on a timer.
//!
//! The decision is tested as a table (each role × each reason × automatic on/off × the fallback none, free or blocked);
//! the service's side with stand-in tool states (the tools' "programs" are scripts printing a version: no tool runs)
//! and lists recorded by hand. `claude` stands in as `sh` printing its arguments; a Codex launch runs a script that
//! exits at once.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::agent_tools::connected::AgentToolState;
use otaq_core::agent_tools::models::{ModelCopy, ModelEntry, ModelKind, ModelList};
use otaq_core::agent_tools::{LaunchContext, Tool};
use otaq_core::board::Refusal;
use otaq_core::board::moves::DropTarget;
use otaq_core::board::tickets::{EventKind, NewTicketInput, Ticket};
use otaq_core::client::{Client, ClientConfig};
use otaq_core::domain::{Actor, Kind, StartStep, Status};
use otaq_core::named_agents::gate::{Blocked, Decision, Facts, check, decide};
use otaq_core::named_agents::{
    AgentFields, AgentHold, AgentModel, FixedModels, HoldReason, MoveReason, NamedAgent,
    NamedAgents, Role, RowSetBy,
};
use otaq_core::service::testing::TestService;
use otaq_core::session::SessionKind;
use otaq_core::store::now_ms;
use otaq_core::usage::limit::{LimitHit, LimitKind};
use serde_json::json;
use tempfile::TempDir;

// The decision, as a table.

const A: Tool = Tool::Claude;
const B: Tool = Tool::Codex;

fn entry(id: &str, tool: Tool, model: Option<&str>, fallback: Option<&str>) -> NamedAgent {
    NamedAgent {
        id: id.into(),
        name: id.into(),
        tool,
        model: model.map(str::to_owned),
        use_for: String::new(),
        fallback: fallback.map(str::to_owned),
        mark: None,
        created_at: 0,
        updated_at: 0,
    }
}

fn copy(tool: Tool, ids: &[&str]) -> ModelCopy {
    ModelCopy {
        tool,
        list: ModelList {
            entries: ids
                .iter()
                .map(|id| ModelEntry {
                    id: (*id).into(),
                    display_name: (*id).into(),
                    description: None,
                    resolves_to: None,
                    kind: ModelKind::Exact,
                    family: None,
                    released_at: None,
                })
                .collect(),
        },
        read_at: 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Reason {
    Unavailable,
    Limit,
    Model,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Fallback {
    None,
    Free,
    Blocked,
}

/// What blocks `strong` (on A, model `m1`) for `reason`, and its fallback `gpt` (on B) when `fallback` is blocked.
fn facts(reason: Reason, fallback: Fallback) -> Facts {
    let mut facts = Facts::default();
    match reason {
        Reason::Unavailable => {
            facts.unavailable.insert(A, "logged out".into());
        }
        Reason::Limit => {
            facts.at_limit.insert(A);
        }
        Reason::Model => {
            facts.lists.insert(A, copy(A, &["m2"]));
        }
    }
    if fallback == Fallback::Blocked {
        facts.at_limit.insert(B);
    }
    facts
}

#[test]
fn every_role_every_reason_with_and_without_the_fallback() {
    for &role in Role::ALL {
        for reason in [Reason::Unavailable, Reason::Limit, Reason::Model] {
            for automatic in [false, true] {
                for fallback in [Fallback::None, Fallback::Free, Fallback::Blocked] {
                    let strong = entry(
                        "strong",
                        A,
                        Some("m1"),
                        (fallback != Fallback::None).then_some("gpt"),
                    );
                    let agents = [strong.clone(), entry("gpt", B, None, None)];
                    let decision =
                        decide(role, &strong, &agents, &facts(reason, fallback), automatic);
                    let case =
                        format!("{role} {reason:?} automatic={automatic} fallback={fallback:?}");
                    // A model never falls back, nor does a Project agent (Q-B).
                    let offered = reason != Reason::Model && role != Role::ProjectAgent;
                    let expect_reason = match reason {
                        Reason::Unavailable => HoldReason::ToolUnavailable,
                        Reason::Limit => HoldReason::ToolLimit,
                        Reason::Model => HoldReason::ModelUnavailable,
                    };
                    match (offered, fallback, automatic) {
                        (true, Fallback::Free, true) => {
                            let Decision::Move(moved) = decision else {
                                panic!("{case}: {decision:?}")
                            };
                            assert_eq!(moved.agent.id, "gpt", "{case}");
                            assert_eq!(
                                (moved.from.id.as_str(), moved.tool),
                                ("strong", A),
                                "{case}"
                            );
                            let reason = match reason {
                                Reason::Unavailable => MoveReason::ToolUnavailable,
                                _ => MoveReason::ToolLimit,
                            };
                            assert_eq!(moved.reason, reason, "{case}");
                            assert_eq!(moved.model, None, "{case}");
                        }
                        _ => {
                            let Decision::Hold(held) = decision else {
                                panic!("{case}: {decision:?}")
                            };
                            assert_eq!(held.blocked.reason(), expect_reason, "{case}");
                            assert_eq!(held.agent.id, "strong", "{case}");
                            let (want_fallback, want_held) = match (offered, fallback) {
                                (true, Fallback::Free) => (Some("gpt"), false),
                                (true, Fallback::Blocked) => (None, true),
                                _ => (None, false),
                            };
                            assert_eq!(held.fallback.as_deref(), want_fallback, "{case}");
                            assert_eq!(held.fallback_held, want_held, "{case}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn the_checks_run_in_order_and_pass_on_an_empty_model_or_a_list_never_read() {
    let strong = entry("strong", A, Some("m1"), None);
    let mut all = facts(Reason::Model, Fallback::None);
    all.at_limit.insert(A);
    all.unavailable.insert(A, "not installed".into());
    assert_eq!(
        check(&strong, &all),
        Err(Blocked::Unavailable {
            reason: "not installed".into()
        })
    );
    all.unavailable.clear();
    assert_eq!(check(&strong, &all), Err(Blocked::AtLimit));
    all.at_limit.clear();
    assert_eq!(check(&strong, &all), Err(Blocked::ModelUnlisted));
    // A listed model passes as it is; an empty one passes no model; a list never read never blocks.
    all.lists.insert(A, copy(A, &["m1"]));
    assert_eq!(check(&strong, &all), Ok(Some("m1".into())));
    let default = entry("default", A, None, None);
    assert_eq!(
        check(&default, &facts(Reason::Model, Fallback::None)),
        Ok(None)
    );
    assert_eq!(check(&strong, &Facts::default()), Ok(Some("m1".into())));
    // Another tool's problem changes nothing.
    let gpt = entry("gpt", B, Some("gpt-5.5"), None);
    assert_eq!(
        check(&gpt, &facts(Reason::Unavailable, Fallback::None)),
        Ok(Some("gpt-5.5".into()))
    );
}

#[test]
fn a_family_choice_resolves_to_its_newest_listed_model() {
    let mut list = copy(B, &["gpt-6-sol"]);
    list.list.entries.insert(
        0,
        ModelEntry {
            id: "newest:gpt-sol".into(),
            display_name: "GPT Sol".into(),
            description: None,
            resolves_to: Some("gpt-6-sol".into()),
            kind: ModelKind::Follow,
            family: Some("gpt-sol".into()),
            released_at: None,
        },
    );
    let facts = Facts {
        lists: HashMap::from([(B, list)]),
        ..Facts::default()
    };
    let strong_gpt = entry("strong-gpt", B, Some("newest:gpt-sol"), None);
    assert_eq!(check(&strong_gpt, &facts), Ok(Some("gpt-6-sol".into())));
    let gone = entry("gone", B, Some("newest:gpt-astra"), None);
    assert_eq!(check(&gone, &facts), Err(Blocked::ModelUnlisted));
}

#[test]
fn the_hand_check_seam_is_debug_only() {
    let parsed = otaq_core::agent_tools::models::dev_drop_model(Some("codex:gpt-5.4"));
    assert_eq!(parsed.is_some(), cfg!(debug_assertions));
    if cfg!(debug_assertions) {
        assert_eq!(parsed, Some((B, "gpt-5.4".into())));
    }
    for bad in [
        None,
        Some(""),
        Some("codex"),
        Some("codex:"),
        Some("nope:gpt-5.4"),
    ] {
        assert_eq!(otaq_core::agent_tools::models::dev_drop_model(bad), None);
    }
}

// The service.

fn me() -> Actor {
    Actor::User("me".into())
}

struct World {
    w: TestService,
    rules: NamedAgents,
    /// The stand-in tools' folder (their scripts in `bin`).
    tools: TempDir,
}

impl World {
    async fn agent(&self, name: &str) -> NamedAgent {
        self.rules
            .list()
            .await
            .unwrap()
            .agents
            .into_iter()
            .find(|a| a.name == name)
            .unwrap_or_else(|| panic!("no agent {name}"))
    }

    async fn set_row(&self, key: &str, role: Role, name: &str) {
        let agent = self.agent(name).await;
        self.rules
            .set_row_by_user(key, role, Some(&agent.id))
            .await
            .unwrap();
    }

    async fn hide(&self, tool: Tool, hidden: bool) {
        self.w
            .connected
            .set_hidden(self.w.board.store(), tool, hidden)
            .await
            .unwrap();
    }

    async fn limit(&self, tool: Tool) {
        let hit = LimitHit {
            kind: LimitKind::Session,
            text: "You've hit your session limit · resets 3:45pm".into(),
        };
        self.w
            .plan_limit
            .record_hit(tool, &hit, now_ms())
            .await
            .unwrap();
    }

    async fn client(&self) -> Client {
        Client::connect(ClientConfig::new(&self.w.data))
            .await
            .unwrap()
    }

    fn bin(&self) -> PathBuf {
        self.tools.path().join("bin")
    }
}

const ARGS: &str = "printf 'ARGS %s\\n' \"$*\"; exec cat";

/// Claude Code and Codex found and connected (stand-ins printing a version), with "Claude Code" (every default),
/// "Codex", and the example's Sonnet (Claude Code, `sonnet`), Standard (Codex, fallback Sonnet) and Old GPT (Codex,
/// `gpt-5.4`, fallback Sonnet). Project SHOP (`shop-web`).
async fn world() -> World {
    let tools = TempDir::new().unwrap();
    let bin = tools.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(tools.path().join("claude")).unwrap();
    otaq_core::process::testing::stub_command(&bin, "claude", r#"echo "2.1.4 (Claude Code)""#);
    otaq_core::process::testing::stub_command(&bin, "codex", r#"echo "codex-cli 0.157.1""#);
    let root = tools.path().to_path_buf();
    let agent_path = std::env::join_paths([bin.clone(), "/usr/bin".into(), "/bin".into()]).unwrap();
    let w = TestService::start_with(move |mut config| {
        config.tools.path = bin.into_os_string();
        config.tools.claude_config_dir = Some(root.join("claude"));
        config.tools.managed_settings_dir = root.join("managed");
        config.check_tool_states = true;
        config.scheduler_tick = Duration::from_millis(50);
        config.launch = Some(LaunchContext {
            bin_dir: "/nonexistent".into(),
            plugin_dir: "/nonexistent/plugin".into(),
            // Only the stand-ins: a Codex launch runs the script, never a real `codex`.
            path: agent_path,
            claude: ClaudeLaunch {
                program: vec!["sh".into(), "-c".into(), ARGS.into(), "sh".into()],
                config_dir: None,
            },
        });
        config
    })
    .await;
    until("both tools connected", || async {
        [A, B]
            .iter()
            .all(|&t| w.connected.status(t).state.is_connected())
    })
    .await;
    until("the starter entries", || async {
        w.board
            .store()
            .agent_defaults()
            .await
            .unwrap()
            .developer
            .is_some()
            && w.board.store().named_agents().await.unwrap().len() >= 2
    })
    .await;
    w.add_project("shop-web").await;
    let rules = NamedAgents::new(
        w.board.clone(),
        Arc::new(FixedModels::new(vec![
            AgentModel::ToolDefault,
            AgentModel::Follow("sonnet".into()),
            AgentModel::Exact("gpt-5.4".into()),
            AgentModel::Exact("gpt-5.5".into()),
        ])),
    );
    let sonnet = rules
        .add(&AgentFields {
            name: "Sonnet".into(),
            tool: A,
            model: Some("sonnet".into()),
            use_for: String::new(),
            fallback: None,
        })
        .await
        .unwrap();
    for (name, model) in [("Standard", None), ("Old GPT", Some("gpt-5.4"))] {
        rules
            .add(&AgentFields {
                name: name.into(),
                tool: B,
                model: model.map(str::to_owned),
                use_for: String::new(),
                fallback: Some(sonnet.id.clone()),
            })
            .await
            .unwrap();
    }
    World { w, rules, tools }
}

async fn until<F, Fut>(what: &str, done: F)
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let reached = tokio::time::timeout(Duration::from_secs(20), async {
        while !done().await {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(reached.is_ok(), "never: {what}");
}

async fn ticket(w: &TestService, key: &str) -> Ticket {
    w.board.ticket(key).await.unwrap()
}

/// The user's drag to Ready for Dev; the scheduler starts it.
async fn queue(w: &TestService, key: &str) {
    w.board
        .move_ticket(key, DropTarget::ReadyForDev, true, &me())
        .await
        .unwrap();
}

async fn until_status(w: &TestService, key: &str, want: Status) {
    let reached = tokio::time::timeout(Duration::from_secs(20), async {
        while w.status(key).await != want {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    if reached.is_err() {
        let t = ticket(w, key).await;
        panic!(
            "{key} never became {want}: {} {:?}",
            t.status, t.start_problem
        );
    }
}

/// The ticket left Ready for Dev's waiting states: its agent started (or its start was tried and stopped).
async fn until_started(w: &TestService, key: &str) {
    until(&format!("{key} started"), || async {
        !matches!(
            w.status(key).await,
            Status::Held | Status::Queued | Status::WaitingForSlot
        )
    })
    .await;
}

async fn holds(w: &TestService, key: &str) -> Vec<AgentHold> {
    let t = ticket(w, key).await;
    w.board.store().ticket_holds(&t.id).await.unwrap()
}

async fn count(w: &TestService, key: &str, kind: EventKind) -> usize {
    w.board
        .history(key)
        .await
        .unwrap()
        .iter()
        .filter(|e| e.kind == kind)
        .count()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tool_that_isnt_available_holds_only_its_tickets_until_it_is_back() {
    let world = world().await;
    let w = &world.w;
    let codex_ticket = w.create("SHOP", "Add coupon codes").await;
    w.create("SHOP", "Update footer links").await;
    world.set_row("SHOP-1", Role::Developer, "Standard").await;
    world.hide(B, true).await;
    queue(w, "SHOP-1").await;
    queue(w, "SHOP-2").await;

    // SHOP-2 runs on Claude Code; SHOP-1 waits in Ready for Dev, nothing made for it.
    until_status(w, "SHOP-2", Status::Working).await;
    until_status(w, "SHOP-1", Status::Held).await;
    let held = ticket(w, "SHOP-1").await;
    let problem = held.start_problem.clone().unwrap();
    assert_eq!(problem.step, StartStep::ToolUnavailable);
    assert_eq!(
        problem.message,
        "Codex isn\u{2019}t available \u{b7} hidden in Settings"
    );
    assert!(
        held.work
            .as_ref()
            .and_then(|w| w.worktree.as_ref())
            .is_none()
    );
    let standard = world.agent("Standard").await;
    let sonnet = world.agent("Sonnet").await;
    let [hold] = &holds(w, "SHOP-1").await[..] else {
        panic!("one hold")
    };
    assert_eq!(
        (hold.role, hold.reason, hold.tool),
        (Role::Developer, HoldReason::ToolUnavailable, B)
    );
    assert_eq!(hold.named_agent_id.as_deref(), Some(standard.id.as_str()));
    assert_eq!(hold.fallback_agent_id.as_deref(), Some(sonnet.id.as_str()));
    assert!(!hold.fallback_held);
    assert!(
        !world
            .rules
            .lock_facts(&codex_ticket.id)
            .await
            .unwrap()
            .developer_started,
        "nothing locks"
    );

    // Nothing retries on a timer: many scheduler passes later it is held once.
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(w.status("SHOP-1").await, Status::Held);
    assert_eq!(count(w, "SHOP-1", EventKind::Held).await, 1);

    // Shown again: the start runs once, as the drag asked for it.
    world.hide(B, false).await;
    until_started(w, "SHOP-1").await;
    assert!(holds(w, "SHOP-1").await.is_empty());
    assert_eq!(count(w, "SHOP-1", EventKind::Held).await, 1);
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE ticket_id = ?")
        .bind(&codex_ticket.id)
        .fetch_one(w.board.store().pool())
        .await
        .unwrap();
    assert_eq!(sessions, 1, "one launch on Codex");
    world.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tool_at_its_limit_holds_its_agents_while_other_tools_keep_starting() {
    let world = world().await;
    let w = &world.w;
    w.create("SHOP", "Checkout flow").await;
    w.create("SHOP", "Add coupon codes").await;
    world.set_row("SHOP-2", Role::Developer, "Standard").await;
    world.limit(A).await;
    queue(w, "SHOP-1").await;
    queue(w, "SHOP-2").await;
    until_status(w, "SHOP-1", Status::Held).await;
    let problem = ticket(w, "SHOP-1").await.start_problem.unwrap();
    assert_eq!(
        (problem.step, problem.message.as_str()),
        (StartStep::ToolLimit, "Claude Code limit reached")
    );
    // The Codex ticket started meanwhile.
    until_started(w, "SHOP-2").await;
    assert!(holds(w, "SHOP-2").await.is_empty());

    // The limit is resumed: the held start runs.
    w.plan_limit.resume_all().await.unwrap();
    until_status(w, "SHOP-1", Status::Working).await;
    assert!(holds(w, "SHOP-1").await.is_empty());
    world.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_model_no_longer_listed_holds_marks_and_lifts_on_a_new_row_or_the_list() {
    let world = world().await;
    let w = &world.w;
    // Codex's list without gpt-5.4 (as a refresh with the hand-check seam would leave it).
    w.models
        .record(B, Ok(copy(B, &["gpt-5.5"]).list), now_ms())
        .await;
    w.create("SHOP", "Export orders as CSV").await;
    w.create("SHOP", "Order totals rounding").await;
    world.set_row("SHOP-1", Role::Developer, "Old GPT").await;
    world.set_row("SHOP-2", Role::Developer, "Old GPT").await;
    // Even with the automatic fallback on, a model never falls back.
    w.gate.set_automatic(true).await.unwrap();
    queue(w, "SHOP-1").await;
    queue(w, "SHOP-2").await;
    until_status(w, "SHOP-1", Status::Held).await;
    until_status(w, "SHOP-2", Status::Held).await;
    let problem = ticket(w, "SHOP-1").await.start_problem.unwrap();
    assert_eq!(problem.step, StartStep::ModelUnavailable);
    assert_eq!(
        problem.message,
        "Model unavailable \u{b7} Codex no longer lists gpt-5.4"
    );
    let [hold] = &holds(w, "SHOP-1").await[..] else {
        panic!("one hold")
    };
    assert_eq!(hold.reason, HoldReason::ModelUnavailable);
    assert_eq!(
        (hold.fallback_agent_id.as_deref(), hold.fallback_held),
        (None, false)
    );
    let mark = world.agent("Old GPT").await.mark.expect("marked");
    assert_eq!(mark.text, "Codex no longer lists gpt-5.4");
    assert_eq!(mark.model.as_deref(), Some("gpt-5.4"));

    // Change agent: SHOP-1 on Standard starts.
    let standard = world.agent("Standard").await;
    world
        .client()
        .await
        .request(
            serde_json::from_value(json!({"method": "ticket.agent_row",
                "params": {"key": "SHOP-1", "role": "developer", "agent": standard.id}}))
            .unwrap(),
        )
        .await
        .unwrap();
    until_started(w, "SHOP-1").await;
    assert_eq!(w.status("SHOP-2").await, Status::Held);

    // The list has the model again: the mark clears and SHOP-2 starts.
    w.models
        .record(B, Ok(copy(B, &["gpt-5.5", "gpt-5.4"]).list), now_ms())
        .await;
    until_started(w, "SHOP-2").await;
    until("the mark cleared", || async {
        world.agent("Old GPT").await.mark.is_none()
    })
    .await;
    world.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_automatic_fallback_moves_even_a_row_set_by_you_and_nothing_moves_back() {
    let world = world().await;
    let w = &world.w;
    w.create("SHOP", "Add coupon codes").await;
    w.create("SHOP", "Coupon field").await;
    world.set_row("SHOP-1", Role::Developer, "Standard").await;
    world.hide(B, true).await;
    queue(w, "SHOP-1").await;
    until_status(w, "SHOP-1", Status::Held).await;

    // "Use fallback agents automatically" on: the waiting ticket moves to Sonnet and starts there.
    w.gate.set_automatic(true).await.unwrap();
    until_status(w, "SHOP-1", Status::Working).await;
    let standard = world.agent("Standard").await;
    let sonnet = world.agent("Sonnet").await;
    let t = ticket(w, "SHOP-1").await;
    let rows = w.board.store().ticket_agent_rows(&t.id).await.unwrap();
    let row = rows.iter().find(|r| r.role == Role::Developer).unwrap();
    assert_eq!(row.agent.as_deref(), Some(sonnet.id.as_str()));
    assert_eq!(row.set_by, RowSetBy::User, "who set it stays");
    let moved = w
        .board
        .store()
        .ticket_row_move(&t.id, Role::Developer)
        .await
        .unwrap()
        .expect("the move is recorded");
    assert_eq!(moved.from_id.as_deref(), Some(standard.id.as_str()));
    assert_eq!(
        (moved.from_name.as_str(), moved.reason, moved.tool),
        ("Standard", MoveReason::ToolUnavailable, Some(B))
    );
    let session = w.sessions.running_of(&t.id, SessionKind::Agent).unwrap();
    assert_eq!(session.named_agent.unwrap().name, "Sonnet");

    // A new start on the project's default (Standard, no row) moves before anything holds it, as Otaq's pick.
    world
        .rules
        .set_project_default("SHOP", Role::Developer, Some(&standard.id))
        .await
        .unwrap();
    queue(w, "SHOP-2").await;
    until_status(w, "SHOP-2", Status::Working).await;
    assert_eq!(count(w, "SHOP-2", EventKind::Held).await, 0);
    let second = ticket(w, "SHOP-2").await;
    let rows = w.board.store().ticket_agent_rows(&second.id).await.unwrap();
    let row = rows.iter().find(|r| r.role == Role::Developer).unwrap();
    assert_eq!(row.agent.as_deref(), Some(sonnet.id.as_str()));
    assert_eq!(
        row.set_by,
        RowSetBy::Agent {
            picked_by: otaq_core::naming::PRODUCT_NAME.into(),
            reason: "Codex not available".into()
        },
        "never set by you"
    );
    assert!(
        w.board
            .store()
            .ticket_row_move(&second.id, Role::Developer)
            .await
            .unwrap()
            .is_some()
    );

    // Codex is back: nothing moves back.
    world.hide(B, false).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let rows = w.board.store().ticket_agent_rows(&t.id).await.unwrap();
    let row = rows.iter().find(|r| r.role == Role::Developer).unwrap();
    assert_eq!(row.agent.as_deref(), Some(sonnet.id.as_str()));
    world.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn use_fallback_moves_and_starts_and_a_blocked_fallback_is_both_held() {
    let world = world().await;
    let w = &world.w;
    w.create("SHOP", "Add coupon codes").await;
    w.create("SHOP", "Checkout flow").await;
    world.set_row("SHOP-1", Role::Developer, "Standard").await;
    world.set_row("SHOP-2", Role::Developer, "Standard").await;
    world.hide(B, true).await;
    queue(w, "SHOP-1").await;
    until_status(w, "SHOP-1", Status::Held).await;
    w.gate
        .use_fallback("SHOP-1", Role::Developer)
        .await
        .unwrap();
    until_status(w, "SHOP-1", Status::Working).await;
    let t = ticket(w, "SHOP-1").await;
    let moved = w
        .board
        .store()
        .ticket_row_move(&t.id, Role::Developer)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (moved.reason, moved.tool),
        (MoveReason::ToolUnavailable, Some(B))
    );
    let rows = w.board.store().ticket_agent_rows(&t.id).await.unwrap();
    let row = rows.iter().find(|r| r.role == Role::Developer).unwrap();
    assert_eq!(row.set_by, RowSetBy::User, "who set it is unchanged");

    // Use fallback on a ticket with no row (the project's default): the user's click.
    let standard = world.agent("Standard").await;
    world
        .rules
        .set_project_default("SHOP", Role::Developer, Some(&standard.id))
        .await
        .unwrap();
    w.create("SHOP", "Order email").await;
    queue(w, "SHOP-3").await;
    until_status(w, "SHOP-3", Status::Held).await;
    w.gate
        .use_fallback("SHOP-3", Role::Developer)
        .await
        .unwrap();
    until_status(w, "SHOP-3", Status::Working).await;
    let third = ticket(w, "SHOP-3").await;
    let rows = w.board.store().ticket_agent_rows(&third.id).await.unwrap();
    let row = rows.iter().find(|r| r.role == Role::Developer).unwrap();
    assert_eq!(row.set_by, RowSetBy::User);

    // Sonnet's tool is at its limit too: both held, nothing to offer.
    world.limit(A).await;
    queue(w, "SHOP-2").await;
    until_status(w, "SHOP-2", Status::Held).await;
    let [hold] = &holds(w, "SHOP-2").await[..] else {
        panic!("one hold")
    };
    assert_eq!(
        (hold.fallback_agent_id.as_deref(), hold.fallback_held),
        (None, true)
    );
    let e = w
        .gate
        .use_fallback("SHOP-2", Role::Developer)
        .await
        .unwrap_err();
    assert_eq!(e.refusal(), Some(Refusal::MoveRefused));
    assert_eq!(
        e.to_string(),
        "Sonnet can't start either: Claude Code limit reached"
    );
    // A ticket that isn't held has nothing to move.
    let e = w
        .gate
        .use_fallback("SHOP-1", Role::Developer)
        .await
        .unwrap_err();
    assert_eq!(e.to_string(), "SHOP-1's Developer isn't waiting");
    world.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_launch_that_fails_because_of_its_tool_rechecks_it_and_holds() {
    let world = world().await;
    let w = &world.w;
    w.create("SHOP", "Add coupon codes").await;
    world.set_row("SHOP-1", Role::Developer, "Standard").await;
    // Codex went away since the last check: the launch can't find it.
    std::fs::remove_file(world.bin().join("codex")).unwrap();
    queue(w, "SHOP-1").await;
    until_status(w, "SHOP-1", Status::Held).await;
    let problem = ticket(w, "SHOP-1").await.start_problem.unwrap();
    assert_eq!(problem.step, StartStep::ToolUnavailable);
    assert_eq!(
        problem.message,
        "Codex isn\u{2019}t available \u{b7} not installed"
    );
    assert_eq!(w.connected.status(B).state, AgentToolState::NotInstalled);
    world.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn resumes_are_never_checked() {
    let world = world().await;
    let w = &world.w;
    w.create("SHOP", "Price table").await;
    queue(w, "SHOP-1").await;
    until_status(w, "SHOP-1", Status::Working).await;
    w.review.park("SHOP-1", true, &me()).await.unwrap();
    // Claude Code at its limit and hidden: the parked conversation still resumes on it.
    world.limit(A).await;
    world.hide(A, true).await;
    queue(w, "SHOP-1").await;
    until_status(w, "SHOP-1", Status::Working).await;
    assert_eq!(count(w, "SHOP-1", EventKind::Held).await, 0);
    assert!(holds(w, "SHOP-1").await.is_empty());
    world.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_held_validator_waits_in_dev_and_starts_once_its_tool_is_back() {
    let world = world().await;
    let w = &world.w;
    let t = w.create("SHOP", "Cart totals").await;
    world.set_row("SHOP-1", Role::Validator, "Sonnet").await;
    w.give_worktree(&t).await;
    w.force_status(&t, Status::Working).await;
    world.hide(A, true).await;
    let after = w
        .validation
        .submit(
            "SHOP-1",
            "Cart totals add up.",
            &Actor::Agent(t.key.clone()),
            otaq_core::domain::ValidationTrigger::Agent,
        )
        .await
        .unwrap();
    assert_eq!(after.status, Status::Validating);
    assert!(
        w.sessions
            .running_of(&t.id, SessionKind::Validator)
            .is_none()
    );
    let [hold] = &holds(w, "SHOP-1").await[..] else {
        panic!("one hold")
    };
    assert_eq!(
        (hold.role, hold.reason),
        (Role::Validator, HoldReason::ToolUnavailable)
    );
    assert!(
        !world.rules.lock_facts(&t.id).await.unwrap().round_running,
        "the Validator row stays editable"
    );
    world.hide(A, false).await;
    until("the validator started", || async {
        w.sessions
            .running_of(&t.id, SessionKind::Validator)
            .is_some()
    })
    .await;
    assert!(holds(w, "SHOP-1").await.is_empty());
    world.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lead_and_a_project_agent_are_held_too_and_a_project_agent_never_falls_back() {
    let world = world().await;
    let w = &world.w;
    let epic = w
        .board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Wishlist".into(),
                kind: Kind::Epic,
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    world.hide(A, true).await;
    let not_available = "Claude Code isn\u{2019}t available \u{b7} hidden in Settings";
    let e = w.project_agents.leads().open("SHOP-1").await.unwrap_err();
    assert_eq!(e.to_string(), not_available);
    let [hold] = &holds(w, "SHOP-1").await[..] else {
        panic!("one hold")
    };
    assert_eq!(hold.role, Role::EpicLead);
    // A Project agent on Standard (fallback Sonnet, both blocked here or not): no fallback is ever offered.
    world.hide(B, true).await;
    let standard = world.agent("Standard").await;
    world
        .rules
        .set_project_default("SHOP", Role::ProjectAgent, Some(&standard.id))
        .await
        .unwrap();
    let e = w
        .project_agents
        .open("SHOP", "Why is checkout slow?")
        .await
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "Codex isn\u{2019}t available \u{b7} hidden in Settings"
    );
    let project = w.board.project("SHOP").await.unwrap();
    let [hold] = &w.board.store().project_holds(&project.id).await.unwrap()[..] else {
        panic!("one hold")
    };
    assert_eq!(
        (
            hold.role,
            hold.fallback_agent_id.as_deref(),
            hold.fallback_held
        ),
        (Role::ProjectAgent, None, false)
    );

    // Back: the lead starts (the user's Open lead asked for it); the Project agent's message waits for a send.
    world.hide(A, false).await;
    world.hide(B, false).await;
    until("the lead started", || async {
        w.project_agents
            .leads()
            .get(&epic.key.to_string())
            .await
            .unwrap()
            .is_some()
    })
    .await;
    until("the project hold went", || async {
        w.board
            .store()
            .project_holds(&project.id)
            .await
            .unwrap()
            .is_empty()
    })
    .await;
    assert!(w.project_agents.list("SHOP").await.unwrap().open.is_empty());
    world.w.stop().await;
}

/// A swap (AGT-05, Q-J): on a tool that quietly runs another model (OpenCode here), only an exact model asked for
/// and another one run marks the entry; a follow choice choosing a model never does (a tool that refuses instead is
/// `mark.rs`'s unit test).
#[tokio::test(flavor = "multi_thread")]
async fn the_swap_mark_is_only_for_an_exact_model_on_a_swapping_tool() {
    let world = world().await;
    let w = &world.w;
    let payload: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/opencode/events/session.step.started.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut marks = Vec::new();
    for (n, (tool, asked)) in [
        (Tool::OpenCode, "opencode/gpt-5.5"),
        (Tool::OpenCode, "newest:big"),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("OC {n}");
        let agent = world
            .rules
            .add(&AgentFields {
                name: name.clone(),
                tool,
                model: None,
                use_for: String::new(),
                fallback: None,
            })
            .await
            .unwrap();
        let t = w.create("SHOP", "Order totals rounding").await;
        w.force_status(&t, Status::Working).await;
        let shell = w.agent_shell(&t).await;
        sqlx::query("UPDATE sessions SET tool = ?, model = ?, named_agent_id = ?, named_agent_name = ? WHERE id = ?")
            .bind(tool.as_str())
            .bind(asked)
            .bind(&agent.id)
            .bind(&name)
            .bind(&shell.session)
            .execute(w.board.store().pool())
            .await
            .unwrap();
        let caller = otaq_core::caller::Caller::Agent {
            ticket: t.key.clone(),
            ticket_id: t.id.clone(),
            session: shell.session.clone(),
        };
        w.signals
            .hook(&caller, tool, "session.step.started", &payload)
            .await
            .unwrap();
        marks.push(world.agent(&name).await.mark.map(|m| m.text));
    }
    assert_eq!(
        marks,
        [
            Some(
                "OpenCode ran opencode/big-pickle instead of opencode/gpt-5.5 on SHOP-1".to_owned()
            ),
            None
        ]
    );
    world.w.stop().await;
}
