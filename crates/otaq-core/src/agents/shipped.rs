//! What Otaq ships about agents (AGM-02, d0108): the account-level `otaq-agents.yaml` (the Project agent **Planner**
//! and the **Epic lead**), embedded at build time like the default flow, and the skills a shipped skill may name.
//! A flow file's agents (the default flow's Developer and Validator, the test flow's) become stored agents through
//! [`new_agent`].
//!
//! A file's `model:` is `default` (the account's default model; every shipped file says it), a tool's stored name
//! (`claude`: its default model) or `<tool>:<model>` (`claude:opus`).

use std::sync::LazyLock;

use super::store::NewAgent;
use super::{AgentModel, Role, SkillRef};
use crate::flow::FlowAgent;
use crate::flow::yaml;

pub use crate::naming::{AGENTS_FILE, AGENTS_YAML};

/// The skills Otaq ships (`plugin/skills/<name>/`): the names an agent's shipped skill can take.
pub const SKILLS: &[&str] = &[
    "address-review",
    "answer-question",
    "commit",
    "epic-lead",
    "open-pr",
    "project-agent",
    "push",
    "restack",
    "validate",
    "work-ticket",
];

static AGENTS: LazyLock<Vec<FlowAgent>> = LazyLock::new(|| {
    yaml::read_agents(AGENTS_YAML).expect("the shipped agents file reads (tested)")
});

/// The shipped agents file's agents, as read.
pub fn agents_file() -> &'static [FlowAgent] {
    &AGENTS
}

/// The model a file's `model:` says, or why it can't be read.
pub fn model_of(text: &str) -> Result<AgentModel, String> {
    let text = text.trim();
    if text == "default" {
        return Ok(AgentModel::Default);
    }
    let (tool, model) = match text.split_once(':') {
        Some((tool, model)) => (
            tool,
            Some(model.trim().to_owned()).filter(|m| !m.is_empty()),
        ),
        None => (text, None),
    };
    let tool = tool
        .trim()
        .parse()
        .map_err(|_| format!("{text} isn't a model: say `default`, a tool, or `tool:model`"))?;
    Ok(AgentModel::Tool { tool, model })
}

/// A file's agent as stored: its shipped skills by name, its role when it names one, and its model (`default` when
/// the file's doesn't read).
pub fn new_agent(agent: &FlowAgent) -> NewAgent {
    NewAgent {
        name: agent.name.trim().to_owned(),
        instructions: agent.instructions.clone(),
        skills: agent
            .skills
            .iter()
            .map(|s| SkillRef::Shipped(s.clone()))
            .collect(),
        imported: Vec::new(),
        model: model_of(&agent.model).unwrap_or(AgentModel::Default),
        effort: None,
        role: agent.role.as_deref().and_then(|r| r.parse::<Role>().ok()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_tools::Tool;

    #[test]
    fn the_agents_file_reads_and_writes_back() {
        let agents = agents_file();
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Planner", "Epic lead"]);
        assert_eq!(yaml::write_agents(agents), AGENTS_YAML);
        let planner = new_agent(&agents[0]);
        assert_eq!(planner.role, Some(Role::ProjectAgent));
        assert_eq!(planner.skills, [SkillRef::Shipped("project-agent".into())]);
        assert_eq!(planner.model, AgentModel::Default);
        assert_eq!(new_agent(&agents[1]).role, Some(Role::EpicLead));
    }

    #[test]
    fn models_read_as_default_a_tool_or_tool_and_model() {
        assert_eq!(model_of("default"), Ok(AgentModel::Default));
        assert_eq!(
            model_of("claude"),
            Ok(AgentModel::Tool {
                tool: Tool::Claude,
                model: None
            })
        );
        assert_eq!(
            model_of("opencode:anthropic/claude-sonnet-5-5#high"),
            Ok(AgentModel::Tool {
                tool: Tool::OpenCode,
                model: Some("anthropic/claude-sonnet-5-5#high".into())
            })
        );
        assert!(model_of("gpt").is_err());
    }
}
