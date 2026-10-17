//! Launch and resume (ADP-07, SES-03; T-0349): `codex --no-daemon -c check_for_update_on_startup=false [-m <model>]
//! "<first prompt>"`, resume `codex --no-daemon … resume <id>`; the tool id captured after start. Until T-0349
//! lands, every launch is refused ([`crate::agent_tools::not_ready`]).

use std::path::Path;

use super::{COMMAND_NAME, CodexContext};
use crate::agent_tools::{LaunchError, LaunchPlan, LaunchRequest, Tool, not_ready};

pub(super) fn launch(
    _context: &CodexContext,
    _request: &LaunchRequest,
) -> Result<LaunchPlan, LaunchError> {
    Err(not_ready(Tool::Codex))
}

pub(super) fn unavailable(_context: &CodexContext) -> Option<LaunchError> {
    Some(not_ready(Tool::Codex))
}

/// T-0349: its session file (`sessions/**/rollout-*-<id>.jsonl` in Codex's home).
pub(super) fn can_resume(_context: &CodexContext, _tool_id: &str, _cwd: &Path) -> bool {
    false
}

/// T-0349: Codex's text for an unknown id.
pub(super) fn conversation_not_found(_output: &[u8]) -> bool {
    false
}

/// "codex resume <id>".
pub(crate) fn resume_hint(tool_id: &str) -> String {
    format!("{COMMAND_NAME} resume {tool_id}")
}
