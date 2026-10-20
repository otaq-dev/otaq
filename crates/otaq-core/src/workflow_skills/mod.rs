//! Imported skills' files (WSK-02…WSK-05; d0092): the checks and Otaq's own copy of an imported skill folder
//! ([`folder`]), and the worktree copy of the skill a workflow step runs ([`worktree`], until T-0408's copy writer).
//! Which agent holds which imported skill is the agents' ([`crate::agents::skills`], AGM-01); the skill each step
//! runs is [`crate::agents::skills::step_skills`]'s.

pub mod folder;
pub mod testing;
pub mod worktree;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::board::settings::WorkflowStep;

pub use folder::{copy_missing, copy_problem};

/// The skill a step runs (WSK-07), its copy checked: the built-in, or an imported skill with Otaq's copy's folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedSkill {
    BuiltIn(WorkflowStep),
    Imported { name: String, folder: PathBuf },
}

impl ResolvedSkill {
    pub fn is_built_in(&self) -> bool {
        matches!(self, Self::BuiltIn(_))
    }

    /// How Settings and the Open PR flow name it: the built-in as Otaq's own skill (`/otaq:open-pr`), an imported
    /// one by its `name:`.
    pub fn label(&self) -> String {
        match self {
            Self::BuiltIn(step) => step.own_default(),
            Self::Imported { name, .. } => name.clone(),
        }
    }
}

/// The skill each step runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedSkills {
    pub commit: ResolvedSkill,
    pub push: ResolvedSkill,
    pub open_pr: ResolvedSkill,
    pub validate: ResolvedSkill,
}

impl ResolvedSkills {
    pub fn get(&self, step: WorkflowStep) -> &ResolvedSkill {
        match step {
            WorkflowStep::Commit => &self.commit,
            WorkflowStep::Push => &self.push,
            WorkflowStep::OpenPr => &self.open_pr,
            WorkflowStep::Validate => &self.validate,
        }
    }
}

/// Runs blocking file work off the service's async threads.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(work)
        .await
        .expect("workflow skill file work panicked")
}
