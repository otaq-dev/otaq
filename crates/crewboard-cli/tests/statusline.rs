//! `crewboard statusline` (T-0127, USG-04, SES-03; d0020, d0012): real-shaped status-line JSON (Claude Code docs,
//! "Customize your status line", 2026-10-10) on stdin, with and without `rate_limits`; an ordinary shell script
//! stands in for the user's own status-line command. The user's Claude Code folder is a temporary one
//! (`CLAUDE_CONFIG_DIR`); the real `~/.claude` is never read.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use crewboard_core::data_dir::DataDir;
use crewboard_core::service::testing::TestService;
use crewboard_core::session::statusline;
use crewboard_core::usage::plan::{self, Window};
use serde_json::{Value, json};

const CB: &str = env!("CARGO_BIN_EXE_crewboard");

fn payload(with_limits: bool) -> Value {
    let mut p = json!({
        "hook_event_name": "Status",
        "session_id": "0199d2a4-3b1c-7c3e-9f00-123456789abc",
        "transcript_path": "/Users/me/.claude/projects/x/0199d2a4.jsonl",
        "cwd": "/nonexistent/wt/shop-web/SHOP-1",
        "model": {"id": "claude-opus-5-5", "display_name": "Opus"},
        "workspace": {"current_dir": "/nonexistent/wt/shop-web/SHOP-1",
                      "project_dir": "/nonexistent/wt/shop-web/SHOP-1"},
        "version": "2.1.296",
        "output_style": {"name": "default"},
        "cost": {"total_cost_usd": 0.01234, "total_duration_ms": 45000, "total_api_duration_ms": 2300,
                 "total_lines_added": 156, "total_lines_removed": 23},
        "context_window": {"total_input_tokens": 15234, "total_output_tokens": 4521,
                           "context_window_size": 1000000, "used_percentage": 8, "remaining_percentage": 92}
    });
    if with_limits {
        p["rate_limits"] = json!({
            "five_hour": {"used_percentage": 82.0, "resets_at": 1791648300},
            "seven_day": {"used_percentage": 40.0, "resets_at": 1791849600}
        });
    }
    p
}

/// A Claude Code folder whose user settings run `command` as the status line.
fn claude_dir(command: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    if let Some(command) = command {
        std::fs::write(
            dir.path().join("settings.json"),
            json!({"statusLine": {"type": "command", "command": command, "padding": 1}})
                .to_string(),
        )
        .unwrap();
    }
    dir
}

fn statusline(home: &Path, claude: &Path, input: &str, extra_env: &[(String, &str)]) -> Output {
    let mut cmd = Command::new(CB);
    cmd.arg(statusline::SUBCOMMAND)
        .env(DataDir::env_var(), home)
        .env("CLAUDE_CONFIG_DIR", claude)
        .env_remove(statusline::running_env_var())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_users_command_gets_the_same_input_and_its_output_passes_through() {
    let w = TestService::start().await;
    // An ordinary script as the user's status line: a marker, an ANSI colour, then its whole input.
    let claude = claude_dir(Some(r"printf '\033[32mUSER\033[0m|'; cat; exit 3"));
    let input = payload(true).to_string();
    let out = tokio::task::spawn_blocking({
        let home = w.data.root().to_owned();
        let claude = claude.path().to_owned();
        move || statusline(&home, &claude, &input, &[])
    })
    .await
    .unwrap();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("\u{1b}[32mUSER\u{1b}[0m|{}", payload(true))
    );
    assert_eq!(out.status.code(), Some(3), "the user's exit status");
    let state = plan::latest(w.board.store())
        .await
        .unwrap()
        .expect("recorded");
    assert_eq!(
        state.windows.five_hour,
        Some(Window {
            used_percentage: 82.0,
            resets_at: 1_791_648_300_000
        })
    );
    assert_eq!(
        state.windows.seven_day.map(|w| w.used_percentage),
        Some(40.0)
    );
    assert!(state.limit.is_none());

    // The same windows again write nothing new.
    let home = w.data.root().to_owned();
    let dir = claude.path().to_owned();
    tokio::task::spawn_blocking(move || statusline(&home, &dir, &payload(true).to_string(), &[]))
        .await
        .unwrap();
    let again = plan::latest(w.board.store())
        .await
        .unwrap()
        .expect("recorded");
    assert_eq!(again.observed_at, state.observed_at);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn without_rate_limits_nothing_is_recorded_and_without_a_command_nothing_is_printed() {
    let w = TestService::start().await;
    let home = w.data.root().to_owned();
    // API-key users: no `rate_limits`.
    let claude = claude_dir(Some("cat >/dev/null; echo plain"));
    let dir = claude.path().to_owned();
    let h = home.clone();
    let out =
        tokio::task::spawn_blocking(move || statusline(&h, &dir, &payload(false).to_string(), &[]))
            .await
            .unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "plain\n");
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(plan::latest(w.board.store()).await.unwrap(), None);

    // No status line of their own: Otaq's prints nothing.
    let none = claude_dir(None);
    let dir = none.path().to_owned();
    let h = home.clone();
    let out =
        tokio::task::spawn_blocking(move || statusline(&h, &dir, &payload(true).to_string(), &[]))
            .await
            .unwrap();
    assert!(out.stdout.is_empty());
    assert_eq!(out.status.code(), Some(0));
    assert!(plan::latest(w.board.store()).await.unwrap().is_some());

    // Inside the user's command (a status line that calls Otaq's again): nothing, no loop.
    let dir = claude.path().to_owned();
    let out = tokio::task::spawn_blocking(move || {
        statusline(
            &home,
            &dir,
            &payload(false).to_string(),
            &[(statusline::running_env_var(), "1")],
        )
    })
    .await
    .unwrap();
    assert!(out.stdout.is_empty());
    w.stop().await;
}

#[test]
fn a_stopped_service_never_blocks_the_status_line() {
    let home = tempfile::tempdir().unwrap();
    let claude = claude_dir(Some("cat >/dev/null; printf ok"));
    let started = Instant::now();
    let out = statusline(home.path(), claude.path(), &payload(true).to_string(), &[]);
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "ok");
    assert_eq!(out.status.code(), Some(0));
    assert!(started.elapsed() < Duration::from_secs(3));
    // It never starts the service.
    assert!(!DataDir::new(home.path()).socket_path().exists());
    assert!(!DataDir::new(home.path()).database_path().exists());
}
