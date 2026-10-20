//! Settings → Agents as the page draws it until T-0410 draws the new page (AGM-04; d0108, d0088): the list of agents
//! (name, role, where it came from, model as Settings shows it with the tool's effort, skills, the amber mark) and the
//! default model; editing comes with T-0410's page. Also the shared pieces the other screens use: the one formatter of
//! a model as Settings shows it ([`model_label`], [`agent_model`]), the agent tools' lists ([`AgentTools`]) and the
//! session roles as the page sends them back ([`RoleId`]).

use otaq_core::agent_tools::{ModelKind, ModelList, Tool};
use otaq_core::agents::{Agent, AgentList, AgentModel, Mark, ModelRef, SessionRole, SkillRef};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::tool_avatar::{AvatarTool, ToolAvatarView, shown_with};

/// The tool default as a field or a table cell shows it.
pub const TOOL_DEFAULT_LABEL: &str = "Tool default";
/// The tool default beside an agent's name ("Claude Code · tool default").
pub const TOOL_DEFAULT: &str = "tool default";
/// An agent on the account's default model (AGM-03): "default · Claude Code · sonnet".
pub const DEFAULT: &str = "default";

/// Whether `model` means the tool default: nothing, or the entry the tool lists as its own default.
pub fn is_tool_default(model: Option<&str>, list: Option<&ModelList>) -> bool {
    match model.map(str::trim) {
        None | Some("") => true,
        Some(id) => match list.and_then(|l| l.entry(id)) {
            Some(entry) => entry.kind == ModelKind::ToolDefault,
            None => id == "default" && list.is_none(),
        },
    }
}

/// A model as Settings shows it (AGT-02, AUDIT-y C2): "tool default", "<Display> · always the newest" for a follow
/// choice that resolves to one model, just its name for one that resolves per task (Copilot's "Auto · balance", Q-A),
/// else the exact id. A model the list doesn't have (or no list yet) shows as saved.
pub fn model_label(model: Option<&str>, list: Option<&ModelList>) -> String {
    if is_tool_default(model, list) {
        return TOOL_DEFAULT.to_owned();
    }
    let id = model.unwrap_or_default().trim();
    match list.and_then(|l| l.entry(id)) {
        Some(e) if e.kind == ModelKind::Follow && e.resolves_to.is_none() => e.display_name.clone(),
        Some(e) if e.kind == ModelKind::Follow => {
            format!("{} \u{b7} always the newest", e.display_name)
        }
        _ => id.to_owned(),
    }
}

/// The tool's name as people read it.
fn tool_name(tool: Tool) -> &'static str {
    AvatarTool::from(tool).name()
}

/// The tool's name in a sentence ("Copilot", "Claude Code"), as the panel's lines say it.
pub use super::tool_problems::tool_word;

/// The tool a new agent's model gets before the service said which tools are connected: the first.
pub fn new_entry_tool() -> Tool {
    Tool::ALL[0]
}

/// Where a tool's model list stands for the picker (`GTuGI`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListState<'a> {
    /// Otaq's copy (the last one, also when a later read failed).
    Listed(&'a ModelList),
    /// No copy yet and the tool is connected: its first read runs.
    Loading,
    /// No copy, and the last read gave none: "Couldn't load … · Retry".
    Failed,
    /// No copy of a tool that isn't connected: nothing to say (as M22 before the first read).
    Unread,
}

impl<'a> ListState<'a> {
    pub fn list(self) -> Option<&'a ModelList> {
        match self {
            Self::Listed(list) => Some(list),
            _ => None,
        }
    }
}

/// What Settings → Agents knows about the agent tools (d0086): the connected ones (only Connected counts), each
/// tool's cached model list, and the tools whose last read gave no list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentTools {
    pub connected: Vec<Tool>,
    pub lists: Vec<(Tool, ModelList)>,
    pub failed: Vec<Tool>,
}

impl AgentTools {
    /// The first tool connected with its list: the tests' default.
    pub fn one(list: Option<ModelList>) -> Self {
        let tool = Tool::ALL[0];
        Self {
            connected: vec![tool],
            lists: list.map(|l| (tool, l)).into_iter().collect(),
            failed: Vec::new(),
        }
    }

    /// Two or more tools connected: tool names and avatars show.
    pub fn several(&self) -> bool {
        shown_with(
            &self
                .connected
                .iter()
                .copied()
                .map(AvatarTool::from)
                .collect::<Vec<_>>(),
        )
    }

    pub fn is_connected(&self, tool: Tool) -> bool {
        self.connected.contains(&tool)
    }

    /// `tool`'s cached list.
    pub fn list(&self, tool: Tool) -> Option<&ModelList> {
        self.lists.iter().find(|(t, _)| *t == tool).map(|(_, l)| l)
    }

    /// Where `tool`'s list stands.
    pub fn state(&self, tool: Tool) -> ListState<'_> {
        match self.list(tool) {
            Some(list) => ListState::Listed(list),
            None if self.failed.contains(&tool) => ListState::Failed,
            None if self.is_connected(tool) => ListState::Loading,
            None => ListState::Unread,
        }
    }

    /// The first connected tool (in d0086's order).
    pub fn first(&self) -> Tool {
        Tool::ALL
            .iter()
            .copied()
            .find(|t| self.is_connected(*t))
            .unwrap_or_else(new_entry_tool)
    }

    /// The avatar of `tool`, only with several tools connected (SHL-08).
    pub fn avatar(&self, tool: Tool) -> Option<ToolAvatarView> {
        self.several().then(|| ToolAvatarView::of(tool, None))
    }

    /// A tool and model as Settings shows it: with several tools its tool first ("Claude Code · Opus · always the
    /// newest").
    pub fn model_ref(&self, model: &ModelRef) -> String {
        let label = model_label(model.model.as_deref(), self.list(model.tool));
        if self.several() {
            format!("{} \u{b7} {label}", tool_name(model.tool))
        } else {
            label
        }
    }
}

/// An agent's model as Settings shows it (AGM-01, AGM-03): its tool's model with the effort ("Opus · always the newest
/// · effort high", with several tools the tool first), or "default" with what it is today ("default · Claude Code ·
/// sonnet").
pub fn agent_model(agent: &Agent, tools: &AgentTools, default: Option<&ModelRef>) -> String {
    match &agent.model {
        AgentModel::Default => match default {
            Some(model) => {
                let label = model_label(model.model.as_deref(), tools.list(model.tool));
                format!("{DEFAULT} \u{b7} {} \u{b7} {label}", tool_name(model.tool))
            }
            None => DEFAULT.to_owned(),
        },
        AgentModel::Tool { tool, model } => {
            let mut text = tools.model_ref(&ModelRef {
                tool: *tool,
                model: model.clone(),
            });
            if let Some(effort) = &agent.effort {
                text.push_str(&format!(" \u{b7} effort {effort}"));
            }
            text
        }
    }
}

/// A session role as the page sends it back, as stored (`developer`, `validator`, `lead`, `project_agent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "AgentRole")]
pub enum RoleId {
    Developer,
    Validator,
    #[serde(rename = "lead")]
    EpicLead,
    ProjectAgent,
}

impl From<SessionRole> for RoleId {
    fn from(role: SessionRole) -> Self {
        match role {
            SessionRole::Developer => Self::Developer,
            SessionRole::Validator => Self::Validator,
            SessionRole::EpicLead => Self::EpicLead,
            SessionRole::ProjectAgent => Self::ProjectAgent,
        }
    }
}

impl From<RoleId> for SessionRole {
    fn from(role: RoleId) -> Self {
        match role {
            RoleId::Developer => Self::Developer,
            RoleId::Validator => Self::Validator,
            RoleId::EpicLead => Self::EpicLead,
            RoleId::ProjectAgent => Self::ProjectAgent,
        }
    }
}

/// The amber mark (AGT-05, `st-paused`): "Claude Code couldn't run `claude-fable-5-1` on SHOP-48", the model in mono.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentMarkView")]
pub struct MarkView {
    pub before: String,
    /// The refused model, drawn in mono between `before` and `after`.
    pub model: Option<String>,
    pub after: String,
    /// The tool's own message, in quotes.
    pub message: Option<String>,
}

/// The mark as shown, from T-0337's line: the model is cut out so it can be drawn in mono.
pub fn mark_view(mark: &Mark) -> MarkView {
    let (before, model, after) = match mark.model.as_deref().and_then(|m| {
        let at = mark.text.rfind(m)?;
        Some((at, m))
    }) {
        Some((at, m)) => (
            mark.text[..at].to_owned(),
            Some(m.to_owned()),
            mark.text[at + m.len()..].to_owned(),
        ),
        None => (mark.text.clone(), None, String::new()),
    };
    MarkView {
        before,
        model,
        after,
        message: mark.message.clone(),
    }
}

/// One agent in the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentListRow")]
pub struct TableRow {
    pub id: String,
    pub name: String,
    /// "Project agent", "Epic lead": the role tag.
    pub role: Option<String>,
    /// "From Otaq's agents file", "Added by the Otaq default preset", "Added by you".
    pub came_from: String,
    /// The model as Settings shows it ([`agent_model`]).
    pub model: String,
    /// Its skills by name, in order: a shipped one by its name, an imported one by its `name:`.
    pub skills: Vec<String>,
    pub mark: Option<MarkView>,
}

/// The Agents section: the default model and the list (AGM-04; T-0410 draws the page with its editing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentsSection")]
pub struct Section {
    pub title: &'static str,
    /// "Default model for new agents: Claude Code · sonnet"; "… the first agent tool you connect" before one is.
    pub default_model: String,
    pub rows: Vec<TableRow>,
}

/// The skills of `agent` by name.
fn skill_names(agent: &Agent) -> Vec<String> {
    agent
        .skills
        .iter()
        .map(|s| match s {
            SkillRef::Shipped(name) => name.clone(),
            SkillRef::Imported(id) => agent
                .imported_skill(id)
                .map_or_else(|| id.clone(), |i| i.name.clone()),
        })
        .collect()
}

/// The section from `agents.list`.
pub fn section(list: &AgentList, tools: &AgentTools) -> Section {
    let default = list.default_model.as_ref();
    Section {
        title: "Agents",
        default_model: match default {
            Some(model) => format!(
                "Default model for new agents: {} \u{b7} {}",
                tool_name(model.tool),
                model_label(model.model.as_deref(), tools.list(model.tool))
            ),
            None => "Default model for new agents: the first agent tool you connect".to_owned(),
        },
        rows: list
            .agents
            .iter()
            .map(|a| TableRow {
                id: a.id.clone(),
                name: a.name.clone(),
                role: a.role.map(|r| r.label().to_owned()),
                came_from: a.came_from.label(),
                model: agent_model(a, tools, default),
                skills: skill_names(a),
                mark: a.mark.as_ref().map(mark_view),
            })
            .collect(),
    }
}

/// The agent tools' model lists the tests share.
pub mod fixtures {
    use otaq_core::agent_tools::models::ModelEntry;
    use otaq_core::agent_tools::{ModelKind, ModelList, Tool};
    use otaq_core::agents::{Agent, AgentModel, CameFrom};

    pub fn entry(id: &str, display: &str, kind: ModelKind, now: Option<&str>) -> ModelEntry {
        ModelEntry {
            id: id.to_owned(),
            display_name: display.to_owned(),
            description: None,
            resolves_to: now.map(str::to_owned),
            kind,
            family: None,
            released_at: None,
        }
    }

    /// Claude Code's list as its `initialize` request gives it: its default and its four aliases.
    pub fn claude_list() -> ModelList {
        ModelList {
            entries: vec![
                entry(
                    "default",
                    "Default (recommended)",
                    ModelKind::ToolDefault,
                    Some("claude-opus-5-5"),
                ),
                entry("opus", "Opus", ModelKind::Follow, Some("claude-opus-5-5")),
                entry(
                    "sonnet",
                    "Sonnet",
                    ModelKind::Follow,
                    Some("claude-sonnet-5-5"),
                ),
                entry(
                    "haiku",
                    "Haiku",
                    ModelKind::Follow,
                    Some("claude-haiku-5-5"),
                ),
                entry(
                    "fable",
                    "Fable",
                    ModelKind::Follow,
                    Some("claude-fable-5-1"),
                ),
            ],
        }
    }

    /// An agent on Claude Code with `model` (`None`: the tool's default).
    pub fn agent(id: &str, name: &str, model: Option<&str>) -> Agent {
        Agent {
            id: id.to_owned(),
            name: name.to_owned(),
            instructions: String::new(),
            skills: Vec::new(),
            imported: Vec::new(),
            model: AgentModel::Tool {
                tool: Tool::ALL[0],
                model: model.map(str::to_owned),
            },
            effort: None,
            role: None,
            came_from: CameFrom::You,
            mark: None,
            created_at: 1,
            updated_at: 1,
        }
    }

    /// `agent` on `tool` instead.
    pub fn on(tool: Tool, mut agent: Agent) -> Agent {
        let model = match agent.model {
            AgentModel::Tool { model, .. } => model,
            AgentModel::Default => None,
        };
        agent.model = AgentModel::Tool { tool, model };
        agent
    }

    pub fn codex_list() -> ModelList {
        ModelList {
            entries: vec![
                entry(
                    "newest:gpt-sol",
                    "GPT Sol",
                    ModelKind::Follow,
                    Some("gpt-6-sol"),
                ),
                entry(
                    "default",
                    "Default",
                    ModelKind::ToolDefault,
                    Some("gpt-6-astra"),
                ),
                entry("gpt-6-sol", "GPT-6-Sol", ModelKind::Exact, None),
                entry("gpt-5.5", "GPT-5.5", ModelKind::Exact, None),
                entry("gpt-5.5-pro", "GPT-5.5-Pro", ModelKind::Exact, None),
                entry("gpt-5.5-mini", "GPT-5.5-Mini", ModelKind::Exact, None),
            ],
        }
    }

    /// Copilot's list (T-0353): the three Auto tiers (resolved per task) with Copilot's descriptions, and one exact
    /// model a paid account lists.
    pub fn copilot_list() -> ModelList {
        let auto = |tier: &str, display: &str, description: &str| ModelEntry {
            description: Some(description.to_owned()),
            ..entry(&format!("auto@{tier}"), display, ModelKind::Follow, None)
        };
        ModelList {
            entries: vec![
                auto(
                    "efficiency",
                    "Auto \u{b7} efficiency",
                    "Fast and light on credits",
                ),
                auto(
                    "balance",
                    "Auto \u{b7} balance",
                    "A balance of speed and quality",
                ),
                auto(
                    "intelligence",
                    "Auto \u{b7} intelligence",
                    "The most capable model for the task",
                ),
                entry("gpt-5.5", "GPT-5.5", ModelKind::Exact, None),
            ],
        }
    }

    /// OpenCode's list (T-0355): its default, a family, exact `provider/id` models.
    pub fn opencode_list() -> ModelList {
        ModelList {
            entries: vec![
                entry(
                    "newest:glm-flash",
                    "GLM Flash",
                    ModelKind::Follow,
                    Some("opencode/glm-5-flash"),
                ),
                entry(
                    "default",
                    "Default",
                    ModelKind::ToolDefault,
                    Some("opencode/glm-5-flash"),
                ),
                entry(
                    "opencode/glm-5-flash",
                    "GLM 5 Flash",
                    ModelKind::Exact,
                    None,
                ),
                entry(
                    "google/gemini-3-pro",
                    "Gemini 3 Pro",
                    ModelKind::Exact,
                    None,
                ),
            ],
        }
    }

    /// Every tool's list.
    pub fn all_lists() -> Vec<(Tool, ModelList)> {
        vec![
            (Tool::Claude, claude_list()),
            (Tool::Codex, codex_list()),
            (Tool::Copilot, copilot_list()),
            (Tool::OpenCode, opencode_list()),
        ]
    }

    /// `connected` tools, with every tool's list.
    pub fn tools(connected: &[Tool]) -> super::AgentTools {
        super::AgentTools {
            connected: connected.to_vec(),
            lists: all_lists(),
            failed: Vec::new(),
        }
    }

    /// Claude Code and Codex connected (`soGMU`).
    pub fn two_tools() -> super::AgentTools {
        tools(&[Tool::Claude, Tool::Codex])
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use otaq_core::agents::{AgentList, CameFrom, ImportedSkill, Role};
    use otaq_core::naming::PRODUCT_NAME;

    #[test]
    fn models_read_as_settings_shows_them() {
        let list = claude_list();
        assert_eq!(model_label(None, Some(&list)), "tool default");
        assert_eq!(model_label(Some("default"), Some(&list)), "tool default");
        assert_eq!(
            model_label(Some("opus"), Some(&list)),
            "Opus \u{b7} always the newest"
        );
        assert_eq!(model_label(Some("claude-x"), Some(&list)), "claude-x");
        let tools = AgentTools::one(Some(list));
        let mut strong = agent("s", "Strong", Some("opus"));
        strong.effort = Some("high".into());
        assert_eq!(
            agent_model(&strong, &tools, None),
            "Opus \u{b7} always the newest \u{b7} effort high"
        );
        let mut planner = agent("p", "Planner", None);
        planner.model = AgentModel::Default;
        assert_eq!(agent_model(&planner, &tools, None), "default");
        let haiku = ModelRef {
            tool: Tool::ALL[0],
            model: Some("haiku".into()),
        };
        assert_eq!(
            agent_model(&planner, &tools, Some(&haiku)),
            "default \u{b7} Claude Code \u{b7} Haiku \u{b7} always the newest"
        );
        let two = two_tools();
        assert_eq!(
            agent_model(
                &on(Tool::Codex, agent("g", "GPT", Some("gpt-5.5"))),
                &two,
                None
            ),
            "Codex \u{b7} gpt-5.5"
        );
    }

    #[test]
    fn the_section_lists_every_agent_with_its_role_origin_model_and_skills() {
        let mut planner = agent("p", "Planner", None);
        planner.model = AgentModel::Default;
        planner.role = Some(Role::ProjectAgent);
        planner.came_from = CameFrom::AgentsFile;
        planner.skills = vec![SkillRef::Shipped("project-agent".into())];
        let mut validator = agent("v", "Validator", Some("opus"));
        validator.came_from = CameFrom::DefaultPreset;
        validator.skills = vec![SkillRef::Imported("c1".into())];
        validator.imported = vec![ImportedSkill {
            id: "c1".into(),
            name: "team-validate".into(),
            original: "/team/validate".into(),
            imported_at: 1,
            fingerprint: String::new(),
            warnings: Vec::new(),
            stands_for: Some("validate".into()),
        }];
        let list = AgentList {
            agents: vec![planner, validator],
            default_model: Some(ModelRef {
                tool: Tool::ALL[0],
                model: Some("haiku".into()),
            }),
            fallbacks: Vec::new(),
            dropped: None,
        };
        let section = section(&list, &AgentTools::one(Some(claude_list())));
        assert_eq!(
            section.default_model,
            "Default model for new agents: Claude Code \u{b7} Haiku \u{b7} always the newest"
        );
        type Shown<'a> = (&'a str, Option<&'a str>, &'a str, &'a str, Vec<String>);
        let rows: Vec<Shown> = section
            .rows
            .iter()
            .map(|r| {
                (
                    r.name.as_str(),
                    r.role.as_deref(),
                    r.came_from.as_str(),
                    r.model.as_str(),
                    r.skills.clone(),
                )
            })
            .collect();
        let from_file = format!("From {PRODUCT_NAME}'s agents file");
        let from_preset = format!("Added by the {PRODUCT_NAME} default preset");
        assert_eq!(
            rows,
            [
                (
                    "Planner",
                    Some("Project agent"),
                    from_file.as_str(),
                    "default \u{b7} Claude Code \u{b7} Haiku \u{b7} always the newest",
                    vec!["project-agent".to_owned()]
                ),
                (
                    "Validator",
                    None,
                    from_preset.as_str(),
                    "Opus \u{b7} always the newest",
                    vec!["team-validate".to_owned()]
                ),
            ]
        );
    }
}
