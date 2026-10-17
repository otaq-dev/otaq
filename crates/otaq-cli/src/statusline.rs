//! `otaq statusline`: the status line agents run with (USG-04, SES-03; d0020, d0012).
//!
//! Claude Code runs it with the status-line JSON on stdin. It starts the **user's own** status-line command (found
//! read-only in their settings, [`statusline::user_command`]) with the same input, its output going straight to
//! Claude Code, so the user's status line looks exactly as without Otaq. Meanwhile the Claude Code adapter reads
//! the usage windows from the payload (Pro/Max only, [`limits`]) and they go to the service as Claude Code's
//! (`usage.record_limits`, USG-09). Otaq runs no other tool's status line (d0088). It never gets in the way: it
//! never starts the service, gives the service at most [`RECORD_TIMEOUT`], and exits with the user's command's
//! status (0 when there is none, printing nothing).

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use otaq_core::agent_tools::claude::{limits, statusline};
use otaq_core::agent_tools::{LimitWindow, Tool};
use otaq_core::client::request_once;
use otaq_core::data_dir::DataDir;
use otaq_core::protocol::{RecordLimitsParams, Request};
use otaq_core::tools::CheckEnv;
use serde_json::Value;

/// The longest the service may take to take the windows.
pub const RECORD_TIMEOUT: Duration = Duration::from_millis(500);
/// More stdin than this isn't a status-line payload; the rest is ignored.
const MAX_PAYLOAD: u64 = 4 * 1024 * 1024;

/// Runs the status line; returns the exit status.
pub fn run() -> i32 {
    let mut raw = Vec::new();
    let _ = std::io::stdin().take(MAX_PAYLOAD).read_to_end(&mut raw);
    let payload: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);

    // The user's command, unless this is already running inside it (a status line that calls ours again).
    let nested = std::env::var_os(statusline::running_env_var()).is_some();
    let child = (!nested)
        .then(|| {
            statusline::user_command(
                CheckEnv::from_process().claude_config_dir.as_deref(),
                &cwd(&payload),
            )
        })
        .flatten()
        .and_then(|command| spawn(&command, &raw));

    if let Some(windows) = limits::windows_from_status_line(&payload) {
        record(windows);
    }

    match child {
        Some(mut child) => child.wait().ok().and_then(|s| s.code()).unwrap_or(1),
        None => 0,
    }
}

/// The session's folder: `workspace.project_dir`, else `cwd`, else this process's.
fn cwd(payload: &Value) -> PathBuf {
    payload
        .get("workspace")
        .and_then(|w| w.get("project_dir"))
        .or_else(|| payload.get("cwd"))
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

/// Starts `command` in the platform's shell with `input` on its stdin and this process's stdout and stderr, so its output
/// reaches Claude Code byte for byte.
fn spawn(command: &str, input: &[u8]) -> Option<std::process::Child> {
    // `/bin/sh -c`; Git Bash on Windows, as Claude Code runs status lines there.
    let shell =
        otaq_core::shell::status_line(command, &std::env::var_os("PATH").unwrap_or_default());
    let mut child = Command::new(&shell.program)
        .args(&shell.args)
        .envs(shell.env)
        .env(statusline::running_env_var(), "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        // A command that doesn't read its input closes the pipe early; that's fine.
        let _ = stdin.write_all(input);
    }
    Some(child)
}

/// Hands the windows to the service, if it is running; never waits longer than [`RECORD_TIMEOUT`].
fn record(windows: Vec<LimitWindow>) {
    let Ok(data) = DataDir::from_env() else {
        return;
    };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    let request = Request::UsageRecordLimits(RecordLimitsParams {
        tool: Tool::Claude,
        windows,
    });
    let _ = runtime.block_on(async {
        tokio::time::timeout(RECORD_TIMEOUT, request_once(&data.socket_path(), request)).await
    });
}
