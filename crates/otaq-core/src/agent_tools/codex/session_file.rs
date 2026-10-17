//! Codex's session file, watched while its agent runs (ADP-03, ADP-07; T-0350): failed turns, limits, tokens and
//! windows that no hook reports.

use std::path::{Path, PathBuf};

use super::CodexContext;
use crate::agent_tools::{SignalContext, ToolEvent};

/// T-0350: the conversation's session file in Codex's home.
pub(super) fn watched_file(
    _context: &CodexContext,
    _tool_id: &str,
    _cwd: &Path,
) -> Option<PathBuf> {
    None
}

/// T-0350: `task_complete` errors, `usage_limit_*`, `token_count`.
pub(super) fn map_lines(_lines: &str, _context: SignalContext<'_>) -> Vec<ToolEvent> {
    Vec::new()
}
