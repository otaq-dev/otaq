//! Token counts read **read-only** from Claude Code's saved conversations (USG-01; d0020).
//!
//! Claude Code 2.1.296 writes each conversation as JSON lines to `<config>/projects/<folder>/<session-id>.jsonl`
//! (see [`crate::recovery::transcript`] for the folder), and the conversations of its helper agents to
//! `<folder>/<session-id>/subagents/agent-<id>.jsonl`. Every API response is an `assistant` record whose `message`
//! carries the response `id`, the `model` and a `usage` object with `input_tokens`, `output_tokens`,
//! `cache_creation_input_tokens` and `cache_read_input_tokens`. A response with several content blocks is written
//! as several records with the same `message.id` and the same `usage`, so counts are kept **per response id**: a
//! repeated id replaces its earlier counts instead of adding to them.
//!
//! Files only grow, so a [`TranscriptFile`] remembers how far it has read and reads only the lines added since; a
//! line that isn't complete yet is left for the next read. A line that isn't JSON is skipped. A file none of whose
//! lines is a Claude Code record (an object with a `type`) is **unrecognised**: like an unreadable file it makes
//! the conversation's usage unknown. Nothing here writes, moves or deletes anything.

use std::collections::{BTreeMap, HashMap};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::Tokens;
use crate::recovery::transcript::iso_ms;

/// One API response's counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageUsage {
    /// `message.id`, else the record's `uuid`.
    pub id: String,
    /// When the record was written (Unix milliseconds).
    pub at: Option<i64>,
    pub model: Option<String>,
    pub tokens: Tokens,
    /// The part of the cache writes that went to the 1-hour cache (`usage.cache_creation`), for the estimate.
    pub cache_write_1h: u64,
}

/// What one line of a transcript is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// An `assistant` record with usage.
    Usage(MessageUsage),
    /// Another Claude Code record (user message, bookkeeping, …).
    Record,
    /// Not JSON, or not a record.
    Unrecognised,
}

/// Reads one transcript line.
pub fn parse_line(line: &str) -> Line {
    let Ok(record) = serde_json::from_str::<Value>(line) else {
        return Line::Unrecognised;
    };
    let Some(kind) = record.get("type").and_then(Value::as_str) else {
        return Line::Unrecognised;
    };
    if kind != "assistant" {
        return Line::Record;
    }
    let message = record.get("message");
    let Some(usage) = message
        .and_then(|m| m.get("usage"))
        .filter(|u| u.is_object())
    else {
        return Line::Record;
    };
    let count = |field: &str| usage.get(field).and_then(Value::as_u64).unwrap_or(0);
    let text = |v: Option<&Value>| v.and_then(Value::as_str).map(str::to_owned);
    let Some(id) = text(message.and_then(|m| m.get("id"))).or_else(|| text(record.get("uuid")))
    else {
        return Line::Record;
    };
    Line::Usage(MessageUsage {
        id,
        at: record
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(iso_ms),
        model: text(message.and_then(|m| m.get("model"))),
        cache_write_1h: usage
            .get("cache_creation")
            .and_then(|c| c.get("ephemeral_1h_input_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        tokens: Tokens {
            input: count("input_tokens"),
            output: count("output_tokens"),
            cache_write: count("cache_creation_input_tokens"),
            cache_read: count("cache_read_input_tokens"),
        },
    })
}

/// One transcript file and how far it has been read.
#[derive(Debug, Clone, Default)]
pub struct TranscriptFile {
    /// Bytes read so far (always the end of a complete line).
    offset: u64,
    /// At least one line was a Claude Code record.
    recognised: bool,
    /// At least one complete line was read.
    any_line: bool,
}

impl TranscriptFile {
    /// Reads the lines added since the last read and hands every response's counts to `found`. A file that got
    /// shorter (rewritten) is read again from the start; repeated response ids replace, so nothing counts twice.
    pub fn read_new(
        &mut self,
        path: &Path,
        found: &mut impl FnMut(MessageUsage),
    ) -> io::Result<()> {
        let mut file = std::fs::File::open(path)?;
        let len = file.metadata()?.len();
        if len < self.offset {
            *self = Self::default();
        }
        if len == self.offset {
            return Ok(());
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut added = Vec::new();
        file.take(len - self.offset).read_to_end(&mut added)?;
        // Only complete lines: Claude Code may be writing the last one right now.
        let Some(end) = added.iter().rposition(|b| *b == b'\n') else {
            return Ok(());
        };
        for line in String::from_utf8_lossy(&added[..end]).lines() {
            if line.trim().is_empty() {
                continue;
            }
            self.any_line = true;
            match parse_line(line) {
                Line::Usage(usage) => {
                    self.recognised = true;
                    found(usage);
                }
                Line::Record => self.recognised = true,
                Line::Unrecognised => {}
            }
        }
        self.offset += end as u64 + 1;
        Ok(())
    }

    /// Lines were read and none was a Claude Code record.
    pub fn unrecognised(&self) -> bool {
        self.any_line && !self.recognised
    }
}

/// Why a conversation's usage is unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unknown {
    /// No transcript file for the session ID.
    Missing,
    /// The file couldn't be read.
    Unreadable(String),
    /// The file holds no line Claude Code would write.
    Unrecognised,
}

/// One Claude conversation (a stored Claude session ID): its files and every response's counts.
#[derive(Debug, Clone, Default)]
pub struct Conversation {
    /// The main transcript and its helper agents' transcripts.
    files: BTreeMap<PathBuf, TranscriptFile>,
    /// Counts per response id.
    messages: HashMap<String, MessageUsage>,
}

impl Conversation {
    /// Reads what was added to `main` and its helper agents' files since the last read. Returns whether any
    /// counts changed, or why the usage is unknown.
    pub fn read(&mut self, main: &Path) -> Result<bool, Unknown> {
        let mut paths = vec![main.to_owned()];
        if let (Some(dir), Some(stem)) = (main.parent(), main.file_stem())
            && let Ok(entries) = std::fs::read_dir(dir.join(stem).join("subagents"))
        {
            let mut helpers: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
                .collect();
            helpers.sort();
            paths.extend(helpers);
        }
        let mut changed = false;
        let messages = &mut self.messages;
        let mut found = |usage: MessageUsage| {
            if messages.get(&usage.id) != Some(&usage) {
                changed = true;
                messages.insert(usage.id.clone(), usage);
            }
        };
        for path in &paths {
            let file = self.files.entry(path.clone()).or_default();
            let read = file.read_new(path, &mut found);
            if path == main {
                if let Err(e) = read {
                    return Err(Unknown::Unreadable(e.to_string()));
                }
                if file.unrecognised() {
                    return Err(Unknown::Unrecognised);
                }
            } else if let Err(e) = read {
                tracing::debug!(path = %path.display(), error = %e, "helper agent transcript unreadable");
            }
        }
        Ok(changed)
    }

    /// Every response's counts.
    pub fn messages(&self) -> impl Iterator<Item = &MessageUsage> {
        self.messages.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_records_carry_usage_and_others_are_records() {
        let line = r#"{"type":"assistant","timestamp":"1970-01-01T00:00:01.000Z","uuid":"u1","message":{"id":"msg_1","model":"claude-opus-5-5","role":"assistant","content":[],"usage":{"input_tokens":2,"cache_creation_input_tokens":11524,"cache_read_input_tokens":28134,"output_tokens":226,"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":11524,"ephemeral_5m_input_tokens":0}}}}"#;
        assert_eq!(
            parse_line(line),
            Line::Usage(MessageUsage {
                id: "msg_1".into(),
                at: Some(1000),
                model: Some("claude-opus-5-5".into()),
                cache_write_1h: 11524,
                tokens: Tokens {
                    input: 2,
                    output: 226,
                    cache_write: 11524,
                    cache_read: 28134
                },
            })
        );
        assert_eq!(
            parse_line(r#"{"type":"user","message":{"content":"hi"}}"#),
            Line::Record
        );
        assert_eq!(
            parse_line(r#"{"type":"assistant","message":{"content":[]}}"#),
            Line::Record
        );
        assert_eq!(parse_line("not json"), Line::Unrecognised);
        assert_eq!(parse_line("[1, 2]"), Line::Unrecognised);
    }
}
