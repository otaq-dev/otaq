//! Detect (ADP-10): `copilot` found on PATH, its version, and "update needed" before [`OLDEST`] (Q-G; T-0347);
//! signed in from its `config.json` or a token variable, and Sign in… (T-0353 fills [`signed_in`] and
//! [`login_command`]).

use super::{COMMAND_NAME, CopilotContext};
use crate::agent_tools::{Detection, version_output};

/// The oldest Copilot Otaq runs: older ones lack the flags and hooks the adapter uses (the Homebrew 0.0.404).
pub(super) const OLDEST: [u32; 3] = [1, 0, 98];

/// Installed when `copilot --version` names a version; older than [`OLDEST`] → update needed (never launched, and
/// not asked whether it is signed in).
pub(super) async fn detect(context: &CopilotContext) -> Detection {
    let Some(output) = version_output(COMMAND_NAME, &context.tool.path).await else {
        return Detection::default();
    };
    let Some(version) = output.as_deref().and_then(parse_version) else {
        return Detection::default();
    };
    if update_needed(&version) {
        return Detection {
            installed: true,
            version: Some(version),
            update_needed: true,
            ..Detection::default()
        };
    }
    let (signed_in, account) = signed_in(context).await;
    Detection {
        installed: true,
        signed_in,
        version: Some(version),
        update_needed: false,
        account,
    }
}

/// `GitHub Copilot CLI 0.0.404.` (then a second line) → `0.0.404`.
pub(super) fn parse_version(output: &str) -> Option<String> {
    let first = output.lines().next()?.trim();
    let version = first.split_whitespace().last()?.trim_end_matches('.');
    numbers(version).map(|_| version.to_owned())
}

/// Whether `version` is older than [`OLDEST`].
pub(super) fn update_needed(version: &str) -> bool {
    numbers(version).is_none_or(|found| found < OLDEST)
}

/// `1.0.98` → `[1, 0, 98]`; a pre-release suffix (`1.0.98-1`) is ignored.
fn numbers(version: &str) -> Option<[u32; 3]> {
    let core = version.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u32>().ok());
    let found = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(found)
}

/// Whether Copilot is signed in, and as whom ("signed in as <login>"). T-0353: `config.json`'s `logged_in_users` /
/// `last_logged_in_user`, or a Copilot token variable. `None`: not known yet.
pub(super) async fn signed_in(_context: &CopilotContext) -> (Option<bool>, Option<String>) {
    (None, None)
}

/// **Sign in…**: T-0353 verifies Copilot's login command.
pub(super) fn login_command(_context: &CopilotContext) -> Option<Vec<String>> {
    None
}
