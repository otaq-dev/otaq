//! Launch and resume (ADP-09, SES-03; T-0354): `opencode --standalone --session <id> --prompt "<first prompt>"`
//! with `OPENCODE_CONFIG_CONTENT`, the session made first by [`super::prepare`]. Until T-0354 lands, every launch
//! is refused ([`crate::agent_tools::not_ready`]).

use super::{COMMAND_NAME, OpenCodeContext};
use crate::agent_tools::{LaunchError, LaunchPlan, LaunchRequest, Tool, not_ready};

pub(super) fn launch(
    _context: &OpenCodeContext,
    _request: &LaunchRequest,
) -> Result<LaunchPlan, LaunchError> {
    Err(not_ready(Tool::OpenCode))
}

pub(super) fn unavailable(_context: &OpenCodeContext) -> Option<LaunchError> {
    Some(not_ready(Tool::OpenCode))
}

/// "opencode --session <id>".
pub(crate) fn resume_hint(tool_id: &str) -> String {
    format!("{COMMAND_NAME} --session {tool_id}")
}

/// One of Otaq's skills named for the agent: for now the skill's name, which OpenCode's skill tool loads from the
/// launch's `skills` folder (the hands-on check, row 6). T-0354 verifies the form.
pub(super) fn skill_reference(skill: &str) -> String {
    skill.to_owned()
}
