//! `otaq hook --tool <tool> <event>`: what an agent tool's hooks run (PLG-04, ADP-03; d0009, d0042, d0088).
//!
//! Reads the hook's JSON from stdin and passes it to the service with the tool, the event name as the tool spells
//! it and the session's role (validator, Project agent; d0063, d0068). The service hands it to that tool's
//! adapter, which turns it into Otaq's events. It never gets in the agent's way: with both `$OTAQ_TICKET` and the
//! role unset (an ordinary session of the tool) it does nothing at all, and otherwise it prints nothing, never
//! starts the service, ignores a tool it doesn't know, gives up after [`TIMEOUT`] and always exits 0 (a non-zero
//! exit would show the user a hook error).

use std::io::Read;
use std::time::Duration;

use otaq_core::agent_tools::Tool;
use otaq_core::client::request_once;
use otaq_core::data_dir::DataDir;
use otaq_core::protocol::{HookParams, Request};
use serde_json::Value;

use crate::service::ticket_env;

/// The longest a hook may take, connection included.
pub const TIMEOUT: Duration = Duration::from_secs(3);
/// More stdin than this isn't a hook payload; the rest is ignored.
const MAX_PAYLOAD: u64 = 4 * 1024 * 1024;

pub fn run(tool: &str, event: &str) {
    let role = std::env::var(otaq_core::naming::role_env_var())
        .ok()
        .filter(|r| !r.trim().is_empty());
    if ticket_env().is_none() && role.is_none() {
        return;
    }
    let Ok(tool) = tool.parse::<Tool>() else {
        return;
    };
    let mut raw = String::new();
    let _ = std::io::stdin().take(MAX_PAYLOAD).read_to_string(&mut raw);
    let payload = serde_json::from_str(&raw).unwrap_or(Value::Null);
    let Ok(data) = DataDir::from_env() else {
        return;
    };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    let request = Request::Hook(HookParams {
        tool,
        event: event.to_owned(),
        payload,
        role,
    });
    let _ = runtime.block_on(async {
        tokio::time::timeout(TIMEOUT, request_once(&data.socket_path(), request)).await
    });
}
