//! Copilot's hooks file (ADP-08, ADP-03; T-0352): `plugin-copilot/hooks/hooks.json` passes `--tool copilot` for
//! `Notification`, `Stop`, `UserPromptSubmit`, `SessionStart` (Claude Code's names, which Copilot maps, with
//! snake_case payloads except Notification's camelCase `sessionId`) and Copilot's own `errorOccurred`. Every payload
//! is logged raw at debug level (ADP-11). Copilot's signals all come through hooks: no watched file.
//!
//! - `Notification` `permission_prompt` → needs you; `elicitation_dialog` → asks you; any other type (such as
//!   `shell_completed`) → nothing.
//! - `Stop` → turn ended, without background work (Copilot doesn't report it: ADP-12), with the agent's last
//!   message and, first, the model that ran from `events.jsonl` (AGT-05).
//! - `UserPromptSubmit` → new turn. `SessionStart` → nothing (the id was chosen at launch). `permissionRequest` →
//!   nothing: it fires for every permission check, also pre-approved `otaq` commands that never prompt.
//! - `errorOccurred` is only a trigger: Copilot calls it on every retry of a failing model call, and writes the
//!   `session.error` after the last one (about 15 ms later, captured on 1.0.98), so Otaq reads the latest
//!   `session.error` written since the hook, waiting up to [`ERROR_WAIT`] for it: `rate_limit` or `quota` → limit
//!   reached (a quota resets at the latest `quotaSnapshots`' `resetDate`); `model` → nothing (the swap, read at
//!   `Stop`); any other → failed. With none written, a recoverable error is a retry (nothing) and one that isn't
//!   recoverable fails the turn.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::CopilotContext;
use super::events_file::{self, Event, SessionError};
use super::usage::quota_reset;
use crate::agent_tools::{SignalContext, Tool, ToolEvent, unrecognised};
use crate::signals::background::BackgroundWork;
use crate::usage::limit::{LimitHit, LimitKind};

/// How long an `errorOccurred` waits for its `session.error`.
const ERROR_WAIT: Duration = if cfg!(test) {
    Duration::from_millis(150)
} else {
    Duration::from_secs(1)
};

/// How often the file is read again while waiting.
const ERROR_POLL: Duration = Duration::from_millis(25);

/// A `session.error` counts for a hook when written at most this long before the hook's own time.
const ERROR_SLACK_MS: i64 = 2_000;

pub(super) fn map_hook(
    copilot: &CopilotContext,
    event: &str,
    payload: &Value,
    context: SignalContext<'_>,
) -> Vec<ToolEvent> {
    tracing::debug!(tool = %Tool::Copilot, event, payload = %payload, "hook");
    let text = |key: &str| {
        payload
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    // The session's own id, else the payload's (camelCase in Notification and errorOccurred).
    let tool_id = context
        .tool_id
        .map(str::to_owned)
        .or_else(|| text("session_id"))
        .or_else(|| text("sessionId"));
    let events_path = || {
        tool_id
            .as_deref()
            .and_then(|id| events_file::events_path(copilot, id))
    };
    match event {
        "Notification" => {
            let prompt = || {
                text("message")
                    .or_else(|| text("title"))
                    .unwrap_or_default()
            };
            match payload.get("notification_type").and_then(Value::as_str) {
                Some("permission_prompt") => vec![ToolEvent::NeedsYou {
                    prompt: prompt(),
                    helper_agent: false,
                }],
                Some("elicitation_dialog") => vec![ToolEvent::AsksYou { prompt: prompt() }],
                _ => Vec::new(),
            }
        }
        "Stop" => {
            let events = events_path()
                .and_then(|path| events_file::read_events(&path))
                .unwrap_or_default();
            let mut out: Vec<ToolEvent> = events_file::model_ran(&events)
                .map(|model| ToolEvent::ModelKnown { model })
                .into_iter()
                .collect();
            out.push(ToolEvent::TurnEnded {
                background: BackgroundWork::default(),
                last_message: events_file::last_message(&events),
            });
            out
        }
        "UserPromptSubmit" => vec![ToolEvent::NewTurn {
            prompt: text("prompt"),
        }],
        "SessionStart" | "permissionRequest" => Vec::new(),
        "errorOccurred" => error_occurred(payload, events_path().as_deref()),
        _ => unrecognised(Tool::Copilot, event, payload),
    }
}

/// An `errorOccurred` hook: the `session.error` it was about, else the hook's own `recoverable`.
fn error_occurred(payload: &Value, events_path: Option<&Path>) -> Vec<ToolEvent> {
    let since = payload
        .get("timestamp")
        .and_then(Value::as_i64)
        .map(|at| at - ERROR_SLACK_MS);
    if let Some(path) = events_path
        && let Some((error, events)) = wait_for_error(path, since)
    {
        return from_session_error(&error, &events);
    }
    let recoverable = payload
        .get("recoverable")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    if recoverable {
        return Vec::new();
    }
    vec![ToolEvent::Failed {
        error: payload
            .get("errorContext")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned(),
        message: payload
            .pointer("/error/message")
            .and_then(Value::as_str)
            .map(str::to_owned),
    }]
}

/// Reads `path` until a `session.error` written since `since` is there, for [`ERROR_WAIT`] at most; with the
/// file's events. Off the async workers' way when it runs inside the service.
fn wait_for_error(path: &Path, since: Option<i64>) -> Option<(SessionError, Vec<Event>)> {
    let path: PathBuf = path.to_path_buf();
    let wait = move || {
        let started = Instant::now();
        loop {
            if let Some(events) = events_file::read_events(&path)
                && let Some(error) = events_file::latest_error(&events, since)
            {
                return Some((error, events));
            }
            if started.elapsed() >= ERROR_WAIT {
                return None;
            }
            std::thread::sleep(ERROR_POLL);
        }
    };
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(wait)
        }
        _ => wait(),
    }
}

/// Otaq's events for a `session.error` (ADP-08).
fn from_session_error(error: &SessionError, events: &[Event]) -> Vec<ToolEvent> {
    let hit = |kind| LimitHit {
        kind,
        text: error.message.clone(),
    };
    match error.error_type.as_str() {
        "rate_limit" => {
            let weekly = error.code.as_deref().is_some_and(|c| c.contains("weekly"));
            vec![ToolEvent::LimitReached {
                hit: hit(if weekly {
                    LimitKind::Weekly
                } else {
                    LimitKind::Session
                }),
                resets_at: None,
            }]
        }
        // The plan's month used up: until its reset.
        "quota" => vec![ToolEvent::LimitReached {
            hit: hit(LimitKind::Other),
            resets_at: events_file::latest_quota(events).and_then(quota_reset),
        }],
        // An unavailable `--model`: Copilot runs on Auto instead (read at Stop).
        "model" => Vec::new(),
        other => vec![ToolEvent::Failed {
            error: other.to_owned(),
            message: Some(error.message.clone()).filter(|m| !m.is_empty()),
        }],
    }
}

pub(super) fn watched_file(_tool_id: &str, _cwd: &Path) -> Option<PathBuf> {
    None
}

pub(super) fn map_watched_lines(_lines: &str, _context: SignalContext<'_>) -> Vec<ToolEvent> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::super::events_file::tests::fixture_text;
    use super::*;
    use crate::agent_tools::test_support::context;

    /// 2026-11-01T00:00:00Z.
    const NOV_1: i64 = 1_793_491_200_000;

    /// A saved hook payload (`tests/fixtures/copilot/hooks/<name>`).
    fn payload(name: &str) -> Value {
        let path = format!(
            "{}/tests/fixtures/copilot/hooks/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
    }

    /// Copilot's home in a temporary folder, with conversation `id`'s `events.jsonl` from fixture `events` (if any).
    struct Home {
        _dir: tempfile::TempDir,
        copilot: CopilotContext,
    }

    fn home(id: &str, events: Option<&str>) -> Home {
        let dir = tempfile::tempdir().unwrap();
        let copilot_home = dir.path().join(".copilot");
        if let Some(name) = events {
            let folder = copilot_home.join("session-state").join(id);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("events.jsonl"), fixture_text(name)).unwrap();
        }
        Home {
            copilot: CopilotContext {
                tool: context(dir.path(), Some(dir.path())),
                copilot_home: Some(copilot_home),
            },
            _dir: dir,
        }
    }

    fn map(home: &Home, event: &str, payload: &Value) -> Vec<ToolEvent> {
        map_hook(&home.copilot, event, payload, SignalContext::default())
    }

    #[test]
    fn notifications() {
        let h = home("x", None);
        // The real-account payload has camelCase `sessionId` only.
        assert_eq!(
            map(
                &h,
                "Notification",
                &payload("Notification-permission_prompt.json")
            ),
            [ToolEvent::NeedsYou {
                prompt: "Run command: sleep 1".into(),
                helper_agent: false
            }]
        );
        assert_eq!(
            map(
                &h,
                "Notification",
                &payload("Notification-elicitation_dialog.json")
            ),
            [ToolEvent::AsksYou {
                prompt: "Which payment provider should the coupon field call?".into()
            }]
        );
        assert_eq!(
            map(
                &h,
                "Notification",
                &payload("Notification-shell_completed.json")
            ),
            []
        );
    }

    #[test]
    fn turns_start_and_end() {
        let h = home("x", None);
        assert_eq!(
            map(&h, "UserPromptSubmit", &payload("UserPromptSubmit.json")),
            [ToolEvent::NewTurn {
                prompt: Some("hi".into())
            }]
        );
        assert_eq!(map(&h, "SessionStart", &payload("SessionStart.json")), []);
        // Without its events file: a turn end with nothing else.
        assert_eq!(
            map(&h, "Stop", &payload("Stop.json")),
            [ToolEvent::TurnEnded {
                background: BackgroundWork::default(),
                last_message: None
            }]
        );
        assert_eq!(
            map(&h, "permissionRequest", &payload("permissionRequest.json")),
            [],
            "never needs you"
        );
        assert_eq!(
            map(&h, "agentStop", &serde_json::json!({})),
            [ToolEvent::Unrecognised {
                event: "agentStop".into()
            }]
        );
    }

    /// AGT-05: at Stop, the model that ran comes first, then the turn end with the last message.
    #[test]
    fn the_stop_reports_the_model_that_ran() {
        let stop = payload("Stop.json");
        let id = stop["session_id"].as_str().unwrap();
        let h = home(id, Some("swapped.jsonl"));
        assert_eq!(
            map(&h, "Stop", &stop),
            [
                ToolEvent::ModelKnown {
                    model: "auto".into()
                },
                ToolEvent::TurnEnded {
                    background: BackgroundWork::default(),
                    last_message: Some("Done.".into())
                }
            ]
        );
        let h = home(id, Some("free_auto.jsonl"));
        assert_eq!(
            map(&h, "Stop", &stop)[0],
            ToolEvent::ModelKnown {
                model: "mai-code-1.1-flash".into()
            }
        );
        // The session's stored id wins over the payload's.
        let h = home("stored", Some("swapped.jsonl"));
        let ctx = SignalContext {
            tool_id: Some("stored"),
            cwd: None,
        };
        assert_eq!(map_hook(&h.copilot, "Stop", &stop, ctx).len(), 2);
    }

    /// The captured 429: `rate_limit` → limit reached, with no reset of its own.
    #[test]
    fn a_rate_limit() {
        let error = payload("errorOccurred-rate_limit.json");
        let id = error["sessionId"].as_str().unwrap();
        let h = home(id, Some("rate_limit.jsonl"));
        let events = map(&h, "errorOccurred", &error);
        let [ToolEvent::LimitReached { hit, resets_at }] = events.as_slice() else {
            panic!("not a limit: {events:?}");
        };
        assert_eq!(hit.kind, LimitKind::Session);
        assert!(hit.text.contains("429 Rate limit reached"), "{hit:?}");
        assert_eq!(*resets_at, None);
    }

    /// A quota (402, the plan's month used up) → limit reached until the latest `resetDate`.
    #[test]
    fn a_quota() {
        let error = payload("errorOccurred-rate_limit.json");
        let id = error["sessionId"].as_str().unwrap();
        let h = home(id, Some("quota.jsonl"));
        let events = map(&h, "errorOccurred", &error);
        assert_eq!(
            events,
            [ToolEvent::LimitReached {
                hit: LimitHit {
                    kind: LimitKind::Other,
                    text: "You've used all the AI credits in your Copilot Free plan this month."
                        .into()
                },
                resets_at: Some(NOV_1)
            }]
        );
    }

    /// Another error type → failed; a recoverable retry with no `session.error` yet → nothing; an unrecoverable
    /// one with none → failed with the hook's message.
    #[test]
    fn other_errors() {
        let retry = payload("errorOccurred-rate_limit.json");
        let id = retry["sessionId"].as_str().unwrap();
        let h = home(id, Some("other_error.jsonl"));
        assert_eq!(
            map(&h, "errorOccurred", &retry),
            [ToolEvent::Failed {
                error: "authentication".into(),
                message: Some("Your GitHub token has expired. Sign in again.".into())
            }]
        );
        let h = home(id, Some("two_turns.jsonl"));
        assert_eq!(map(&h, "errorOccurred", &retry), []);
        assert_eq!(
            map(
                &h,
                "errorOccurred",
                &payload("errorOccurred-unrecoverable.json")
            ),
            [ToolEvent::Failed {
                error: "session".into(),
                message: Some("The session ended unexpectedly.".into())
            }]
        );
        // An old error (a turn before) isn't this hook's.
        let mut later = retry.clone();
        later["timestamp"] = serde_json::json!(retry["timestamp"].as_i64().unwrap() + 60_000);
        let h = home(id, Some("rate_limit.jsonl"));
        assert_eq!(map(&h, "errorOccurred", &later), []);
        // The model error is a swap, not a failure.
        let h = home(id, Some("swapped.jsonl"));
        let mut early = retry.clone();
        early["timestamp"] = serde_json::json!(0);
        assert_eq!(map(&h, "errorOccurred", &early), []);
    }

    /// The `session.error` written just after the hook (as on 1.0.98) is waited for.
    #[test]
    fn the_error_written_after_the_hook_is_waited_for() {
        let error = payload("errorOccurred-rate_limit.json");
        let id = error["sessionId"].as_str().unwrap();
        let h = home(id, Some("two_turns.jsonl"));
        let path = events_file::events_path(&h.copilot, id).unwrap();
        let text = fixture_text("rate_limit.jsonl");
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40));
            std::fs::write(path, text).unwrap();
        });
        let events = map(&h, "errorOccurred", &error);
        writer.join().unwrap();
        assert!(
            matches!(events.as_slice(), [ToolEvent::LimitReached { .. }]),
            "{events:?}"
        );
    }
}
