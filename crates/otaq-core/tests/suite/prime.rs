//! `otaq prime` output (T-0071, PLG-02; d0040): snapshots of the Markdown for sample tickets, with and without
//! Agent instructions; and the named agents every prime lists (T-0339, AGT-06; d0086).

use std::sync::Arc;

use otaq_core::agent_tools::Tool;
use otaq_core::agent_tools::models::{ModelCopy, ModelEntry, ModelKind, ModelList, settings_key};
use otaq_core::board::tickets::TicketUpdate;
use otaq_core::client::{Client, ClientConfig};
use otaq_core::data_dir::DataDir;
use otaq_core::domain::Actor;
use otaq_core::git::testing::TempRepos;
use otaq_core::named_agents::starter::{self, ConnectedTool};
use otaq_core::named_agents::{AgentFields, FixedModels, NamedAgents, Role};
use otaq_core::prime::{model_text, render};
use otaq_core::protocol::{KeyParams, ProjectParams, Reply, Request};
use otaq_core::service::testing::TestService;
use otaq_core::store::{Store, new_id, now_ms};

async fn sample() -> TestService {
    let w = TestService::start().await;
    w.shop_sample().await;
    let me = Actor::User("me".into());
    w.board
        .update_ticket(
            "SHOP-20",
            &TicketUpdate {
                body: Some(
                    "The checkout page shows the cart, the price table and a pay button.\n\n## Done when\n\n\
                     - pay button calls the Checkout API\n- works on mobile widths"
                        .into(),
                ),
                ..Default::default()
            },
            &me,
        )
        .await
        .unwrap();
    // SHOP-18's agent submitted with a summary.
    let shop18 = w.board.ticket("SHOP-18").await.unwrap();
    sqlx::query("INSERT INTO events (id, ticket_id, kind, actor, payload_json, at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(new_id())
        .bind(&shop18.id)
        .bind("review_submitted")
        .bind("agent:SHOP-18")
        .bind(r#"{"summary": "POST /checkout creates a Stripe session.\nReturns the session URL."}"#)
        .bind(now_ms())
        .execute(w.board.store().pool())
        .await
        .unwrap();
    w
}

const RULES: &str = "## Otaq's rules

1. Follow the Agent instructions below. They add to these rules; they can't skip getting context or submitting for review.
2. Commit everything with `/otaq:commit` before you submit. Never push and never open a PR: the user does that.
3. When the ticket is done, as its description describes, submit: `otaq review submit --stdin` with a short summary of what you changed on stdin. It is refused while files are uncommitted.
4. When you are stuck or a decision is the user's, ask: `otaq ask human --stdin` with the question on stdin, then end your turn.
5. Long text (summaries, questions, ticket bodies) always goes through stdin (`--stdin`), never quoted arguments.
6. `otaq show <KEY>` reads any ticket; `otaq create \"Title\" --stdin` files a follow-up in Backlog.
";

#[tokio::test(flavor = "multi_thread")]
async fn without_instructions() {
    let w = sample().await;
    let context = w
        .board
        .prime_context("SHOP-20", w.adapters.for_new_session())
        .await
        .unwrap();
    assert_eq!(
        render(&context),
        format!(
            "# SHOP-20 · Checkout page

You are the agent for Otaq ticket SHOP-20, working in its own worktree on branch `otaq/SHOP-20`. It merges into `main` once the user approves it.
Priority: P3 · Normal.

## What and why

The checkout page shows the cart, the price table and a pay button.

## Done when

- pay button calls the Checkout API
- works on mobile widths

## Depends on

- SHOP-18 · Checkout API (Ready for Review)
  > POST /checkout creates a Stripe session.
  > Returns the session URL.
- SHOP-19 · Price table (Working)
  (no summary yet)

{RULES}"
        )
    );
    assert!(context.review_feedback.is_empty() && context.pending_questions.is_empty());
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn with_global_and_project_instructions() {
    let w = sample().await;
    let mut global = w.board.global_settings().await.unwrap();
    global.agent_instructions = "Use my /tdd skill.\nRun `pnpm test` before submitting.".into();
    w.board.set_global_settings(&global).await.unwrap();

    let epic_child = w
        .board
        .prime_context("SHOP-40.2", w.adapters.for_new_session())
        .await
        .unwrap();
    let text = render(&epic_child);
    assert!(text.contains("It merges into the epic's branch (SHOP-40) once the user approves it.\nIt is part of the epic SHOP-40 · Wishlist.\nPriority: P3 · Normal.\n"), "{text}");
    assert!(
        !text.contains("Done when"),
        "no Done when section (d0067): {text}"
    );
    assert!(text.ends_with(&format!(
        "{RULES}\n## Agent instructions (global)\n\nUse my /tdd skill.\nRun `pnpm test` before submitting.\n"
    )), "{text}");

    // The project's override wins and is labelled with the project.
    let mut project = w.board.project_settings("SHOP").await.unwrap();
    project.agent_instructions = "Ask before touching the payment code.".into();
    w.board
        .set_project_settings("SHOP", &project)
        .await
        .unwrap();
    let text = render(
        &w.board
            .prime_context("SHOP-20", w.adapters.for_new_session())
            .await
            .unwrap(),
    );
    assert!(
        text.ends_with(
            "## Agent instructions (project shop-web)\n\nAsk before touching the payment code.\n"
        ),
        "{text}"
    );
    assert!(!text.contains("/tdd"));

    // Review feedback and pending questions render once they exist (M6, M10).
    let mut context = w
        .board
        .prime_context("SHOP-20", w.adapters.for_new_session())
        .await
        .unwrap();
    context.review_feedback = vec!["src/pay.ts:12 · handle a declined card".into()];
    context.pending_questions = vec!["SHOP-21 asks: which email template?".into()];
    let text = render(&context);
    assert!(text.contains("\n## Review feedback\n\n- src/pay.ts:12 · handle a declined card\n"));
    assert!(text.contains("\n## Pending questions\n\n- SHOP-21 asks: which email template?\n"));
    w.stop().await;
}

/// T-0184 (VAL-10): `prime --validate` for a plain ticket: the ticket, the summary, the parents with theirs, the
/// Agent instructions, the diff and how to report.
#[tokio::test(flavor = "multi_thread")]
async fn validate_prime_plain() {
    let w = sample().await;
    let mut global = w.board.global_settings().await.unwrap();
    global.agent_instructions = "Run `pnpm test` before submitting.".into();
    w.board.set_global_settings(&global).await.unwrap();
    let ctx = w.board.validate_prime("SHOP-20").await.unwrap();
    assert_eq!((ctx.round, ctx.rounds), (1, 3));
    let text = otaq_core::prime::render_validate(&ctx);
    for part in [
        "# Validate SHOP-20 · Checkout page\n",
        "a read-only judge of its agent's work (round 1 of 3). Never edit files, never commit and never fix anything.",
        "## What and why\n\nThe checkout page shows the cart",
        "## The agent's summary\n\n(none)",
        "## Linked tickets",
        "- depends on SHOP-18 · Checkout API (Ready for Review)\n  > POST /checkout creates a Stripe session.",
        "- depends on SHOP-19 · Price table (Working)",
        "## Agent instructions (global)\n\nRun `pnpm test` before submitting.",
        "## The diff\n\n```diff\n",
        "`otaq validate pass --stdin` or `otaq validate fail --stdin`",
    ] {
        assert!(text.contains(part), "missing {part:?} in\n{text}");
    }
    assert!(!text.contains("## The user's review"), "never sent back");
    w.stop().await;
}

/// An epic child: the epic's description and its siblings.
#[tokio::test(flavor = "multi_thread")]
async fn validate_prime_epic_child_with_siblings() {
    let w = sample().await;
    w.board
        .update_ticket(
            "SHOP-40",
            &TicketUpdate {
                body: Some("Customers keep a wishlist.".into()),
                ..Default::default()
            },
            &Actor::User("me".into()),
        )
        .await
        .unwrap();
    let ctx = w.board.validate_prime("SHOP-40.2").await.unwrap();
    let epic = ctx.epic.as_ref().unwrap();
    assert_eq!(epic.key.to_string(), "SHOP-40");
    let siblings: Vec<String> = epic.siblings.iter().map(|s| s.key.to_string()).collect();
    assert_eq!(siblings, ["SHOP-40.1", "SHOP-40.3"]);
    let text = otaq_core::prime::render_validate(&ctx);
    assert!(
        text.contains("Epic SHOP-40 · Wishlist:\nCustomers keep a wishlist.\n- sibling SHOP-40.1"),
        "{text}"
    );
    w.stop().await;
}

/// Claude Code's list as its `initialize` request gives it: the tool default and four follow choices.
fn claude_list() -> ModelList {
    let entry = |id: &str, name: &str, kind| ModelEntry {
        id: id.into(),
        display_name: name.into(),
        description: None,
        resolves_to: None,
        kind,
    };
    let mut entries = vec![entry(
        "default",
        "Default (recommended)",
        ModelKind::ToolDefault,
    )];
    for (id, name) in [
        ("opus", "Opus"),
        ("sonnet", "Sonnet"),
        ("haiku", "Haiku"),
        ("fable", "Fable"),
    ] {
        entries.push(entry(id, name, ModelKind::Follow));
    }
    ModelList { entries }
}

/// The sample board on a service whose copy of Claude Code's list was read earlier, with the starter entry
/// ("Claude Code", every app default on it).
async fn with_agents() -> (TestService, NamedAgents) {
    let home = tempfile::TempDir::new().unwrap();
    let store = Store::open_in(&DataDir::new(home.path().join("h")))
        .await
        .unwrap();
    let tool = Tool::ALL[0];
    let copy = ModelCopy {
        tool,
        list: claude_list(),
        read_at: 1,
    };
    store
        .set_setting(&settings_key(tool), &serde_json::to_string(&copy).unwrap())
        .await
        .unwrap();
    store.close().await;
    let w = TestService::start_in(home, TempRepos::new(), |config| config).await;
    w.shop_sample().await;
    let claude_code = ConnectedTool {
        tool: tool.as_str().into(),
        name: "Claude Code".into(),
    };
    assert!(
        starter::ensure(w.board.store(), &[claude_code])
            .await
            .unwrap()
    );
    let agents = NamedAgents::new(w.board.clone(), Arc::new(FixedModels::sample()));
    (w, agents)
}

/// The three primes as the user's terminal gets them, rendered.
async fn primes(w: &TestService) -> (String, String, String) {
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let ticket = match client
        .request(Request::TicketPrime(KeyParams {
            key: "SHOP-20".into(),
        }))
        .await
        .unwrap()
    {
        Reply::Prime(c) => render(&c),
        other => panic!("{other:?}"),
    };
    let project = match client
        .request(Request::ProjectAgentPrime(ProjectParams {
            project: "SHOP".into(),
        }))
        .await
        .unwrap()
    {
        Reply::ProjectPrime(c) => otaq_core::project_agents::prime::render(&c),
        other => panic!("{other:?}"),
    };
    let epic = match client
        .request(Request::LeadPrime(KeyParams {
            key: "SHOP-40".into(),
        }))
        .await
        .unwrap()
    {
        Reply::EpicPrime(c) => otaq_core::epic_lead::prime::render(&c),
        other => panic!("{other:?}"),
    };
    (ticket, project, epic)
}

/// T-0339 (AGT-06): with four named agents, every prime lists them with tool, model as Settings shows it, "Use for"
/// and the project's default per role (shop-web overrides Developer → Strong); the Project agent and the lead are
/// told how to pick, the ticket's agent that it can't choose.
#[tokio::test(flavor = "multi_thread")]
async fn every_prime_lists_the_named_agents_and_the_projects_defaults() {
    let (w, agents) = with_agents().await;
    let add = |name: &str, model: &str, use_for: &str| AgentFields {
        name: name.into(),
        tool: Tool::ALL[0],
        model: Some(model.into()),
        use_for: use_for.into(),
    };
    let strong = agents
        .add(&add("Strong", "opus", "Complex features, tricky bugs"))
        .await
        .unwrap();
    agents
        .add(&add("Light", "haiku", "Typos, docs, small config"))
        .await
        .unwrap();
    agents.add(&add("Sonnet", "sonnet", "")).await.unwrap();
    agents
        .set_project_default("SHOP", Role::Developer, Some(&strong.id))
        .await
        .unwrap();

    let list =
        "- Claude Code · tool default · project default for Validator, Epic lead, Project agent
  Use for: Everything, until you add more agents
- Strong · Claude Code · Opus · always the newest · project default for Developer
  Use for: Complex features, tricky bugs
- Light · Claude Code · Haiku · always the newest
  Use for: Typos, docs, small config
- Sonnet · Claude Code · Sonnet · always the newest
";
    let (ticket, project, epic) = primes(&w).await;
    let section = |lead: &str| format!("\n## Agents\n\n{lead}\n\n{list}\n## Otaq's rules\n");
    assert!(
        project.contains(&section(
            "Pick one for each ticket you propose by how hard the work is, using its \"Use for\": `otaq create … \
             --developer <name> --reason \"…\"`, and `--validator <name>` only when you have a reason. When unsure, \
             leave the project default."
        )),
        "{project}"
    );
    assert!(
        epic.contains(&section(
            "Pick for never-started children by how hard the work is, using each \"Use for\": `otaq update <KEY> \
             --developer <name> --reason \"…\"`, and `--validator <name>` only when you have a reason. Never change a \
             row the user set, and never your own Epic lead row."
        )),
        "{epic}"
    );
    assert!(
        ticket.contains(&section(
            "Ticket agents can't choose agents: the user, a Project agent or the epic lead picks them. This list is \
             for reading only."
        )),
        "{ticket}"
    );
    w.stop().await;
}

/// T-0339 (d0086: "Until a second entry exists, agents have nothing to choose between"): with only the starter
/// entry, every prime says so in one line instead of the list.
#[tokio::test(flavor = "multi_thread")]
async fn one_named_agent_is_one_line() {
    let (w, _agents) = with_agents().await;
    let one = "\n## Agents\n\nOne agent (Claude Code · tool default): every ticket runs on it. Make no agent picks and \
               leave the Agent column out of proposals.\n\n## Otaq's rules\n";
    let (ticket, project, epic) = primes(&w).await;
    for text in [&ticket, &project, &epic] {
        assert!(text.contains(one), "{text}");
        assert!(!text.contains("Use for:"), "{text}");
    }
    w.stop().await;
}

/// The model as Settings shows it, from the tool's list.
#[test]
fn models_read_as_settings_shows_them() {
    let list = claude_list();
    assert_eq!(model_text(None, Some(&list)), "tool default");
    assert_eq!(model_text(Some("default"), Some(&list)), "tool default");
    assert_eq!(model_text(Some("default"), None), "tool default");
    assert_eq!(
        model_text(Some("opus"), Some(&list)),
        "Opus · always the newest"
    );
    assert_eq!(model_text(Some("opus"), None), "opus");
    assert_eq!(
        model_text(Some("claude-opus-5-5"), Some(&list)),
        "claude-opus-5-5"
    );
}
