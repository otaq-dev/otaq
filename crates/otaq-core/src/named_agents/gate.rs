//! The check before every launch (SCH-04 `[M23]`, AGT-05, AGT-08; d0086, d0088; T-0357): the agent's tool and model
//! are checked, a tool at its limit or not available holds only the agents that would use it ([`super::AgentHold`]),
//! a model no longer listed holds with "Model unavailable", and the named agent's fallback can take the work
//! (`ticket.use_fallback`, and "Use fallback agents automatically", `settings.auto_fallback`). Not built yet.

use super::Role;
use crate::board::{Board, BoardError, Refusal};

/// `ticket.use_fallback`: the held `role` of `key` moves to its named agent's fallback (T-0357).
pub async fn use_fallback(_board: &Board, _key: &str, _role: Role) -> Result<(), BoardError> {
    Err(not_built("ticket.use_fallback"))
}

/// `settings.auto_fallback`: "Use fallback agents automatically", off by default (T-0357).
pub async fn set_auto_fallback(_board: &Board, _on: bool) -> Result<(), BoardError> {
    Err(not_built("settings.auto_fallback"))
}

fn not_built(request: &str) -> BoardError {
    BoardError::refused(
        Refusal::NotAvailableYet,
        format!("{request} isn't built yet"),
    )
}
