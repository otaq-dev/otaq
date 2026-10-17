//! Launch and resume (ADP-08, SES-03; T-0352): `copilot --session-id <uuid> [--model <model>] --plugin-dir <otaq
//! plugin> --allow-tool 'shell(otaq:*)' -i "<first prompt>"`, resume `copilot --resume <id> …`. Until T-0352 lands,
//! every launch is refused ([`crate::agent_tools::not_ready`]).

use std::path::Path;

use super::{COMMAND_NAME, CopilotContext};
use crate::agent_tools::{LaunchError, LaunchPlan, LaunchRequest, Tool, not_ready};

pub(super) fn launch(
    _context: &CopilotContext,
    _request: &LaunchRequest,
) -> Result<LaunchPlan, LaunchError> {
    Err(not_ready(Tool::Copilot))
}

pub(super) fn unavailable(_context: &CopilotContext) -> Option<LaunchError> {
    Some(not_ready(Tool::Copilot))
}

/// T-0352: `session-state/<id>/` in Copilot's home.
pub(super) fn can_resume(_context: &CopilotContext, _tool_id: &str, _cwd: &Path) -> bool {
    false
}

/// T-0352: Copilot's text for an unknown id.
pub(super) fn conversation_not_found(_output: &[u8]) -> bool {
    false
}

/// "copilot --resume <id>".
pub(crate) fn resume_hint(tool_id: &str) -> String {
    format!("{COMMAND_NAME} --resume {tool_id}")
}
