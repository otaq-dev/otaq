//! `otaq prime` output (T-0071, PLG-02; d0040): snapshots of the Markdown for sample tickets, with and without
//! Agent instructions.

use otaq_core::board::tickets::TicketUpdate;
use otaq_core::domain::Actor;
use otaq_core::prime::render;
use otaq_core::service::testing::TestService;
use otaq_core::store::{new_id, now_ms};

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
