//! Copilot's permission warning (SET-02, d0011; T-0353). Read-only; Otaq never changes the user's settings.

use super::CopilotContext;

pub(super) fn warning(_context: &CopilotContext) -> Option<String> {
    None
}
