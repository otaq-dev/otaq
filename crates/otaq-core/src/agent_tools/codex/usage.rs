//! Codex's tokens and limit windows (USG-09; T-0350): its session file's `token_count`, windows `primary` /
//! `secondary` by length.

use std::path::Path;

use serde_json::Value;

use super::CodexContext;
use crate::agent_tools::events::LimitWindow;
use crate::agent_tools::{TokenSource, WindowsSource};
use crate::usage::limit::LimitKind;

pub(super) fn token_source(_context: &CodexContext, _tool_id: &str, _cwd: &Path) -> TokenSource {
    TokenSource::NotReported
}

pub(super) fn windows_source() -> WindowsSource {
    WindowsSource::NotReported
}

pub(super) fn windows_from_report(_report: &Value) -> Option<Vec<LimitWindow>> {
    None
}

pub(super) fn limit_window_key(_kind: LimitKind) -> Option<&'static str> {
    None
}

pub(super) fn limit_reset_from_text(_text: &str, _now: i64) -> Option<i64> {
    None
}
