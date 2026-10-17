//! Codex's model list (AGT-02; T-0351): `codex app-server`'s `model/list`, hidden models left out.

use super::CodexContext;
use crate::agent_tools::{ModelList, ModelListError, Tool};

pub(super) async fn read_list(_context: &CodexContext) -> Result<ModelList, ModelListError> {
    Err(ModelListError {
        message: format!("{} didn't list its models", Tool::Codex.display_name()),
        reason: "not built yet".to_owned(),
    })
}
