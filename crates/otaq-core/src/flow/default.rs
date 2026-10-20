//! The default flow and the derived flow (STG-03, d0108, d0004). Otaq ships the default as a file, embedded at build
//! time; every project uses it in M31. Until M33 removes the settings, a project's flow is the default adjusted by
//! its Validation settings: off removes Validate, and Validation rounds set Validate's rounds. Auto-merge isn't part
//! of the flow in M31; it keeps today's behaviour on top of the stages.

use std::sync::LazyLock;

use super::model::{Flow, FlowFile};
use super::yaml;
use crate::board::settings::EffectiveSettings;
use crate::board::{Board, BoardError};
use crate::naming;

/// The default flow's stage ids.
pub const DEV: &str = "dev";
pub const VALIDATE: &str = "validate";
pub const REVIEW: &str = "review";
pub const DELIVER: &str = "deliver";

static DEFAULT: LazyLock<FlowFile> = LazyLock::new(|| {
    yaml::read(naming::DEFAULT_FLOW_YAML).expect("the shipped default flow reads (tested)")
});

/// The shipped default flow file, as read.
pub fn default_file() -> &'static FlowFile {
    &DEFAULT
}

/// The shipped default flow: Dev → Validate → Review → Done.
pub fn default_flow() -> Flow {
    DEFAULT.flow.clone()
}

/// A project's flow from its effective settings ([`crate::board::settings::effective`]).
pub fn effective_flow(settings: &EffectiveSettings) -> Flow {
    let mut flow = default_flow();
    if settings.validation {
        if let Some(validate) = flow.stages.iter_mut().find(|s| s.id == VALIDATE) {
            validate.rounds = Some(settings.validation_rounds);
        }
    } else {
        flow.stages.retain(|s| s.id != VALIDATE);
    }
    flow
}

impl Board {
    /// The flow of a project (by name or key), read from its settings now.
    pub async fn effective_flow(&self, project: &str) -> Result<Flow, BoardError> {
        if let Some(flow) = self.test_flow(project) {
            return Ok(flow);
        }
        Ok(effective_flow(&self.effective_settings(project).await?))
    }

    /// Tests only: puts the project `key` on `file`'s flow (the test flow, `tests/flows/every-kind.yaml`) for this
    /// board's life, wherever the board reads a project's flow, and creates the file's agents that aren't there yet
    /// ("Added by <key>'s flow"; a role another agent holds stays with it). Only the flow isn't stored: storing
    /// applied flows is M33's.
    #[doc(hidden)]
    pub async fn use_flow_for_tests(&self, key: &str, file: &FlowFile) {
        let store = self.store();
        for agent in &file.agents {
            if store
                .agent_named(&agent.name)
                .await
                .expect("agents read")
                .is_some()
            {
                continue;
            }
            let mut new = crate::agents::shipped::new_agent(agent);
            if let Some(role) = new.role
                && store.role_agent(role).await.expect("agents read").is_some()
            {
                new.role = None;
            }
            store
                .insert_agent(
                    &new,
                    &crate::agents::CameFrom::Flow {
                        project: key.to_owned(),
                    },
                    None,
                )
                .await
                .expect("a test flow's agent added");
        }
        self.test_flows
            .lock()
            .expect("test flows")
            .insert(key.to_owned(), file.flow.clone());
    }

    pub(crate) fn test_flow(&self, project: &str) -> Option<Flow> {
        self.test_flows
            .lock()
            .expect("test flows")
            .get(project)
            .cloned()
    }
}
