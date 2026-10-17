//! The one-time setup in Codex's home (ADP-07, d0085, d0086; T-0351): folder trust per project, trust for Otaq's
//! hooks, `rules/otaq.rules`, Otaq's local plugin; removal on disconnect (folder trust stays).

use std::path::Path;

use super::CodexContext;
use crate::agent_tools::{SetupEntry, SetupOutcome, SetupReport};

/// T-0351: `[projects."<root>"] trust_level = "trusted"` in `config.toml`.
pub(super) fn record_project(_context: &CodexContext, _repo: &Path) -> Option<SetupOutcome> {
    None
}

pub(super) async fn set_up_tool(_context: &CodexContext) -> SetupReport {
    SetupReport::default()
}

pub(super) fn setup_entries() -> Vec<SetupEntry> {
    Vec::new()
}

pub(super) fn remove_setup(_context: &CodexContext) -> SetupReport {
    SetupReport::default()
}
