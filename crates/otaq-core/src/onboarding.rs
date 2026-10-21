//! Who Flow onboarding can ask (ASK-02, ASK-05; d0108 "Each connected tool's agent looks for itself, when the user
//! asks", d0086): the connected agent tools, one **Ask <Tool>** each; none shows the presets. Otaq checks no
//! sources and reads no history: each tool's flow agent looks for itself.

use serde::{Deserialize, Serialize};

use crate::agent_tools::Tool;
use crate::agent_tools::connected::ConnectedTools;
use crate::board::{Board, BoardError};

/// The answer to `onboarding.sources`: the project and the tools the go-ahead offers to ask.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sources {
    /// The project's name, for "proposes a flow for shop-web".
    pub project_name: String,
    /// The connected tools, in d0086's order.
    pub tools: Vec<Tool>,
}

impl Sources {
    /// Whether any tool can be asked (else the presets, "Connect an agent tool…").
    pub fn any(&self) -> bool {
        !self.tools.is_empty()
    }
}

/// Who can be asked for `project` (its key): the tools connected now.
pub async fn sources(
    board: &Board,
    connected: &ConnectedTools,
    project: &str,
) -> Result<Sources, BoardError> {
    let project = board.project(project).await?;
    Ok(Sources {
        project_name: project.name,
        tools: connected.connected(),
    })
}
