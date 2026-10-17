//! "Ask the agent" (RUN-02; T-0349): a one-off `codex exec` with a read-only sandbox. Refused until it lands.

use super::CodexContext;
use crate::agent_tools::{LaunchError, OneOffCommand, OneOffRequest, Tool, not_ready};

pub(super) fn command(
    _context: &CodexContext,
    _request: &OneOffRequest,
) -> Result<OneOffCommand, LaunchError> {
    Err(not_ready(Tool::Codex))
}
