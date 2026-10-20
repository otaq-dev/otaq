//! `otaq prime` output (T-0071, PLG-02; d0040): snapshots of the Markdown for sample tickets, with and without
//! instructions. T-0409 (AGM-06; d0108): the agent's instructions, then the project's notes, then the stage and its
//! skill, then the earlier stages; the parents' last stage summaries; the stage choices the reader may set, with their
//! models (none on the default flow).

use otaq_core::agent_tools::Tool;
use otaq_core::agent_tools::models::{ModelCopy, ModelEntry, ModelKind, ModelList, settings_key};
use otaq_core::board::tickets::TicketUpdate;
use otaq_core::client::{Client, ClientConfig};
use otaq_core::data_dir::DataDir;
use otaq_core::domain::{Actor, Status};
use otaq_core::flow::{FlowFile, yaml};
use otaq_core::git::testing::TempRepos;
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
    // SHOP-18's agent ended its Dev stage with a summary (its last Agent stage run).
    let shop18 = w.board.ticket("SHOP-18").await.unwrap();
    finished_run(
        &w,
        &shop18.id,
        "dev",
        "Developer",
        "POST /checkout creates a Stripe session.\nReturns the session URL.",
    )
    .await;
    w
}

/// An Agent stage run of `stage` that ended with `summary`, as `stage next` leaves it.
async fn finished_run(w: &TestService, ticket_id: &str, stage: &str, agent: &str, summary: &str) {
    let now = now_ms();
    sqlx::query(
        "INSERT INTO stage_runs (id, ticket_id, stage_id, kind, agent, result, summary, started_at, ended_at)
         VALUES (?, ?, ?, 'agent', ?, 'done', ?, ?, ?)",
    )
    .bind(new_id())
    .bind(ticket_id)
    .bind(stage)
    .bind(agent)
    .bind(summary)
    .bind(now)
    .bind(now)
    .execute(w.board.store().pool())
    .await
    .unwrap();
}

/// The test flow `tests/flows/every-kind.yaml`: Grill (skippable) → Dev (Quick developer an option) → Validate →
/// Security → QA (skippable) → Done.
fn every_kind_file() -> FlowFile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/flows/every-kind.yaml");
    yaml::read(&std::fs::read_to_string(path).unwrap()).unwrap()
}

const RULES: &str = "## Otaq's rules

1. Follow the Agent instructions and the project's notes above. They add to these rules; they can't skip getting context or ending your stage.
2. Commit everything with `/otaq-commit` before you end your stage. Never push and never open a PR: the user does that.
3. When your stage's work is done, as the ticket's description describes, end it: `otaq stage next --stdin` with a short summary of what you did on stdin; the next stage reads it. It is refused while files are uncommitted.
4. Only when there is nothing to deliver (no commits on the ticket's branch, for example it was already fixed elsewhere), end the ticket instead: `otaq stage done --stdin` with the reason on stdin. It is refused once the branch has commits.
5. When you are stuck or a decision is the user's, ask: `otaq ask human --stdin` with the question on stdin, then end your turn.
6. Long text (summaries, questions, ticket bodies) always goes through stdin (`--stdin`), never quoted arguments.
7. `otaq show <KEY>` reads any ticket; `otaq create \"Title\" --stdin` files a follow-up in Backlog.
";

/// The default flow's Dev, as the ticket prime names it.
const DEV: &str = "## Your stage\n\nDev, as Developer, with the skill `/otaq-work-ticket`.\n";

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

{DEV}
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

/// AGM-06 (Done when, the example's NOTE-4 in Dev): the prime gives Developer's instructions, then the project's
/// notes, then the stage and its skill; no stage choices on the default flow.
#[tokio::test(flavor = "multi_thread")]
async fn with_the_agents_instructions_and_the_projects_notes() {
    let w = sample().await;
    // AGM-02: the global Agent instructions are Developer's own now.
    w.edit_agent("Developer", |a| {
        a.instructions = "Use my /tdd skill.\nRun `pnpm test` before submitting.".into();
    })
    .await;

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
    assert!(
        text.contains(&format!(
            "\n## Agent instructions (Developer)\n\nUse my /tdd skill.\nRun `pnpm test` before submitting.\n\n{DEV}"
        )),
        "{text}"
    );
    assert!(text.ends_with(RULES), "{text}");

    // The project's notes come after the agent's own, labelled with the project (AGM-06).
    let mut project = w.board.project_settings("SHOP").await.unwrap();
    project.project_notes = "Ask before touching the payment code.".into();
    w.board
        .set_project_settings("SHOP", &project)
        .await
        .unwrap();
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let text = match client
        .request(Request::TicketPrime(KeyParams {
            key: "SHOP-20".into(),
        }))
        .await
        .unwrap()
    {
        Reply::Prime(c) => render(&c),
        other => panic!("{other:?}"),
    };
    let own = text
        .find("\n## Agent instructions (Developer)\n\nUse my /tdd skill.\nRun `pnpm test` before submitting.\n")
        .expect(&text);
    let notes = text
        .find("\n## Notes every agent in shop-web gets\n\nAsk before touching the payment code.\n")
        .expect(&text);
    let stage = text.find(DEV).expect(&text);
    let depends = text.find("\n## Depends on\n").expect(&text);
    assert!(own < notes && notes < stage && stage < depends, "{text}");
    assert!(!text.contains("## Stage choices"), "{text}");
    assert!(!text.contains("## Agents"), "{text}");

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

/// AGM-06 on the test flow: Dev's prime names Rust developer's instructions and its stage, lists Grill's run under
/// "Earlier stages" (its latest round, with its summary), and the later stages whose choice its agent may still set
/// (QA's skip); it names no commit skill, since Rust developer has none (`commit-compact` isn't `commit`).
#[tokio::test(flavor = "multi_thread")]
async fn on_the_test_flow_dev_reads_grills_summary() {
    let w = with_agents().await;
    w.board.use_flow_for_tests("SHOP", &every_kind_file()).await;
    let t = w.create("SHOP", "Sort notes by date").await;
    // Grill ran twice; the latest run's summary is the one passed on.
    finished_run(&w, &t.id, "grill", "Griller", "First round.").await;
    sqlx::query(
        "UPDATE stage_runs SET started_at = 1, ended_at = 2 WHERE summary = 'First round.'",
    )
    .execute(w.board.store().pool())
    .await
    .unwrap();
    finished_run(
        &w,
        &t.id,
        "grill",
        "Griller",
        "Agreed: newest first.\nDates in the user's time zone.",
    )
    .await;
    w.force_status(&t, Status::Working).await; // in Dev
    let key = t.key.to_string();
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let context = match client
        .request(Request::TicketPrime(KeyParams { key: key.clone() }))
        .await
        .unwrap()
    {
        Reply::Prime(c) => c,
        other => panic!("{other:?}"),
    };
    let text = render(&context);
    for part in [
        "\n## Agent instructions (Rust developer)\n\nFollow our error-handling rules: no unwrap in library code.\n",
        "\n## Your stage\n\nDev, as Rust developer, with the skill `/otaq-work-ticket`.\n",
        "\n## Earlier stages\n\n- Grill · Griller · finished\n  > Agreed: newest first.\n  > Dates in the user's time zone.\n",
        "\n## Stage choices\n\nYou may change these later stages' choices for this ticket",
        &format!("`otaq update {key} --stage \"QA=skip\" --reason \"…\"`"),
        "\n- QA (now: required)\n  - skip: the ticket passes this stage\n",
        "2. Commit everything with git (`git add -A`, `git commit`) before you end your stage.",
    ] {
        assert!(text.contains(part), "missing {part:?} in\n{text}");
    }
    assert!(
        !text.contains("First round."),
        "only the latest run: {text}"
    );
    assert!(!text.contains("- Dev ("), "Dev has started: {text}");
    assert_eq!(context.commit_skill, "");
    w.stop().await;
}

/// AGM-06, AGM-08 on the test flow: the Project agent's prime lists every stage with options, each agent with its
/// model and its instructions' first line, and how to set a choice; the epic lead's the same, for its children.
#[tokio::test(flavor = "multi_thread")]
async fn on_the_test_flow_the_project_agent_reads_the_stage_options() {
    let w = with_agents().await;
    w.board.use_flow_for_tests("SHOP", &every_kind_file()).await;
    let (_, project, epic) = primes(&w).await;
    let default = match w.board.store().default_model().await.unwrap() {
        Some(m) => format!(
            "default · {} · {}",
            m.tool.display_name(),
            model_text(m.model.as_deref(), Some(&claude_list()))
        ),
        None => "default".to_owned(),
    };
    let options = format!(
        "- Grill\n  - Griller (the stage's own) · {default}: Ask one question at a time.\n  - skip: the ticket passes \
         this stage\n- Dev\n  - Rust developer (the stage's own) · {default}: Follow our error-handling rules: no \
         unwrap in library code.\n  - Quick developer · {default}\n- QA\n  - skip: the ticket passes this stage\n"
    );
    assert!(
        project.contains(&format!(
            "\n## Stage choices\n\nA new ticket runs each stage's own agent. Pick another option for a ticket you \
             create only when how hard the work is says so, with a reason: `otaq create \"Title\" --stage \
             \"Grill=skip\" --reason \"…\" --stdin`. When unsure, leave the stage's own agent.\n\n{options}"
        )),
        "{project}"
    );
    assert!(
        epic.contains(&format!(
            "\n## Stage choices\n\nSet them on never-started children only, when how hard the work is says so, \
             always with a reason: `otaq update <KEY> --stage \"Grill=skip\" --reason \"…\"`, or on `otaq create \
             --parent`. Never change a choice the user set.\n\n{options}"
        )),
        "{epic}"
    );
    w.stop().await;
}

/// T-0184 (VAL-10): `prime --validate` for a plain ticket: the ticket, the summary, the parents with theirs, the
/// Agent instructions, the diff and how to report.
#[tokio::test(flavor = "multi_thread")]
async fn validate_prime_plain() {
    let w = sample().await;
    // The check's own agent's instructions (AGM-06), not Developer's.
    w.edit_agent("Validator", |a| {
        a.instructions = "Run `pnpm test` before submitting.".into();
    })
    .await;
    let ctx = w.board.validate_prime("SHOP-20").await.unwrap();
    assert_eq!((ctx.round, ctx.rounds), (1, 3));
    let text = otaq_core::prime::render_validate(&ctx);
    for part in [
        "# Validate SHOP-20 · Checkout page\n",
        "a read-only judge of its agent's work (round 1 of 3). Never edit files, never commit and never fix anything.",
        "## What and why\n\nThe checkout page shows the cart",
        "## Agent instructions (Validator)\n\nRun `pnpm test` before submitting.",
        "## Linked tickets",
        "- depends on SHOP-18 · Checkout API (Ready for Review)\n  > POST /checkout creates a Stripe session.",
        "- depends on SHOP-19 · Price table (Working)",
        "## The diff\n\n```diff\n",
        "`otaq check pass --stdin` or `otaq check fail --stdin`",
    ] {
        assert!(text.contains(part), "missing {part:?} in\n{text}");
    }
    assert!(!text.contains("## The user's review"), "never sent back");
    assert!(
        !text.contains("## Earlier stages"),
        "no stage ran yet: {text}"
    );
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
        family: None,
        released_at: None,
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

/// The sample board on a service whose copy of Claude Code's list was read earlier.
async fn with_agents() -> TestService {
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
    w
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

/// AGM-06: the default flow has no stage options, so no prime lists choices to set.
#[tokio::test(flavor = "multi_thread")]
async fn no_prime_lists_choices_on_the_default_flow() {
    let w = with_agents().await;
    let (ticket, project, epic) = primes(&w).await;
    for text in [&ticket, &project, &epic] {
        assert!(!text.contains("\n## Stage choices\n"), "{text}");
        assert!(!text.contains("\n## Agents\n"), "{text}");
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
