//! Copilot's tokens and its monthly window (USG-09; T-0352), from its `events.jsonl` ([`super::events_file`]):
//! - tokens: one step per `session.usage_record` (`usage.inputTokens`, `cacheReadTokens`, `outputTokens`); its AI
//!   credits, `usage.copilotUsage.totalNanoAiu` ÷ 1e9, per conversation ([`EventsReader::credits`]);
//! - the windows: the latest `quotaSnapshots`, one window per entry that has an allowance (`chat`: 200 AI credits a
//!   month on Copilot Free), never `completions` (editor completions, Q-I). Copilot's status line carries no quota and
//!   Otaq never writes it (d0088).

use std::collections::HashMap;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::CopilotContext;
use super::events_file::{self, Event, parse_event, quota_snapshots};
use crate::agent_tools::claude::transcript::iso_ms;
use crate::agent_tools::events::{Credits, LimitWindow};
use crate::agent_tools::{TokenReader, TokenSource, TokenStep, TokensUnknown, WindowsSource};
use crate::usage::Tokens;
use crate::usage::limit::LimitKind;

/// The window of the plan's month (Copilot Free: 200 AI credits), the one a quota limit belongs to.
pub(super) const CHAT_WINDOW: &str = "chat";

/// Editor completions' allowance: not an agent's, never shown (Q-I).
const COMPLETIONS_WINDOW: &str = "completions";

/// Nano-AI-units in one AI credit (`total_nano_aiu` 400072000 is shown as "0.4").
const NANO_AIU_PER_CREDIT: f64 = 1e9;

/// Conversation `tool_id`'s `events.jsonl`, read as it grows.
pub(super) fn token_source(context: &CopilotContext, tool_id: &str, _cwd: &Path) -> TokenSource {
    TokenSource::Reader(Box::new(EventsReader::new(events_file::events_path(
        context, tool_id,
    ))))
}

/// The windows come with the tokens, from the same file ([`TokenReader::windows`]).
pub(super) fn windows_source() -> WindowsSource {
    WindowsSource::ToolFiles
}

/// Copilot sends no report.
pub(super) fn windows_from_report(_report: &Value) -> Option<Vec<LimitWindow>> {
    None
}

/// A quota limit (the plan's month used up) belongs to `chat`; Copilot's rate limits have no window of their own.
pub(super) fn limit_window_key(kind: LimitKind) -> Option<&'static str> {
    match kind {
        LimitKind::Other => Some(CHAT_WINDOW),
        LimitKind::Session | LimitKind::Weekly => None,
    }
}

/// Copilot's limit signal carries its reset itself (`resetDate`); its texts are never read for one.
pub(super) fn limit_reset_from_text(_text: &str, _now: i64) -> Option<i64> {
    None
}

/// The windows of one `quotaSnapshots` object: each entry with an allowance (`entitlementRequests` above 0, not
/// unlimited), except `completions`. Used is `100 − remainingPercentage`; credits `usedRequests` of
/// `entitlementRequests`; the reset `resetDate`; no fixed length (a month).
pub(super) fn windows_from_quota(snapshots: &Value) -> Vec<LimitWindow> {
    let Some(entries) = snapshots.as_object() else {
        return Vec::new();
    };
    let mut windows: Vec<LimitWindow> = entries
        .iter()
        .filter(|(key, _)| key.as_str() != COMPLETIONS_WINDOW)
        .filter_map(|(key, entry)| {
            let number = |field: &str| entry.get(field).and_then(Value::as_f64);
            let total = number("entitlementRequests").filter(|t| *t > 0.0)?;
            if entry
                .get("isUnlimitedEntitlement")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                return None;
            }
            let used_percent =
                number("remainingPercentage").map_or(0.0, |left| (100.0 - left).clamp(0.0, 100.0));
            Some(LimitWindow {
                key: key.clone(),
                length_minutes: None,
                used_percent,
                resets_at: entry
                    .get("resetDate")
                    .and_then(Value::as_str)
                    .and_then(iso_ms),
                credits: Some(Credits {
                    used: number("usedRequests").unwrap_or(0.0),
                    total: Some(total),
                }),
            })
        })
        .collect();
    // The plan's month first.
    windows.sort_by_key(|w| (w.key != CHAT_WINDOW, w.key.clone()));
    windows
}

/// When the plan's month resets, from the latest `quotaSnapshots`: `chat`'s `resetDate`, else any entry's.
pub(super) fn quota_reset(snapshots: &Value) -> Option<i64> {
    let reset = |entry: &Value| {
        entry
            .get("resetDate")
            .and_then(Value::as_str)
            .and_then(iso_ms)
    };
    snapshots
        .get(CHAT_WINDOW)
        .and_then(reset)
        .or_else(|| snapshots.as_object()?.values().find_map(reset))
}

/// One step's counts from a `session.usage_record`, and its AI credits.
fn usage_step(event: &Event) -> Option<(TokenStep, f64)> {
    if event.kind != "session.usage_record" {
        return None;
    }
    let usage = event.data.get("usage").unwrap_or(&event.data);
    let count = |field: &str| usage.get(field).and_then(Value::as_u64).unwrap_or(0);
    let cached = count("cacheReadTokens");
    let step = TokenStep {
        id: event.id.clone().or_else(|| {
            usage
                .pointer("/accounting/usageId")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })?,
        at: event.at,
        model: usage
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_owned),
        tokens: Tokens {
            // `inputTokens` counts the cached part too, as OpenAI-style providers report it: Otaq's input is
            // the rest.
            input: count("inputTokens").saturating_sub(cached),
            cached,
            // Reasoning is part of the output.
            output: count("outputTokens"),
            reasoning: None,
        },
    };
    let nano = usage
        .pointer("/copilotUsage/totalNanoAiu")
        .or_else(|| event.data.pointer("/copilotUsage/totalNanoAiu"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    Some((step, nano / NANO_AIU_PER_CREDIT))
}

/// One conversation's `events.jsonl`, read a little more on every call: its steps, its AI credits and the latest
/// windows. The file is looked for until it is there.
pub struct EventsReader {
    path: Option<PathBuf>,
    /// Bytes read so far (always the end of a complete line).
    offset: u64,
    /// Steps per line id, with their credits.
    steps: HashMap<String, (TokenStep, f64)>,
    windows: Option<Vec<LimitWindow>>,
    any_line: bool,
    recognised: bool,
}

impl EventsReader {
    pub(super) fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            offset: 0,
            steps: HashMap::new(),
            windows: None,
            any_line: false,
            recognised: false,
        }
    }

    /// The AI credits the conversation used so far (`totalNanoAiu` ÷ 1e9).
    pub fn credits(&self) -> f64 {
        self.steps.values().map(|(_, credits)| credits).sum()
    }

    /// Reads the complete lines added since the last call.
    fn read_new(&mut self, path: &Path) -> io::Result<bool> {
        let mut file = std::fs::File::open(path)?;
        let len = file.metadata()?.len();
        if len < self.offset {
            // Rewritten: start again; repeated ids replace.
            self.offset = 0;
        }
        if len == self.offset {
            return Ok(false);
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut added = Vec::new();
        file.take(len - self.offset).read_to_end(&mut added)?;
        // Only complete lines: Copilot may be writing the last one right now.
        let Some(end) = added.iter().rposition(|b| *b == b'\n') else {
            return Ok(false);
        };
        let mut changed = false;
        for line in String::from_utf8_lossy(&added[..end]).lines() {
            if line.trim().is_empty() {
                continue;
            }
            self.any_line = true;
            let Some(event) = parse_event(line) else {
                continue;
            };
            self.recognised = true;
            if let Some(snapshots) = quota_snapshots(&event) {
                tracing::debug!(%snapshots, "copilot quotaSnapshots");
                let windows = windows_from_quota(snapshots);
                changed |= self.windows.as_ref() != Some(&windows);
                self.windows = Some(windows);
            }
            if let Some((step, credits)) = usage_step(&event) {
                tracing::debug!(data = %event.data, "copilot session.usage_record");
                if self.steps.get(&step.id) != Some(&(step.clone(), credits)) {
                    changed = true;
                    self.steps.insert(step.id.clone(), (step, credits));
                }
            }
        }
        self.offset += end as u64 + 1;
        Ok(changed)
    }
}

impl TokenReader for EventsReader {
    fn read(&mut self) -> Result<bool, TokensUnknown> {
        let Some(path) = self.path.clone() else {
            return Err(TokensUnknown::Missing);
        };
        match self.read_new(&path) {
            Ok(_) if self.any_line && !self.recognised => Err(TokensUnknown::Unrecognised),
            Ok(changed) => Ok(changed),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(TokensUnknown::Missing),
            Err(e) => Err(TokensUnknown::Unreadable(e.to_string())),
        }
    }

    fn steps(&self) -> Vec<TokenStep> {
        self.steps.values().map(|(step, _)| step.clone()).collect()
    }

    fn windows(&self) -> Option<Vec<LimitWindow>> {
        self.windows.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::super::events_file::tests::fixture_text;
    use super::*;

    /// 2026-11-01T00:00:00Z.
    const NOV_1: i64 = 1_793_491_200_000;

    fn reader_on(text: &str) -> (tempfile::TempDir, PathBuf, EventsReader) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(&path, text).unwrap();
        let reader = EventsReader::new(Some(path.clone()));
        (dir, path, reader)
    }

    /// USG-09: tokens per usage record (cached input apart), the model Auto ran, the credits.
    #[test]
    fn tokens_and_credits() {
        let (_dir, _path, mut reader) = reader_on(&fixture_text("free_auto.jsonl"));
        assert_eq!(reader.read(), Ok(true));
        let mut steps = reader.steps();
        steps.sort_by_key(|s| s.at);
        assert_eq!(steps.len(), 2);
        assert_eq!(
            steps[0].tokens,
            Tokens {
                input: 4000,
                cached: 8000,
                output: 300,
                reasoning: None
            }
        );
        assert_eq!(steps[0].model.as_deref(), Some("mai-code-1.1-flash"));
        assert_eq!(steps[0].at, iso_ms("2026-10-17T21:07:20.000Z"));
        assert_eq!(steps[1].tokens.input, 2000);
        assert!(
            (reader.credits() - 0.400_072).abs() < 1e-9,
            "{}",
            reader.credits()
        );
        assert_eq!(reader.read(), Ok(false), "nothing new");
    }

    /// The captured run: `usage` without credits, its usage ids repeated across turns (the stand-in server's), so
    /// steps are keyed by the line's own id.
    #[test]
    fn captured_records_count_once_each() {
        let (_dir, _path, mut reader) = reader_on(&fixture_text("two_turns.jsonl"));
        assert_eq!(reader.read(), Ok(true));
        let steps = reader.steps();
        assert_eq!(steps.len(), 2);
        assert!(
            steps
                .iter()
                .all(|s| s.tokens.input == 120 && s.tokens.output == 5)
        );
        assert_eq!(reader.credits(), 0.0);
        assert_eq!(reader.windows(), None, "no quota in custom-provider mode");
    }

    /// The `chat` window (80% left, resets Nov 1); `premium_interactions` at 0 and `completions` left out (Q-I).
    #[test]
    fn the_monthly_window() {
        let (_dir, _path, mut reader) = reader_on(&fixture_text("free_auto.jsonl"));
        reader.read().unwrap();
        assert_eq!(
            reader.windows(),
            Some(vec![LimitWindow {
                key: "chat".into(),
                length_minutes: None,
                used_percent: 20.0,
                resets_at: Some(NOV_1),
                credits: Some(Credits {
                    used: 40.0,
                    total: Some(200.0)
                }),
            }])
        );
        // A paid plan's premium requests count as a window of their own; unlimited ones don't.
        let paid = serde_json::json!({
            "chat": {"isUnlimitedEntitlement": true, "entitlementRequests": 0, "remainingPercentage": 100},
            "premium_interactions": {"entitlementRequests": 300, "usedRequests": 30, "remainingPercentage": 90,
                "resetDate": "2026-11-01T00:00:00Z"},
            "completions": {"entitlementRequests": 2000, "remainingPercentage": 100}
        });
        let windows = windows_from_quota(&paid);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].key, "premium_interactions");
        assert!((windows[0].used_percent - 10.0).abs() < 1e-9);
        assert_eq!(quota_reset(&paid), Some(NOV_1));
    }

    /// Lines arrive as Copilot writes them; a half-written line waits.
    #[test]
    fn the_file_is_read_as_it_grows() {
        let text = fixture_text("free_auto.jsonl");
        let (cut, rest) = text.split_at(text.find("\"session.usage_record\"").unwrap() + 10);
        let (_dir, path, mut reader) = reader_on(cut);
        reader.read().unwrap();
        assert!(reader.steps().is_empty());
        assert!(reader.windows().is_some());
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(rest.as_bytes())
            .unwrap();
        assert_eq!(reader.read(), Ok(true));
        assert_eq!(reader.steps().len(), 2);
    }

    #[test]
    fn a_missing_or_foreign_file() {
        let mut none = EventsReader::new(None);
        assert_eq!(none.read(), Err(TokensUnknown::Missing));
        let dir = tempfile::tempdir().unwrap();
        let mut missing = EventsReader::new(Some(dir.path().join("events.jsonl")));
        assert_eq!(missing.read(), Err(TokensUnknown::Missing));
        let (_dir, _path, mut foreign) = reader_on("hello\nworld\n");
        assert_eq!(foreign.read(), Err(TokensUnknown::Unrecognised));
    }

    #[test]
    fn limits_and_their_window() {
        assert_eq!(windows_source(), WindowsSource::ToolFiles);
        assert_eq!(windows_from_report(&serde_json::json!({})), None);
        assert_eq!(limit_window_key(LimitKind::Other), Some("chat"));
        assert_eq!(limit_window_key(LimitKind::Session), None);
        assert_eq!(limit_window_key(LimitKind::Weekly), None);
        assert_eq!(limit_reset_from_text("resets Nov 1", 0), None);
    }
}
