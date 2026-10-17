//! "Ask the agent" (RUN-02; T-0353): a one-off `copilot -p` with reading tools only. Refused until it lands.

use super::CopilotContext;
use crate::agent_tools::{LaunchError, OneOffCommand, OneOffRequest, Tool, not_ready};

pub(super) fn command(
    _context: &CopilotContext,
    _request: &OneOffRequest,
) -> Result<OneOffCommand, LaunchError> {
    Err(not_ready(Tool::Copilot))
}
