//! Copilot's hooks file (ADP-08, ADP-03; T-0352): `Notification`, `Stop`, `UserPromptSubmit`, `SessionStart` and
//! `errorOccurred` (which reads the latest `session.error` in [`super::events_file`]). Until T-0352 lands, every
//! hook is [`ToolEvent::Unrecognised`], logged raw. Copilot's signals all come through hooks: no watched file.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::CopilotContext;
use crate::agent_tools::{SignalContext, Tool, ToolEvent, unrecognised};

pub(super) fn map_hook(
    _copilot: &CopilotContext,
    event: &str,
    payload: &Value,
    _context: SignalContext<'_>,
) -> Vec<ToolEvent> {
    unrecognised(Tool::Copilot, event, payload)
}

pub(super) fn watched_file(_tool_id: &str, _cwd: &Path) -> Option<PathBuf> {
    None
}

pub(super) fn map_watched_lines(_lines: &str, _context: SignalContext<'_>) -> Vec<ToolEvent> {
    Vec::new()
}
