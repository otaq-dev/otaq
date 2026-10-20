//! Skills by one name, in the worktree and in the checkout (AGM-07; d0108, d0092, d0098).
//!
//! Before a session starts, **every skill of its agent** is copied into its working folder as
//! `otaq-<the skill's own name>` ([`copy`]), shipped and imported alike, for all four tools; the skill the session
//! is told to run first (the stage's own skill, Q261, or Otaq's `address-review`, `open-pr`…) is copied with them.
//! Shipped skills are read from the installed app's bundle (`<plugin>/skills/<name>/`, [`SHIPPED_FOLDER`]), so they
//! update with Otaq; an imported skill from Otaq's own copy (`$OTAQ_HOME/skills/<id>/`, [`folder`]), with its own
//! folder's contents. The tools' plugins carry only Otaq's hooks, so a skill appears once, under its copy's name.
//!
//! A copy that can't be written is logged and the session still starts (the tool then says it has no such skill).

pub mod copy;
pub mod folder;
pub mod testing;

use std::path::{Path, PathBuf};

pub use copy::{Marker, marked_copies, remove_copy, write_copy};

use crate::agent_tools::FirstPrompt;
use crate::agents::{Agent, SkillRef};
use crate::board::Board;
use crate::data_dir::DataDir;
use crate::naming;

/// The folder of Otaq's shipped skills inside its plugin folder ([`crate::agent_tools::LaunchContext::plugin_dir`]):
/// `<plugin>/skills/<name>/`.
pub const SHIPPED_FOLDER: &str = "skills";

/// A skill to copy: its own name (its SKILL.md's `name:`, a shipped skill's folder) and the folder it is read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub own: String,
    pub folder: PathBuf,
}

impl Source {
    /// The copy's name: `otaq-commit-compact`.
    pub fn copy_name(&self) -> String {
        naming::copy_name(&self.own)
    }
}

/// Where skills are read from: the bundle's shipped skills (`None` when Otaq's plugin wasn't found) and Otaq's
/// copies of imported ones.
#[derive(Debug, Clone, Copy)]
pub struct Sources<'a> {
    pub shipped: Option<&'a Path>,
    pub data: &'a DataDir,
}

impl Sources<'_> {
    /// The shipped skill `name`: `<plugin>/skills/<name>/` (a folder that isn't there when the plugin wasn't
    /// found: its copy fails and is logged).
    pub fn shipped(&self, name: &str) -> Source {
        Source {
            own: name.to_owned(),
            folder: self.shipped.map(|dir| dir.join(name)).unwrap_or_default(),
        }
    }

    /// The imported skill `id` of `agent`, read from Otaq's copy, when that copy is there and reads (WSK-04).
    fn imported(&self, agent: &Agent, id: &str) -> Option<Source> {
        let skill = agent.imported_skill(id)?;
        let folder = self.data.skill_path(id).ok()?;
        folder::copy_problem(&folder).is_none().then(|| Source {
            own: skill.name.clone(),
            folder,
        })
    }

    /// Every skill of `agent`, in its order.
    pub fn of_agent(&self, agent: &Agent) -> Vec<Source> {
        agent
            .skills
            .iter()
            .filter_map(|skill| match skill {
                SkillRef::Shipped(name) => Some(self.shipped(name)),
                SkillRef::Imported(id) => self.imported(agent, id),
            })
            .collect()
    }

    /// The skill a session runs for a stage's or step's skill `name` (Q261): `agent`'s imported skill standing for
    /// it (AGM-02's move-over: an imported Validate skill in place of `validate`), else its skill of that name,
    /// else the shipped one.
    pub fn runs(&self, agent: Option<&Agent>, name: &str) -> Source {
        let Some(agent) = agent else {
            return self.shipped(name);
        };
        let standing_for = agent.skills.iter().find_map(|skill| match skill {
            SkillRef::Imported(id) => agent
                .imported_skill(id)
                .filter(|s| s.replaces.as_deref() == Some(name))
                .and_then(|_| self.imported(agent, id)),
            SkillRef::Shipped(_) => None,
        });
        let own_name = || {
            agent.skills.iter().find_map(|skill| match skill {
                SkillRef::Imported(id) => self.imported(agent, id).filter(|s| s.own == name),
                SkillRef::Shipped(_) => None,
            })
        };
        standing_for
            .or_else(own_name)
            .unwrap_or_else(|| self.shipped(name))
    }
}

/// What a session's copies came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Written {
    /// The skill the session runs first, by its own name as resolved for the agent (`team-validate` for a stage's
    /// `validate`, [`Sources::runs`]); its prompt names its copy (`/otaq-team-validate`).
    pub runs: Option<String>,
    /// The copies written.
    pub names: Vec<String>,
    /// The copies that couldn't be written, and why.
    pub failures: Vec<(String, String)>,
}

/// Writes `runs` (the skill the session runs first) and `skills` into `root`, each once: when two skills would get
/// the same copy name, the first wins (`runs` first). Removes M27's `otaq-custom-*` copies. Blocking.
pub fn write_session(
    root: &Path,
    runs: Option<Source>,
    skills: Vec<Source>,
    marker: Marker,
) -> Written {
    copy::remove_old_copies(root);
    let mut written = Written {
        runs: runs.as_ref().map(|s| s.own.clone()),
        ..Written::default()
    };
    let mut seen: Vec<(String, PathBuf)> = Vec::new();
    for source in runs.into_iter().chain(skills) {
        let name = source.copy_name();
        if let Some((_, folder)) = seen.iter().find(|(n, _)| *n == name) {
            if *folder != source.folder {
                tracing::warn!(
                    copy = name,
                    kept = %folder.display(),
                    left_out = %source.folder.display(),
                    "two skills share a copy name; the first is copied"
                );
            }
            continue;
        }
        seen.push((name.clone(), source.folder.clone()));
        match write_copy(root, &name, &source.folder, marker) {
            Ok(()) => written.names.push(name),
            Err(e) => written.failures.push((name, e.to_string())),
        }
    }
    written
}

impl Board {
    /// The copies right before a session starts in (or is told to run a skill in) `root` (AGM-07): every skill of
    /// `agent` and the skill `runs`, resolved for it ([`Sources::runs`]), from the bundle's shipped skills at
    /// `shipped`. A copy that can't be written is logged; the session still starts.
    pub async fn copy_skills(
        &self,
        root: &Path,
        shipped: Option<&Path>,
        agent: Option<&Agent>,
        runs: Option<&str>,
        marker: Marker,
    ) -> Written {
        let sources = Sources {
            shipped,
            data: self.data(),
        };
        let runs = runs.map(|name| sources.runs(agent, name));
        let skills = agent.map(|a| sources.of_agent(a)).unwrap_or_default();
        let at = root.to_owned();
        let written = tokio::task::spawn_blocking(move || write_session(&at, runs, skills, marker))
            .await
            .expect("skill copy panicked");
        for (name, why) in &written.failures {
            tracing::warn!(
                folder = %root.display(),
                copy = name,
                error = %why,
                "a skill wasn't copied for the session"
            );
        }
        written
    }

    /// [`Self::copy_skills`] for a session about to start with `first_prompt`: a skill prompt runs the skill as
    /// resolved for `agent` (its imported skill standing for the stage's skill), with the message kept; text and no
    /// prompt copy only the agent's skills. Returns the prompt to launch with and what was written.
    pub async fn copy_for_session(
        &self,
        root: &Path,
        shipped: Option<&Path>,
        agent: Option<&Agent>,
        first_prompt: Option<FirstPrompt>,
        marker: Marker,
    ) -> (Option<FirstPrompt>, Written) {
        let runs = match &first_prompt {
            Some(FirstPrompt::Skill { name, .. }) => Some(name.as_str()),
            _ => None,
        };
        let written = self.copy_skills(root, shipped, agent, runs, marker).await;
        let prompt = match (first_prompt, &written.runs) {
            (Some(FirstPrompt::Skill { message, .. }), Some(name)) => Some(FirstPrompt::Skill {
                name: name.clone(),
                message,
            }),
            (prompt, _) => prompt,
        };
        (prompt, written)
    }

    /// The skill the PR path runs for `step` (Open PR, Push), by its own name: the Delivery stage's agent's
    /// imported skill standing for it, else Otaq's (`open-pr`). The Open PR flow's lines and REV-11's "use your own
    /// Open PR skill" name it (WSK-08).
    pub async fn delivery_skill(
        &self,
        project: &str,
        step: crate::board::settings::WorkflowStep,
    ) -> Result<String, crate::board::BoardError> {
        let agent = crate::agents::resolve::delivery_agent(self, project)
            .await
            .ok();
        let sources = Sources {
            shipped: None,
            data: self.data(),
        };
        Ok(sources.runs(agent.as_ref(), step.own_skill()).own)
    }
}
