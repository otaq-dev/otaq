//! `otaq hook --tool claude <event>` (T-0066, T-0310; PLG-04, ADP-03, ATT-01, ATT-04): the saved Claude Code
//! payloads (`otaq-core/tests/fixtures/claude/hooks/`) fed to the command inside an agent session (an ordinary
//! `sh`), for every row of SDD §11.4.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use otaq_core::data_dir::DataDir;
use otaq_core::domain::Status;
use otaq_core::naming;
use otaq_core::service::testing::{AgentShell, TestService};
use serde_json::Value;

const CB: &str = env!("CARGO_BIN_EXE_otaq");

/// A saved Claude Code hook payload.
fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/../otaq-core/tests/fixtures/claude/hooks/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// Runs the hook of the saved payload `name` (its event is the payload's `hook_event_name`) in `agent`'s session.
async fn hook(agent: &AgentShell, name: &str) {
    let payload = fixture(name);
    let event = payload["hook_event_name"].as_str().unwrap();
    let ran = agent
        .run_with_stdin(
            &format!("'{CB}' hook --tool claude {event}"),
            &payload.to_string(),
        )
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    assert_eq!(ran.stdout, "", "hooks print nothing");
}

#[tokio::test(flavor = "multi_thread")]
async fn every_row_of_the_hook_mapping() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    w.force_status(&shop1, Status::Working).await;
    let agent = w.agent_shell(&shop1).await;

    // Notification · permission prompt → Needs you.
    hook(&agent, "Notification-permission_prompt.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::NeedsYou);

    // UserPromptSubmit → back to working.
    hook(&agent, "UserPromptSubmit.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::Working);

    // Notification · elicitation → Needs you.
    hook(&agent, "Notification-elicitation_dialog.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::NeedsYou);
    hook(&agent, "UserPromptSubmit.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::Working);

    // Notification · idle_prompt (and other types) → ignored.
    for file in [
        "Notification-idle_prompt.json",
        "Notification-auth_success.json",
    ] {
        hook(&agent, file).await;
        assert_eq!(w.status("SHOP-1").await, Status::Working, "{file}");
    }

    // Notification · permission prompt inside a helper agent → Needs you.
    hook(&agent, "Notification-helper_agent.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::NeedsYou);
    hook(&agent, "UserPromptSubmit.json").await;

    // Stop with background tasks → Working in background.
    hook(&agent, "Stop-background.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::WorkingInBackground);

    // A new turn (re-invoked after the background work), then Stop with nothing pending → Idle.
    hook(&agent, "UserPromptSubmit.json").await;
    hook(&agent, "Stop-idle.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::Idle);

    // Stop with a scheduled wakeup → Working in background.
    hook(&agent, "UserPromptSubmit.json").await;
    hook(&agent, "Stop-cron.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::WorkingInBackground);

    // Stop after `ask human` / `review submit` this turn → not Idle.
    hook(&agent, "UserPromptSubmit.json").await;
    w.signals.mark_submitted_or_asked(&agent.session);
    hook(&agent, "Stop-idle.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::Working);
    // …and the fact resets with the next turn.
    hook(&agent, "UserPromptSubmit.json").await;
    hook(&agent, "Stop-idle.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::Idle);

    // StopFailure that isn't the plan limit (another error, or a 429 without the limit text) → logged only.
    for file in ["StopFailure-other.json", "StopFailure-capacity.json"] {
        hook(&agent, file).await;
        assert_eq!(w.status("SHOP-1").await, Status::Idle, "{file}");
    }

    // StopFailure with rate_limit and the limit text → Waiting for limit reset (M8, T-0128; tests/limit.rs).
    hook(&agent, "StopFailure-session_limit.json").await;
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
    let permission = fixture("Notification-permission_prompt.json");
    let ran = agent
        .run_with_stdin(
            &format!(
                "env -u {} '{CB}' hook --tool claude Notification",
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
        .args(["hook", "--tool", "claude", "Notification"])
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
        .args(["hook", "--tool", "claude", "Stop"])
        .env(DataDir::env_var(), home.path())
        .env(naming::ticket_env_var(), "SHOP-1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
    assert!(started.elapsed() < Duration::from_secs(3));
    // Unknown events, unknown tools and a missing variable are silent too.
    let out = Command::new(CB)
        .args(["hook", "--tool", "claude", "SessionStart"])
        .env(DataDir::env_var(), home.path())
        .env(naming::ticket_env_var(), "SHOP-1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success() && out.stdout.is_empty());
    let out = Command::new(CB)
        .args(["hook", "--tool", "codex", "Stop"])
        .env(DataDir::env_var(), home.path())
        .env(naming::ticket_env_var(), "SHOP-1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success() && out.stdout.is_empty() && out.stderr.is_empty());
    let out = Command::new(CB)
        .args(["hook", "--tool", "claude", "Stop"])
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
    use otaq_core::session::deliveries::DeliveryKind;
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
    hook(&agent, "Stop-idle.json").await;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() {
        assert!(Instant::now() < deadline, "never delivered");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    w.stop().await;
}

/// The plugin's hook commands run as Claude Code runs them (`sh -c`): through `otaq hook --tool claude <event>` when the
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
    let payload = fixture("Stop-idle.json");
    // Git for Windows' sh takes PATH entries in its own form (`/c/Users/…`).
    let bin = if cfg!(windows) {
        format!("\"$(cygpath -u '{bin}')\"")
    } else {
        format!("'{bin}'")
    };
    let ran = agent
        .run_with_stdin(
            &format!("PATH={bin}:/usr/bin:/bin sh -c '{stop}'"),
            &payload.to_string(),
        )
        .await;
    assert_eq!((ran.status, ran.stdout.as_str()), (0, ""), "{ran:?}");
    assert_eq!(w.status("SHOP-1").await, Status::Idle, "{ran:?}");
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

/// Waits until SHOP-1 shows `status`.
async fn wait_status(w: &TestService, key: &str, status: Status) {
    let start = Instant::now();
    while w.status(key).await != status {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{key} never became {status}, it is {}",
            w.status(key).await
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// T-0186 (§11.4 M12 rows, VAL-07, VAL-11): the validator's hooks, fed saved payloads inside a validator session
/// (an ordinary `sh`), and the agent's while its ticket validates.
#[tokio::test(flavor = "multi_thread")]
async fn validator_and_agent_hooks_while_validating() {
    use otaq_core::session::SessionKind;
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    w.give_worktree(&shop1).await;
    let validator = w.shell(&shop1, SessionKind::Validator).await;
    let agent = w.agent_shell(&shop1).await;
    w.validating_with(&shop1, &validator.session).await;
    // The agent's Stop: still Validating, never Idle.
    hook(&agent, "Stop-idle.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::Validating);
    // The agent's permission prompt still turns it turquoise; its next prompt brings back Validating….
    hook(&agent, "Notification-permission_prompt.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::NeedsYou);
    hook(&agent, "UserPromptSubmit.json").await;
    assert_eq!(w.status("SHOP-1").await, Status::Validating);

    // The validator's permission prompt → Needs you · validator; typing into it → Validating again.
    hook(&validator, "Notification-permission_prompt.json").await;
    wait_status(&w, "SHOP-1", Status::ValidatorNeedsYou).await;
    hook(&validator, "UserPromptSubmit.json").await;
    wait_status(&w, "SHOP-1", Status::Validating).await;

    // The usage limit → Waiting for limit reset (the validation restarts on Resume all, T-0188).
    hook(&validator, "StopFailure-session_limit.json").await;
    wait_status(&w, "SHOP-1", Status::WaitingForLimitReset).await;
    w.force_status(&shop1, Status::Validating).await;

    // Its turn ends with no verdict → Validation ended without a result; the validator is stopped (typed, not
    // awaited: the command's own session ends).
    let stop = fixture("Stop-idle.json");
    let line = format!("printf '%s' '{stop}' | '{CB}' hook --tool claude Stop\r");
    w.sessions
        .input(&validator.session, line.as_bytes())
        .unwrap();
    wait_status(&w, "SHOP-1", Status::ValidationNoResult).await;
    w.stop().await;
}

/// Outside Otaq's sessions (no ticket, no role) a hook does nothing at all.
#[tokio::test(flavor = "multi_thread")]
async fn no_ticket_and_no_role_is_a_no_op() {
    let w = TestService::start().await;
    let mut child = Command::new(CB)
        .args(["hook", "--tool", "claude", "Stop"])
        .env(DataDir::env_var(), w.data.root())
        .env_remove(naming::ticket_env_var())
        .env_remove(naming::role_env_var())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    w.stop().await;
}

/// `--tool` is required (d0088: no backward compatibility): without it the command is a usage error, like any
/// other missing argument.
#[test]
fn the_tool_is_required() {
    let home = tempfile::tempdir().unwrap();
    let out = Command::new(CB)
        .args(["hook", "Stop"])
        .env(DataDir::env_var(), home.path())
        .env(naming::ticket_env_var(), "SHOP-1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--tool"),
        "{out:?}"
    );
}
