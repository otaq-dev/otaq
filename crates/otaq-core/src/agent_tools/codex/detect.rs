//! Detect (ADP-10): `codex` found on PATH and its version (T-0347); signed in from `codex login status` (T-0351
//! fills [`signed_in`]).

use super::{COMMAND_NAME, CodexContext};
use crate::agent_tools::{Detection, version_output};

/// Installed when `codex --version` names a version; signed in as [`signed_in`] says.
pub(super) async fn detect(context: &CodexContext) -> Detection {
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

/// `codex-cli 0.157.1` → `0.157.1`.
pub(super) fn parse_version(output: &str) -> Option<String> {
    let version = output
        .lines()
        .next()?
        .trim()
        .strip_prefix("codex-cli ")?
        .trim();
    version
        .starts_with(|c: char| c.is_ascii_digit())
        .then(|| version.to_owned())
}

/// Whether Codex is signed in, and how (the account line: "signed in with ChatGPT"). T-0351: `codex login status`.
/// `None`: not known yet.
pub(super) async fn signed_in(_context: &CodexContext) -> (Option<bool>, Option<String>) {
    (None, None)
}

/// **Sign in…**: `codex login` (d0086).
pub(super) fn login_command() -> Option<Vec<String>> {
    Some(vec![COMMAND_NAME.to_owned(), "login".to_owned()])
}
