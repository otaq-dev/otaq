//! Settings → Agents as the page draws it (T-0340; AGT-01…AGT-03, AGT-05's mark; d0086, d0088; *15 · Settings*
//! `bVdGw`, `FYvzM`, `cnRA5` in their one-tool form, `XlyOa`, `WFHlh`, `Z4gQQQ`, `Wjp2r`, `O7UTA`): the **Named
//! agents** table, the fresh-install note, the four **role defaults**, the Add / Edit dialog with its checks, the ⋯
//! menu and the three ways Delete goes. Every word and every enabled rule is here; the page only draws.
//!
//! While one tool is connected there is no tool column, no tool mark and no Fallbacks block (d0086; M23 adds them).
//! With one named agent nothing offers a choice: the default pickers are read-only and the note says how picking
//! starts.
//!
//! [`model_label`] is **the one formatter** of a named agent's model as Settings shows it ("tool default", "Opus ·
//! always the newest", an exact id), used by the agent rows and pickers elsewhere too (never a raw alias).

use otaq_core::agent_tools::{ModelKind, ModelList, Tool};
use otaq_core::named_agents::{InUse, Mark, NamedAgent, Role, RoleDefaults};
use otaq_core::naming::PRODUCT_NAME;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::model_picker::{self, Picker};
use super::tool_avatar::AvatarTool;
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
/// for a follow choice, else the exact id. A model the list doesn't have (or no list yet) shows as saved.
pub fn model_label(model: Option<&str>, list: Option<&ModelList>) -> String {
    if is_tool_default(model, list) {
        return TOOL_DEFAULT.to_owned();
    }
    let id = model.unwrap_or_default().trim();
    match list.and_then(|l| l.entry(id)) {
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

/// The tool a new entry gets: the connected one (M23 adds the Tool field's choice).
pub fn new_entry_tool() -> Tool {
    Tool::ALL[0]
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
        let at = mark.text.find(m)?;
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

/// One row of the Named agents table (NAME · MODEL · USE FOR).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentTableRow")]
pub struct TableRow {
    pub id: String,
    pub name: String,
    /// "Tool default", "Opus · always the newest", an exact id.
    pub model: String,
    pub model_mono: bool,
    pub use_for: String,
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
    /// What is sent back; `None` = "Use app default" in a project.
    pub agent: Option<String>,
    pub name: String,
    /// "tool default", "Opus · always the newest"; none for "Use app default (…)".
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

/// The whole Agents section of the global page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentsSection")]
pub struct Section {
    pub title: String,
    pub intro: String,
    pub table_title: String,
    /// NAME · MODEL · USE FOR (no TOOL column while one tool is connected).
    pub columns: Vec<String>,
    pub rows: Vec<TableRow>,
    pub add_label: String,
    pub fresh_note: Option<FreshNote>,
    pub defaults_title: String,
    pub defaults: Vec<DefaultRow>,
    /// "Projects can override any row…".
    pub defaults_note: String,
}

/// An agent as a picker option, with its model.
fn option(agent: &NamedAgent, list: Option<&ModelList>, selected: bool) -> AgentOption {
    AgentOption {
        agent: Some(agent.id.clone()),
        name: agent.name.clone(),
        model: Some(agent_model(agent, list)),
        selected,
    }
}

/// An app-wide default's picker: every entry, the default's one picked.
fn default_picker(
    agents: &[NamedAgent],
    on: Option<&str>,
    list: Option<&ModelList>,
) -> AgentPicker {
    let options: Vec<AgentOption> = agents
        .iter()
        .map(|a| option(a, list, on == Some(a.id.as_str())))
        .collect();
    let value = options
        .iter()
        .find(|o| o.selected)
        .cloned()
        // Every default is set once an entry exists; a missing one shows empty.
        .unwrap_or(AgentOption {
            agent: None,
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

/// The Agents section from `agents.list` and the cached model list (T-0335).
pub fn section(
    agents: &[NamedAgent],
    defaults: &RoleDefaults,
    list: Option<&ModelList>,
) -> Section {
    let rows = agents
        .iter()
        .map(|a| TableRow {
            id: a.id.clone(),
            name: a.name.clone(),
            model: if is_tool_default(a.model.as_deref(), list) {
                TOOL_DEFAULT_LABEL.to_owned()
            } else {
                agent_model(a, list)
            },
            model_mono: is_exact(a.model.as_deref(), list),
            use_for: a.use_for.clone(),
            mark: a.mark.as_ref().map(mark_view),
            menu_label: format!("More for {}", a.name),
            menu: menu(agents.len()),
        })
        .collect();
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
    Section {
        title: "Agents".to_owned(),
        intro: "Named agents are a tool, a model and what you use them for. Agents pick from these names when they create tickets.".to_owned(),
        table_title: "Named agents".to_owned(),
        columns: ["NAME", "MODEL", "USE FOR"].map(str::to_owned).to_vec(),
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
                picker: default_picker(agents, defaults.get(role), list),
            })
            .collect(),
        defaults_note: "Projects can override any row in their own settings. A ticket starts at its project's default; agents may pick another name when they create it.".to_owned(),
    }
}

/// A project's picker for `role` (AGT-03, `mQijC`): "Use app default (<name>)" first, then every entry.
pub fn project_picker(
    agents: &[NamedAgent],
    app: &RoleDefaults,
    project: &RoleDefaults,
    role: Role,
    list: Option<&ModelList>,
) -> AgentPicker {
    let own = project
        .get(role)
        .filter(|id| agents.iter().any(|a| a.id == *id));
    let app_name = app
        .get(role)
        .and_then(|id| agents.iter().find(|a| a.id == id))
        .map(|a| a.name.as_str())
        .unwrap_or_default();
    let use_app = AgentOption {
        agent: None,
        name: format!("Use app default ({app_name})"),
        model: None,
        selected: own.is_none(),
    };
    let mut options = vec![use_app];
    options.extend(
        agents
            .iter()
            .map(|a| option(a, list, own == Some(a.id.as_str()))),
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

/// What the dialog's fields hold while the user types (React keeps the text; this view checks it).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "AgentDraft")]
pub struct Draft {
    pub name: String,
    /// `None` = tool default.
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub use_for: String,
}

impl Draft {
    pub fn of(agent: &NamedAgent) -> Self {
        Self {
            name: agent.name.clone(),
            model: agent.model.clone(),
            use_for: agent.use_for.clone(),
        }
    }
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

/// Add agent or Edit agent · <name> (`k8WyHN`, `WFHlh`). No Fallback field while one tool is connected.
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
    /// Read-only: "Claude Code".
    pub tool: String,
    /// "the only connected tool".
    pub tool_note: String,
    pub model_label: String,
    pub picker: Picker,
    /// Red under the model (the service refused it: `model_not_listed`).
    pub model_problem: Option<String>,
    pub use_for_label: String,
    pub use_for_hint: String,
    pub delete: Option<DeleteButton>,
    pub cancel_label: String,
    /// "Add agent" or "Save".
    pub save_label: String,
    pub save_enabled: bool,
}

/// The dialog for `editing` (an entry's id; `None` = Add agent) with `draft` as typed so far. `refused`: the
/// service's answer to Save, as `(refusal code, message)`.
pub fn dialog(
    agents: &[NamedAgent],
    editing: Option<&str>,
    draft: Draft,
    list: Option<&ModelList>,
    refused: Option<(&str, &str)>,
) -> Dialog {
    let agent = editing.and_then(|id| agents.iter().find(|a| a.id == id));
    let tool = agent.map_or_else(new_entry_tool, |a| a.tool);
    let tool = tool_name(tool);
    let mark = agent.and_then(|a| a.mark.as_ref());
    // The model the mark is about is still the one picked: the hint says the account can't use it.
    let refused_model = agent.zip(mark).is_some_and(|(a, _)| a.model == draft.model);
    let mut name_problem = name_problem(&draft.name, agents, editing);
    let mut model_problem = None;
    match refused {
        Some(("name_taken", _)) => name_problem = Some(name_taken(&draft.name)),
        Some(("empty_field", _)) => name_problem = Some(NAME_NEEDED.to_owned()),
        Some((_, message)) => model_problem = Some(message.to_owned()),
        None => {}
    }
    let save_enabled = !draft.name.trim().is_empty() && name_problem.is_none();
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
        tool: tool.to_owned(),
        tool_note: "the only connected tool".to_owned(),
        model_label: "Model".to_owned(),
        picker: model_picker::picker(tool, list, draft.model.as_deref(), refused_model),
        model_problem,
        use_for_label: "Use for".to_owned(),
        use_for_hint: "Agents read this when they pick an agent for a ticket.".to_owned(),
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
            // No fallback is named: fallbacks are M23.
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

    let list = fixtures::claude_list();
    let list = Some(&list);
    fn cell<T>(label: &str, view: T) -> GalleryCell<T> {
        GalleryCell {
            label: label.to_owned(),
            view,
        }
    }
    let (fresh, fresh_defaults) = fixtures::fresh();
    let (made, defaults) = fixtures::made();
    let (marked, _) = fixtures::marked();
    let fable_draft = Draft {
        name: "Fable test".to_owned(),
        model: Some("fable".to_owned()),
        use_for: "Trying Fable on a few tickets".to_owned(),
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
    Gallery {
        sections: vec![
            cell("Fresh install", section(&fresh, &fresh_defaults, list)),
            cell("A list you made", section(&made, &defaults, list)),
            cell("Fable test marked", section(&marked, &defaults, list)),
        ],
        dialogs: vec![
            cell(
                "Add agent · model picker",
                dialog(&made[..4], None, fable_draft.clone(), list, None),
            ),
            cell(
                "Edit agent · the mark",
                dialog(&marked, Some("fable"), Draft::of(&marked[4]), list, None),
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
                    list,
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
                    list,
                    None,
                ),
            ),
            cell(
                "The only agent",
                dialog(&fresh, Some("cc"), Draft::of(&fresh[0]), list, None),
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
                project_agents_block("BILL", &made, &defaults, &RoleDefaults::default(), list),
            ),
            cell(
                "shop-web · Developer → Strong",
                project_agents_block("SHOP", &made, &defaults, &shop_web, list),
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
        let s = section(&agents, &defaults, Some(&list()));
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
        // No Fallbacks block: the section has nothing for it (M23).
        let json = serde_json::to_string(&section(&agents, &defaults, Some(&list()))).unwrap();
        assert!(!json.to_lowercase().contains("fallback"));
    }

    #[test]
    fn a_list_of_five_words_each_model_and_drops_the_note() {
        let (agents, defaults) = fixtures::made();
        let s = section(&agents, &defaults, Some(&list()));
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
        let s = section(&agents, &defaults, Some(&list()));
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
        let l = list();
        let d = dialog(&agents, None, Draft::default(), Some(&l), None);
        assert_eq!(d.title, "Add agent");
        assert_eq!(d.id, None);
        assert_eq!(
            (
                d.name_label.as_str(),
                d.tool_label.as_str(),
                d.tool.as_str(),
                d.tool_note.as_str()
            ),
            ("Name", "Tool", "Claude Code", "the only connected tool")
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
        let json = serde_json::to_string(&d).unwrap();
        assert!(!json.to_lowercase().contains("fallback"));

        let typed = |name: &str| Draft {
            name: name.to_owned(),
            ..Draft::default()
        };
        let d = dialog(&agents, None, typed(" light "), Some(&l), None);
        assert_eq!(
            d.name_problem.as_deref(),
            Some("There's already an agent named light")
        );
        assert!(!d.save_enabled);
        let d = dialog(&agents, None, typed("Lighter"), Some(&l), None);
        assert_eq!(d.name_problem, None);
        assert!(d.save_enabled);
    }

    #[test]
    fn edit_keeps_its_own_name_and_shows_the_mark_and_delete() {
        let (agents, _) = fixtures::marked();
        let l = list();
        let fable = &agents[4];
        let d = dialog(&agents, Some("fable"), Draft::of(fable), Some(&l), None);
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
        let d = dialog(&agents, Some("fable"), opus, Some(&l), None);
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
        let d = dialog(&agents, Some("fable"), taken, Some(&l), None);
        assert_eq!(
            d.name_problem.as_deref(),
            Some("There's already an agent named STRONG")
        );
    }

    #[test]
    fn the_last_agents_delete_is_disabled_with_its_reason() {
        let (agents, _) = fixtures::fresh();
        let d = dialog(
            &agents,
            Some("cc"),
            Draft::of(&agents[0]),
            Some(&list()),
            None,
        );
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
        let l = list();
        let draft = Draft {
            name: "Light".to_owned(),
            ..Draft::default()
        };
        let d = dialog(
            &agents[..1],
            None,
            draft.clone(),
            Some(&l),
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
            Some(&l),
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
        let d = dialog(&agents, None, draft, Some(&list()), None);
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
        assert_eq!(g.sections.len(), 3);
        assert!(g.sections[0].view.fresh_note.is_some());
        assert!(g.sections[2].view.rows[4].mark.is_some());
        assert_eq!(g.dialogs[2].view.draft.name, "Strong copy");
        assert!(matches!(g.deletes[0].view, DeleteDialog::Refused { .. }));
        assert!(matches!(g.deletes[1].view, DeleteDialog::Confirm { .. }));
        assert!(!g.projects[0].view.expanded);
        assert!(g.projects[1].view.expanded);
    }
}
