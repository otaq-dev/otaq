//! `crewboard hook <event>` (T-0066, PLG-04, ATT-01, ATT-04): real-shaped hook JSON (Claude Code hooks reference,
//! 2026-10-10) fed to the command inside an agent session (an ordinary `sh`), for every row of SDD §11.4.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crewboard_core::data_dir::DataDir;
use crewboard_core::domain::Status;
use crewboard_core::naming;
use crewboard_core::service::testing::{AgentShell, TestService};
use serde_json::{Value, json};

const CB: &str = env!("CARGO_BIN_EXE_crewboard");

async fn hook(agent: &AgentShell, event: &str, payload: Value) {
    let ran = agent
        .run_with_stdin(&format!("'{CB}' hook {event}"), &payload.to_string())
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    assert_eq!(ran.stdout, "", "hooks print nothing");
}

fn common(event: &str) -> Value {
    json!({"session_id": "0199d2a4-3b1c-7c3e-9f00-123456789abc",
           "transcript_path": "/Users/me/.claude/projects/x/0199d2a4.jsonl",
           "cwd": "/Users/me/.crewboard/wt/shop-web/SHOP-1", "permission_mode": "default",
           "hook_event_name": event})
}

fn with(event: &str, fields: Value) -> Value {
    let mut payload = common(event);
    for (k, v) in fields.as_object().unwrap() {
        payload[k] = v.clone();
    }
    payload
}

#[tokio::test(flavor = "multi_thread")]
async fn every_row_of_the_hook_mapping() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    w.force_status(&shop1, Status::Working).await;
    let agent = w.agent_shell(&shop1).await;

    // Notification · permission prompt → Needs you.
    let permission = with(
        "Notification",
        json!({"message": "Claude needs your permission to use Bash", "title": "Permission needed",
               "notification_type": "permission_prompt"}),
    );
    hook(&agent, "Notification", permission).await;
    assert_eq!(w.status("SHOP-1").await, Status::NeedsYou);

    // UserPromptSubmit → back to working.
    hook(
        &agent,
        "UserPromptSubmit",
        with("UserPromptSubmit", json!({"prompt": "1"})),
    )
    .await;
    assert_eq!(w.status("SHOP-1").await, Status::Working);

    // Notification · idle_prompt (and other types) → ignored.
    for kind in ["idle_prompt", "auth_success"] {
        let other = with(
            "Notification",
            json!({"message": "Claude is waiting for your input", "notification_type": kind}),
        );
        hook(&agent, "Notification", other).await;
        assert_eq!(w.status("SHOP-1").await, Status::Working, "{kind}");
    }

    // Notification · permission prompt inside a helper agent → Needs you.
    let helper = with(
        "Notification",
        json!({"message": "Claude needs your permission to use Bash", "notification_type": "permission_prompt",
               "agent_id": "a-17", "agent_type": "general-purpose"}),
    );
    hook(&agent, "Notification", helper).await;
    assert_eq!(w.status("SHOP-1").await, Status::NeedsYou);
    hook(
        &agent,
        "UserPromptSubmit",
        with("UserPromptSubmit", json!({"prompt": "go on"})),
    )
    .await;

    // Stop with background tasks → Working in background.
    let background = with(
        "Stop",
        json!({"stop_hook_active": false, "last_assistant_message": "Running the tests in the background.",
               "background_tasks": [{"id": "task-001", "type": "shell", "status": "running",
                                     "description": "test run", "command": "npm test"}],
               "session_crons": []}),
    );
    hook(&agent, "Stop", background).await;
    assert_eq!(w.status("SHOP-1").await, Status::WorkingInBackground);

    // A new turn (re-invoked after the background work), then Stop with nothing pending → Idle.
    hook(
        &agent,
        "UserPromptSubmit",
        with("UserPromptSubmit", json!({"prompt": "<task-notification>"})),
    )
    .await;
    let idle = with(
        "Stop",
        json!({"stop_hook_active": false, "last_assistant_message": "Should I also update the tests?",
               "background_tasks": [], "session_crons": []}),
    );
    hook(&agent, "Stop", idle.clone()).await;
    assert_eq!(w.status("SHOP-1").await, Status::Idle);

    // Stop with a scheduled wakeup → Working in background.
    hook(
        &agent,
        "UserPromptSubmit",
        with("UserPromptSubmit", json!({"prompt": "wait for CI"})),
    )
    .await;
    let cron = with(
        "Stop",
        json!({"background_tasks": [], "session_crons": [{"id": "cron-001", "schedule": "10 16 * * *",
               "recurring": false, "prompt": "check the build"}]}),
    );
    hook(&agent, "Stop", cron).await;
    assert_eq!(w.status("SHOP-1").await, Status::WorkingInBackground);

    // Stop after `ask human` / `review submit` this turn → not Idle.
    hook(
        &agent,
        "UserPromptSubmit",
        with("UserPromptSubmit", json!({"prompt": "continue"})),
    )
    .await;
    w.signals.mark_submitted_or_asked(&agent.session);
    hook(&agent, "Stop", idle.clone()).await;
    assert_eq!(w.status("SHOP-1").await, Status::Working);
    // …and the fact resets with the next turn.
    hook(
        &agent,
        "UserPromptSubmit",
        with("UserPromptSubmit", json!({"prompt": "next"})),
    )
    .await;
    hook(&agent, "Stop", idle).await;
    assert_eq!(w.status("SHOP-1").await, Status::Idle);

    // StopFailure that isn't the plan limit → logged only.
    let failure = with(
        "StopFailure",
        json!({"error": "server_error", "error_details": "500 Internal Server Error",
               "last_assistant_message": "API Error: 500"}),
    );
    hook(&agent, "StopFailure", failure).await;
    assert_eq!(w.status("SHOP-1").await, Status::Idle);

    // StopFailure with rate_limit and the limit text → Waiting for limit reset (M8, T-0128; tests/limit.rs).
    let limit = with(
        "StopFailure",
        json!({"error": "rate_limit", "error_details": "429 Too Many Requests",
               "last_assistant_message": "You've hit your session limit · resets 3:45pm"}),
    );
    hook(&agent, "StopFailure", limit).await;
    assert_eq!(w.status("SHOP-1").await, Status::WaitingForLimitReset);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_ticket_variable_the_hook_does_nothing() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    w.force_status(&shop1, Status::Working).await;
    let agent = w.agent_shell(&shop1).await;
    let permission = with(
        "Notification",
        json!({"message": "Claude needs your permission", "notification_type": "permission_prompt"}),
    );
    let ran = agent
        .run_with_stdin(
            &format!(
                "env -u {} '{CB}' hook Notification",
                naming::ticket_env_var()
            ),
            &permission.to_string(),
        )
        .await;
    assert_eq!(
        (ran.status, ran.stdout.as_str(), ran.stderr.as_str()),
        (0, "", "")
    );
    assert_eq!(w.status("SHOP-1").await, Status::Working);

    // Outside any agent session (the plugin used by hand in the user's terminal): nothing changes either.
    let mut child = Command::new(CB)
        .args(["hook", "Notification"])
        .env(DataDir::env_var(), w.data.root())
        .env(naming::ticket_env_var(), "SHOP-1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(permission.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    assert_eq!(w.status("SHOP-1").await, Status::Working);
    w.stop().await;
}

#[test]
fn an_unreachable_service_never_blocks_the_agent() {
    let home = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let out = Command::new(CB)
        .args(["hook", "Stop"])
        .env(DataDir::env_var(), home.path())
        .env(naming::ticket_env_var(), "SHOP-1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
    assert!(started.elapsed() < Duration::from_secs(3));
    // Unknown events and a missing variable are silent too.
    let out = Command::new(CB)
        .args(["hook", "SessionStart"])
        .env(DataDir::env_var(), home.path())
        .env(naming::ticket_env_var(), "SHOP-1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success() && out.stdout.is_empty());
    let out = Command::new(CB)
        .args(["hook", "Stop"])
        .env(DataDir::env_var(), home.path())
        .env_remove(naming::ticket_env_var())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success() && out.stdout.is_empty());
}

/// A message waiting for SHOP-1 is typed as soon as its agent's `Stop` hook makes it idle (T-0067, SES-06).
#[tokio::test(flavor = "multi_thread")]
async fn the_stop_hook_delivers_waiting_messages() {
    use crewboard_core::session::deliveries::DeliveryKind;
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    w.force_status(&shop1, Status::Working).await;
    let agent = w.agent_shell(&shop1).await;
    let marker = w.home.path().join("delivered");
    w.deliveries
        .queue_delivery(
            &agent.session,
            DeliveryKind::TicketChange,
            &format!("touch '{}'", marker.display()),
        )
        .await
        .unwrap();
    hook(
        &agent,
        "Stop",
        with("Stop", json!({"background_tasks": [], "session_crons": []})),
    )
    .await;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() {
        assert!(Instant::now() < deadline, "never delivered");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    w.stop().await;
}

/// The plugin's hook commands run as Claude Code runs them (`sh -c`): through `crewboard hook <event>` when the
/// command is on PATH, and as a silent no-op when it isn't (T-0073).
#[tokio::test(flavor = "multi_thread")]
async fn the_plugin_hook_commands_run_under_sh() {
    let hooks: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../plugin/hooks/hooks.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let stop = hooks["hooks"]["Stop"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .to_owned();
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    w.force_status(&shop1, Status::Working).await;
    let agent = w.agent_shell(&shop1).await;
    let bin = std::path::Path::new(CB)
        .parent()
        .unwrap()
        .display()
        .to_string();
    let payload = with("Stop", json!({"background_tasks": [], "session_crons": []}));
    let ran = agent
        .run_with_stdin(
            &format!("PATH='{bin}':/usr/bin:/bin sh -c '{stop}'"),
            &payload.to_string(),
        )
        .await;
    assert_eq!((ran.status, ran.stdout.as_str()), (0, ""), "{ran:?}");
    assert_eq!(w.status("SHOP-1").await, Status::Idle);
    let ran = agent
        .run_with_stdin(
            &format!("PATH=/usr/bin:/bin sh -c '{stop}'"),
            &payload.to_string(),
        )
        .await;
    assert_eq!(
        (ran.status, ran.stdout.as_str(), ran.stderr.as_str()),
        (0, "", ""),
        "{ran:?}"
    );
    w.stop().await;
}
