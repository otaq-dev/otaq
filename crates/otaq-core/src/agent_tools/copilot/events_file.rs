//! Copilot's `session-state/<id>/events.jsonl` (ADP-08; T-0352): the latest `session.error`, the model that ran,
//! and "Before the restart" (REC-02).

use std::path::Path;

use super::CopilotContext;
use crate::agent_tools::TranscriptEntry;

pub(super) fn last_entries(
    _context: &CopilotContext,
    _tool_id: &str,
    _cwd: &Path,
    _limit: usize,
) -> Option<Vec<TranscriptEntry>> {
    None
}
