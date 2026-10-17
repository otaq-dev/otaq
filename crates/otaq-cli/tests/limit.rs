//! The plan limit being hit (T-0128, USG-03, USG-04; d0020, d0046): real-shaped `StopFailure` JSON (Claude Code
//! hooks reference and error reference, 2026-10-10) fed to `otaq hook --tool claude StopFailure` inside an agent
//! session (an ordinary `sh`), for the session limit, the weekly limit, a limit with no readable time and a failure
//! that isn't the limit.

use otaq_core::domain::Status;
use otaq_core::service::testing::{AgentShell, TestService};
use otaq_core::store::now_ms;
use otaq_core::usage::limit::{LimitKind, ResetSource};
use otaq_core::usage::plan::{self, RateLimits, Window};
use serde_json::{Value, json};

const CB: &str = env!("CARGO_BIN_EXE_otaq");

async fn hook(agent: &AgentShell, payload: Value) {
    let ran = agent
        .run_with_stdin(
            &format!("'{CB}' hook --tool claude StopFailure"),
            &payload.to_string(),
        )
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    assert_eq!(ran.stdout, "", "hooks print nothing");
}

fn stop_failure(error: &str, message: &str) -> Value {
    json!({"session_id": "0199d2a4-3b1c-7c3e-9f00-123456789abc",
           "transcript_path": "/Users/me/.claude/projects/x/0199d2a4.jsonl",
           "cwd": "/Users/me/.otaq/wt/shop-web/SHOP-1", "permission_mode": "default",
           "hook_event_name": "StopFailure", "error": error, "error_details": "429 Too Many Requests",
           "last_assistant_message": message})
}

/// Waits for the limit to be recorded (the service records it right after the hook answers).
async fn limit(w: &TestService) -> plan::PlanLimits {
    for _ in 0..200 {
        if let Some(state) = w.plan_limit.state().await.unwrap()
            && state.limit_active()
        {
            return state;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("no limit recorded")
}

#[tokio::test(flavor = "multi_thread")]
async fn the_session_limit_waits_for_its_reset_from_the_status_line() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    w.force_status(&shop1, Status::Working).await;
    let agent = w.agent_shell(&shop1).await;
    // The status line already said when the 5-hour window resets.
    let resets = now_ms() + 80 * 60_000;
    w.plan_limit
        .record_windows(
            &RateLimits {
                five_hour: Some(Window {
                    used_percentage: 100.0,
                    resets_at: resets,
                }),
                seven_day: Some(Window {
                    used_percentage: 40.0,
                    resets_at: now_ms() + 3 * 86_400_000,
                }),
            },
            now_ms(),
        )
        .await
        .unwrap();

    hook(
        &agent,
        stop_failure(
            "rate_limit",
            "You've hit your session limit · resets 3:45pm",
        ),
    )
    .await;
    assert_eq!(w.status("SHOP-1").await, Status::WaitingForLimitReset);
    let state = limit(&w).await;
    let active = state.limit.unwrap();
    assert_eq!(active.kind, LimitKind::Session);
    assert_eq!(active.resets_at, Some(resets));
    assert_eq!(active.reset_source, Some(ResetSource::StatusLine));
    assert_eq!(state.observed_at, active.hit_at);
    // Neutral: no inbox item for it.
    assert!(w.inbox.list().await.unwrap().items.is_empty());

    // The window passes its reset and the status line shows room again: marked as reset, nothing resumes.
    assert!(!w.plan_limit.check_reset(resets - 1).await.unwrap());
    w.plan_limit
        .record_windows(
            &RateLimits {
                five_hour: Some(Window {
                    used_percentage: 3.0,
                    resets_at: resets + 5 * 3_600_000,
                }),
                seven_day: None,
            },
            resets + 1000,
        )
        .await
        .unwrap();
    let after = w.plan_limit.state().await.unwrap().unwrap();
    assert_eq!(after.limit.unwrap().reset_seen_at, Some(resets + 1000));
    assert_eq!(w.status("SHOP-1").await, Status::WaitingForLimitReset);

    // Claude Code continues by itself after the reset (its own setting, Q27): the new turn makes it Working.
    let ran = agent
        .run_with_stdin(
            &format!("'{CB}' hook --tool claude UserPromptSubmit"),
            &json!({"hook_event_name": "UserPromptSubmit", "prompt": "continue"}).to_string(),
        )
        .await;
    assert_eq!(ran.status, 0);
    assert_eq!(w.status("SHOP-1").await, Status::Working);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_weekly_limit_reads_its_reset_from_the_text_and_an_unreadable_one_is_unknown() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Coupon codes").await;
    let shop2 = w.create("SHOP", "Order email").await;
    w.force_status(&shop1, Status::Working).await;
    w.force_status(&shop2, Status::Idle).await;
    let a1 = w.agent_shell(&shop1).await;
    let a2 = w.agent_shell(&shop2).await;

    hook(
        &a1,
        stop_failure(
            "rate_limit",
            "You've hit your weekly limit · resets Mon 12:00am",
        ),
    )
    .await;
    let active = limit(&w).await.limit.unwrap();
    assert_eq!(active.kind, LimitKind::Weekly);
    assert_eq!(active.reset_source, Some(ResetSource::LimitText));
    let at = active.resets_at.unwrap();
    assert!(at > now_ms() && at <= now_ms() + 8 * 86_400_000);
    // A second agent stopped by the same limit: one limit, both waiting.
    hook(
        &a2,
        stop_failure(
            "rate_limit",
            "You've hit your weekly limit · resets Mon 12:00am",
        ),
    )
    .await;
    assert_eq!(w.status("SHOP-2").await, Status::WaitingForLimitReset);
    assert_eq!(limit(&w).await.limit.unwrap().hit_at, active.hit_at);
    w.stop().await;

    // No time in the text and no status line (an enterprise plan): "resets at an unknown time".
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Invoices").await;
    w.force_status(&shop1, Status::Working).await;
    let agent = w.agent_shell(&shop1).await;
    hook(
        &agent,
        stop_failure("rate_limit", "You've hit your individual usage limit"),
    )
    .await;
    let active = limit(&w).await.limit.unwrap();
    assert_eq!(active.kind, LimitKind::Other);
    assert_eq!(active.resets_at, None);
    assert_eq!(active.reset_source, None);
    assert_eq!(w.status("SHOP-1").await, Status::WaitingForLimitReset);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn other_failures_leave_the_limit_and_the_status_alone() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Checkout API").await;
    w.force_status(&shop1, Status::Working).await;
    let agent = w.agent_shell(&shop1).await;
    for (error, message) in [
        ("rate_limit", "API Error: Rate limit reached"),
        (
            "rate_limit",
            "API Error: Request rejected (429) · this may be a temporary capacity issue.",
        ),
        ("overloaded", "API Error: Overloaded"),
        ("server_error", "API Error: 500"),
    ] {
        hook(&agent, stop_failure(error, message)).await;
        assert_eq!(w.status("SHOP-1").await, Status::Working, "{message}");
    }
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(w.plan_limit.state().await.unwrap(), None);
    w.stop().await;
}
