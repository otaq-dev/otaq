//! The onboarding start (ONB-05, d0108): **Look and propose a flow** opens the project's first Project agent on the
//! `flow` skill, through [`ProjectAgents::open`] (the Project agent role's model, the check before every launch),
//! with a first message naming the project and what there is to look at (ONB-03's [`Sources`]). The skill's "A first
//! flow" section says how to read each source. While a `/flow` Project agent of the project runs, a second start
//! returns it instead of opening another (one proposal per project waits at a time).

use super::{AgentState, FLOW_COMMAND, ProjectAgent, ProjectAgents, first_skill};
use crate::board::{BoardError, Refusal};
use crate::naming;
use crate::onboarding::Sources;

/// Why the start is refused when there is nothing to look at: the go-ahead shows the presets instead (ONB-03).
pub const NOTHING_TO_LOOK_AT: &str = "Nothing to look at yet: pick a preset instead";

/// The onboarding agent's first message: `/flow` and what there is ("This is shop-web's first flow. Look at how the
/// user works: their Claude Code history (`otaq history`), this repo, GitHub (`gh`). Then propose a whole flow.").
/// `None` when there is nothing to look at.
pub fn first_flow_message(sources: &Sources) -> Option<String> {
    let cmd = naming::COMMAND_NAME;
    let mut what = Vec::new();
    if sources.history {
        what.push(format!("their Claude Code history (`{cmd} history`)"));
    }
    if sources.commits {
        what.push("this repo".to_owned());
    }
    if sources.gh {
        what.push("GitHub (`gh`)".to_owned());
    }
    if sources.glab {
        what.push("GitLab (`glab`)".to_owned());
    }
    (!what.is_empty()).then(|| {
        format!(
            "{FLOW_COMMAND} This is {}'s first flow. Look at how the user works: {}. Then propose a whole flow.",
            sources.project_name,
            what.join(", ")
        )
    })
}

impl ProjectAgents {
    /// **Look and propose a flow** for `project` (ONB-05): opens a Project agent with [`first_flow_message`], or
    /// returns the project's running `/flow` Project agent. Refused with nothing to look at, and while the
    /// Project agent's tool is at its limit.
    pub async fn start_flow(
        &self,
        project: &str,
        sources: &Sources,
    ) -> Result<ProjectAgent, BoardError> {
        if let Some(running) = self.running_flow_agent(project).await? {
            return Ok(running);
        }
        let Some(message) = first_flow_message(sources) else {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                NOTHING_TO_LOOK_AT,
            ));
        };
        self.refuse_during_limit(Some(sources.tool)).await?;
        self.open(project, &message).await
    }

    /// The project's open, running Project agent that started on `/flow` (not an epic lead), if any.
    async fn running_flow_agent(&self, project: &str) -> Result<Option<ProjectAgent>, BoardError> {
        let project = self.inner.board.project(project).await?;
        let rows: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT id, first_request FROM project_agents
             WHERE project_id = ? AND state = ? AND closed_at IS NULL AND epic_id IS NULL
             ORDER BY created_at, id",
        )
        .bind(&project.id)
        .bind(AgentState::Running.as_str())
        .fetch_all(self.inner.board.store().pool())
        .await?;
        let Some((id, _)) = rows
            .into_iter()
            .find(|(_, first)| first.as_deref().is_some_and(|m| first_skill(m).0 == "flow"))
        else {
            return Ok(None);
        };
        self.get(&id).await.map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_tools::Tool;

    fn sources(history: bool, commits: bool, gh: bool, glab: bool) -> Sources {
        Sources {
            project_name: "shop-web".into(),
            history,
            commits,
            gh,
            glab,
            tool: Tool::Claude,
        }
    }

    #[test]
    fn the_message_names_the_project_and_only_what_there_is() {
        let cmd = naming::COMMAND_NAME;
        assert_eq!(
            first_flow_message(&sources(true, true, true, false)).unwrap(),
            format!(
                "/flow This is shop-web's first flow. Look at how the user works: their Claude Code history \
                 (`{cmd} history`), this repo, GitHub (`gh`). Then propose a whole flow."
            )
        );
        assert_eq!(
            first_flow_message(&sources(false, true, false, true)).unwrap(),
            "/flow This is shop-web's first flow. Look at how the user works: this repo, GitLab (`glab`). Then \
             propose a whole flow."
        );
        assert_eq!(
            first_flow_message(&sources(false, false, false, false)),
            None
        );
    }

    #[test]
    fn the_message_runs_the_flow_skill() {
        let message = first_flow_message(&sources(true, false, false, false)).unwrap();
        let (skill, asks) = first_skill(&message);
        assert_eq!(skill, "flow");
        assert!(asks.unwrap().starts_with("This is shop-web's first flow."));
    }
}
