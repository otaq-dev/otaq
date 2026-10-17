//! `otaq prime --epic` (T-0283; LEAD-03; d0083, d0084, d0040): the lead's context for SHOP-40 mid-flight. 40.1
//! merged with a recorded decision the lead marked; 40.2 and 40.4 running and changing the same file; 40.3 waiting
//! for 40.2; a question of 40.2 waiting for the lead and one of 40.4 for the user; an unread note to 40.4; a
//! dismissed suggestion on 40.6; 40.5 added by the lead; 40.4's first stop waiting for the lead. The request is
//! sent from inside the lead's session (an ordinary `sh` started as the lead) and by the user. No test starts
//! `claude`.

use std::path::Path;

use otaq_core::agent_tools::LaunchContext;
use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::board::epic_flight::{EpicPlace, EpicQueueMode};
use otaq_core::board::tickets::{NewTicketInput, Ticket};
use otaq_core::client::{Client, ClientConfig, ClientError};
use otaq_core::decisions::{self, NewDecision};
use otaq_core::domain::{Actor, Status};
use otaq_core::epic_lead::overlap::Overlap;
use otaq_core::epic_lead::prime::{EpicPrime, context, render, rules_line};
use otaq_core::epic_lead::suggest::{self, SuggestionKind, SuggestionState};
use otaq_core::epic_lead::{notes, questions::LeadQuestionState};
use otaq_core::git::testing::TempRepos;
use otaq_core::project_agents::lead::Lead;
use otaq_core::protocol::{KeyParams, Reply, Request};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::{AgentShell, TestService, path_without_build_folders};
use otaq_core::store::{new_id, now_ms};
use serde_json::{Value, json};

const PLAN: &str =
    "Signed-in users save items to a wishlist (PAY-123).\n\n1. API 2. Button 3. Page";
const SUMMARY: &str =
    "GET/POST /wishlist, items newest first.\nNotes for later tickets: the API returns `items[]`.";
const NOTE: &str =
    "40.1 returns items newest first (recorded decision on SHOP-40.1). Use the same order.";
const GUESTS: &str = "Can guests save items without logging in?";
const EXPIRY: &str = "Should share links expire?";
const STUCK: &str = "share.test.ts expects a 6-character token";
const MARK: &str = "PAY-123 says 'in the order added'";

fn me() -> Actor {
    Actor::User("me".into())
}

async fn child(w: &TestService, title: &str, by: &Actor) -> Ticket {
    w.board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: title.into(),
                epic: Some("SHOP-40".into()),
                ..Default::default()
            },
            by,
        )
        .await
        .unwrap()
}

fn write(wt: &Path, file: &str, content: &str) {
    std::fs::create_dir_all(wt.join(file).parent().unwrap()).unwrap();
    std::fs::write(wt.join(file), content).unwrap();
}

struct World {
    w: TestService,
    lead: Lead,
    lead_shell: AgentShell,
    _child_shell: AgentShell,
    question: String,
}

/// SHOP-40 mid-flight; `claude` is an interactive `sh`.
async fn world() -> World {
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
    let epic = w.board.ticket("SHOP-40").await.unwrap();
    sqlx::query("UPDATE tickets SET body = ? WHERE id = ?")
        .bind(PLAN)
        .bind(&epic.id)
        .execute(w.board.store().pool())
        .await
        .unwrap();
    let lead = w.project_agents.leads().open("SHOP-40").await.unwrap();
    let lead_actor = Actor::ProjectAgent(lead.agent.id.clone());
    let share = child(&w, "Wishlist share link", &me()).await;
    child(&w, "Wishlist empty state", &lead_actor).await;
    child(&w, "Wishlist export (CSV)", &me()).await;
    w.board
        .add_dependency("SHOP-40.3", "SHOP-40.2", &me())
        .await
        .unwrap();

    // 40.2 and 40.4 run in their own worktrees and both changed lib/wishlist.ts.
    let button = w.board.ticket("SHOP-40.2").await.unwrap();
    let button_wt = w.start_worktree(&button).await;
    let share_wt = w.start_worktree(&share).await;
    w.force_status(&button, Status::Working).await;
    w.force_status(&share, Status::Working).await;
    write(&button_wt, "lib/wishlist.ts", "export const button = 1;\n");
    write(&share_wt, "lib/wishlist.ts", "export const share = 1;\n");

    // 40.1 merged: its summary and a recorded decision the lead marked.
    let api = w.board.ticket("SHOP-40.1").await.unwrap();
    sqlx::query("INSERT INTO events (id, ticket_id, kind, actor, payload_json, at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(new_id())
        .bind(&api.id)
        .bind("review_submitted")
        .bind("agent:SHOP-40.1")
        .bind(json!({ "summary": SUMMARY }).to_string())
        .bind(now_ms())
        .execute(w.board.store().pool())
        .await
        .unwrap();
    decisions::record(
        &w.board,
        "SHOP-40.1",
        &NewDecision {
            question: "Which order do the items come in?".into(),
            built: "newest first".into(),
            other_options: vec!["in the order added".into()],
            why: "the usual order for saved lists".into(),
        },
        &Actor::Agent("SHOP-40.1".parse().unwrap()),
    )
    .await
    .unwrap();
    decisions::mark::mark(
        &w.board,
        "SHOP-40.1#1",
        "PAY-123",
        MARK,
        &lead.name,
        &lead_actor,
    )
    .await
    .unwrap();

    // 40.2's agent asks: the question goes to the live lead. 40.4's question waits for the user.
    let child_shell = w.agent_shell(&button).await;
    let asked = send(
        &w,
        &child_shell,
        "ticket.ask_human",
        json!({"key": "SHOP-40.2", "question": GUESTS, "options": ["Logged-in users only", "Guests too"]}),
    )
    .await;
    assert!(asked.get("error").is_none(), "{asked}");
    let question = otaq_core::epic_lead::questions::waiting(&w.board, &epic.id)
        .await
        .unwrap()
        .pop()
        .expect("a question waits for the lead")
        .id;
    w.board
        .store_question(&share.id, EXPIRY, &[], None)
        .await
        .unwrap();

    notes::leave(
        &w.board,
        &epic.id,
        &lead.agent.id,
        "SHOP-40.4",
        NOTE,
        None,
        &lead_actor,
    )
    .await
    .unwrap();
    let suggestion = suggest::suggest(
        &w.board,
        &epic.id,
        "SHOP-40.6",
        SuggestionKind::Close,
        "PAY-123 leaves export out.",
        &lead.name,
        &lead_actor,
    )
    .await
    .unwrap();
    suggest::resolve(&w.board, &suggestion, SuggestionState::Dismissed, &me())
        .await
        .unwrap();

    // 40.4's first stop waits for the lead's one note (T-0285's stored attempt).
    sqlx::query(
        "INSERT INTO lead_stuck (child_id, epic_id, lead_id, stop, detail, rounds, state, held_at)
         VALUES (?, ?, ?, 'validation_failed', ?, 3, 'held', ?)",
    )
    .bind(&share.id)
    .bind(&epic.id)
    .bind(&lead.agent.id)
    .bind(STUCK)
    .bind(now_ms())
    .execute(w.board.store().pool())
    .await
    .unwrap();

    let mut global = w.board.global_settings().await.unwrap();
    global.agent_instructions = "Use my /tdd skill.".into();
    w.board.set_global_settings(&global).await.unwrap();

    let lead_shell = w.shell_in(&lead.agent.session.as_ref().unwrap().id);
    World {
        w,
        lead,
        lead_shell,
        _child_shell: child_shell,
        question,
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

fn section<'a>(text: &'a str, heading: &str) -> &'a str {
    let start = text
        .find(&format!("\n## {heading}\n"))
        .unwrap_or_else(|| panic!("no {heading} section: {text}"));
    let rest = &text[start + 1..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |i| i + 3);
    &rest[..end]
}

#[tokio::test(flavor = "multi_thread")]
async fn shop_40_mid_flight_renders_every_section() {
    let World {
        w,
        lead,
        lead_shell,
        question,
        ..
    } = world().await;
    let ctx = context(&w.board, "SHOP-40").await.unwrap();
    let text = render(&ctx);
    let cmd = otaq_core::naming::COMMAND_NAME;
    let product = otaq_core::naming::PRODUCT_NAME;

    // The epic.
    assert_eq!(ctx.lead, "Wishlist lead");
    assert_eq!(ctx.epic.place, EpicPlace::Track);
    assert_eq!(ctx.epic.queue_mode, EpicQueueMode::None);
    assert!(ctx.epic.branch_created);
    assert!(
        text.starts_with(&format!(
            "# Epic lead · SHOP-40 · Wishlist\n\nYou are Wishlist lead, the {product} lead of epic SHOP-40."
        )),
        "{text}"
    );
    assert_eq!(
        section(&text, "The epic"),
        format!(
            "## The epic\n\n- External ID: PAY-123\n- Branch: `{}` from `main`\n- Place: a track in Epics in \
             flight\n- Queue: not queued\n\n### Description\n\n{PLAN}\n",
            ctx.epic.branch
        )
    );

    // Every child, in start order: the started ones, then the rest as they will start.
    let keys: Vec<String> = ctx.children.iter().map(|c| c.key.to_string()).collect();
    assert_eq!(keys[..3], ["SHOP-40.1", "SHOP-40.2", "SHOP-40.4"]);
    let mut rest = keys[3..].to_vec();
    rest.sort();
    assert_eq!(rest, ["SHOP-40.3", "SHOP-40.5", "SHOP-40.6"]);
    let children = section(&text, "Children, in start order");
    assert!(
        children.contains(
            "- SHOP-40.1 · Wishlist API (Merged · P3 · Normal) · started · added by you\n  Submit summary:\n  > \
             GET/POST /wishlist, items newest first.\n  > Notes for later tickets: the API returns `items[]`.\n  \
             Recorded decisions:\n  - SHOP-40.1#1 Which order do the items come in?\n    Built: newest first\n    \
             Other option: in the order added\n    Why: the usual order for saved lists\n    Marked by Wishlist \
             lead: contradicts PAY-123: PAY-123 says 'in the order added'\n"
        ),
        "{children}"
    );
    let button = ctx
        .children
        .iter()
        .find(|c| c.key.to_string() == "SHOP-40.2")
        .unwrap();
    let worktree = button.worktree.as_deref().expect("40.2's worktree");
    let branch = button.branch.as_deref().expect("40.2's branch");
    assert!(
        children.contains(&format!(
            "- SHOP-40.2 · Wishlist button (Working · P3 · Normal) · started · added by you\n  depends on \
             SHOP-40.1\n  worktree {worktree} · branch `{branch}`\n"
        )),
        "{children}"
    );
    assert!(
        children.contains(
            "- SHOP-40.3 · Wishlist page (Backlog · P3 · Normal) · never started · added by you\n  depends on \
             SHOP-40.2\n"
        ),
        "{children}"
    );
    assert!(
        children.contains("- SHOP-40.5 · Wishlist empty state (Backlog · P3 · Normal) · never started · added by Wishlist lead\n"),
        "{children}"
    );

    // The overlap.
    assert_eq!(
        ctx.overlaps,
        vec![Overlap {
            a: "SHOP-40.2".parse().unwrap(),
            b: "SHOP-40.4".parse().unwrap(),
            files: vec!["lib/wishlist.ts".into()],
        }]
    );
    assert_eq!(
        section(&text, "Same file"),
        "## Same file\n\n- SHOP-40.2 and SHOP-40.4: lib/wishlist.ts\n"
    );

    // Open questions: 40.2's waits for the lead, 40.4's for the user.
    assert_eq!(ctx.lead_questions.len(), 1);
    assert_eq!(ctx.lead_questions[0].state, LeadQuestionState::WithLead);
    assert_eq!(ctx.user_questions.len(), 1);
    let user_question = &ctx.user_questions[0].question.id;
    assert_eq!(
        section(&text, "Open questions"),
        format!(
            "## Open questions\n\nWaiting for you: answer with `{cmd} answer <ID> --stdin`, or pass it on with \
             `{cmd} ask human --pass-on <ID> --stdin`.\n\n- {question} from SHOP-40.2: {GUESTS} Options: 1. \
             Logged-in users only 2. Guests too\n\nWaiting for the user:\n\n- {user_question} from SHOP-40.4: \
             {EXPIRY}\n"
        )
    );

    // The unread note and the dismissed suggestion.
    assert_eq!(
        section(&text, "Your notes not read yet"),
        format!("## Your notes not read yet\n\n- to SHOP-40.4: {NOTE}\n")
    );
    assert_eq!(
        section(&text, "Your suggestions"),
        "## Your suggestions\n\n- closing SHOP-40.6 Wishlist export (CSV) · dismissed by the user; don't suggest \
         it again: PAY-123 leaves export out.\n"
    );

    // The stuck child.
    assert_eq!(
        section(&text, "Stuck children"),
        format!(
            "## Stuck children\n\nEach waits for one note from you (`{cmd} note <KEY> --stdin`), or goes to the \
             user.\n\n- SHOP-40.4 stopped: Validation failed 3 times\n  > {STUCK}\n"
        )
    );

    // The rule and the Agent instructions.
    assert_eq!(
        ctx.rules,
        "You may change only never-started children of SHOP-40; name a written source for every answer, note and mark."
    );
    assert_eq!(rules_line(&"SHOP-40".parse().unwrap()), ctx.rules);
    assert!(
        text.ends_with(&format!(
            "## {product}'s rules\n\n{}\n\n## Agent instructions (global)\n\nUse my /tdd skill.\n",
            ctx.rules
        )),
        "{text}"
    );

    // `--json` is the same data: from the lead's session and from the user's terminal.
    let from_lead = send(&w, &lead_shell, "lead.prime", json!({"key": "SHOP-40"})).await;
    assert!(from_lead.get("error").is_none(), "{from_lead}");
    let parsed: EpicPrime = serde_json::from_value(from_lead["reply"]["epic_prime"].clone())
        .unwrap_or_else(|e| panic!("{e}: {from_lead}"));
    assert_eq!(parsed.children, ctx.children);
    assert_eq!(parsed.overlaps, ctx.overlaps);
    assert_eq!(parsed.lead_questions, ctx.lead_questions);
    assert_eq!(parsed.user_questions, ctx.user_questions);
    assert_eq!(parsed.unread_notes, ctx.unread_notes);
    assert_eq!(parsed.suggestions, ctx.suggestions);
    assert_eq!(parsed.rules, ctx.rules);
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    match client
        .request(Request::LeadPrime(KeyParams {
            key: "SHOP-40".into(),
        }))
        .await
        .unwrap()
    {
        Reply::EpicPrime(by_user) => assert_eq!(by_user.epic, ctx.epic),
        other => panic!("{other:?}"),
    }
    // A ticket that isn't an epic is refused.
    match client
        .request(Request::LeadPrime(KeyParams {
            key: "SHOP-18".into(),
        }))
        .await
    {
        Err(ClientError::Service(e)) => assert_eq!(e.message, "SHOP-18 isn't an epic"),
        other => panic!("{other:?}"),
    }
    // The lead's mark reads with its name in the child's own prime too.
    let api = otaq_core::prime::render(&w.board.prime_context("SHOP-40.1").await.unwrap());
    assert!(
        api.contains(&format!("Marked by {}: contradicts PAY-123", lead.name)),
        "{api}"
    );
    w.stop().await;
}

/// Before anything started: a card in Backlog, the branch not created yet, and every empty section says so.
#[tokio::test(flavor = "multi_thread")]
async fn an_epic_that_never_started_says_what_is_empty() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let epic = w
        .board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Search".into(),
                kind: otaq_core::domain::Kind::Epic,
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    let ctx = context(&w.board, &epic.key.to_string()).await.unwrap();
    let text = render(&ctx);
    assert_eq!(ctx.epic.place, EpicPlace::Card);
    assert!(!ctx.epic.branch_created);
    assert!(
        text.contains(&format!(
            "- Branch: `{}` from `main` (created when the first child starts)\n",
            ctx.epic.branch
        )),
        "{text}"
    );
    for empty in [
        "### Description\n\n(no description)\n",
        "## Children, in start order\n\n(no children yet)\n",
        "## Same file\n\n(no running children changed the same file)\n",
        "## Open questions\n\n(none)\n",
        "## Your notes not read yet\n\n(none)\n",
        "## Your suggestions\n\n(none)\n",
    ] {
        assert!(text.contains(empty), "{empty}: {text}");
    }
    assert!(!text.contains("## Agent instructions"), "{text}");
    w.stop().await;
}
