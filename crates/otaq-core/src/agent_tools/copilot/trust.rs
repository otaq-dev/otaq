//! Copilot's one-time setup (ADP-08, d0085; T-0353): folder trust in its `config.json` `trustedFolders`, its only
//! setup (hooks and skills come with every launch). Folder trust stays on disconnect, so removal removes nothing.

use std::path::Path;

use super::CopilotContext;
use crate::agent_tools::{SetupEntry, SetupOutcome, SetupReport};

/// T-0353: the repo root in `trustedFolders`.
pub(super) fn record_project(_context: &CopilotContext, _repo: &Path) -> Option<SetupOutcome> {
    None
}

pub(super) fn set_up_tool() -> SetupReport {
    SetupReport::default()
}

/// T-0353: "trust for your projects".
pub(super) fn setup_entries() -> Vec<SetupEntry> {
    Vec::new()
}

pub(super) fn remove_setup() -> SetupReport {
    SetupReport::default()
}
