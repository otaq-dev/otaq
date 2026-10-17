//! "Before the restart" (REC-02; T-0349): the last messages and commands of a conversation, from its session file.

use std::path::Path;

use super::CodexContext;
use crate::agent_tools::TranscriptEntry;

pub(super) fn last_entries(
    _context: &CodexContext,
    _tool_id: &str,
    _cwd: &Path,
    _limit: usize,
) -> Option<Vec<TranscriptEntry>> {
    None
}
