//! Detect (ADP-10): `opencode` found on PATH and its version (T-0347); signed in from `opencode auth list` and Sign
//! in… (T-0355 fills [`signed_in`] and [`login_command`]).

use super::{COMMAND_NAME, OpenCodeContext};
use crate::agent_tools::{Detection, version_output};

/// Installed when `opencode --version` names a version; signed in as [`signed_in`] says.
pub(super) async fn detect(context: &OpenCodeContext) -> Detection {
    let Some(output) = version_output(COMMAND_NAME, &context.tool.path).await else {
        return Detection::default();
    };
    let Some(version) = output.as_deref().and_then(parse_version) else {
        return Detection::default();
    };
    let (signed_in, account) = signed_in(context).await;
    Detection {
        installed: true,
        signed_in,
        version: Some(version),
        update_needed: false,
        account,
    }
}

/// `v2.0.17` → `2.0.17`.
pub(super) fn parse_version(output: &str) -> Option<String> {
    let first = output.lines().next()?.trim();
    let version = first.strip_prefix('v').unwrap_or(first);
    (version.starts_with(|c: char| c.is_ascii_digit()) && !version.contains(char::is_whitespace))
        .then(|| version.to_owned())
}

/// Whether OpenCode is signed in to a provider, and the account line. T-0355: `opencode auth list` (with none it
/// still runs the free models: Connected, "free models only", Q-K). `None`: not known yet.
pub(super) async fn signed_in(_context: &OpenCodeContext) -> (Option<bool>, Option<String>) {
    (None, None)
}

/// **Sign in…**: T-0355 (`opencode auth login`).
pub(super) fn login_command(_context: &OpenCodeContext) -> Option<Vec<String>> {
    None
}
