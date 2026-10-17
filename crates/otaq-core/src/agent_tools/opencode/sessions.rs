//! OpenCode's sessions (REC-02, REC-03; T-0355): the resume check (`opencode session list`), its text for an unknown
//! `--session`, and "Before the restart" (`opencode session export <id>`).

use std::path::Path;

use super::OpenCodeContext;
use crate::agent_tools::TranscriptEntry;

pub(super) fn can_resume(_context: &OpenCodeContext, _tool_id: &str, _cwd: &Path) -> bool {
    false
}

pub(super) fn conversation_not_found(_output: &[u8]) -> bool {
    false
}

pub(super) fn last_entries(
    _context: &OpenCodeContext,
    _tool_id: &str,
    _cwd: &Path,
    _limit: usize,
) -> Option<Vec<TranscriptEntry>> {
    None
}
