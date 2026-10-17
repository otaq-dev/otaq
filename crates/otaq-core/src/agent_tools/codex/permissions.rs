//! Codex's permission warning (SET-02, d0011; T-0351): whether the user's own approval settings stop agents on
//! prompts. Read-only; Otaq never changes them.

use super::CodexContext;

pub(super) fn warning(_context: &CodexContext) -> Option<String> {
    None
}
