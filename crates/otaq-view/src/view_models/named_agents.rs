//! Settings → Agents as the page draws it (T-0340; AGT-01…AGT-03, AGT-05's mark; d0086, d0088; *15 · Settings*
//! `bVdGw`, `FYvzM`, `cnRA5` in their one-tool form, `XlyOa`, `WFHlh`, `Z4gQQQ`, `Wjp2r`, `O7UTA`): the **Named
//! agents** table, the fresh-install note, the four **role defaults**, the Add / Edit dialog with its checks, the ⋯
//! menu and the three ways Delete goes. Every word and every enabled rule is here; the page only draws.
//!
//! With several tools connected (M23, T-0361; `cnRA5` / `E4VW3`, `soGMU` / `tkPSn`, `t1sI3` / `BZB0T`, `ObF25` /
//! `l5T21`, `OCna6` / `aPdlu`, `mQijC` / `w6fVf`) the table gains the **TOOL** column (avatar and name) and the
//! **FALLBACK** column, every picker names the agent's tool ("Strong · Claude Code · Opus · always the newest"), the
//! dialog's Tool is a picker of the connected tools and it has a **Fallback** field. While one tool is connected
//! there is no tool column, no avatar and no Fallback field, and the Fallbacks block's checkbox is disabled with its
//! reason (d0086, `bVdGw`). With one named agent nothing offers a choice: the default pickers are read-only and the
//! note says how picking starts.
//!
//! [`model_label`] is **the one formatter** of a named agent's model as Settings shows it ("tool default", "Opus ·
//! always the newest", an exact id), used by the agent rows and pickers elsewhere too (never a raw alias).

use otaq_core::agent_tools::{ModelKind, ModelList, Tool};
use otaq_core::named_agents::{InUse, Mark, NamedAgent, Role, RoleDefaults};
use otaq_core::naming::PRODUCT_NAME;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::model_picker::{self, Picker};
use super::tool_avatar::{AvatarTool, ToolAvatarView, shown_with};
use crate::icons::IconName;

/// The tool default as a field or a table cell shows it.
pub const TOOL_DEFAULT_LABEL: &str = "Tool default";
/// The tool default beside an agent's name ("Claude Code · tool default").
pub const TOOL_DEFAULT: &str = "tool default";

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

/// A named agent's model as Settings shows it (AGT-02, AUDIT-y C2): "tool default", "<Display> · always the newest"
/// for a follow choice that resolves to one model, just its name for one that resolves per task (Copilot's "Auto ·
/// balance", Q-A), else the exact id. A model the list doesn't have (or no list yet) shows as saved.
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

/// [`model_label`] of an entry.
pub fn agent_model(agent: &NamedAgent, list: Option<&ModelList>) -> String {
    model_label(agent.model.as_deref(), list)
}

/// Whether the label is an exact model id (drawn in mono).
fn is_exact(model: Option<&str>, list: Option<&ModelList>) -> bool {
    !is_tool_default(model, list)
        && list
            .and_then(|l| l.entry(model.unwrap_or_default()))
            .is_none_or(|e| e.kind == ModelKind::Exact)
}

/// The tool's name as people read it.
fn tool_name(tool: Tool) -> &'static str {
    AvatarTool::from(tool).name()
}

/// The tool's name in a sentence ("Copilot", "Claude Code"), as the panel's lines say it.
pub use super::tool_problems::tool_word;

/// The tool a new entry gets before the service said which tools are connected: the first.
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
    /// The first tool connected with its list: what Settings read before M23, and the tests' default.
    pub fn one(list: Option<ModelList>) -> Self {
        let tool = Tool::ALL[0];
        Self {
            connected: vec![tool],
            lists: list.map(|l| (tool, l)).into_iter().collect(),
            failed: Vec::new(),
        }
    }

    /// Two or more tools connected: the tool column, avatars, tool names and the Fallback field show.
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

    /// The tool a new entry starts on: the first connected one (in d0086's order).
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

    /// An agent's model as a picker of agents shows it: with several tools its tool first ("Claude Code · Opus ·
    /// always the newest"), dropped when the agent is named after its tool ("Claude Code · tool default").
    pub fn agent_model(&self, agent: &NamedAgent) -> String {
        let model = agent_model(agent, self.list(agent.tool));
        let tool = tool_name(agent.tool);
        if self.several() && !agent.name.eq_ignore_ascii_case(tool) {
            format!("{tool} \u{b7} {model}")
        } else {
            model
        }
    }
}

/// What the page itself adds to the section: the automatic-move setting, and the entries a tool connected while the
/// page was open added (`t1sI3`, "added" until the page is left).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageState {
    pub auto_fallback: bool,
    pub added: Vec<String>,
}

/// A role as the page sends it back, as stored (`developer`, `validator`, `lead`, `project_agent`).
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

impl From<Role> for RoleId {
    fn from(role: Role) -> Self {
        match role {
            Role::Developer => Self::Developer,
            Role::Validator => Self::Validator,
            Role::EpicLead => Self::EpicLead,
            Role::ProjectAgent => Self::ProjectAgent,
        }
    }
}

impl From<RoleId> for Role {
    fn from(role: RoleId) -> Self {
        match role {
            RoleId::Developer => Self::Developer,
            RoleId::Validator => Self::Validator,
            RoleId::EpicLead => Self::EpicLead,
            RoleId::ProjectAgent => Self::ProjectAgent,
        }
    }
}

/// The four roles in the frame's order.
pub const ROLES: [Role; 4] = [
    Role::Developer,
    Role::Validator,
    Role::EpicLead,
    Role::ProjectAgent,
];

/// What a role does, under its label in the defaults.
pub fn role_description(role: Role) -> &'static str {
    match role {
        Role::Developer => "Writes the ticket's code",
        Role::Validator => "Checks the work before Review (0063)",
        Role::EpicLead => "Plans and guides an epic's children (0083)",
        Role::ProjectAgent => "Investigates and creates tickets (0068)",
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
    /// The tool's own message, in quotes, in the dialog (`WFHlh`).
    pub message: Option<String>,
    /// "Edit…" beside it in the table.
    pub edit_label: String,
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
        edit_label: "Edit\u{2026}".to_owned(),
    }
}

/// What a ⋯ item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "AgentMenuAction")]
pub enum MenuAction {
    /// Opens Edit agent.
    Edit,
    /// Opens Add agent prefilled "<name> copy" (`settings.agent_dialog` with `duplicate`).
    Duplicate,
    /// Asks first (`settings.agent_delete_dialog`): the refused dialog or the plain confirmation.
    Delete,
}

/// One item of a named agent's ⋯ menu (update Y §7): Edit…, Duplicate, a separator, Delete… (red).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentMenuItem")]
pub struct MenuItem {
    pub action: MenuAction,
    /// Pencil, copy, trash (`Z4gQQQ`).
    pub icon: IconName,
    pub label: String,
    pub danger: bool,
    /// A separator above it.
    pub separated: bool,
    pub enabled: bool,
    /// Why it is disabled: "Otaq needs at least one agent".
    pub tooltip: Option<String>,
}

/// Why the last entry can't be deleted (the core's `last_agent` refusal).
pub fn last_agent_reason() -> String {
    format!("{PRODUCT_NAME} needs at least one agent")
}

/// The ⋯ menu of an entry; `count` is how many entries there are.
pub fn menu(count: usize) -> Vec<MenuItem> {
    let last = count <= 1;
    vec![
        MenuItem {
            action: MenuAction::Edit,
            icon: IconName::Pencil,
            label: "Edit\u{2026}".to_owned(),
            danger: false,
            separated: false,
            enabled: true,
            tooltip: None,
        },
        MenuItem {
            action: MenuAction::Duplicate,
            icon: IconName::Copy,
            label: "Duplicate".to_owned(),
            danger: false,
            separated: false,
            enabled: true,
            tooltip: None,
        },
        MenuItem {
            action: MenuAction::Delete,
            icon: IconName::Trash2,
            label: "Delete\u{2026}".to_owned(),
            danger: true,
            separated: true,
            enabled: !last,
            tooltip: last.then(last_agent_reason),
        },
    ]
}

/// One row of the Named agents table: NAME · MODEL · USE FOR, or with several tools NAME · TOOL · MODEL · USE FOR ·
/// FALLBACK (`cnRA5`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentTableRow")]
pub struct TableRow {
    pub id: String,
    pub name: String,
    /// With several tools: the tool's avatar, before its name.
    pub tool: Option<ToolAvatarView>,
    /// With several tools: "Claude Code", before the model ("Claude Code · Opus · always the newest").
    pub tool_name: Option<String>,
    /// "Tool default", "Opus · always the newest", an exact id ("tool default" beside a tool's name).
    pub model: String,
    pub model_mono: bool,
    pub use_for: String,
    /// With several tools: the fallback's name, or "none".
    pub fallback: Option<String>,
    /// Amber, when the entry's tool isn't connected: "Codex isn't connected · its tickets wait".
    pub tool_problem: Option<String>,
    /// "added" beside the entry a tool connected while the page was open added (`t1sI3`).
    pub added: Option<String>,
    /// The amber line under the row.
    pub mark: Option<MarkView>,
    /// The ⋯ button's name: "More for Strong".
    pub menu_label: String,
    pub menu: Vec<MenuItem>,
}

/// The note while there is only one entry (`bVdGw`): neutral, gone at two.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentsFreshNote")]
pub struct FreshNote {
    pub title: String,
    pub text: String,
    pub add_label: String,
}

/// A named agent offered by a picker of agents: the name, and its model as Settings shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentOption")]
pub struct AgentOption {
    /// What is sent back; `None` = "Use app default" in a project, or no fallback.
    pub agent: Option<String>,
    /// With several tools: its tool's avatar (for "Use app default (Verifier)", the app default's).
    pub tool: Option<ToolAvatarView>,
    pub name: String,
    /// "tool default", "Opus · always the newest", with several tools "Codex · gpt-5.5"; none for "Use app default
    /// (…)" and "None".
    pub model: Option<String>,
    pub selected: bool,
}

/// A picker of named agents: the value shown closed, and the options when it can open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentPicker")]
pub struct AgentPicker {
    pub value: AgentOption,
    pub options: Vec<AgentOption>,
    /// With one named agent nothing offers a choice (d0086): no chevron, no list.
    pub read_only: bool,
}

/// One row of "Default agent for each kind".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentDefaultRow")]
pub struct DefaultRow {
    pub role: RoleId,
    pub label: String,
    pub description: String,
    pub picker: AgentPicker,
}

/// The **Fallbacks** block (AGT-08, d0086; `cnRA5`, `bVdGw`): "Use fallback agents automatically", off by default,
/// disabled while only one tool is connected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentsFallbacks")]
pub struct Fallbacks {
    pub title: String,
    pub label: String,
    pub help: String,
    /// As stored (`settings.auto_fallback`).
    pub on: bool,
    pub enabled: bool,
    /// Muted under it: why it is off by default, or why it is disabled.
    pub note: String,
}

/// The note under the defaults after a tool connected while the page was open (`t1sI3`): "DEFAULTS · UNCHANGED".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentsDefaultsNote")]
pub struct DefaultsNote {
    pub heading: String,
    /// "Developer, Validator, Epic lead, Project agent → Claude Code".
    pub text: String,
}

/// The whole Agents section of the global page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentsSection")]
pub struct Section {
    pub title: String,
    pub intro: String,
    pub table_title: String,
    /// NAME · MODEL · USE FOR while one tool is connected; NAME · TOOL · MODEL · USE FOR · FALLBACK with several.
    pub columns: Vec<String>,
    pub rows: Vec<TableRow>,
    pub add_label: String,
    pub fresh_note: Option<FreshNote>,
    pub defaults_title: String,
    pub defaults: Vec<DefaultRow>,
    /// "Projects can override any row…".
    pub defaults_note: String,
    /// "DEFAULTS · UNCHANGED" while a row is marked "added".
    pub defaults_unchanged: Option<DefaultsNote>,
    pub fallbacks: Fallbacks,
}

/// An agent as a picker option, with its model (and its tool, with several tools).
fn option(agent: &NamedAgent, tools: &AgentTools, selected: bool) -> AgentOption {
    AgentOption {
        agent: Some(agent.id.clone()),
        tool: tools.avatar(agent.tool),
        name: agent.name.clone(),
        model: Some(tools.agent_model(agent)),
        selected,
    }
}

/// An app-wide default's picker: every entry, the default's one picked.
fn default_picker(agents: &[NamedAgent], on: Option<&str>, tools: &AgentTools) -> AgentPicker {
    let options: Vec<AgentOption> = agents
        .iter()
        .map(|a| option(a, tools, on == Some(a.id.as_str())))
        .collect();
    let value = options
        .iter()
        .find(|o| o.selected)
        .cloned()
        // Every default is set once an entry exists; a missing one shows empty.
        .unwrap_or(AgentOption {
            agent: None,
            tool: None,
            name: String::new(),
            model: None,
            selected: false,
        });
    AgentPicker {
        value,
        read_only: agents.len() <= 1,
        options,
    }
}

/// "Codex isn't connected · its tickets wait".
pub fn not_connected_text(tool: Tool) -> String {
    format!(
        "{} isn't connected \u{b7} its tickets wait",
        tool_word(tool)
    )
}

/// The Fallbacks block: enabled only with several tools connected.
pub fn fallbacks(tools: &AgentTools, on: bool) -> Fallbacks {
    let enabled = tools.several();
    Fallbacks {
        title: "Fallbacks".to_owned(),
        label: "Use fallback agents automatically".to_owned(),
        help: "When an agent's tool hits its usage limit or isn't available, waiting tickets start on that agent's fallback instead. Off: they wait, and the banner offers Move waiting to\u{2026}.".to_owned(),
        on,
        enabled,
        note: if enabled {
            format!(
                "Off by default: {PRODUCT_NAME} can't guess which fallbacks you want. Running agents never move; a conversation can't change tools."
            )
        } else {
            "Disabled: a fallback must be on another tool. Connect a second agent tool to use fallbacks."
                .to_owned()
        },
    }
}

/// "Developer, Validator, Epic lead, Project agent → Claude Code", one part per agent the defaults point at.
pub fn defaults_line(agents: &[NamedAgent], defaults: &RoleDefaults) -> String {
    let mut parts: Vec<(String, Vec<&str>)> = Vec::new();
    for role in ROLES {
        let Some(name) = defaults
            .get(role)
            .and_then(|id| agents.iter().find(|a| a.id == id))
            .map(|a| a.name.clone())
        else {
            continue;
        };
        match parts.iter_mut().find(|(n, _)| *n == name) {
            Some((_, roles)) => roles.push(role.label()),
            None => parts.push((name, vec![role.label()])),
        }
    }
    parts
        .iter()
        .map(|(name, roles)| format!("{} \u{2192} {name}", roles.join(", ")))
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// The Agents section from `agents.list`, the agent tools and their cached lists (T-0335), and the page's state.
pub fn section(
    agents: &[NamedAgent],
    defaults: &RoleDefaults,
    tools: &AgentTools,
    page: &PageState,
) -> Section {
    let several = tools.several();
    let rows = agents
        .iter()
        .map(|a| {
            let list = tools.list(a.tool);
            TableRow {
                id: a.id.clone(),
                name: a.name.clone(),
                tool: tools.avatar(a.tool),
                tool_name: several.then(|| tool_name(a.tool).to_owned()),
                // Beside a tool's name the tool default reads as the frames write it: "Claude Code · tool default".
                model: if is_tool_default(a.model.as_deref(), list) && !several {
                    TOOL_DEFAULT_LABEL.to_owned()
                } else {
                    agent_model(a, list)
                },
                model_mono: is_exact(a.model.as_deref(), list),
                use_for: a.use_for.clone(),
                fallback: several.then(|| {
                    a.fallback
                        .as_ref()
                        .and_then(|id| agents.iter().find(|f| f.id == *id))
                        .map_or_else(|| "none".to_owned(), |f| f.name.clone())
                }),
                tool_problem: (!tools.is_connected(a.tool)).then(|| not_connected_text(a.tool)),
                added: page.added.contains(&a.id).then(|| "added".to_owned()),
                mark: a.mark.as_ref().map(mark_view),
                menu_label: format!("More for {}", a.name),
                menu: menu(agents.len()),
            }
        })
        .collect::<Vec<TableRow>>();
    let fresh_note = match agents {
        [only] => Some(FreshNote {
            title: "One agent, so nothing picks yet".to_owned(),
            text: format!(
                "The Project agent and epic leads choose between your agents for each ticket. With only {} there is nothing to choose, so every ticket runs on it. Add a second one, for example a Light agent on Haiku for docs and small config, and they start picking.",
                only.name
            ),
            add_label: "Add agent".to_owned(),
        }),
        _ => None,
    };
    let columns: &[&str] = if several {
        &["NAME", "TOOL \u{b7} MODEL", "USE FOR", "FALLBACK"]
    } else {
        &["NAME", "MODEL", "USE FOR"]
    };
    Section {
        title: "Agents".to_owned(),
        intro: "Named agents are a tool, a model and what you use them for. Agents pick from these names when they create tickets.".to_owned(),
        table_title: "Named agents".to_owned(),
        columns: columns.iter().map(|c| (*c).to_owned()).collect(),
        defaults_unchanged: rows.iter().any(|r| r.added.is_some()).then(|| DefaultsNote {
            heading: "DEFAULTS \u{b7} UNCHANGED".to_owned(),
            text: defaults_line(agents, defaults),
        }),
        rows,
        add_label: "Add agent".to_owned(),
        fresh_note,
        defaults_title: "Default agent for each kind".to_owned(),
        defaults: ROLES
            .iter()
            .map(|&role| DefaultRow {
                role: role.into(),
                label: role.label().to_owned(),
                description: role_description(role).to_owned(),
                picker: default_picker(agents, defaults.get(role), tools),
            })
            .collect(),
        defaults_note: "Projects can override any row in their own settings. A ticket starts at its project's default; agents may pick another name when they create it.".to_owned(),
        fallbacks: fallbacks(tools, page.auto_fallback),
    }
}

/// A project's picker for `role` (AGT-03, `mQijC`): "Use app default (<name>)" first, then every entry; with several
/// tools each names its tool ("Strong · Claude Code · Opus · always the newest").
pub fn project_picker(
    agents: &[NamedAgent],
    app: &RoleDefaults,
    project: &RoleDefaults,
    role: Role,
    tools: &AgentTools,
) -> AgentPicker {
    let own = project
        .get(role)
        .filter(|id| agents.iter().any(|a| a.id == *id));
    let app_agent = app
        .get(role)
        .and_then(|id| agents.iter().find(|a| a.id == id));
    let use_app = AgentOption {
        agent: None,
        tool: app_agent.and_then(|a| tools.avatar(a.tool)),
        name: format!(
            "Use app default ({})",
            app_agent.map(|a| a.name.as_str()).unwrap_or_default()
        ),
        model: None,
        selected: own.is_none(),
    };
    let mut options = vec![use_app];
    options.extend(
        agents
            .iter()
            .map(|a| option(a, tools, own == Some(a.id.as_str()))),
    );
    let value = options
        .iter()
        .find(|o| o.selected)
        .cloned()
        .expect("one option is picked");
    AgentPicker {
        value,
        read_only: agents.len() <= 1,
        options,
    }
}

/// What the desktop keeps while Settings → Agents is open (`t1sI3`): the entries there were when it opened, the
/// ones the user added since (Add agent, Duplicate), and the tools whose last read of their list gave none. An entry
/// that is new since the opening and wasn't the user's was added by a tool connecting: it is marked "added" until the
/// page opens again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Visit {
    seen: Option<Vec<String>>,
    mine: Vec<String>,
    failed: Vec<Tool>,
}

impl Visit {
    /// The page opened (`settings.agents` with `refresh`) on `agents`.
    pub fn open(&mut self, agents: &[NamedAgent]) {
        self.seen = Some(agents.iter().map(|a| a.id.clone()).collect());
        self.mine.clear();
    }

    /// The user added `id` (Add agent, Duplicate).
    pub fn added_by_user(&mut self, id: &str) {
        self.mine.push(id.to_owned());
    }

    /// A read of `tool`'s list gave a copy (`true`) or none.
    pub fn read(&mut self, tool: Tool, listed: bool) {
        self.failed.retain(|t| *t != tool);
        if !listed {
            self.failed.push(tool);
        }
    }

    /// The tools whose last read gave no list ([`AgentTools::failed`]).
    pub fn failed(&self) -> Vec<Tool> {
        self.failed.clone()
    }

    /// The page's state for `agents`: the entries a tool connecting added since the opening.
    pub fn page(&self, agents: &[NamedAgent], auto_fallback: bool) -> PageState {
        let added = match &self.seen {
            Some(seen) => agents
                .iter()
                .filter(|a| !seen.contains(&a.id) && !self.mine.contains(&a.id))
                .map(|a| a.id.clone())
                .collect(),
            None => Vec::new(),
        };
        PageState {
            auto_fallback,
            added,
        }
    }
}

/// What the dialog's fields hold while the user types (React keeps the text; this view checks it).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "AgentDraft")]
pub struct Draft {
    pub name: String,
    /// The tool, as stored (`codex`); `None` = the entry's own, or the first connected tool for a new one.
    #[serde(default)]
    #[ts(type = "string | null")]
    pub tool: Option<Tool>,
    /// `None` = tool default.
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub use_for: String,
    /// The fallback entry's id; `None` = none.
    #[serde(default)]
    pub fallback: Option<String>,
}

impl Draft {
    pub fn of(agent: &NamedAgent) -> Self {
        Self {
            name: agent.name.clone(),
            tool: Some(agent.tool),
            model: agent.model.clone(),
            use_for: agent.use_for.clone(),
            fallback: agent.fallback.clone(),
        }
    }
}

/// The Tool picker's change (`settings.agent_dialog` with `tool`): another tool clears the model to the tool default
/// (another tool's list) and a fallback on that same tool (d0086: a fallback is on another tool).
pub fn change_tool(
    mut draft: Draft,
    tool: Tool,
    agents: &[NamedAgent],
    editing: Option<&str>,
) -> Draft {
    let current = draft_tool(&draft, agents, editing, &AgentTools::default());
    if current != tool {
        draft.model = None;
    }
    draft.tool = Some(tool);
    if draft
        .fallback
        .as_ref()
        .and_then(|id| agents.iter().find(|a| a.id == *id))
        .is_some_and(|f| f.tool == tool)
    {
        draft.fallback = None;
    }
    draft
}

/// The draft's tool: as picked, else the entry's own, else the first connected tool.
pub fn draft_tool(
    draft: &Draft,
    agents: &[NamedAgent],
    editing: Option<&str>,
    tools: &AgentTools,
) -> Tool {
    draft
        .tool
        .or_else(|| {
            editing
                .and_then(|id| agents.iter().find(|a| a.id == id))
                .map(|a| a.tool)
        })
        .unwrap_or_else(|| tools.first())
}

/// The entries a fallback can be for an entry on `tool` (`editing` itself excluded): every entry on another tool.
pub fn fallback_choices<'a>(
    agents: &'a [NamedAgent],
    tool: Tool,
    editing: Option<&str>,
) -> impl Iterator<Item = &'a NamedAgent> {
    agents
        .iter()
        .filter(move |a| a.tool != tool && Some(a.id.as_str()) != editing)
}

/// The name check, live: required, and unique ignoring case (Q-E). The view's words win over the core's refusal
/// ("Another agent is already called Light"), which the desktop maps to the same text.
pub fn name_problem(name: &str, agents: &[NamedAgent], editing: Option<&str>) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let folded = name.to_lowercase();
    agents
        .iter()
        .find(|a| Some(a.id.as_str()) != editing && a.name.to_lowercase() == folded)
        .map(|_| name_taken(name))
}

/// "There's already an agent named Light".
pub fn name_taken(name: &str) -> String {
    format!("There's already an agent named {}", name.trim())
}

/// The empty name's message, when the service refuses one (Save is disabled before that).
pub const NAME_NEEDED: &str = "An agent needs a name";

/// The Delete… button of Edit agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentDialogDelete")]
pub struct DeleteButton {
    pub label: String,
    pub enabled: bool,
    pub tooltip: Option<String>,
}

/// One tool of the dialog's Tool picker (several tools connected).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentToolOption")]
pub struct ToolOption {
    /// As stored, sent back as `settings.agent_dialog`'s `tool`.
    #[ts(type = "string")]
    pub tool: Tool,
    pub avatar: ToolAvatarView,
    pub name: String,
    pub selected: bool,
}

/// The dialog's Fallback field (AGT-08; `ObF25`): "None", or a named agent on another tool. Only with several tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentFallbackField")]
pub struct FallbackField {
    pub label: String,
    pub picker: AgentPicker,
    /// "Another named agent on a different tool. Used when Codex hits its limit or isn't available. One step: …".
    pub hint: String,
    /// Red under it (the service refused it: `bad_fallback`).
    pub problem: Option<String>,
}

/// Add agent or Edit agent · <name> (`k8WyHN`, `WFHlh`, `ObF25`). No Tool choice and no Fallback field while one
/// tool is connected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentDialog")]
pub struct Dialog {
    /// The entry being edited; `None` = Add agent (also Duplicate's draft).
    pub id: Option<String>,
    pub title: String,
    pub mark: Option<MarkView>,
    pub name_label: String,
    pub draft: Draft,
    /// Red under the field: "There's already an agent named Light".
    pub name_problem: Option<String>,
    pub tool_label: String,
    /// The tool's name: "Claude Code" (read-only with one tool, the picker's value with several).
    pub tool: String,
    /// With several tools: the picked tool's avatar.
    pub tool_avatar: Option<ToolAvatarView>,
    /// With several tools: the connected tools to pick from; empty = read-only.
    pub tool_options: Vec<ToolOption>,
    /// "the only connected tool" while one is connected.
    pub tool_note: Option<String>,
    /// Amber: the entry's tool isn't connected ("Codex isn't connected · its tickets wait").
    pub tool_problem: Option<String>,
    pub model_label: String,
    pub picker: Picker,
    /// Red under the model (the service refused it: `model_not_listed`).
    pub model_problem: Option<String>,
    pub use_for_label: String,
    pub use_for_hint: String,
    pub fallback: Option<FallbackField>,
    pub delete: Option<DeleteButton>,
    pub cancel_label: String,
    /// "Add agent" or "Save".
    pub save_label: String,
    pub save_enabled: bool,
}

/// The Fallback field for an entry on `tool`, `picked` the draft's fallback.
fn fallback_field(
    agents: &[NamedAgent],
    tool: Tool,
    editing: Option<&str>,
    picked: Option<&str>,
    tools: &AgentTools,
    problem: Option<String>,
) -> FallbackField {
    let choices: Vec<&NamedAgent> = fallback_choices(agents, tool, editing).collect();
    let picked = picked.filter(|id| choices.iter().any(|a| a.id == *id));
    let none = AgentOption {
        agent: None,
        tool: None,
        name: "None".to_owned(),
        model: None,
        selected: picked.is_none(),
    };
    let mut options = vec![none];
    options.extend(
        choices
            .iter()
            .map(|a| option(a, tools, picked == Some(a.id.as_str()))),
    );
    let value = options
        .iter()
        .find(|o| o.selected)
        .cloned()
        .expect("one option is picked");
    let word = tool_word(tool);
    FallbackField {
        label: "Fallback".to_owned(),
        picker: AgentPicker {
            value,
            read_only: options.len() <= 1,
            options,
        },
        hint: format!(
            "Another named agent on a different tool. Used when {word} hits its limit or isn't available. One step: a fallback's own fallback isn't used."
        ),
        problem,
    }
}

/// The dialog for `editing` (an entry's id; `None` = Add agent) with `draft` as typed so far. `refused`: the
/// service's answer to Save, as `(refusal code, message)`.
pub fn dialog(
    agents: &[NamedAgent],
    editing: Option<&str>,
    draft: Draft,
    tools: &AgentTools,
    refused: Option<(&str, &str)>,
) -> Dialog {
    let agent = editing.and_then(|id| agents.iter().find(|a| a.id == id));
    let tool = draft_tool(&draft, agents, editing, tools);
    let several = tools.several();
    let mark = agent.and_then(|a| a.mark.as_ref());
    // The model the mark is about is still the one picked: the hint says the account can't use it.
    let refused_model = agent
        .zip(mark)
        .is_some_and(|(a, _)| a.model == draft.model && a.tool == tool);
    let mut name_problem = name_problem(&draft.name, agents, editing);
    let mut model_problem = None;
    let mut fallback_problem = None;
    match refused {
        Some(("name_taken", _)) => name_problem = Some(name_taken(&draft.name)),
        Some(("empty_field", _)) => name_problem = Some(NAME_NEEDED.to_owned()),
        Some(("bad_fallback", message)) => fallback_problem = Some(message.to_owned()),
        Some((_, message)) => model_problem = Some(message.to_owned()),
        None => {}
    }
    let save_enabled = !draft.name.trim().is_empty() && name_problem.is_none();
    // The connected tools, and the entry's own one when it isn't connected (so the value shows).
    let tool_options = if several {
        Tool::ALL
            .iter()
            .copied()
            .filter(|t| tools.is_connected(*t) || *t == tool)
            .map(|t| ToolOption {
                tool: t,
                avatar: ToolAvatarView::of(t, None),
                name: tool_name(t).to_owned(),
                selected: t == tool,
            })
            .collect()
    } else {
        Vec::new()
    };
    Dialog {
        id: agent.map(|a| a.id.clone()),
        title: match agent {
            Some(a) => format!("Edit agent \u{b7} {}", a.name),
            None => "Add agent".to_owned(),
        },
        mark: mark.map(mark_view),
        name_label: "Name".to_owned(),
        name_problem,
        tool_label: "Tool".to_owned(),
        tool: tool_name(tool).to_owned(),
        tool_avatar: tools.avatar(tool),
        tool_options,
        tool_note: (!several).then(|| "the only connected tool".to_owned()),
        tool_problem: (!tools.is_connected(tool)).then(|| not_connected_text(tool)),
        model_label: "Model".to_owned(),
        picker: model_picker::picker(
            tool,
            tools.state(tool),
            draft.model.as_deref(),
            refused_model,
            agent.map(|a| a.name.as_str()),
        ),
        model_problem,
        use_for_label: "Use for".to_owned(),
        use_for_hint: "Agents read this when they pick an agent for a ticket.".to_owned(),
        fallback: several.then(|| {
            fallback_field(
                agents,
                tool,
                editing,
                draft.fallback.as_deref(),
                tools,
                fallback_problem.clone(),
            )
        }),
        delete: agent.map(|_| DeleteButton {
            label: "Delete\u{2026}".to_owned(),
            enabled: agents.len() > 1,
            tooltip: (agents.len() <= 1).then(last_agent_reason),
        }),
        cancel_label: "Cancel".to_owned(),
        save_label: if agent.is_some() { "Save" } else { "Add agent" }.to_owned(),
        save_enabled,
        draft,
    }
}

/// The entry's fields as Save sends them (`agents.add` / `agents.edit`): the draft's tool and, with several tools,
/// its fallback only if it is still on another tool. With one tool the dialog has no Fallback field and an edit keeps
/// the entry's (the draft carries it).
pub fn fields(
    draft: &Draft,
    agents: &[NamedAgent],
    editing: Option<&str>,
    tools: &AgentTools,
) -> otaq_core::named_agents::AgentFields {
    let tool = draft_tool(draft, agents, editing, tools);
    let fallback = draft
        .fallback
        .clone()
        .filter(|id| fallback_choices(agents, tool, editing).any(|a| a.id == *id));
    otaq_core::named_agents::AgentFields {
        name: draft.name.trim().to_owned(),
        tool,
        model: draft.model.clone(),
        use_for: draft.use_for.clone(),
        fallback,
    }
}

/// How many keys the refused dialog names before "and 9 more".
pub const KEYS_SHOWN: usize = 3;

/// What Delete… opens (AGT-02): the refused dialog while anything uses the entry (`O7UTA`), else a plain
/// confirmation (`Wjp2r`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "AgentDeleteDialog")]
pub enum DeleteDialog {
    Refused {
        /// "Light can't be deleted".
        title: String,
        /// The core's sentence and what to do: "Light is used by 12 tickets, …. Change those first, then delete it."
        text: String,
        /// "12 tickets · SHOP-24, SHOP-40.3, SHOP-41 and 9 more", then a line per default and project
        /// ("docs-site · Validator default").
        lines: Vec<String>,
        /// Every ticket key, for Show tickets (Q-F): each opens its panel.
        tickets: Vec<String>,
        /// "Show tickets"; none without tickets.
        show_tickets_label: Option<String>,
        ok_label: String,
    },
    Confirm {
        id: String,
        /// "Delete Fable test?"
        title: String,
        text: String,
        note: String,
        cancel_label: String,
        delete_label: String,
    },
}

/// "the Validator default", "the Developer and Validator defaults".
fn defaults_of(roles: &[Role]) -> String {
    let labels: Vec<&str> = roles.iter().map(|r| r.label()).collect();
    let noun = if labels.len() == 1 {
        "default"
    } else {
        "defaults"
    };
    format!("{} {noun}", join(&labels))
}

fn join(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The dialog for `id`, from `agents.in_use`.
pub fn delete_dialog(id: &str, name: &str, uses: &InUse) -> DeleteDialog {
    if uses.is_empty() {
        return DeleteDialog::Confirm {
            id: id.to_owned(),
            title: format!("Delete {name}?"),
            text: format!("Nothing uses {name}: no unfinished ticket and no default."),
            note: format!(
                "Finished tickets that ran on it keep their history as \u{201c}{name} (deleted)\u{201d}."
            ),
            cancel_label: "Cancel".to_owned(),
            delete_label: "Delete".to_owned(),
        };
    }
    let tickets: Vec<String> = uses.tickets.iter().map(ToString::to_string).collect();
    let mut lines = Vec::new();
    if !tickets.is_empty() {
        let count = match tickets.len() {
            1 => "1 ticket".to_owned(),
            n => format!("{n} tickets"),
        };
        let shown: Vec<&str> = tickets
            .iter()
            .take(KEYS_SHOWN)
            .map(String::as_str)
            .collect();
        let keys = match tickets.len().saturating_sub(KEYS_SHOWN) {
            0 => join(&shown),
            more => format!("{} and {more} more", shown.join(", ")),
        };
        lines.push(format!("{count} \u{b7} {keys}"));
    }
    if !uses.defaults.is_empty() {
        lines.push(format!("App-wide \u{b7} {}", defaults_of(&uses.defaults)));
    }
    for project in &uses.projects {
        lines.push(format!(
            "{} \u{b7} {}",
            project.name,
            defaults_of(&project.roles)
        ));
    }
    for other in &uses.fallback_of {
        lines.push(format!("{other} \u{b7} fallback"));
    }
    DeleteDialog::Refused {
        title: format!("{name} can't be deleted"),
        text: format!("{}. Change those first, then delete it.", uses.message()),
        lines,
        show_tickets_label: (!tickets.is_empty()).then(|| "Show tickets".to_owned()),
        tickets,
        ok_label: "OK".to_owned(),
    }
}

/// Sample data for the tests and the gallery (*15* frames, the example of the ticket file).
pub mod fixtures {
    use otaq_core::agent_tools::models::ModelEntry;
    use otaq_core::agent_tools::{ModelKind, ModelList, Tool};
    use otaq_core::named_agents::{Mark, NamedAgent, RoleDefaults};

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

    pub fn agent(id: &str, name: &str, model: Option<&str>, use_for: &str) -> NamedAgent {
        NamedAgent {
            id: id.to_owned(),
            name: name.to_owned(),
            tool: Tool::ALL[0],
            model: model.map(str::to_owned),
            use_for: use_for.to_owned(),
            fallback: None,
            mark: None,
            created_at: 1,
            updated_at: 1,
        }
    }

    /// The starter entry (`bVdGw`).
    pub fn starter() -> NamedAgent {
        agent(
            "cc",
            "Claude Code",
            None,
            "Everything, until you add more agents",
        )
    }

    /// A fresh install: the starter entry, all four defaults on it.
    pub fn fresh() -> (Vec<NamedAgent>, RoleDefaults) {
        (vec![starter()], RoleDefaults::all_on("cc"))
    }

    /// The mark of SHOP-48 (`XlyOa`).
    pub fn fable_mark() -> Mark {
        Mark {
            text: "Claude Code couldn't run claude-fable-5-1 on SHOP-48".to_owned(),
            model: Some("claude-fable-5-1".to_owned()),
            ticket: Some("SHOP-48".to_owned()),
            message: Some(
                "There's an issue with the selected model (claude-fable-5-1). It may not exist or you may \
                 not have access to it."
                    .to_owned(),
            ),
            at: 2,
        }
    }

    /// "A list you made" (`FYvzM`, the ticket file's example): the starter, Strong, Light, Sonnet and Fable test,
    /// the defaults still on Claude Code.
    pub fn made() -> (Vec<NamedAgent>, RoleDefaults) {
        (
            vec![
                starter(),
                agent(
                    "strong",
                    "Strong",
                    Some("opus"),
                    "Complex features, tricky bugs",
                ),
                agent("light", "Light", Some("haiku"), "Typos, docs, small config"),
                agent("sonnet", "Sonnet", Some("sonnet"), "Normal tickets"),
                agent(
                    "fable",
                    "Fable test",
                    Some("fable"),
                    "Trying Fable on a few tickets",
                ),
            ],
            RoleDefaults::all_on("cc"),
        )
    }

    /// [`made`] with Fable test marked.
    pub fn marked() -> (Vec<NamedAgent>, RoleDefaults) {
        let (mut agents, defaults) = made();
        agents[4].mark = Some(fable_mark());
        (agents, defaults)
    }

    /// `agent` on `tool`, with `fallback`.
    pub fn on(tool: Tool, mut agent: NamedAgent, fallback: Option<&str>) -> NamedAgent {
        agent.tool = tool;
        agent.fallback = fallback.map(str::to_owned);
        agent
    }

    /// Codex's list (T-0351, T-0356): its default, the GPT Sol family, then every listed model exact.
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

    /// The fresh install with Claude Code and Codex (`soGMU`): two starter entries, the defaults on the first.
    pub fn fresh_two() -> (Vec<NamedAgent>, RoleDefaults) {
        let codex = on(
            Tool::Codex,
            agent("cx", "Codex", None, "Everything, until you add more agents"),
            None,
        );
        (vec![starter(), codex], RoleDefaults::all_on("cc"))
    }

    /// [`fresh_two`] after Copilot connected (`t1sI3`): its entry added, nothing else changed.
    pub fn copilot_added() -> (Vec<NamedAgent>, RoleDefaults) {
        let (mut agents, defaults) = fresh_two();
        agents.push(on(
            Tool::Copilot,
            agent(
                "cp",
                "GitHub Copilot",
                None,
                "Everything, until you add more agents",
            ),
            None,
        ));
        (agents, defaults)
    }

    /// The list of update U (`cnRA5`), on every tool, with its defaults.
    pub fn several() -> (Vec<NamedAgent>, RoleDefaults) {
        use otaq_core::named_agents::Role;
        let agents = vec![
            on(
                Tool::Claude,
                agent(
                    "strong",
                    "Strong",
                    Some("opus"),
                    "Complex features, tricky bugs",
                ),
                Some("strong-gpt"),
            ),
            on(
                Tool::Codex,
                agent("standard", "Standard", Some("gpt-5.5"), "Normal tickets"),
                Some("sonnet"),
            ),
            on(
                Tool::Claude,
                agent("light", "Light", Some("haiku"), "Typos, docs, small config"),
                None,
            ),
            on(
                Tool::Copilot,
                agent(
                    "verifier",
                    "Verifier",
                    Some("gpt-5.5"),
                    "Validating other agents' work",
                ),
                Some("strong"),
            ),
            on(
                Tool::Codex,
                agent(
                    "strong-gpt",
                    "Strong GPT",
                    Some("gpt-5.5-pro"),
                    "Strong's fallback",
                ),
                None,
            ),
            on(
                Tool::Claude,
                agent("sonnet", "Sonnet", Some("sonnet"), "Standard's fallback"),
                None,
            ),
            on(
                Tool::OpenCode,
                agent(
                    "gemini",
                    "Gemini Pro",
                    Some("google/gemini-3-pro"),
                    "Large refactors, long files",
                ),
                None,
            ),
        ];
        let mut defaults = RoleDefaults::default();
        defaults.set(Role::Developer, Some("standard".to_owned()));
        defaults.set(Role::Validator, Some("verifier".to_owned()));
        defaults.set(Role::EpicLead, Some("strong".to_owned()));
        defaults.set(Role::ProjectAgent, Some("standard".to_owned()));
        (agents, defaults)
    }

    /// Old GPT (`OCna6`): Codex · gpt-5.4, gone from Codex's list, marked.
    pub fn old_gpt() -> NamedAgent {
        let mut old = on(
            Tool::Codex,
            agent("old-gpt", "Old GPT", Some("gpt-5.4"), "Older tickets"),
            None,
        );
        old.mark = Some(otaq_core::named_agents::mark::no_longer_lists(
            "Codex", "gpt-5.4", "SHOP-49",
        ));
        old
    }

    /// [`several`] with Old GPT and Verifier marked (`OCna6`): "no longer lists" and the swap.
    pub fn several_marked() -> (Vec<NamedAgent>, RoleDefaults) {
        let (mut agents, defaults) = several();
        agents[3].mark = Some(otaq_core::named_agents::mark::swapped(
            "Copilot", "auto", "gpt-5.5", "SHOP-29",
        ));
        agents.insert(1, old_gpt());
        (agents, defaults)
    }
}

/// One state of the gallery's "15 · Settings → Agents" section (T-0343 draws it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentsGalleryCell")]
pub struct GalleryCell<T> {
    pub label: String,
    pub view: T,
}

/// The gallery: every part of Settings → Agents in its states, from [`fixtures`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentsGallery")]
pub struct Gallery {
    pub sections: Vec<GalleryCell<Section>>,
    pub dialogs: Vec<GalleryCell<Dialog>>,
    pub deletes: Vec<GalleryCell<DeleteDialog>>,
    pub projects: Vec<GalleryCell<super::settings_page::ProjectAgentsBlock>>,
}

pub fn gallery() -> Gallery {
    use otaq_core::domain::TicketKey;
    use otaq_core::named_agents::in_use::ProjectUse;

    use super::settings_page::project_agents_block;

    let one = AgentTools::one(Some(fixtures::claude_list()));
    let page = PageState::default();
    fn cell<T>(label: &str, view: T) -> GalleryCell<T> {
        GalleryCell {
            label: label.to_owned(),
            view,
        }
    }
    let (fresh, fresh_defaults) = fixtures::fresh();
    let (made, defaults) = fixtures::made();
    let (marked, _) = fixtures::marked();
    let (fresh_two, two_defaults) = fixtures::fresh_two();
    let (added, added_defaults) = fixtures::copilot_added();
    let (several, several_defaults) = fixtures::several();
    let (several_marked, _) = fixtures::several_marked();
    let all = fixtures::tools(Tool::ALL);
    let two = fixtures::two_tools();
    let three = fixtures::tools(&[Tool::Claude, Tool::Codex, Tool::Copilot]);
    // `cnRA5`: Copilot isn't signed in.
    let no_copilot = fixtures::tools(&[Tool::Claude, Tool::Codex, Tool::OpenCode]);
    let mut loading = all.clone();
    loading.lists.retain(|(t, _)| *t != Tool::Codex);
    let mut offline = all.clone();
    offline.lists.retain(|(t, _)| *t != Tool::Copilot);
    offline.failed.push(Tool::Copilot);
    let fable_draft = Draft {
        name: "Fable test".to_owned(),
        model: Some("fable".to_owned()),
        use_for: "Trying Fable on a few tickets".to_owned(),
        ..Draft::default()
    };
    let key = |k: &str| k.parse::<TicketKey>().expect("a sample key");
    let light_uses = InUse {
        agent: "Light".to_owned(),
        tickets: [
            "SHOP-24",
            "SHOP-40.3",
            "SHOP-41",
            "SHOP-50",
            "SHOP-51",
            "SHOP-52",
            "SHOP-53",
            "SHOP-54",
            "SHOP-55",
            "SHOP-56",
            "SHOP-57",
            "SHOP-58",
        ]
        .map(key)
        .to_vec(),
        defaults: Vec::new(),
        projects: vec![ProjectUse {
            key: "DOCS".to_owned(),
            name: "docs-site".to_owned(),
            roles: vec![Role::Validator],
        }],
        fallback_of: Vec::new(),
    };
    let mut shop_web = RoleDefaults::default();
    shop_web.set(Role::Developer, Some("strong".to_owned()));
    let edit = |agents: &[NamedAgent], id: &str, tools: &AgentTools| {
        let agent = agents.iter().find(|a| a.id == id).expect("a sample entry");
        dialog(agents, Some(id), Draft::of(agent), tools, None)
    };
    Gallery {
        sections: vec![
            cell(
                "Fresh install",
                section(&fresh, &fresh_defaults, &one, &page),
            ),
            cell("A list you made", section(&made, &defaults, &one, &page)),
            cell(
                "Fable test marked",
                section(&marked, &defaults, &one, &page),
            ),
            cell(
                "Fresh install · Claude Code + Codex",
                section(&fresh_two, &two_defaults, &two, &page),
            ),
            cell(
                "Connecting Copilot later · added",
                section(
                    &added,
                    &added_defaults,
                    &three,
                    &PageState {
                        added: vec!["cp".to_owned()],
                        ..PageState::default()
                    },
                ),
            ),
            cell(
                "Several tools · Copilot not signed in",
                section(&several, &several_defaults, &no_copilot, &page),
            ),
            cell(
                "Several tools · the M23 marks",
                section(&several_marked, &several_defaults, &all, &page),
            ),
        ],
        dialogs: vec![
            cell(
                "Add agent · model picker",
                dialog(&made[..4], None, fable_draft.clone(), &one, None),
            ),
            cell(
                "Edit agent · the mark",
                dialog(&marked, Some("fable"), Draft::of(&marked[4]), &one, None),
            ),
            cell(
                "Duplicate: Strong copy",
                dialog(
                    &made,
                    None,
                    Draft {
                        name: "Strong copy".to_owned(),
                        ..Draft::of(&made[1])
                    },
                    &one,
                    None,
                ),
            ),
            cell(
                "A taken name",
                dialog(
                    &made,
                    None,
                    Draft {
                        name: "light".to_owned(),
                        ..Draft::default()
                    },
                    &one,
                    None,
                ),
            ),
            cell(
                "The only agent",
                dialog(&fresh, Some("cc"), Draft::of(&fresh[0]), &one, None),
            ),
            cell(
                "Edit agent · Codex with Fallback",
                edit(&several, "standard", &all),
            ),
            cell(
                "Edit agent · Copilot's Auto tiers",
                dialog(
                    &several,
                    Some("verifier"),
                    Draft {
                        model: Some("auto@balance".to_owned()),
                        ..Draft::of(&several[3])
                    },
                    &all,
                    None,
                ),
            ),
            cell(
                "Loading Codex's models",
                edit(&several, "standard", &loading),
            ),
            cell(
                "Couldn't load Copilot's models",
                edit(&several, "verifier", &offline),
            ),
            cell(
                "Old GPT · no longer listed",
                edit(&several_marked, "old-gpt", &all),
            ),
            cell(
                "Verifier · ran auto instead",
                edit(&several_marked, "verifier", &all),
            ),
        ],
        deletes: vec![
            cell("In use", delete_dialog("light", "Light", &light_uses)),
            cell(
                "Nobody uses it",
                delete_dialog(
                    "fable",
                    "Fable test",
                    &InUse {
                        agent: "Fable test".to_owned(),
                        ..InUse::default()
                    },
                ),
            ),
        ],
        projects: vec![
            cell(
                "billing-api · collapsed",
                project_agents_block("BILL", &made, &defaults, &RoleDefaults::default(), &one),
            ),
            cell(
                "shop-web · Developer → Strong",
                project_agents_block("SHOP", &made, &defaults, &shop_web, &one),
            ),
            cell(
                "shop-web · several tools",
                project_agents_block("SHOP", &several, &several_defaults, &shop_web, &all),
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use otaq_core::domain::TicketKey;
    use otaq_core::named_agents::in_use::ProjectUse;

    use super::*;

    fn list() -> ModelList {
        fixtures::claude_list()
    }

    /// Claude Code alone, with its list: M22's Settings.
    fn one() -> AgentTools {
        AgentTools::one(Some(list()))
    }

    #[test]
    fn the_model_is_worded_as_settings_shows_it() {
        let l = list();
        let l = Some(&l);
        assert_eq!(model_label(None, l), "tool default");
        assert_eq!(model_label(Some(""), l), "tool default");
        assert_eq!(model_label(Some("default"), l), "tool default");
        assert_eq!(model_label(Some("opus"), l), "Opus · always the newest");
        assert_eq!(model_label(Some("claude-opus-4-0"), l), "claude-opus-4-0");
        // Before the first read of the list, a saved model shows as saved.
        assert_eq!(model_label(Some("opus"), None), "opus");
        assert_eq!(model_label(Some("default"), None), "tool default");
    }

    #[test]
    fn a_fresh_install_has_one_row_the_note_and_four_defaults_on_claude_code() {
        let (agents, defaults) = fixtures::fresh();
        let s = section(&agents, &defaults, &one(), &PageState::default());
        assert_eq!(s.title, "Agents");
        assert_eq!(
            s.intro,
            "Named agents are a tool, a model and what you use them for. Agents pick from these names when they create tickets."
        );
        assert_eq!(s.columns, ["NAME", "MODEL", "USE FOR"], "no tool column");
        assert_eq!(s.rows.len(), 1);
        let row = &s.rows[0];
        assert_eq!(
            (row.name.as_str(), row.model.as_str(), row.use_for.as_str()),
            (
                "Claude Code",
                "Tool default",
                "Everything, until you add more agents"
            )
        );
        assert!(!row.model_mono);
        assert_eq!(row.mark, None);
        let note = s.fresh_note.expect("the note");
        assert_eq!(note.title, "One agent, so nothing picks yet");
        assert_eq!(
            note.text,
            "The Project agent and epic leads choose between your agents for each ticket. With only Claude Code there is nothing to choose, so every ticket runs on it. Add a second one, for example a Light agent on Haiku for docs and small config, and they start picking."
        );
        assert_eq!(note.add_label, "Add agent");
        assert_eq!(s.defaults_title, "Default agent for each kind");
        let rows: Vec<(&str, &str, &str, Option<&str>, bool)> = s
            .defaults
            .iter()
            .map(|d| {
                (
                    d.label.as_str(),
                    d.description.as_str(),
                    d.picker.value.name.as_str(),
                    d.picker.value.model.as_deref(),
                    d.picker.read_only,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                (
                    "Developer",
                    "Writes the ticket's code",
                    "Claude Code",
                    Some("tool default"),
                    true
                ),
                (
                    "Validator",
                    "Checks the work before Review (0063)",
                    "Claude Code",
                    Some("tool default"),
                    true
                ),
                (
                    "Epic lead",
                    "Plans and guides an epic's children (0083)",
                    "Claude Code",
                    Some("tool default"),
                    true
                ),
                (
                    "Project agent",
                    "Investigates and creates tickets (0068)",
                    "Claude Code",
                    Some("tool default"),
                    true
                ),
            ]
        );
        assert_eq!(
            s.defaults_note,
            "Projects can override any row in their own settings. A ticket starts at its project's default; agents may pick another name when they create it."
        );
        // One tool: no FALLBACK column, and the checkbox is disabled with its reason (`bVdGw`).
        let s = section(&agents, &defaults, &one(), &PageState::default());
        assert!(
            s.rows
                .iter()
                .all(|r| r.fallback.is_none() && r.tool.is_none() && r.tool_name.is_none())
        );
        let f = &s.fallbacks;
        assert_eq!(
            (f.title.as_str(), f.label.as_str(), f.on, f.enabled),
            (
                "Fallbacks",
                "Use fallback agents automatically",
                false,
                false
            )
        );
        assert_eq!(
            f.note,
            "Disabled: a fallback must be on another tool. Connect a second agent tool to use fallbacks."
        );
        assert_eq!(s.defaults_unchanged, None);
    }

    #[test]
    fn a_list_of_five_words_each_model_and_drops_the_note() {
        let (agents, defaults) = fixtures::made();
        let s = section(&agents, &defaults, &one(), &PageState::default());
        let rows: Vec<(&str, &str)> = s
            .rows
            .iter()
            .map(|r| (r.name.as_str(), r.model.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("Claude Code", "Tool default"),
                ("Strong", "Opus · always the newest"),
                ("Light", "Haiku · always the newest"),
                ("Sonnet", "Sonnet · always the newest"),
                ("Fable test", "Fable · always the newest"),
            ]
        );
        assert_eq!(s.fresh_note, None);
        let picker = &s.defaults[0].picker;
        assert!(!picker.read_only);
        let options: Vec<(&str, Option<&str>, bool)> = picker
            .options
            .iter()
            .map(|o| (o.name.as_str(), o.model.as_deref(), o.selected))
            .collect();
        assert_eq!(
            options,
            [
                ("Claude Code", Some("tool default"), true),
                ("Strong", Some("Opus · always the newest"), false),
                ("Light", Some("Haiku · always the newest"), false),
                ("Sonnet", Some("Sonnet · always the newest"), false),
                ("Fable test", Some("Fable · always the newest"), false),
            ]
        );
        assert_eq!(s.rows[1].menu_label, "More for Strong");
    }

    #[test]
    fn a_marked_row_carries_the_amber_line_with_the_model_cut_out() {
        let (agents, defaults) = fixtures::marked();
        let s = section(&agents, &defaults, &one(), &PageState::default());
        let mark = s.rows[4].mark.as_ref().expect("the mark");
        assert_eq!(mark.before, "Claude Code couldn't run ");
        assert_eq!(mark.model.as_deref(), Some("claude-fable-5-1"));
        assert_eq!(mark.after, " on SHOP-48");
        assert_eq!(mark.edit_label, "Edit…");
        assert!(s.rows[..4].iter().all(|r| r.mark.is_none()));

        // A line without a model to cut out is shown whole.
        let plain = mark_view(&Mark {
            model: None,
            ..fixtures::fable_mark()
        });
        assert_eq!(
            plain.before,
            "Claude Code couldn't run claude-fable-5-1 on SHOP-48"
        );
        assert_eq!((plain.model, plain.after.as_str()), (None, ""));
    }

    #[test]
    fn the_menu_is_edit_duplicate_then_delete_after_a_separator() {
        let items = menu(5);
        let shape: Vec<(MenuAction, &str, bool, bool, bool)> = items
            .iter()
            .map(|i| (i.action, i.label.as_str(), i.danger, i.separated, i.enabled))
            .collect();
        assert_eq!(
            shape,
            [
                (MenuAction::Edit, "Edit…", false, false, true),
                (MenuAction::Duplicate, "Duplicate", false, false, true),
                (MenuAction::Delete, "Delete…", true, true, true),
            ]
        );
        assert!(items.iter().all(|i| i.tooltip.is_none()));
        let icons: Vec<IconName> = items.iter().map(|i| i.icon).collect();
        assert_eq!(icons, [IconName::Pencil, IconName::Copy, IconName::Trash2]);

        // The last agent: Delete… is disabled with the reason in its tooltip.
        let last = &menu(1)[2];
        assert!(!last.enabled);
        assert_eq!(
            last.tooltip.as_deref(),
            Some(format!("{PRODUCT_NAME} needs at least one agent").as_str())
        );
    }

    #[test]
    fn add_agent_has_no_fallback_field_and_saves_once_the_name_is_new() {
        let (agents, _) = fixtures::made();
        let d = dialog(&agents, None, Draft::default(), &one(), None);
        assert_eq!(d.title, "Add agent");
        assert_eq!(d.id, None);
        assert_eq!(
            (
                d.name_label.as_str(),
                d.tool_label.as_str(),
                d.tool.as_str(),
                d.tool_note.as_deref()
            ),
            (
                "Name",
                "Tool",
                "Claude Code",
                Some("the only connected tool")
            )
        );
        assert_eq!(d.model_label, "Model");
        assert_eq!(d.picker.value.label, "Tool default");
        assert_eq!(d.use_for_label, "Use for");
        assert_eq!(
            d.use_for_hint,
            "Agents read this when they pick an agent for a ticket."
        );
        assert_eq!(
            (d.cancel_label.as_str(), d.save_label.as_str()),
            ("Cancel", "Add agent")
        );
        assert_eq!(d.delete, None, "Delete… only in Edit");
        assert_eq!(d.mark, None);
        assert!(!d.save_enabled, "no name yet");
        assert_eq!(d.name_problem, None, "an empty name isn't red before Save");
        assert_eq!(d.fallback, None, "no Fallback field with one tool");
        assert!(d.tool_options.is_empty(), "the tool is read-only");
        assert_eq!(d.tool_avatar, None);

        let typed = |name: &str| Draft {
            name: name.to_owned(),
            ..Draft::default()
        };
        let d = dialog(&agents, None, typed(" light "), &one(), None);
        assert_eq!(
            d.name_problem.as_deref(),
            Some("There's already an agent named light")
        );
        assert!(!d.save_enabled);
        let d = dialog(&agents, None, typed("Lighter"), &one(), None);
        assert_eq!(d.name_problem, None);
        assert!(d.save_enabled);
    }

    #[test]
    fn edit_keeps_its_own_name_and_shows_the_mark_and_delete() {
        let (agents, _) = fixtures::marked();
        let fable = &agents[4];
        let d = dialog(&agents, Some("fable"), Draft::of(fable), &one(), None);
        assert_eq!(d.title, "Edit agent · Fable test");
        assert_eq!(d.name_problem, None, "its own name isn't taken");
        assert!(d.save_enabled);
        assert_eq!(d.save_label, "Save");
        assert_eq!(
            d.mark.as_ref().and_then(|m| m.model.as_deref()),
            Some("claude-fable-5-1")
        );
        assert_eq!(
            d.picker.hint,
            "From Claude Code's own list. The list offered it, but this account can't use it."
        );
        assert_eq!(
            d.picker.value.detail.as_deref(),
            Some("now claude-fable-5-1")
        );
        let delete = d.delete.expect("Delete…");
        assert_eq!((delete.label.as_str(), delete.enabled), ("Delete…", true));

        // Picking another model: the usual hint (saving it clears the mark, T-0334).
        let opus = Draft {
            model: Some("opus".to_owned()),
            ..Draft::of(fable)
        };
        let d = dialog(&agents, Some("fable"), opus, &one(), None);
        assert!(
            d.picker
                .hint
                .starts_with("From Claude Code's own list. A follow choice")
        );

        // Renaming onto another entry, ignoring case.
        let taken = Draft {
            name: "STRONG".to_owned(),
            ..Draft::of(fable)
        };
        let d = dialog(&agents, Some("fable"), taken, &one(), None);
        assert_eq!(
            d.name_problem.as_deref(),
            Some("There's already an agent named STRONG")
        );
    }

    #[test]
    fn the_last_agents_delete_is_disabled_with_its_reason() {
        let (agents, _) = fixtures::fresh();
        let d = dialog(&agents, Some("cc"), Draft::of(&agents[0]), &one(), None);
        let delete = d.delete.expect("Delete…");
        assert!(!delete.enabled);
        assert_eq!(
            delete.tooltip.as_deref(),
            Some(format!("{PRODUCT_NAME} needs at least one agent").as_str())
        );
    }

    #[test]
    fn the_services_refusals_are_worded_as_the_dialog_words_them() {
        let (agents, _) = fixtures::made();
        let draft = Draft {
            name: "Light".to_owned(),
            ..Draft::default()
        };
        let d = dialog(
            &agents[..1],
            None,
            draft.clone(),
            &one(),
            Some(("name_taken", "Another agent is already called Light")),
        );
        assert_eq!(
            d.name_problem.as_deref(),
            Some("There's already an agent named Light")
        );
        assert!(!d.save_enabled);
        let d = dialog(
            &agents[..1],
            None,
            draft,
            &one(),
            Some((
                "model_not_listed",
                "claude-opus-9 isn't in the tool's model list",
            )),
        );
        assert_eq!(
            d.model_problem.as_deref(),
            Some("claude-opus-9 isn't in the tool's model list")
        );
    }

    #[test]
    fn duplicates_draft_opens_add_agent_prefilled() {
        let (agents, _) = fixtures::made();
        let draft = Draft {
            name: format!("{} copy", agents[1].name),
            ..Draft::of(&agents[1])
        };
        let d = dialog(&agents, None, draft, &one(), None);
        assert_eq!(d.title, "Add agent");
        assert_eq!(d.draft.name, "Strong copy");
        assert_eq!(d.draft.use_for, "Complex features, tricky bugs");
        assert_eq!(d.picker.value.label, "Opus · always the newest");
        assert!(d.save_enabled);
    }

    fn key(k: &str) -> TicketKey {
        k.parse().unwrap()
    }

    #[test]
    fn delete_in_use_is_refused_with_what_uses_it() {
        let uses = InUse {
            agent: "Light".to_owned(),
            tickets: [
                "SHOP-24",
                "SHOP-40.3",
                "SHOP-41",
                "SHOP-50",
                "SHOP-51",
                "SHOP-52",
                "SHOP-53",
                "SHOP-54",
                "SHOP-55",
                "SHOP-56",
                "SHOP-57",
                "SHOP-58",
            ]
            .map(key)
            .to_vec(),
            defaults: Vec::new(),
            projects: vec![ProjectUse {
                key: "DOCS".to_owned(),
                name: "docs-site".to_owned(),
                roles: vec![Role::Validator],
            }],
            fallback_of: Vec::new(),
        };
        let DeleteDialog::Refused {
            title,
            text,
            lines,
            tickets,
            show_tickets_label,
            ok_label,
        } = delete_dialog("light", "Light", &uses)
        else {
            panic!("refused")
        };
        assert_eq!(title, "Light can't be deleted");
        assert_eq!(
            text,
            "Light is used by 12 tickets and project docs-site. Change those first, then delete it."
        );
        assert_eq!(
            lines,
            [
                "12 tickets · SHOP-24, SHOP-40.3, SHOP-41 and 9 more",
                "docs-site · Validator default"
            ]
        );
        assert_eq!(tickets.len(), 12, "Show tickets lists every key");
        assert_eq!(tickets[11], "SHOP-58");
        assert_eq!(show_tickets_label.as_deref(), Some("Show tickets"));
        assert_eq!(ok_label, "OK");

        // App-wide defaults, one ticket.
        let uses = InUse {
            agent: "Light".to_owned(),
            tickets: vec![key("SHOP-24")],
            defaults: vec![Role::Developer, Role::Validator],
            ..InUse::default()
        };
        let DeleteDialog::Refused { lines, .. } = delete_dialog("light", "Light", &uses) else {
            panic!("refused")
        };
        assert_eq!(
            lines,
            [
                "1 ticket · SHOP-24",
                "App-wide · Developer and Validator defaults"
            ]
        );

        // Only defaults: no Show tickets.
        let uses = InUse {
            agent: "Light".to_owned(),
            defaults: vec![Role::Validator],
            ..InUse::default()
        };
        let DeleteDialog::Refused {
            show_tickets_label, ..
        } = delete_dialog("light", "Light", &uses)
        else {
            panic!("refused")
        };
        assert_eq!(show_tickets_label, None);
    }

    #[test]
    fn delete_of_an_unused_agent_asks_once() {
        let uses = InUse {
            agent: "Fable test".to_owned(),
            ..InUse::default()
        };
        assert_eq!(
            delete_dialog("fable", "Fable test", &uses),
            DeleteDialog::Confirm {
                id: "fable".to_owned(),
                title: "Delete Fable test?".to_owned(),
                text: "Nothing uses Fable test: no unfinished ticket and no default.".to_owned(),
                note:
                    "Finished tickets that ran on it keep their history as “Fable test (deleted)”."
                        .to_owned(),
                cancel_label: "Cancel".to_owned(),
                delete_label: "Delete".to_owned(),
            }
        );
    }

    #[test]
    fn roles_go_back_as_stored() {
        for role in ROLES {
            let id = RoleId::from(role);
            assert_eq!(serde_json::to_value(id).unwrap(), role.as_str());
            assert_eq!(Role::from(id), role);
        }
    }

    #[test]
    fn the_gallery_has_every_state() {
        let g = gallery();
        assert_eq!(g.sections.len(), 7);
        assert!(g.sections[0].view.fresh_note.is_some());
        assert!(g.sections[2].view.rows[4].mark.is_some());
        assert!(g.sections[3].view.fallbacks.enabled);
        assert!(g.sections[4].view.defaults_unchanged.is_some());
        assert!(g.sections[5].view.rows[3].tool_problem.is_some());
        assert_eq!(g.dialogs[2].view.draft.name, "Strong copy");
        assert!(g.dialogs[5].view.fallback.is_some());
        assert!(
            g.dialogs[7]
                .view
                .picker
                .status
                .as_ref()
                .is_some_and(|s| s.loading)
        );
        assert!(
            g.dialogs[8]
                .view
                .picker
                .status
                .as_ref()
                .is_some_and(|s| !s.loading)
        );
        assert!(g.dialogs[9].view.picker.value.no_longer_listed.is_some());
        assert!(matches!(g.deletes[0].view, DeleteDialog::Refused { .. }));
        assert!(matches!(g.deletes[1].view, DeleteDialog::Confirm { .. }));
        assert!(!g.projects[0].view.expanded);
        assert!(g.projects[1].view.expanded);
        assert!(g.projects[2].view.rows[0].picker.value.tool.is_some());
    }

    // M23 (T-0361): several tools.

    fn rows_of(s: &Section) -> Vec<(&str, Option<&str>, &str, Option<&str>)> {
        s.rows
            .iter()
            .map(|r| {
                (
                    r.name.as_str(),
                    r.tool_name.as_deref(),
                    r.model.as_str(),
                    r.fallback.as_deref(),
                )
            })
            .collect()
    }

    #[test]
    fn a_fresh_install_with_two_tools_has_both_columns_and_the_checkbox_off() {
        let (agents, defaults) = fixtures::fresh_two();
        let s = section(
            &agents,
            &defaults,
            &fixtures::two_tools(),
            &PageState::default(),
        );
        assert_eq!(s.columns, ["NAME", "TOOL · MODEL", "USE FOR", "FALLBACK"]);
        assert_eq!(
            rows_of(&s),
            [
                (
                    "Claude Code",
                    Some("Claude Code"),
                    "tool default",
                    Some("none")
                ),
                ("Codex", Some("Codex"), "tool default", Some("none")),
            ]
        );
        let avatars: Vec<Option<&str>> = s
            .rows
            .iter()
            .map(|r| r.tool.as_ref().map(|t| t.letters.as_str()))
            .collect();
        assert_eq!(avatars, [Some("CC"), Some("CX")]);
        assert!(
            s.rows
                .iter()
                .all(|r| r.tool_problem.is_none() && r.added.is_none())
        );
        assert_eq!(s.fresh_note, None);
        // Every default on the first entry, named after its tool: no "Claude Code · " before its model.
        let picker = &s.defaults[0].picker;
        assert_eq!(
            (
                picker.value.name.as_str(),
                picker.value.model.as_deref(),
                picker.value.tool.as_ref().map(|t| t.letters.as_str())
            ),
            ("Claude Code", Some("tool default"), Some("CC"))
        );
        let f = &s.fallbacks;
        assert_eq!((f.on, f.enabled), (false, true), "off by default");
        assert_eq!(
            f.help,
            "When an agent's tool hits its usage limit or isn't available, waiting tickets start on that agent's fallback instead. Off: they wait, and the banner offers Move waiting to…."
        );
        assert_eq!(
            f.note,
            format!(
                "Off by default: {PRODUCT_NAME} can't guess which fallbacks you want. Running agents never move; a conversation can't change tools."
            )
        );
        let on = section(
            &agents,
            &defaults,
            &fixtures::two_tools(),
            &PageState {
                auto_fallback: true,
                ..PageState::default()
            },
        );
        assert!(on.fallbacks.on);
    }

    #[test]
    fn several_tools_name_each_tool_its_model_and_fallback() {
        let (agents, defaults) = fixtures::several();
        // `cnRA5`: Copilot isn't signed in.
        let tools = fixtures::tools(&[Tool::Claude, Tool::Codex, Tool::OpenCode]);
        let s = section(&agents, &defaults, &tools, &PageState::default());
        assert_eq!(
            rows_of(&s),
            [
                (
                    "Strong",
                    Some("Claude Code"),
                    "Opus · always the newest",
                    Some("Strong GPT")
                ),
                ("Standard", Some("Codex"), "gpt-5.5", Some("Sonnet")),
                (
                    "Light",
                    Some("Claude Code"),
                    "Haiku · always the newest",
                    Some("none")
                ),
                (
                    "Verifier",
                    Some("GitHub Copilot"),
                    "gpt-5.5",
                    Some("Strong")
                ),
                ("Strong GPT", Some("Codex"), "gpt-5.5-pro", Some("none")),
                (
                    "Sonnet",
                    Some("Claude Code"),
                    "Sonnet · always the newest",
                    Some("none")
                ),
                (
                    "Gemini Pro",
                    Some("OpenCode"),
                    "google/gemini-3-pro",
                    Some("none")
                ),
            ]
        );
        let problems: Vec<Option<&str>> =
            s.rows.iter().map(|r| r.tool_problem.as_deref()).collect();
        assert_eq!(
            problems,
            [
                None,
                None,
                None,
                Some("Copilot isn't connected · its tickets wait"),
                None,
                None,
                None
            ]
        );
        assert!(s.rows[1].model_mono && !s.rows[0].model_mono);
        let defaults: Vec<(&str, Option<&str>)> = s
            .defaults
            .iter()
            .map(|d| {
                (
                    d.picker.value.name.as_str(),
                    d.picker.value.model.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            defaults,
            [
                ("Standard", Some("Codex · gpt-5.5")),
                ("Verifier", Some("GitHub Copilot · gpt-5.5")),
                ("Strong", Some("Claude Code · Opus · always the newest")),
                ("Standard", Some("Codex · gpt-5.5")),
            ]
        );
    }

    #[test]
    fn copilots_auto_reads_without_always_the_newest() {
        let list = fixtures::copilot_list();
        assert_eq!(
            model_label(Some("auto@efficiency"), Some(&list)),
            "Auto · efficiency"
        );
        assert_eq!(
            model_label(Some("auto@intelligence"), Some(&list)),
            "Auto · intelligence"
        );
        let codex = fixtures::codex_list();
        assert_eq!(
            model_label(Some("newest:gpt-sol"), Some(&codex)),
            "GPT Sol · always the newest"
        );
    }

    #[test]
    fn both_m23_marks_show_on_the_row_and_in_the_dialog() {
        let (agents, defaults) = fixtures::several_marked();
        let all = fixtures::tools(Tool::ALL);
        let s = section(&agents, &defaults, &all, &PageState::default());
        let old = s.rows.iter().find(|r| r.name == "Old GPT").unwrap();
        let mark = old.mark.as_ref().unwrap();
        assert_eq!(
            (
                mark.before.as_str(),
                mark.model.as_deref(),
                mark.after.as_str()
            ),
            ("Codex no longer lists ", Some("gpt-5.4"), "")
        );
        let verifier = s.rows.iter().find(|r| r.name == "Verifier").unwrap();
        let mark = verifier.mark.as_ref().unwrap();
        assert_eq!(
            (
                mark.before.as_str(),
                mark.model.as_deref(),
                mark.after.as_str()
            ),
            (
                "Copilot ran auto instead of ",
                Some("gpt-5.5"),
                " on SHOP-29"
            )
        );

        let d = dialog(&agents, Some("old-gpt"), Draft::of(&agents[1]), &all, None);
        assert_eq!(
            d.mark.as_ref().and_then(|m| m.model.as_deref()),
            Some("gpt-5.4")
        );
        assert_eq!(d.picker.value.label, "gpt-5.4");
        assert_eq!(
            d.picker.value.no_longer_listed.as_deref(),
            Some("no longer listed")
        );
        assert_eq!(
            d.picker.hint,
            "Codex's list doesn't have it any more. Pick another model; it isn't removed for you."
        );
        let verifier = agents.iter().find(|a| a.id == "verifier").unwrap();
        let d = dialog(&agents, Some("verifier"), Draft::of(verifier), &all, None);
        assert_eq!(
            d.mark.map(|m| m.before),
            Some("Copilot ran auto instead of ".to_owned())
        );
        assert_eq!(d.picker.value.no_longer_listed, None, "gpt-5.5 is listed");
    }

    #[test]
    fn the_tool_picker_offers_connected_tools_and_changing_it_clears_the_model_and_a_same_tool_fallback()
     {
        let (agents, _) = fixtures::several();
        let tools = fixtures::tools(&[Tool::Claude, Tool::Codex, Tool::OpenCode]);
        let standard = agents.iter().find(|a| a.id == "standard").unwrap();
        let d = dialog(&agents, Some("standard"), Draft::of(standard), &tools, None);
        let options: Vec<(Tool, &str, bool)> = d
            .tool_options
            .iter()
            .map(|o| (o.tool, o.name.as_str(), o.selected))
            .collect();
        assert_eq!(
            options,
            [
                (Tool::Claude, "Claude Code", false),
                (Tool::Codex, "Codex", true),
                (Tool::OpenCode, "OpenCode", false),
            ]
        );
        assert_eq!(d.tool, "Codex");
        assert_eq!(
            d.tool_avatar.as_ref().map(|a| a.letters.as_str()),
            Some("CX")
        );
        assert_eq!(d.tool_note, None);
        assert_eq!(d.picker.value.label, "gpt-5.5");

        // Standard's fallback is Sonnet (Claude Code): moving Standard to Claude Code clears it, and the model.
        let moved = change_tool(Draft::of(standard), Tool::Claude, &agents, Some("standard"));
        assert_eq!(
            (
                moved.tool,
                moved.model.as_deref(),
                moved.fallback.as_deref()
            ),
            (Some(Tool::Claude), None, None)
        );
        let d = dialog(&agents, Some("standard"), moved, &tools, None);
        assert_eq!(d.picker.value.label, "Tool default");
        assert!(d.picker.hint.starts_with("From Claude Code's own list."));
        // Moving it to OpenCode keeps Sonnet.
        let moved = change_tool(
            Draft::of(standard),
            Tool::OpenCode,
            &agents,
            Some("standard"),
        );
        assert_eq!(moved.fallback.as_deref(), Some("sonnet"));
        assert_eq!(moved.model, None);
        // The same tool again changes nothing.
        let same = change_tool(Draft::of(standard), Tool::Codex, &agents, Some("standard"));
        assert_eq!(same, Draft::of(standard));

        // An entry whose tool isn't connected still shows its tool, amber.
        let verifier = agents.iter().find(|a| a.id == "verifier").unwrap();
        let d = dialog(&agents, Some("verifier"), Draft::of(verifier), &tools, None);
        assert!(
            d.tool_options
                .iter()
                .any(|o| o.tool == Tool::Copilot && o.selected)
        );
        assert_eq!(
            d.tool_problem.as_deref(),
            Some("Copilot isn't connected · its tickets wait")
        );

        // Add agent starts on the first connected tool.
        let d = dialog(
            &agents,
            None,
            Draft::default(),
            &fixtures::tools(&[Tool::Codex, Tool::Copilot]),
            None,
        );
        assert_eq!(d.tool, "Codex");
    }

    #[test]
    fn fallback_items_are_on_other_tools_and_never_the_entry_itself() {
        let (agents, _) = fixtures::several();
        let all = fixtures::tools(Tool::ALL);
        let strong = agents.iter().find(|a| a.id == "strong").unwrap();
        let d = dialog(&agents, Some("strong"), Draft::of(strong), &all, None);
        let field = d.fallback.expect("several tools: the Fallback field");
        assert_eq!(field.label, "Fallback");
        assert_eq!(
            field.hint,
            "Another named agent on a different tool. Used when Claude Code hits its limit or isn't available. One step: a fallback's own fallback isn't used."
        );
        let options: Vec<(&str, Option<&str>, bool)> = field
            .picker
            .options
            .iter()
            .map(|o| (o.name.as_str(), o.model.as_deref(), o.selected))
            .collect();
        assert_eq!(
            options,
            [
                ("None", None, false),
                ("Standard", Some("Codex · gpt-5.5"), false),
                ("Verifier", Some("GitHub Copilot · gpt-5.5"), false),
                ("Strong GPT", Some("Codex · gpt-5.5-pro"), true),
                ("Gemini Pro", Some("OpenCode · google/gemini-3-pro"), false),
            ]
        );
        // A Codex entry: no Codex entries, and not itself.
        let standard = agents.iter().find(|a| a.id == "standard").unwrap();
        let d = dialog(&agents, Some("standard"), Draft::of(standard), &all, None);
        let names: Vec<String> = d
            .fallback
            .unwrap()
            .picker
            .options
            .into_iter()
            .map(|o| o.name)
            .collect();
        assert_eq!(
            names,
            [
                "None",
                "Strong",
                "Light",
                "Verifier",
                "Sonnet",
                "Gemini Pro"
            ]
        );
        assert_eq!(
            d.picker.hint,
            "From Codex's own list. A follow choice is resolved when each session starts, and the session keeps the exact model it ran on."
        );

        // The service's refusal goes under the field.
        let d = dialog(
            &agents,
            Some("standard"),
            Draft::of(standard),
            &all,
            Some(("bad_fallback", "A fallback must be on another tool")),
        );
        assert_eq!(
            d.fallback.and_then(|f| f.problem).as_deref(),
            Some("A fallback must be on another tool")
        );

        // Save sends the draft's tool and its fallback; one on the same tool is dropped.
        let f = fields(&Draft::of(standard), &agents, Some("standard"), &all);
        assert_eq!(
            (f.tool, f.fallback.as_deref()),
            (Tool::Codex, Some("sonnet"))
        );
        let wrong = Draft {
            fallback: Some("strong-gpt".to_owned()),
            ..Draft::of(standard)
        };
        assert_eq!(
            fields(&wrong, &agents, Some("standard"), &all).fallback,
            None
        );
        // With one tool there's no field: an edit keeps the entry's fallback.
        let f = fields(&Draft::of(strong), &agents, Some("strong"), &one());
        assert_eq!(f.fallback.as_deref(), Some("strong-gpt"));
    }

    #[test]
    fn the_dialog_says_when_a_tools_list_loads_or_couldnt_load() {
        let (agents, _) = fixtures::several();
        let mut tools = fixtures::tools(Tool::ALL);
        tools.lists.retain(|(t, _)| *t != Tool::Copilot);
        let verifier = agents.iter().find(|a| a.id == "verifier").unwrap();
        let d = dialog(&agents, Some("verifier"), Draft::of(verifier), &tools, None);
        assert_eq!(
            d.picker.status.as_ref().map(|s| s.text.as_str()),
            Some("Loading Copilot's models…")
        );
        tools.failed.push(Tool::Copilot);
        let d = dialog(&agents, Some("verifier"), Draft::of(verifier), &tools, None);
        let status = d.picker.status.expect("couldn't load");
        assert_eq!(status.text, "Couldn't load Copilot's models");
        assert_eq!(status.retry_tool, Some(Tool::Copilot));
        assert!(
            status
                .note
                .unwrap()
                .starts_with("Verifier keeps gpt-5.5 as saved.")
        );
        assert_eq!(d.picker.value.label, "gpt-5.5", "the saved model stays");
    }

    #[test]
    fn the_visit_marks_only_entries_a_tool_added_since_the_page_opened() {
        let (two, _) = fixtures::fresh_two();
        let (three, _) = fixtures::copilot_added();
        let mut visit = Visit::default();
        assert_eq!(
            visit.page(&three, false).added,
            Vec::<String>::new(),
            "not opened yet"
        );
        visit.open(&two);
        assert_eq!(
            visit.page(&three, true),
            PageState {
                auto_fallback: true,
                added: vec!["cp".to_owned()],
            }
        );
        // One the user added is never "added".
        let mut mine = three.clone();
        mine.push(fixtures::agent("light", "Light", Some("haiku"), "Docs"));
        visit.added_by_user("light");
        assert_eq!(visit.page(&mine, false).added, ["cp"]);
        // Opening again: nothing is new.
        visit.open(&mine);
        assert!(visit.page(&mine, false).added.is_empty());
        // Reads.
        visit.read(Tool::Copilot, false);
        assert_eq!(visit.failed(), [Tool::Copilot]);
        visit.read(Tool::Copilot, true);
        assert!(visit.failed().is_empty());
    }

    #[test]
    fn a_tool_connected_later_marks_its_row_added_and_the_defaults_unchanged() {
        let (agents, defaults) = fixtures::copilot_added();
        let tools = fixtures::tools(&[Tool::Claude, Tool::Codex, Tool::Copilot]);
        let page = PageState {
            added: vec!["cp".to_owned()],
            ..PageState::default()
        };
        let s = section(&agents, &defaults, &tools, &page);
        let added: Vec<(&str, Option<&str>)> = s
            .rows
            .iter()
            .map(|r| (r.name.as_str(), r.added.as_deref()))
            .collect();
        assert_eq!(
            added,
            [
                ("Claude Code", None),
                ("Codex", None),
                ("GitHub Copilot", Some("added"))
            ]
        );
        assert_eq!(s.rows[2].model, "tool default");
        assert_eq!(
            s.defaults_unchanged,
            Some(DefaultsNote {
                heading: "DEFAULTS · UNCHANGED".to_owned(),
                text: "Developer, Validator, Epic lead, Project agent → Claude Code".to_owned(),
            })
        );
        // Once the page is left, nothing is marked.
        let s = section(&agents, &defaults, &tools, &PageState::default());
        assert_eq!(s.defaults_unchanged, None);
        // Defaults on several entries.
        let (agents, defaults) = fixtures::several();
        assert_eq!(
            defaults_line(&agents, &defaults),
            "Developer, Project agent → Standard · Validator → Verifier · Epic lead → Strong"
        );
    }
}
