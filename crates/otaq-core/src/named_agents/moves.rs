//! Limits per tool (USG-10, AGT-08; d0086; T-0358): Resume per tool (`limits.resume_tool`) and Move waiting to…
//! (`limits.move_waiting`), which moves the Developer rows of that tool's not-yet-started tickets to another named
//! agent, recording each row's move ([`super::RowMove`]). Not built yet.

use crate::agent_tools::Tool;
use crate::board::{Board, BoardError, Refusal};

/// `limits.resume_tool`: Resume for one tool's agents waiting on its limit (T-0358).
pub async fn resume_tool(_board: &Board, _tool: Tool) -> Result<(), BoardError> {
    Err(not_built("limits.resume_tool"))
}

/// `limits.move_waiting`: Move waiting to… `agent` for `tool`'s waiting tickets (T-0358).
pub async fn move_waiting(_board: &Board, _tool: Tool, _agent: &str) -> Result<(), BoardError> {
    Err(not_built("limits.move_waiting"))
}

fn not_built(request: &str) -> BoardError {
    BoardError::refused(
        Refusal::NotAvailableYet,
        format!("{request} isn't built yet"),
    )
}
