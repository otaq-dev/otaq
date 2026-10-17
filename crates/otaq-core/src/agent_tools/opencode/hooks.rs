//! Otaq's OpenCode plugin's events (ADP-09, ADP-03; T-0354): dotted event names, de-duplicated by their `id`. Until
//! T-0354 lands, every event is [`ToolEvent::Unrecognised`], logged raw. OpenCode's signals all come through the
//! plugin: no watched file.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agent_tools::{SignalContext, Tool, ToolEvent, unrecognised};

pub(super) fn map_hook(
    event: &str,
    payload: &Value,
    _context: SignalContext<'_>,
) -> Vec<ToolEvent> {
    unrecognised(Tool::OpenCode, event, payload)
}

pub(super) fn watched_file(_tool_id: &str, _cwd: &Path) -> Option<PathBuf> {
    None
}

pub(super) fn map_watched_lines(_lines: &str, _context: SignalContext<'_>) -> Vec<ToolEvent> {
    Vec::new()
}
