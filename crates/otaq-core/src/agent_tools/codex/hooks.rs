//! Codex's hooks (ADP-07, ADP-02; T-0350): `SessionStart` (the tool id and the model), `PermissionRequest`, `Stop`,
//! `Interrupt`, `UserPromptSubmit`. Until T-0350 lands, every hook is [`ToolEvent::Unrecognised`], logged raw.

use serde_json::Value;

use crate::agent_tools::{SignalContext, Tool, ToolEvent, unrecognised};

pub(super) fn map_hook(
    event: &str,
    payload: &Value,
    _context: SignalContext<'_>,
) -> Vec<ToolEvent> {
    unrecognised(Tool::Codex, event, payload)
}
