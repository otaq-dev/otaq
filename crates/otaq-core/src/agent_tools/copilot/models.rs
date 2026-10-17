//! Copilot's model list and Auto tiers (AGT-02; T-0353): `copilot --acp`'s `session/new` `availableModels`,
//! de-duplicated by `modelId`.

use super::CopilotContext;
use crate::agent_tools::{ModelList, ModelListError, Tool};

pub(super) async fn read_list(_context: &CopilotContext) -> Result<ModelList, ModelListError> {
    Err(ModelListError {
        message: format!("{} didn't list its models", Tool::Copilot.display_name()),
        reason: "not built yet".to_owned(),
    })
}
