//! Copilot's `session-state/<id>/events.jsonl` (ADP-08; T-0352): one JSON event per line (`type`, `data`, `id`,
//! `timestamp`), read-only. From it come the latest `session.error` (what an `errorOccurred` hook was about), the
//! model that ran (AGT-05: an unavailable `--model` starts on Auto instead of failing), the turn's last message,
//! the latest `quotaSnapshots` and "Before the restart" (REC-02). The tokens are read by [`super::usage`].
//!
//! Shapes: Copilot 1.0.98's bundled `schemas/session-events.schema.json`, a captured run (`tests/fixtures/copilot/`)
//! and the reports. Every line kind read here is logged raw at debug level (ADP-11).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::CopilotContext;
use crate::agent_tools::TranscriptEntry;
use crate::agent_tools::claude::transcript::iso_ms;

/// The folder of Copilot's saved sessions, in its home folder.
const SESSION_STATE: &str = "session-state";

/// The events file in a session's folder.
const EVENTS_FILE: &str = "events.jsonl";

/// What Copilot writes for an unavailable `--model`: it runs on Auto instead (the models report, row 4).
pub(super) const AUTO: &str = "auto";

/// The longest action summary in "Before the restart".
const MAX_SUMMARY: usize = 80;

/// The folder of conversation `tool_id`: `$COPILOT_HOME/session-state/<id>/`; `None` without Copilot's home.
pub(super) fn session_dir(context: &CopilotContext, tool_id: &str) -> Option<PathBuf> {
    let id = tool_id.trim();
    if id.is_empty() || id.contains(['/', '\\']) || id == ".." {
        return None;
    }
    Some(context.copilot_home.as_ref()?.join(SESSION_STATE).join(id))
}

/// The events file of conversation `tool_id`.
pub(super) fn events_path(context: &CopilotContext, tool_id: &str) -> Option<PathBuf> {
    session_dir(context, tool_id).map(|dir| dir.join(EVENTS_FILE))
}

/// One line of the file.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Event {
    pub kind: String,
    pub data: Value,
    /// The line's own id (unique in the file).
    pub id: Option<String>,
    /// When it was written, in Unix milliseconds.
    pub at: Option<i64>,
}

/// Reads one line; `None` for a line that isn't one of Copilot's events.
pub(super) fn parse_event(line: &str) -> Option<Event> {
    let mut record: Value = serde_json::from_str(line).ok()?;
    let kind = record.get("type")?.as_str()?.to_owned();
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_owned);
    let id = text(&record, "id");
    let at = record
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(iso_ms);
    let data = record.get_mut("data").map(Value::take).unwrap_or_default();
    Some(Event { kind, data, id, at })
}

/// Every event of a whole file, oldest first; `None` when it can't be read.
pub(super) fn read_events(path: &Path) -> Option<Vec<Event>> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(text.lines().filter_map(parse_event).collect())
}

/// The `session.error` an `errorOccurred` hook was about (ADP-08): `errorType` (`rate_limit`, `quota`, `model`,
/// `authentication`…), `errorCode`, the message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionError {
    pub error_type: String,
    pub code: Option<String>,
    pub message: String,
    pub at: Option<i64>,
}

/// The latest `session.error` written at `since` (Unix milliseconds) or later; any, when `since` is `None`.
pub(super) fn latest_error(events: &[Event], since: Option<i64>) -> Option<SessionError> {
    events
        .iter()
        .rev()
        .filter(|e| e.kind == "session.error")
        .find(|e| since.is_none_or(|since| e.at.is_some_and(|at| at >= since)))
        .map(|e| {
            tracing::debug!(data = %e.data, "copilot session.error");
            let text = |key: &str| e.data.get(key).and_then(Value::as_str).map(str::to_owned);
            SessionError {
                error_type: text("errorType").unwrap_or_else(|| "unknown".to_owned()),
                code: text("errorCode"),
                message: text("message").unwrap_or_default(),
                at: e.at,
            }
        })
}

/// The latest `quotaSnapshots` in the file: on a model call's event (`model.model_call_success` in the real-account
/// check), a failure, or a usage record's `usage`.
pub(super) fn latest_quota(events: &[Event]) -> Option<&Value> {
    events.iter().rev().find_map(quota_snapshots)
}

/// An event's `quotaSnapshots`, if it carries them.
pub(super) fn quota_snapshots(event: &Event) -> Option<&Value> {
    let data = &event.data;
    data.get("quotaSnapshots")
        .or_else(|| data.get("usage").and_then(|u| u.get("quotaSnapshots")))
        .filter(|q| q.is_object())
}

/// The model the conversation actually runs on (AGT-05, ADP-08), from the latest `session.start` on:
/// - an asked model Copilot replaced (`session.model_change` with `cause: "initial_resolution"` to another model,
///   or a `session.error` of type `model` after which it runs on Auto) → that model, `auto` (the swap rule, T-0357,
///   compares it with the asked one);
/// - else the model Auto chose (`session.auto_mode_resolved`), which is never a swap (Q-J);
/// - else the asked model itself, unless it is Auto (still to be resolved).
///
/// `None` before Copilot has written any of them.
pub(super) fn model_ran(events: &[Event]) -> Option<String> {
    let from = events
        .iter()
        .rposition(|e| e.kind == "session.start")
        .unwrap_or(0);
    let events = &events[from..];
    let text = |e: &Event, key: &str| {
        e.data
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let asked = events
        .iter()
        .find(|e| e.kind == "session.start")
        .and_then(|e| text(e, "selectedModel"));
    let mut replaced = None;
    let mut chosen = None;
    for event in events {
        match event.kind.as_str() {
            "session.model_change" => {
                tracing::debug!(data = %event.data, "copilot session.model_change");
                if event.data.get("cause").and_then(Value::as_str) == Some("initial_resolution")
                    && let Some(new) = text(event, "newModel")
                    && asked.as_deref() != Some(new.as_str())
                {
                    replaced = Some(new);
                }
            }
            "session.error"
                if event.data.get("errorType").and_then(Value::as_str) == Some("model") =>
            {
                tracing::debug!(data = %event.data, "copilot model error");
                replaced.get_or_insert_with(|| AUTO.to_owned());
            }
            "session.auto_mode_resolved" => {
                tracing::debug!(data = %event.data, "copilot session.auto_mode_resolved");
                if chosen.is_none() {
                    chosen = text(event, "chosenModel");
                }
            }
            _ => {}
        }
    }
    replaced
        .or(chosen)
        .or_else(|| asked.filter(|model| model != AUTO))
}

/// The agent's latest message with text (`assistant.message`'s `content`).
pub(super) fn last_message(events: &[Event]) -> Option<String> {
    events
        .iter()
        .rev()
        .filter(|e| e.kind == "assistant.message")
        .find_map(|e| {
            e.data
                .get("content")
                .and_then(Value::as_str)
                .filter(|c| !c.trim().is_empty())
                .map(str::to_owned)
        })
}

/// "Before the restart" (REC-02): the last `limit` user messages, agent messages and tools run of conversation
/// `tool_id`; `None` when its file can't be read.
pub(super) fn last_entries(
    context: &CopilotContext,
    tool_id: &str,
    _cwd: &Path,
    limit: usize,
) -> Option<Vec<TranscriptEntry>> {
    let events = read_events(&events_path(context, tool_id)?)?;
    Some(entries(&events, limit))
}

/// The entries of `events`, the last `limit` of them, oldest first.
pub(super) fn entries(events: &[Event], limit: usize) -> Vec<TranscriptEntry> {
    let text = |v: &Value, key: &str| {
        v.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let mut all: Vec<TranscriptEntry> = events
        .iter()
        .filter_map(|e| match e.kind.as_str() {
            "user.message" => {
                text(&e.data, "content").map(|text| TranscriptEntry::User { text, at: e.at })
            }
            "assistant.message" => {
                text(&e.data, "content").map(|text| TranscriptEntry::Assistant { text, at: e.at })
            }
            "tool.execution_start" => {
                text(&e.data, "toolName").map(|tool| TranscriptEntry::Action {
                    summary: action_summary(e.data.get("arguments").unwrap_or(&Value::Null)),
                    tool,
                    at: e.at,
                })
            }
            _ => None,
        })
        .collect();
    let skip = all.len().saturating_sub(limit);
    all.drain(..skip);
    all
}

/// A tool call in a few words: its command, path or pattern, else its first text argument.
fn action_summary(arguments: &Value) -> String {
    let field = |name: &str| {
        arguments
            .get(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let any = || {
        arguments
            .as_object()?
            .values()
            .find_map(|v| v.as_str().map(str::trim).filter(|s| !s.is_empty()))
    };
    let summary = ["command", "path", "file_path", "pattern", "url", "query"]
        .into_iter()
        .find_map(field)
        .or_else(any)
        .unwrap_or_default();
    let line = summary.lines().next().unwrap_or_default();
    if line.chars().count() <= MAX_SUMMARY {
        return line.to_owned();
    }
    let mut cut: String = line.chars().take(MAX_SUMMARY).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A saved `events.jsonl` (`tests/fixtures/copilot/events/<name>`).
    pub fn fixture_text(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/copilot/events/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    pub fn fixture(name: &str) -> Vec<Event> {
        fixture_text(name).lines().filter_map(parse_event).collect()
    }

    #[test]
    fn lines_are_events_with_their_time() {
        let events = fixture("two_turns.jsonl");
        assert!(events.len() > 10);
        let start = &events[0];
        assert_eq!(start.kind, "session.start");
        assert_eq!(start.data["copilotVersion"], "1.0.98");
        assert!(start.at.is_some() && start.id.is_some());
        assert_eq!(parse_event("not json"), None);
        assert_eq!(parse_event(r#"{"data":{}}"#), None);
    }

    /// The captured 429: one `session.error` of type `rate_limit`, written after the last retry's hook.
    #[test]
    fn the_latest_error_since_a_time() {
        let events = fixture("rate_limit.jsonl");
        let error = latest_error(&events, None).unwrap();
        assert_eq!(error.error_type, "rate_limit");
        assert!(
            error.message.contains("429 Rate limit reached"),
            "{error:?}"
        );
        let at = error.at.unwrap();
        assert_eq!(latest_error(&events, Some(at)), Some(error));
        assert_eq!(latest_error(&events, Some(at + 1)), None);
        assert_eq!(latest_error(&fixture("two_turns.jsonl"), None), None);
    }

    /// AGT-05: an exact model Copilot replaced runs on `auto`; under Auto the model it chose (not a swap, Q-J); an
    /// exact model it kept is the asked one.
    #[test]
    fn the_model_that_ran() {
        assert_eq!(
            model_ran(&fixture("swapped.jsonl")).as_deref(),
            Some("auto")
        );
        assert_eq!(
            model_ran(&fixture("free_auto.jsonl")).as_deref(),
            Some("mai-code-1.1-flash")
        );
        assert_eq!(
            model_ran(&fixture("two_turns.jsonl")).as_deref(),
            Some("gpt-4.1")
        );
        // Auto asked and not resolved yet: nothing to report.
        let start = r#"{"type":"session.start","data":{"selectedModel":"auto"},"timestamp":"2026-10-17T21:07:17.795Z"}"#;
        assert_eq!(model_ran(&[parse_event(start).unwrap()]), None);
        // Only the model error, before the model change was written.
        let lines: Vec<Event> = fixture("swapped.jsonl").into_iter().take(2).collect();
        assert_eq!(model_ran(&lines).as_deref(), Some("auto"));
    }

    #[test]
    fn the_last_message_and_the_quota() {
        let events = fixture("free_auto.jsonl");
        assert_eq!(
            last_message(&events).as_deref(),
            Some("The coupon field validates.\nAll checks pass.")
        );
        assert_eq!(
            latest_quota(&events).unwrap()["chat"]["remainingPercentage"],
            80
        );
        assert_eq!(latest_quota(&fixture("two_turns.jsonl")), None);
    }

    /// REC-02: messages and tools run, oldest first, the last `limit`.
    #[test]
    fn before_the_restart() {
        let events = fixture("free_auto.jsonl");
        assert_eq!(
            entries(&events, 10),
            [
                TranscriptEntry::User {
                    text: format!("/{}:validate", crate::naming::PLUGIN_NAMESPACE),
                    at: iso_ms("2026-10-17T21:07:18.141Z"),
                },
                TranscriptEntry::Action {
                    tool: "bash".into(),
                    summary: format!("{} status SHOP-29", crate::naming::COMMAND_NAME),
                    at: iso_ms("2026-10-17T21:07:19.500Z"),
                },
                TranscriptEntry::Assistant {
                    text: "The coupon field validates.\nAll checks pass.".into(),
                    at: iso_ms("2026-10-17T21:07:21.500Z"),
                },
            ]
        );
        assert_eq!(entries(&events, 1).len(), 1);
        let two = entries(&fixture("two_turns.jsonl"), 40);
        let texts: Vec<_> =
            two.iter()
                .map(|e| match e {
                    TranscriptEntry::User { text, .. }
                    | TranscriptEntry::Assistant { text, .. } => text.as_str(),
                    TranscriptEntry::Action { .. } => "action",
                })
                .collect();
        assert_eq!(texts, ["hi", "Hello there.", "again", "Hello there."]);
    }

    #[test]
    fn paths_stay_in_copilots_home() {
        let bin = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let context = CopilotContext {
            tool: crate::agent_tools::test_support::context(bin.path(), Some(home.path())),
            copilot_home: Some(home.path().join(".copilot")),
        };
        assert_eq!(
            events_path(&context, "5f1c").unwrap(),
            home.path().join(".copilot/session-state/5f1c/events.jsonl")
        );
        for id in ["", "../x", "..", "a/b"] {
            assert_eq!(session_dir(&context, id), None, "{id}");
        }
        let none = CopilotContext {
            copilot_home: None,
            ..context
        };
        assert_eq!(events_path(&none, "5f1c"), None);
    }
}
