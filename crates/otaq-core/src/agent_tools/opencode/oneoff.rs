//! "Ask the agent" (RUN-02; T-0355): a one-off `opencode run --standalone` that may not edit or run commands.
//! Refused until it lands.

use super::OpenCodeContext;
use crate::agent_tools::{LaunchError, OneOffCommand, OneOffRequest, Tool, not_ready};

pub(super) fn command(
    _context: &OpenCodeContext,
    _request: &OneOffRequest,
) -> Result<OneOffCommand, LaunchError> {
    Err(not_ready(Tool::OpenCode))
}
