//! `crewboard prime` output (T-0071, PLG-02; d0040): snapshots of the Markdown for sample tickets, with and without
//! Agent instructions.

use crewboard_core::board::tickets::TicketUpdate;
use crewboard_core::domain::Actor;
use crewboard_core::prime::render;
use crewboard_core::service::testing::TestService;
use crewboard_core::store::{new_id, now_ms};

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

const RULES: &str = "## Crewboard's rules

1. Follow the Agent instructions below. They add to these rules; they can't skip getting context or submitting for review.
2. Commit everything with `/crewboard:commit` before you submit. Never push and never open a PR: the user does that.
3. When the ticket is done, as its description describes, submit: `crewboard review submit --stdin` with a short summary of what you changed on stdin. It is refused while files are uncommitted.
4. When you are stuck or a decision is the user's, ask: `crewboard ask human --stdin` with the question on stdin, then end your turn.
5. Long text (summaries, questions, ticket bodies) always goes through stdin (`--stdin`), never quoted arguments.
6. `crewboard show <KEY>` reads any ticket; `crewboard create \"Title\" --stdin` files a follow-up in Backlog.
";

#[tokio::test(flavor = "multi_thread")]
async fn without_instructions() {
    let w = sample().await;
    let context = w.board.prime_context("SHOP-20").await.unwrap();
    assert_eq!(
        render(&context),
        format!(
            "# SHOP-20 · Checkout page

You are the agent for Crewboard ticket SHOP-20, working in its own worktree on branch `crewboard/SHOP-20`. It merges into `main` once the user approves it.
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

    let epic_child = w.board.prime_context("SHOP-40.2").await.unwrap();
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
    let text = render(&w.board.prime_context("SHOP-20").await.unwrap());
    assert!(
        text.ends_with(
            "## Agent instructions (project shop-web)\n\nAsk before touching the payment code.\n"
        ),
        "{text}"
    );
    assert!(!text.contains("/tdd"));

    // Review feedback and pending questions render once they exist (M6, M10).
    let mut context = w.board.prime_context("SHOP-20").await.unwrap();
    context.review_feedback = vec!["src/pay.ts:12 · handle a declined card".into()];
    context.pending_questions = vec!["SHOP-21 asks: which email template?".into()];
    let text = render(&context);
    assert!(text.contains("\n## Review feedback\n\n- src/pay.ts:12 · handle a declined card\n"));
    assert!(text.contains("\n## Pending questions\n\n- SHOP-21 asks: which email template?\n"));
    w.stop().await;
}
