//! Codex's tokens, limit windows and limit text (USG-03, USG-04, USG-09; d0086, d0088).
//!
//! **Tokens** come from the `token_count` records of its session file ([`CodexTokens`]): `info.total_token_usage`
//! is the conversation's running total (`input_tokens`, of which `cached_input_tokens` were read from a cache;
//! `output_tokens`, of which `reasoning_output_tokens` were reasoning), so each record counts what it added to the
//! one before, mapped onto Otaq's four numbers without counting anything twice: input minus cached, cached, output
//! minus reasoning, reasoning.
//!
//! **Windows** come from the same records' `rate_limits` (`primary`, `secondary`: `used_percent`, `window_minutes`,
//! `resets_at` in Unix seconds; the app-server's `account/rateLimits/read` has the same in camelCase). Which slot
//! holds which window depends on the plan (Plus has one weekly window, in `primary`), so a window is keyed by its
//! length: [`SESSION_KEY`] below a week, [`WEEKLY_KEY`] from a week on. The watcher hands them over as
//! [`ToolEvent::LimitWindows`](crate::agent_tools::ToolEvent).
//!
//! **A limit** is a failed turn's `usage_limit_exceeded` / `usage_limit_reached` ([`limit_hit`]); its message ("…
//! or try again at 3:46 PM.") gives the reset when no window does ([`limit_reset_from_text`]).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::CodexContext;
use super::launch::session_file;
use super::session_file::seconds_ms;
use crate::agent_tools::claude::transcript::iso_ms;
use crate::agent_tools::events::LimitWindow;
use crate::agent_tools::{TokenReader, TokenSource, TokenStep, TokensUnknown, WindowsSource};
use crate::usage::Tokens;
use crate::usage::limit::{LimitHit, LimitKind};

/// The key of a window shorter than a week (Pro's 5-hour window).
pub(super) const SESSION_KEY: &str = "session";
/// The key of a window of a week or longer.
pub(super) const WEEKLY_KEY: &str = "weekly";

/// A week, in minutes.
const WEEK_MINUTES: u32 = 7 * 24 * 60;

pub(super) fn token_source(context: &CodexContext, tool_id: &str, _cwd: &Path) -> TokenSource {
    TokenSource::Reader(Box::new(CodexTokens::new(
        context.codex_home.clone(),
        tool_id,
    )))
}

pub(super) fn windows_source() -> WindowsSource {
    WindowsSource::ToolFiles
}

/// The windows of a `rate_limits` report: the session file's, or the app-server's (`rateLimits`).
pub(super) fn windows_from_report(report: &Value) -> Option<Vec<LimitWindow>> {
    let limits = report
        .get("rate_limits")
        .or_else(|| report.get("rateLimits"))
        .unwrap_or(report);
    windows_of(limits)
}

/// The windows in `rate_limits`; `None` when it has none.
pub(super) fn windows_of(limits: &Value) -> Option<Vec<LimitWindow>> {
    let mut windows: Vec<LimitWindow> = Vec::new();
    for slot in ["primary", "secondary"] {
        let Some(w) = limits.get(slot).filter(|w| w.is_object()) else {
            continue;
        };
        let field = |snake: &str, camel: &str| w.get(snake).or_else(|| w.get(camel));
        let Some(used_percent) = field("used_percent", "usedPercent").and_then(Value::as_f64)
        else {
            continue;
        };
        let length_minutes = field("window_minutes", "windowDurationMins")
            .and_then(Value::as_u64)
            .and_then(|m| u32::try_from(m).ok());
        let by_length = if length_minutes.is_some_and(|m| m >= WEEK_MINUTES) {
            WEEKLY_KEY
        } else {
            SESSION_KEY
        };
        // Two windows of one kind keep Codex's own slot names.
        let key = if windows.iter().any(|o| o.key == by_length) {
            slot
        } else {
            by_length
        };
        windows.push(LimitWindow {
            key: key.to_owned(),
            length_minutes,
            used_percent,
            resets_at: field("resets_at", "resetsAt").and_then(seconds_ms),
            credits: None,
        });
    }
    (!windows.is_empty()).then_some(windows)
}

/// Session → the window shorter than a week, weekly → the week; another limit has none.
pub(super) fn limit_window_key(kind: LimitKind) -> Option<&'static str> {
    match kind {
        LimitKind::Session => Some(SESSION_KEY),
        LimitKind::Weekly => Some(WEEKLY_KEY),
        LimitKind::Other => None,
    }
}

/// Whether a failed turn is the usage limit, and which one: its `codex_error_info`, else its message.
pub(super) fn limit_hit(info: Option<&str>, message: &str) -> Option<LimitHit> {
    let lower = message.to_lowercase();
    let by_info = info.is_some_and(|i| i.starts_with("usage_limit"));
    if !by_info && !lower.contains("usage limit") {
        return None;
    }
    let kind = if lower.contains("week") {
        LimitKind::Weekly
    } else if lower.contains("5-hour") || lower.contains("5 hour") || lower.contains("session") {
        LimitKind::Session
    } else {
        LimitKind::Other
    };
    Some(LimitHit {
        kind,
        text: message.trim().to_owned(),
    })
}

/// "… try again at 3:46 PM." → the next local 3:46 pm; "… try again in 2 days 4 hours." → `now` plus that.
pub(super) fn limit_reset_from_text(text: &str, now: i64) -> Option<i64> {
    let lower = text.to_lowercase();
    if let Some(at) = lower.find("try again at ") {
        let rest = &lower[at + "try again at ".len()..];
        let time: String = rest
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join("")
            .trim_end_matches(['.', ','])
            .to_owned();
        return crate::agent_tools::claude::limits::reset_from_text(&format!("resets {time}"), now);
    }
    let rest = &lower[lower.find("try again in ")? + "try again in ".len()..];
    let mut words = rest.split_whitespace();
    let mut total_ms = 0i64;
    while let (Some(n), Some(unit)) = (words.next(), words.next()) {
        let Ok(n) = n.parse::<i64>() else { break };
        let unit_ms = match unit.trim_end_matches(['.', ',']) {
            "day" | "days" => 86_400_000,
            "hour" | "hours" => 3_600_000,
            "minute" | "minutes" => 60_000,
            "second" | "seconds" => 1000,
            _ => break,
        };
        total_ms += n * unit_ms;
        if words.clone().next() == Some("and") {
            words.next();
        }
    }
    (total_ms > 0).then_some(now + total_ms)
}

/// Reads one conversation's `token_count` records from its session file, a little more on every call.
pub(super) struct CodexTokens {
    home: Option<PathBuf>,
    tool_id: String,
    file: Option<PathBuf>,
    /// How far the file was read (whole lines only).
    offset: u64,
    /// The running total of the last record.
    total: Tokens,
    steps: Vec<TokenStep>,
    /// Lines read so far (a record without an `ordinal` is counted by its line).
    lines: usize,
    /// A line of Codex's was read: the file is one of its session files.
    recognised: bool,
}

impl CodexTokens {
    pub(super) fn new(home: Option<PathBuf>, tool_id: &str) -> Self {
        Self {
            home,
            tool_id: tool_id.to_owned(),
            file: None,
            offset: 0,
            total: Tokens::default(),
            steps: Vec::new(),
            lines: 0,
            recognised: false,
        }
    }

    fn line(&mut self, line: &str, index: usize) {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            return;
        };
        if record.get("type").and_then(Value::as_str).is_some() {
            self.recognised = true;
        }
        let payload = record.get("payload");
        if payload.and_then(|p| p.get("type")).and_then(Value::as_str) != Some("token_count") {
            return;
        }
        let Some(total) = payload
            .and_then(|p| p.pointer("/info/total_token_usage"))
            .and_then(total_tokens)
        else {
            return;
        };
        let step = minus(&total, &self.total);
        self.total = total;
        if step == Tokens::default() {
            return;
        }
        let id = record
            .get("ordinal")
            .and_then(Value::as_u64)
            .map_or_else(|| format!("line-{index}"), |o| o.to_string());
        self.steps.push(TokenStep {
            id,
            at: record
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(iso_ms),
            model: None,
            tokens: step,
        });
    }
}

impl TokenReader for CodexTokens {
    fn read(&mut self) -> Result<bool, TokensUnknown> {
        if self.file.is_none() {
            let home = self.home.as_deref().ok_or(TokensUnknown::Missing)?;
            self.file = Some(session_file(home, &self.tool_id).ok_or(TokensUnknown::Missing)?);
        }
        let file = self.file.as_deref().expect("just found");
        let bytes = std::fs::read(file).map_err(|e| TokensUnknown::Unreadable(e.to_string()))?;
        let start = usize::try_from(self.offset)
            .unwrap_or(usize::MAX)
            .min(bytes.len());
        let new = &bytes[start..];
        let Some(end) = new.iter().rposition(|b| *b == b'\n').map(|at| at + 1) else {
            return self.result(false);
        };
        let before = self.steps.len();
        let lines = String::from_utf8_lossy(&new[..end]).into_owned();
        for line in lines.lines() {
            self.lines += 1;
            self.line(line, self.lines);
        }
        self.offset += end as u64;
        self.result(self.steps.len() != before)
    }

    fn steps(&self) -> Vec<TokenStep> {
        self.steps.clone()
    }
}

impl CodexTokens {
    fn result(&self, changed: bool) -> Result<bool, TokensUnknown> {
        if self.offset > 0 && !self.recognised {
            return Err(TokensUnknown::Unrecognised);
        }
        Ok(changed)
    }
}

/// `total_token_usage` as Otaq's four numbers.
fn total_tokens(usage: &Value) -> Option<Tokens> {
    let n = |field: &str| usage.get(field).and_then(Value::as_u64).unwrap_or(0);
    let input = usage.get("input_tokens")?.as_u64()?;
    let output = usage.get("output_tokens")?.as_u64()?;
    let cached = n("cached_input_tokens");
    let reasoning = n("reasoning_output_tokens");
    Some(Tokens {
        input: input.saturating_sub(cached),
        cached,
        output: output.saturating_sub(reasoning),
        reasoning: Some(reasoning),
    })
}

/// What `total` added to `before`; a total smaller than before (a count that started again) counts whole.
fn minus(total: &Tokens, before: &Tokens) -> Tokens {
    let restarted = total.input + total.cached < before.input + before.cached;
    if restarted {
        return *total;
    }
    Tokens {
        input: total.input.saturating_sub(before.input),
        cached: total.cached.saturating_sub(before.cached),
        output: total.output.saturating_sub(before.output),
        reasoning: Some(
            total
                .reasoning
                .unwrap_or(0)
                .saturating_sub(before.reasoning.unwrap_or(0)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::agent_tools::codex::tests::session_lines;

    const ID: &str = "01a1479b-da76-7702-a546-a8847b2bcc57";

    /// A Codex home with the conversation's session file, empty.
    fn home() -> (tempfile::TempDir, PathBuf) {
        let home = tempfile::tempdir().unwrap();
        let day = home.path().join("sessions/2026/10/17");
        std::fs::create_dir_all(&day).unwrap();
        let file = day.join(format!("rollout-2026-10-17T05-05-58-{ID}.jsonl"));
        std::fs::write(&file, "").unwrap();
        (home, file)
    }

    fn append(file: &Path, text: &str) {
        std::fs::OpenOptions::new()
            .append(true)
            .open(file)
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }

    fn sum(steps: &[TokenStep]) -> Tokens {
        steps.iter().fold(Tokens::default(), |sum, step| Tokens {
            input: sum.input + step.tokens.input,
            cached: sum.cached + step.tokens.cached,
            output: sum.output + step.tokens.output,
            reasoning: Some(sum.reasoning.unwrap_or(0) + step.tokens.reasoning.unwrap_or(0)),
        })
    }

    /// The captured session, appended line by line: the steps add up to its last running total, cached input
    /// and reasoning taken out of input and output.
    #[test]
    fn tokens_are_what_each_count_added_with_cached_and_reasoning_apart() {
        let (home, file) = home();
        let mut reader = CodexTokens::new(Some(home.path().to_owned()), ID);
        for line in session_lines("turns.jsonl").lines() {
            append(&file, &format!("{line}\n"));
            reader.read().unwrap();
        }
        let steps = reader.steps();
        assert_eq!(steps.len(), 10);
        assert_eq!(
            sum(&steps),
            Tokens {
                input: 153_719 - 136_320,
                cached: 136_320,
                output: 300 - 41,
                reasoning: Some(41),
            }
        );
        assert_eq!(
            steps[0].tokens,
            Tokens {
                input: 15_068,
                cached: 0,
                output: 6,
                reasoning: Some(0)
            }
        );
        assert_eq!(steps[0].id, "13");
        assert!(steps[0].at.is_some());
        assert!(!reader.read().unwrap(), "nothing new");
    }

    /// Half a line waits for the rest; a missing file is missing; a file of something else is unrecognised.
    #[test]
    fn only_whole_lines_count_and_other_files_are_unknown() {
        let mut nowhere = CodexTokens::new(None, ID);
        assert_eq!(nowhere.read(), Err(TokensUnknown::Missing));
        let (home, file) = self::home();
        let mut missing = CodexTokens::new(Some(home.path().to_owned()), "01a1-other");
        assert_eq!(missing.read(), Err(TokensUnknown::Missing));

        let mut reader = CodexTokens::new(Some(home.path().to_owned()), ID);
        let line = session_lines("turns.jsonl")
            .lines()
            .find(|l| l.contains("token_count"))
            .unwrap()
            .to_owned();
        let (head, tail) = line.split_at(40);
        append(&file, head);
        assert_eq!(reader.read(), Ok(false));
        append(&file, &format!("{tail}\n"));
        assert_eq!(reader.read(), Ok(true));
        assert_eq!(reader.steps().len(), 1);

        let (other_home, file) = super::tests::home();
        append(&file, "plain text\n");
        let mut other = CodexTokens::new(Some(other_home.path().to_owned()), ID);
        assert_eq!(other.read(), Err(TokensUnknown::Unrecognised));
    }

    /// Windows keyed by length: Plus's one weekly `primary`; Pro's 5-hour `primary` and weekly `secondary`; the
    /// app-server's camelCase report.
    #[test]
    fn windows_are_keyed_by_their_length() {
        let plus = serde_json::json!({"rate_limits": {"primary": {"used_percent": 12.5, "window_minutes": 10080,
            "resets_at": 1_792_807_562}, "secondary": null}});
        assert_eq!(
            windows_from_report(&plus),
            Some(vec![LimitWindow {
                key: WEEKLY_KEY.into(),
                length_minutes: Some(10_080),
                used_percent: 12.5,
                resets_at: Some(1_792_807_562_000),
                credits: None
            }])
        );
        let pro = serde_json::json!({"primary": {"used_percent": 40.0, "window_minutes": 300, "resets_at": 100},
            "secondary": {"used_percent": 7.0, "window_minutes": 10080, "resets_at": 200}});
        let keys: Vec<(String, Option<u32>)> = windows_of(&pro)
            .unwrap()
            .into_iter()
            .map(|w| (w.key, w.length_minutes))
            .collect();
        assert_eq!(
            keys,
            [
                (SESSION_KEY.to_owned(), Some(300)),
                (WEEKLY_KEY.to_owned(), Some(10_080))
            ]
        );
        let app_server = serde_json::json!({"rateLimits": {"primary": {"usedPercent": 3, "windowDurationMins": 10080,
            "resetsAt": 1_792_807_562}, "secondary": null, "planType": "plus"}});
        assert_eq!(
            windows_from_report(&app_server).unwrap()[0].used_percent,
            3.0
        );
        assert_eq!(windows_from_report(&serde_json::json!({})), None);
        assert_eq!(limit_window_key(LimitKind::Weekly), Some(WEEKLY_KEY));
        assert_eq!(limit_window_key(LimitKind::Session), Some(SESSION_KEY));
        assert_eq!(limit_window_key(LimitKind::Other), None);
    }

    #[test]
    fn the_limit_text_gives_the_reset() {
        let now = 1_792_500_000_000;
        assert_eq!(
            limit_reset_from_text(
                "You've hit your usage limit. Try again in 2 days 3 hours.",
                now
            ),
            Some(now + 2 * 86_400_000 + 3 * 3_600_000)
        );
        let at = limit_reset_from_text("… or try again at 3:46 PM.", now).unwrap();
        assert!(at > now && at <= now + 86_400_000, "{at}");
        assert_eq!(
            limit_reset_from_text("You've hit your usage limit.", now),
            None
        );
        assert_eq!(limit_hit(Some("other"), "boom"), None);
        assert_eq!(
            limit_hit(Some("usage_limit_exceeded"), "Try later").map(|h| h.kind),
            Some(LimitKind::Other)
        );
    }
}
