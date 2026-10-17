//! OpenCode's permission warning (SET-02, d0011; T-0355): an `ask` rule for shell or edits in the user's config.
//! Read-only; Otaq never changes it.

use super::OpenCodeContext;

pub(super) fn warning(_context: &OpenCodeContext) -> Option<String> {
    None
}
