//! OpenCode's tokens (USG-09; T-0354): its plugin's `session.usage.updated` events ([`super::hooks`]); no limit
//! windows ("Limits not reported", d0086): OpenCode has no plan or remaining-limit data, and a limit shows only as
//! its rate-limit retry, which carries its own reset.

use std::path::Path;

use serde_json::Value;

use super::OpenCodeContext;
use crate::agent_tools::events::LimitWindow;
use crate::agent_tools::{TokenSource, WindowsSource};
use crate::usage::limit::LimitKind;

pub(super) fn token_source(_context: &OpenCodeContext, _tool_id: &str, _cwd: &Path) -> TokenSource {
    TokenSource::Events
}

pub(super) fn windows_source() -> WindowsSource {
    WindowsSource::NotReported
}

pub(super) fn windows_from_report(_report: &Value) -> Option<Vec<LimitWindow>> {
    None
}

pub(super) fn limit_window_key(_kind: LimitKind) -> Option<&'static str> {
    None
}

pub(super) fn limit_reset_from_text(_text: &str, _now: i64) -> Option<i64> {
    None
}
