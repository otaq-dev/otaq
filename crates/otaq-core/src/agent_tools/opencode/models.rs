//! OpenCode's model list (AGT-02; T-0355): `GET /api/model` from a short-lived `opencode serve`.

use super::OpenCodeContext;
use crate::agent_tools::{ModelList, ModelListError, Tool};

pub(super) async fn read_list(_context: &OpenCodeContext) -> Result<ModelList, ModelListError> {
    Err(ModelListError {
        message: format!("{} didn't list its models", Tool::OpenCode.display_name()),
        reason: "not built yet".to_owned(),
    })
}
