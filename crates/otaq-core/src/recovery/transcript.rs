//! "Before the restart" (REC-02; d0045, d0020): what an Interrupted agent was doing, read **read-only** from Claude
//! Code's own saved conversation. Otaq never copies terminal output to disk.
//!
//! Claude Code keeps each conversation as JSON lines in `<config>/projects/<folder>/<session-id>.jsonl`, where
//! `<config>` is `$CLAUDE_CONFIG_DIR` or `~/.claude` and `<folder>` is the session's working directory with every
//! character other than a letter or digit replaced by `-` (checked against Claude Code 2.1.296; very long paths
//! get a hash suffix, so the session's file is also looked for in every project folder). A record has a `type`
//! (`user`, `assistant`, and others that are skipped) and a `message` whose `content` is a string or a list of
//! blocks (`text`, `thinking`, `tool_use`, `tool_result`). Records of helper agents (`isSidechain`) and meta
//! records are skipped.
//!
//! Like usage (d0020), a change of this format may break this view, never a workflow: unknown lines and shapes
//! are skipped, and a missing or unreadable file is [`BeforeRestart::NotAvailable`].

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How many entries the view shows by default.
pub const DEFAULT_ENTRIES: usize = 40;
/// Messages are cut after this many characters.
const MAX_TEXT: usize = 2000;
/// Action summaries are cut after this many characters.
const MAX_SUMMARY: usize = 120;

/// What the Terminal tab of an Interrupted ticket shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BeforeRestart {
    /// The last messages and actions, oldest first.
    Available {
        claude_session_id: String,
        entries: Vec<TranscriptEntry>,
    },
    /// "Conversation no longer available": no stored Claude session ID, or Claude Code has removed (or can't
    /// give) the conversation.
    NotAvailable {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        claude_session_id: Option<String>,
    },
}

/// One line of the view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TranscriptEntry {
    /// The user's (or Otaq's) message to the agent.
    User {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
    },
    /// The agent's message.
    Assistant {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
    },
    /// A tool the agent ran: its name and a short summary ("Bash · pnpm test", "Edit · lib/pricing.ts").
    Action {
        tool: String,
        summary: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
    },
}

/// Claude Code's folder name for a working directory.
pub fn project_folder(cwd: &Path) -> String {
    cwd.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Where Claude Code keeps the conversation `session_id` of a session that ran in `cwd`: the expected folder
/// first, else any project folder holding that file. `None` when there is none.
pub fn find_transcript(config_dir: &Path, cwd: &Path, session_id: &str) -> Option<PathBuf> {
    // A session ID is a UUID; anything else never becomes a path.
    if session_id.is_empty()
        || !session_id
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
    {
        return None;
    }
    let projects = config_dir.join("projects");
    let file = format!("{session_id}.jsonl");
    let mut candidates = vec![projects.join(project_folder(cwd)).join(&file)];
    if let Ok(canonical) = crate::paths::canonical(cwd) {
        candidates.push(projects.join(project_folder(&canonical)).join(&file));
    }
    if let Some(found) = candidates.into_iter().find(|p| p.is_file()) {
        return Some(found);
    }
    std::fs::read_dir(&projects)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join(&file))
        .find(|p| p.is_file())
}

/// Reads the last `limit` entries of a transcript file. `None` when it can't be read.
pub fn read_transcript(path: &Path, limit: usize) -> Option<Vec<TranscriptEntry>> {
    let text = std::fs::read(path).ok()?;
    Some(last_entries(&String::from_utf8_lossy(&text), limit))
}

/// The last `limit` entries of a transcript's lines, oldest first. Lines that aren't understood are skipped.
pub fn last_entries(jsonl: &str, limit: usize) -> Vec<TranscriptEntry> {
    let mut kept: VecDeque<TranscriptEntry> = VecDeque::with_capacity(limit + 1);
    for line in jsonl.lines() {
        for entry in entries_of(line) {
            kept.push_back(entry);
            if kept.len() > limit {
                kept.pop_front();
            }
        }
    }
    kept.into()
}

/// The entries of one record.
fn entries_of(line: &str) -> Vec<TranscriptEntry> {
    let Ok(record) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    let flag = |name: &str| record.get(name).and_then(Value::as_bool) == Some(true);
    if flag("isSidechain") || flag("isMeta") {
        return Vec::new();
    }
    let kind = record.get("type").and_then(Value::as_str);
    let at = record
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(iso_ms);
    let Some(content) = record.get("message").and_then(|m| m.get("content")) else {
        return Vec::new();
    };
    match kind {
        Some("user") => user_text(content)
            .map(|text| vec![TranscriptEntry::User { text, at }])
            .unwrap_or_default(),
        Some("assistant") => assistant_entries(content, at),
        _ => Vec::new(),
    }
}

/// A user record's text; tool results and Claude Code's own wrappers (reminders, command output) are skipped,
/// a slash command shows as its name.
fn user_text(content: &Value) -> Option<String> {
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    let text = text.trim();
    if let Some(name) = between(text, "<command-name>", "</command-name>") {
        let args = between(text, "<command-args>", "</command-args>").unwrap_or_default();
        return Some(cut(format!("{name} {args}").trim(), MAX_TEXT));
    }
    if text.is_empty() || text.starts_with('<') {
        return None;
    }
    Some(cut(text, MAX_TEXT))
}

fn assistant_entries(content: &Value, at: Option<i64>) -> Vec<TranscriptEntry> {
    let blocks: Vec<Value> = match content {
        Value::String(s) => vec![serde_json::json!({ "type": "text", "text": s })],
        Value::Array(blocks) => blocks.clone(),
        _ => return Vec::new(),
    };
    blocks
        .iter()
        .filter_map(|block| match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = block.get("text").and_then(Value::as_str)?.trim();
                (!text.is_empty()).then(|| TranscriptEntry::Assistant {
                    text: cut(text, MAX_TEXT),
                    at,
                })
            }
            Some("tool_use") => {
                let tool = block.get("name").and_then(Value::as_str)?.to_owned();
                let summary = action_summary(&tool, block.get("input").unwrap_or(&Value::Null));
                Some(TranscriptEntry::Action { tool, summary, at })
            }
            _ => None,
        })
        .collect()
}

/// A tool call in a few words: the command, the file, the pattern or the task.
fn action_summary(tool: &str, input: &Value) -> String {
    let field = |name: &str| input.get(name).and_then(Value::as_str).map(str::trim);
    let preferred = match tool {
        "Bash" => field("command"),
        "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => {
            field("file_path").or(field("notebook_path"))
        }
        "Grep" | "Glob" => field("pattern"),
        "WebFetch" => field("url"),
        "WebSearch" => field("query"),
        "Task" | "Agent" => field("description"),
        _ => None,
    };
    let any = || {
        input
            .as_object()?
            .values()
            .find_map(|v| v.as_str().map(str::trim).filter(|s| !s.is_empty()))
    };
    let summary = preferred.or_else(any).unwrap_or_default();
    cut(summary.lines().next().unwrap_or_default(), MAX_SUMMARY)
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = text[start..].find(close)? + start;
    Some(text[start..end].trim())
}

fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(max).collect();
    cut.push('…');
    cut
}

/// `2026-10-09T12:34:56.789Z` (UTC, as Claude Code writes it) in Unix milliseconds.
pub fn iso_ms(s: &str) -> Option<i64> {
    let s = s.strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let mut d = date.splitn(3, '-').map(|p| p.parse::<i64>().ok());
    let (year, month, day) = (d.next()??, d.next()??, d.next()??);
    let (hms, frac) = time.split_once('.').unwrap_or((time, "0"));
    let mut t = hms.splitn(3, ':').map(|p| p.parse::<i64>().ok());
    let (hour, minute, second) = (t.next()??, t.next()??, t.next()??);
    let millis: i64 = format!("{frac:0<3}").get(..3)?.parse().ok()?;
    // Days from 1970-01-01 in the proleptic Gregorian calendar (Howard Hinnant's days_from_civil).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + hour) * 60 + minute) * 60_000 + second * 1000 + millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_follow_claude_code() {
        assert_eq!(
            project_folder(Path::new("/Users/nevzat/.data/wt/shop_web/SHOP-25")),
            "-Users-nevzat--data-wt-shop-web-SHOP-25"
        );
    }

    #[test]
    fn timestamps() {
        assert_eq!(iso_ms("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(iso_ms("2026-10-09T12:34:56.789Z"), Some(1_791_549_296_789));
        assert_eq!(iso_ms("2000-02-29T00:00:01Z"), Some(951_782_401_000));
        assert_eq!(iso_ms("yesterday"), None);
    }

    #[test]
    fn summaries_name_the_command_file_or_task() {
        let s = |tool: &str, input: Value| action_summary(tool, &input);
        assert_eq!(
            s("Bash", serde_json::json!({"command": "pnpm test\nmore"})),
            "pnpm test"
        );
        assert_eq!(
            s(
                "Edit",
                serde_json::json!({"file_path": "/wt/lib/pricing.ts", "old_string": "a"})
            ),
            "/wt/lib/pricing.ts"
        );
        assert_eq!(s("Grep", serde_json::json!({"pattern": "TODO"})), "TODO");
        assert_eq!(s("mcp__x__y", serde_json::json!({"q": "hello"})), "hello");
        assert_eq!(s("Unknown", serde_json::json!({})), "");
    }
}
