//! Named agents (AGT-01…AGT-05; d0086, d0088): a short app-wide list of a name, a tool, a model and a "Use for" note,
//! from which every kind of agent runs. Tickets, defaults and sessions point at an entry's id, so a rename changes
//! nothing.
//!
//! - The shared types, used by every M22 ticket as they are: [`NamedAgent`], [`Role`], [`AgentModel`] and
//!   [`RowSetBy`].
//! - **Role defaults** ([`RoleDefaults`]): app-wide in `settings` (`agents.defaults`), a project's overrides in its own
//!   settings (`agents`), each absent there = "Use app default".
//! - **Rows** ([`AgentRow`]): a ticket's Developer and Validator rows, an epic's Epic lead row, each empty or a named
//!   agent with who set it.
//! - **Resolution** ([`resolve`]), pure: the row's agent, else the project default, else the app default, with
//!   where it came from ([`CameFrom`]).
//! - **At session start** ([`resolve_ticket_role`], [`resolve_project_role`], [`RunsOn`]): every new session
//!   resolves its role right before launch and saves the named agent, the tool and the model as passed on the
//!   session; a resumed conversation runs on what its session stored, never the named agent's current model.
//! - **Locked rows** ([`row_lock`]), pure, from what the store says about the ticket's sessions ([`LockFacts`]).
//! - **The model list** ([`ModelCatalog`]): what picking a model text from a tool's own list means, answered by
//!   Otaq's copies of the tools' lists ([`ModelLists`]). The core never invents model text.
//! - [`NamedAgents`]: the requests' rules (add, edit, duplicate, delete, defaults, rows).
//! - [`starter`]: the starter entry and an entry for a newly connected tool; [`in_use`]: what uses an entry;
//!   [`mark`]: the amber mark of a model the tool refused.

pub mod in_use;
pub mod mark;
pub mod starter;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::agent_tools::Tool;
use crate::agent_tools::models::{ModelKind, ModelLists};
use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError, Refusal};
use crate::domain::{Kind, stored_enum};
use crate::naming;
use crate::session::{SessionAgent, StoredConversation};
use crate::store::named_agents::NewNamedAgent;
use crate::store::projects::Project;
pub use in_use::InUse;

stored_enum! {
    /// The four jobs an agent runs as (AGT-03; d0063, d0068, d0083). Developer and Validator have rows on tickets,
    /// Epic lead on epics; all four have role defaults. Stored as `developer`, `validator`, `lead` and
    /// `project_agent` (the rows' `role` column and the requests' `role`).
    pub enum Role ("agent role") {
        /// The ticket's own agent (the work-ticket session).
        Developer = "developer",
        /// The ticket's validator (d0063).
        Validator = "validator",
        /// An epic's lead (d0083).
        EpicLead = "lead",
        /// A project's Project agent (d0068). Defaults only; no rows.
        ProjectAgent = "project_agent",
    }
}

impl Role {
    /// The role as people read it: "Developer", "Validator", "Epic lead", "Project agent".
    pub fn label(self) -> &'static str {
        match self {
            Self::Developer => "Developer",
            Self::Validator => "Validator",
            Self::EpicLead => "Epic lead",
            Self::ProjectAgent => "Project agent",
        }
    }

    /// Whether a ticket of `kind` has this row: a ticket has Developer and Validator, an epic only Epic lead.
    pub fn is_row_of(self, kind: Kind) -> bool {
        match kind {
            Kind::Ticket => matches!(self, Self::Developer | Self::Validator),
            Kind::Epic => self == Self::EpicLead,
        }
    }
}

/// A named agent's model, by kind (d0088): passed to the tool unchanged, and always one picked from the tool's own
/// list. Stored as its text ([`NamedAgent::model`]); which kind a text is comes from the tool's list
/// ([`ModelCatalog`], [`NamedAgent::model_kind`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum AgentModel {
    /// No model option is passed; the tool uses its own default (shown "tool default").
    ToolDefault,
    /// A follow choice: a tool alias that moves to the newest model ("opus", shown "Opus · always the newest").
    Follow(String),
    /// An exact model id from the tool's list ("claude-opus-5-5").
    Exact(String),
}

impl AgentModel {
    /// What goes to the tool: `None` passes no model option.
    pub fn passed(&self) -> Option<&str> {
        match self {
            Self::ToolDefault => None,
            Self::Follow(id) | Self::Exact(id) => Some(id),
        }
    }
}

/// Who set a row (AGT-04, d0086): *set by you*, or *picked by <agent>: <reason>*. An agent never overwrites a row
/// the user set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum RowSetBy {
    /// The user ("set by you"; with "Let agents pick again").
    User,
    /// An agent's pick: its name at pick time ("Checkout slowness", "Wishlist lead") and its reason ("docs-only
    /// change").
    Agent { picked_by: String, reason: String },
}

/// The amber mark on an entry (AGT-05; written by T-0337): "Claude Code couldn't run `claude-fable-5-1` on
/// SHOP-48". No dismiss; a saved model change clears it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    /// The line as shown.
    pub text: String,
    /// The model that was refused.
    pub model: Option<String>,
    /// The ticket key it happened on.
    pub ticket: Option<String>,
    /// The tool's own message, shown in quotes in the edit dialog.
    pub message: Option<String>,
    pub at: i64,
}

/// One entry of the `named_agents` table (AGT-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedAgent {
    /// Stable: tickets, defaults and sessions point at it.
    pub id: String,
    /// Unique ignoring case ("Strong", "Light", "Claude Code").
    pub name: String,
    pub tool: Tool,
    /// The text passed to the tool as it is; `None` = tool default. See [`Self::model_kind`].
    pub model: Option<String>,
    /// The "Use for" note ("Complex features, tricky bugs").
    pub use_for: String,
    pub mark: Option<Mark>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl NamedAgent {
    /// The model by kind, from the tool's list. A text the list no longer has counts as exact (M23 checks it before
    /// every launch).
    pub fn model_kind(&self, models: &dyn ModelCatalog) -> AgentModel {
        match &self.model {
            None => AgentModel::ToolDefault,
            Some(text) => models
                .pick(self.tool, text)
                .unwrap_or_else(|| AgentModel::Exact(text.clone())),
        }
    }
}

/// The role defaults, app-wide or a project's overrides (AGT-03): each a named agent's id. App-wide, all four are set
/// once the starter entry exists; in a project, `None` = "Use app default".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoleDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epic_lead: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_agent: Option<String>,
}

impl RoleDefaults {
    /// Every role on one entry (the starter's defaults).
    pub fn all_on(id: &str) -> Self {
        Self {
            developer: Some(id.to_owned()),
            validator: Some(id.to_owned()),
            epic_lead: Some(id.to_owned()),
            project_agent: Some(id.to_owned()),
        }
    }

    pub fn get(&self, role: Role) -> Option<&str> {
        self.slot(role).as_deref()
    }

    pub fn set(&mut self, role: Role, agent: Option<String>) {
        *self.slot_mut(role) = agent;
    }

    /// The roles whose default is this entry, in [`Role::ALL`] order.
    pub fn roles_on(&self, id: &str) -> Vec<Role> {
        Role::ALL
            .iter()
            .copied()
            .filter(|role| self.get(*role) == Some(id))
            .collect()
    }

    fn slot(&self, role: Role) -> &Option<String> {
        match role {
            Role::Developer => &self.developer,
            Role::Validator => &self.validator,
            Role::EpicLead => &self.epic_lead,
            Role::ProjectAgent => &self.project_agent,
        }
    }

    fn slot_mut(&mut self, role: Role) -> &mut Option<String> {
        match role {
            Role::Developer => &mut self.developer,
            Role::Validator => &mut self.validator,
            Role::EpicLead => &mut self.epic_lead,
            Role::ProjectAgent => &mut self.project_agent,
        }
    }
}

/// A stored row of a ticket or epic (AGT-04). No stored row = empty ("Project default (Standard)").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRow {
    pub role: Role,
    /// The named agent's id; `None` when it was deleted (only Done and Closed tickets can be left so), which counts
    /// as empty.
    pub agent: Option<String>,
    pub set_by: RowSetBy,
    pub set_at: i64,
}

/// Where a resolved agent came from (AGT-05): what "set by you", "picked by …", "Project default (Strong)" read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum CameFrom {
    /// The row: the user's pick or the latest agent pick.
    Row { set_by: RowSetBy },
    /// The project's override of the role default.
    ProjectDefault,
    /// The app-wide role default.
    AppDefault,
}

/// The named agent a role runs on, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolved {
    pub agent: NamedAgent,
    pub came_from: CameFrom,
}

/// Resolution (AGT-05, d0088), once at session start: the row's agent (the user's pick or the latest agent pick,
/// whichever was written last), else the project default, else the app default. A row or default pointing at an
/// entry that isn't in `agents` (deleted) counts as empty. `None` only while there are no named agents.
pub fn resolve(
    role: Role,
    row: Option<&AgentRow>,
    project: &RoleDefaults,
    app: &RoleDefaults,
    agents: &[NamedAgent],
) -> Option<Resolved> {
    let find = |id: Option<&str>| {
        id.and_then(|id| agents.iter().find(|a| a.id == id))
            .cloned()
    };
    if let Some(row) = row.filter(|row| row.role == role)
        && let Some(agent) = find(row.agent.as_deref())
    {
        return Some(Resolved {
            agent,
            came_from: CameFrom::Row {
                set_by: row.set_by.clone(),
            },
        });
    }
    if let Some(agent) = find(project.get(role)) {
        return Some(Resolved {
            agent,
            came_from: CameFrom::ProjectDefault,
        });
    }
    find(app.get(role)).map(|agent| Resolved {
        agent,
        came_from: CameFrom::AppDefault,
    })
}

/// The role of a ticket's or an epic's new session, resolved from what is stored now (AGT-05): its row, else its
/// project's default, else the app default. `None` only while there are no named agents.
pub async fn resolve_ticket_role(
    board: &Board,
    ticket: &Ticket,
    role: Role,
) -> Result<Option<Resolved>, BoardError> {
    let project = board.project(ticket.key.project.as_str()).await?;
    let store = board.store();
    let rows = store.ticket_agent_rows(&ticket.id).await?;
    Ok(resolve(
        role,
        rows.iter().find(|row| row.role == role),
        &project.agent_defaults(),
        &store.agent_defaults().await?,
        &store.named_agents().await?,
    ))
}

/// A project's role with no row (the Project agent; Delivery's Developer, Q-D), resolved from what is stored now:
/// the project's default, else the app default.
pub async fn resolve_project_role(
    board: &Board,
    project: &Project,
    role: Role,
) -> Result<Option<Resolved>, BoardError> {
    let store = board.store();
    Ok(resolve(
        role,
        None,
        &project.agent_defaults(),
        &store.agent_defaults().await?,
        &store.named_agents().await?,
    ))
}

/// What a session launches on (AGT-05, ADP-04; d0088 "The exact tool, model and named agent are saved on the
/// session"): the tool (`None`: the new-session tool, only without named agents), the model passed to it unchanged
/// (`None` passes no model option) and the named agent saved on the session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunsOn {
    pub tool: Option<Tool>,
    pub model: Option<String>,
    pub agent: Option<SessionAgent>,
}

impl RunsOn {
    /// A new session on the agent its role resolved to; with none (no named agents yet) the new-session tool
    /// with its default model.
    pub fn resolved(resolved: Option<&Resolved>) -> Self {
        resolved.map_or_else(Self::default, |resolved| Self::agent(&resolved.agent))
    }

    /// A new session on `agent` as it is now.
    pub fn agent(agent: &NamedAgent) -> Self {
        Self {
            tool: Some(agent.tool),
            model: agent.model.clone(),
            agent: Some(SessionAgent {
                id: agent.id.clone(),
                name: agent.name.clone(),
            }),
        }
    }

    /// A resumed conversation: the tool, model and named agent its session stored, whatever the named agent is
    /// set to now (the models report: Claude Code doesn't keep the model, so it is passed again).
    pub fn resumed(conversation: &StoredConversation) -> Self {
        Self {
            tool: Some(conversation.tool),
            model: conversation.model.clone(),
            agent: conversation.named_agent.clone(),
        }
    }
}

/// What the store says about a ticket's sessions, for [`row_lock`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LockFacts {
    /// The ticket has a developer session that didn't end in "not started" (its first turn refused on the model,
    /// T-0337): it started.
    pub developer_started: bool,
    /// A validation round of the ticket is running: its validator started and the round hasn't ended.
    pub round_running: bool,
    /// The epic's lead has started (opened, or the epic dragged; d0083).
    pub lead_started: bool,
}

/// Why a row can't change (AGT-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowLock {
    /// The Developer's or the Epic lead's session started: Send back continues the same conversation.
    Started,
    /// A validation round runs; the row can change again after it.
    RoundRunning,
}

impl RowLock {
    /// The lock's tooltip (AGT-04).
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Started => "Locked: Send back continues the same conversation",
            Self::RoundRunning => "Locked until this round ends",
        }
    }

    /// The refusal of a change: "SHOP-19's Developer is locked: Send back continues the same conversation",
    /// "SHOP-29's Validator is locked until this round ends".
    pub fn refusal(self, key: &str, role: Role) -> String {
        match self {
            Self::Started => format!(
                "{key}'s {} is locked: Send back continues the same conversation",
                role.label()
            ),
            Self::RoundRunning => {
                format!("{key}'s {} is locked until this round ends", role.label())
            }
        }
    }
}

/// The lock rule (AGT-04, d0086), one function for the refusals and the rows: Developer once the ticket has started
/// (a developer session that didn't end in "not started"); Validator while a round runs (free again after it ends
/// in a pass, a fail or no result, for the next round); Epic lead once the lead started. A Project agent has no
/// row.
pub fn row_lock(role: Role, facts: LockFacts) -> Option<RowLock> {
    match role {
        Role::Developer => facts.developer_started.then_some(RowLock::Started),
        Role::Validator => facts.round_running.then_some(RowLock::RoundRunning),
        Role::EpicLead => facts.lead_started.then_some(RowLock::Started),
        Role::ProjectAgent => None,
    }
}

/// A tool's own model list, as the rules ask it (AGT-02). Only what the list contains can be picked (d0086). The
/// service answers it from its copies of the tools' lists ([`ModelLists`]); tests from a [`FixedModels`].
pub trait ModelCatalog: Send + Sync {
    /// What picking `text` from `tool`'s list means (Claude Code's `default` is [`AgentModel::ToolDefault`], its
    /// aliases [`AgentModel::Follow`]), or `None` when the list doesn't have it.
    fn pick(&self, tool: Tool, text: &str) -> Option<AgentModel>;
}

impl ModelCatalog for ModelLists {
    /// From the copy of `tool`'s list: nothing can be picked before it was first read.
    fn pick(&self, tool: Tool, text: &str) -> Option<AgentModel> {
        let copy = self.copy(tool)?;
        let entry = copy.list.entry(text)?;
        Some(match entry.kind {
            ModelKind::ToolDefault => AgentModel::ToolDefault,
            ModelKind::Follow => AgentModel::Follow(entry.id.clone()),
            ModelKind::Exact => AgentModel::Exact(entry.id.clone()),
        })
    }
}

/// A fixed list, the same for every tool, for tests. [`Self::sample`] is Claude Code's list as its `initialize`
/// request gives it: `default` (tool default) and the aliases `opus`, `sonnet`, `haiku`, `fable`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedModels {
    entries: Vec<AgentModel>,
}

impl FixedModels {
    pub fn new(entries: Vec<AgentModel>) -> Self {
        Self { entries }
    }

    pub fn sample() -> Self {
        let mut entries = vec![AgentModel::ToolDefault];
        entries.extend(
            ["opus", "sonnet", "haiku", "fable"].map(|alias| AgentModel::Follow(alias.to_owned())),
        );
        Self { entries }
    }
}

impl ModelCatalog for FixedModels {
    fn pick(&self, _tool: Tool, text: &str) -> Option<AgentModel> {
        self.entries
            .iter()
            .find(|entry| match entry {
                AgentModel::ToolDefault => text == "default",
                AgentModel::Follow(id) | AgentModel::Exact(id) => id == text,
            })
            .cloned()
    }
}

/// An entry's fields as the Add and Edit dialogs send them (`agents.add`, `agents.edit`), and as Duplicate's draft
/// ("<name> copy", not saved). `model` is a text from the tool's list (`"default"` or empty = tool default).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentFields {
    pub name: String,
    pub tool: Tool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub use_for: String,
}

/// The answer to `agents.list`: every entry (oldest first, the starter entry first) and the app-wide defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentList {
    pub agents: Vec<NamedAgent>,
    pub defaults: RoleDefaults,
}

/// What `agents.delete` did: deleted, or refused because the entry is in use (the parts for the dialog, `O7UTA`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    InUse(InUse),
}

/// The named agents' rules, for the service's requests. Cheap to clone.
#[derive(Clone)]
pub struct NamedAgents {
    board: Board,
    models: Arc<dyn ModelCatalog>,
}

impl NamedAgents {
    pub fn new(board: Board, models: Arc<dyn ModelCatalog>) -> Self {
        Self { board, models }
    }

    /// The model list the rules check against.
    pub fn models(&self) -> &dyn ModelCatalog {
        self.models.as_ref()
    }

    pub async fn list(&self) -> Result<AgentList, BoardError> {
        let store = self.board.store();
        Ok(AgentList {
            agents: store.named_agents().await?,
            defaults: store.agent_defaults().await?,
        })
    }

    pub async fn agent(&self, id: &str) -> Result<NamedAgent, BoardError> {
        self.board
            .store()
            .named_agent(id)
            .await?
            .ok_or_else(|| BoardError::NotFound(format!("no agent {id}")))
    }

    /// Adds an entry (AGT-02). The name must be new ignoring case; the model must be in the tool's list, or empty.
    pub async fn add(&self, fields: &AgentFields) -> Result<NamedAgent, BoardError> {
        let new = self.checked(fields, None).await?;
        let mut tx = self.board.store().begin_write().await?;
        let id = crate::store::Store::insert_named_agent_in(&mut tx, &new).await?;
        tx.commit().await?;
        self.agent(&id).await
    }

    /// Changes an entry (a rename, a new note, a model change). A saved model change clears the mark (AGT-05).
    /// Started sessions keep what they launched with; only unstarted tickets see the change (resolution at start).
    pub async fn edit(&self, id: &str, fields: &AgentFields) -> Result<NamedAgent, BoardError> {
        let before = self.agent(id).await?;
        let new = self.checked(fields, Some(id)).await?;
        let model_changed = before.model != new.model || before.tool.as_str() != new.tool;
        self.board
            .store()
            .update_named_agent(id, &new, model_changed)
            .await?;
        self.agent(id).await
    }

    /// Duplicate's draft (AGT-02): the entry's fields named "<name> copy". Not saved; the dialog saves it with
    /// [`Self::add`].
    pub async fn duplicate(&self, id: &str) -> Result<AgentFields, BoardError> {
        let agent = self.agent(id).await?;
        Ok(AgentFields {
            name: format!("{} copy", agent.name),
            tool: agent.tool,
            model: agent.model,
            use_for: agent.use_for,
        })
    }

    /// Deletes an entry that nothing uses (AGT-02). Refused while in use ([`in_use`]) and for the last entry. Done
    /// and Closed rows pointing at it become empty; their sessions keep its name.
    pub async fn delete(&self, id: &str) -> Result<DeleteOutcome, BoardError> {
        let agent = self.agent(id).await?;
        let store = self.board.store();
        if store.named_agent_count().await? <= 1 {
            return Err(BoardError::refused(
                Refusal::LastAgent,
                format!("{} needs at least one agent", naming::PRODUCT_NAME),
            ));
        }
        let uses = in_use::in_use(store, &agent).await?;
        if !uses.is_empty() {
            return Ok(DeleteOutcome::InUse(uses));
        }
        store.delete_named_agent(id).await?;
        Ok(DeleteOutcome::Deleted)
    }

    /// What uses an entry, before Delete asks (T-0340): empty = the plain confirmation. Refused for the last entry,
    /// as [`Self::delete`] is.
    pub async fn in_use(&self, id: &str) -> Result<InUse, BoardError> {
        let agent = self.agent(id).await?;
        let store = self.board.store();
        if store.named_agent_count().await? <= 1 {
            return Err(BoardError::refused(
                Refusal::LastAgent,
                format!("{} needs at least one agent", naming::PRODUCT_NAME),
            ));
        }
        Ok(in_use::in_use(store, &agent).await?)
    }

    /// A project's role default overrides (AGT-03); each `None` = "Use app default".
    pub async fn project_defaults(&self, project: &str) -> Result<RoleDefaults, BoardError> {
        Ok(self.board.project(project).await?.agent_defaults())
    }

    /// Sets an app-wide role default (AGT-03).
    pub async fn set_default(&self, role: Role, agent: &str) -> Result<RoleDefaults, BoardError> {
        self.agent(agent).await?;
        let store = self.board.store();
        let mut defaults = store.agent_defaults().await?;
        defaults.set(role, Some(agent.to_owned()));
        store.set_agent_defaults(&defaults).await?;
        Ok(defaults)
    }

    /// Sets or clears (`None` = "Use app default") a project's override of a role default (AGT-03).
    pub async fn set_project_default(
        &self,
        project: &str,
        role: Role,
        agent: Option<&str>,
    ) -> Result<RoleDefaults, BoardError> {
        if let Some(agent) = agent {
            self.agent(agent).await?;
        }
        let project = self.board.project(project).await?;
        let mut defaults = project.agent_defaults();
        defaults.set(role, agent.map(str::to_owned));
        self.board
            .store()
            .set_project_agent_defaults(&project.id, &defaults)
            .await?;
        Ok(defaults)
    }

    /// The user sets a ticket's or an epic's row (`ticket.agent_row`, AGT-04): *set by you*, or with `None` "Let
    /// agents pick again" (the row becomes empty = the project default). Refused for a row the ticket doesn't have
    /// and for a locked row.
    pub async fn set_row_by_user(
        &self,
        key: &str,
        role: Role,
        agent: Option<&str>,
    ) -> Result<(), BoardError> {
        let ticket = self.board.ticket(key).await?;
        let key = ticket.key.to_string();
        if !role.is_row_of(ticket.kind) {
            let message = match ticket.kind {
                Kind::Epic => format!(
                    "{key} is an epic: it has an Epic lead row, not a {} row",
                    role.label()
                ),
                Kind::Ticket => format!(
                    "{key} has no {} row: tickets have Developer and Validator rows",
                    role.label()
                ),
            };
            return Err(BoardError::refused(Refusal::NoSuchRow, message));
        }
        if let Some(lock) = row_lock(role, self.lock_facts(&ticket.id).await?) {
            return Err(BoardError::refused(
                Refusal::RowLocked,
                lock.refusal(&key, role),
            ));
        }
        if let Some(agent) = agent {
            self.agent(agent).await?;
        }
        self.board
            .store()
            .set_ticket_agent_row(&ticket.id, role, agent.map(|id| (id, &RowSetBy::User)))
            .await?;
        Ok(())
    }

    /// What the lock rule reads for a ticket or an epic.
    pub async fn lock_facts(&self, ticket_id: &str) -> Result<LockFacts, BoardError> {
        let store = self.board.store();
        Ok(LockFacts {
            developer_started: store.developer_started(ticket_id).await?,
            round_running: store.validation_running(ticket_id).await?,
            lead_started: store.lead_started(ticket_id).await?,
        })
    }

    /// The agent a ticket's or an epic's role resolves to now ([`resolve_ticket_role`]).
    pub async fn resolve_for_ticket(
        &self,
        key: &str,
        role: Role,
    ) -> Result<Option<Resolved>, BoardError> {
        let ticket = self.board.ticket(key).await?;
        resolve_ticket_role(&self.board, &ticket, role).await
    }

    /// The agent a project's role (the Project agent) resolves to now: the project default, else the app default.
    pub async fn resolve_for_project(
        &self,
        project: &str,
        role: Role,
    ) -> Result<Option<Resolved>, BoardError> {
        let project = self.board.project(project).await?;
        resolve_project_role(&self.board, &project, role).await
    }

    /// The fields as stored, after the rules: a name (trimmed, not empty, new ignoring case among the entries other
    /// than `editing`) and a model from the tool's list.
    async fn checked(
        &self,
        fields: &AgentFields,
        editing: Option<&str>,
    ) -> Result<NewNamedAgent, BoardError> {
        let name = fields.name.trim();
        if name.is_empty() {
            return Err(BoardError::refused(
                Refusal::EmptyField,
                "An agent needs a name",
            ));
        }
        let folded = name.to_lowercase();
        let taken = self
            .board
            .store()
            .named_agent_tools()
            .await?
            .into_iter()
            .any(|(id, other, _)| Some(id.as_str()) != editing && other.to_lowercase() == folded);
        if taken {
            return Err(BoardError::refused(
                Refusal::NameTaken,
                format!("Another agent is already called {name}"),
            ));
        }
        let model = match fields.model.as_deref().map(str::trim) {
            None | Some("") => AgentModel::ToolDefault,
            Some(text) => self.models.pick(fields.tool, text).ok_or_else(|| {
                BoardError::refused(
                    Refusal::ModelNotListed,
                    format!("{text} isn't in the tool's model list"),
                )
            })?,
        };
        Ok(NewNamedAgent {
            name: name.to_owned(),
            tool: fields.tool.as_str().to_owned(),
            model: model.passed().map(str::to_owned),
            use_for: fields.use_for.trim().to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(id: &str, name: &str) -> NamedAgent {
        NamedAgent {
            id: id.into(),
            name: name.into(),
            tool: Tool::ALL[0],
            model: None,
            use_for: String::new(),
            mark: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn row(role: Role, agent: Option<&str>, set_by: RowSetBy) -> AgentRow {
        AgentRow {
            role,
            agent: agent.map(str::to_owned),
            set_by,
            set_at: 0,
        }
    }

    #[test]
    fn roles_are_stored_as_the_rows_and_requests_name_them() {
        let stored: Vec<&str> = Role::ALL.iter().map(|r| r.as_str()).collect();
        assert_eq!(stored, ["developer", "validator", "lead", "project_agent"]);
        assert!(Role::Developer.is_row_of(Kind::Ticket));
        assert!(Role::Validator.is_row_of(Kind::Ticket));
        assert!(!Role::EpicLead.is_row_of(Kind::Ticket));
        assert!(Role::EpicLead.is_row_of(Kind::Epic));
        assert!(!Role::Developer.is_row_of(Kind::Epic));
        assert!(!Role::ProjectAgent.is_row_of(Kind::Ticket));
    }

    #[test]
    fn resolution_goes_row_then_project_then_app() {
        let agents = [
            agent("cc", "Claude Code"),
            agent("strong", "Strong"),
            agent("light", "Light"),
        ];
        let app = RoleDefaults::all_on("cc");
        let mut project = RoleDefaults::default();
        project.set(Role::Developer, Some("strong".into()));
        let picked = RowSetBy::Agent {
            picked_by: "Checkout slowness".into(),
            reason: "docs-only change".into(),
        };

        for role in Role::ALL.iter().copied() {
            let by_app = resolve(role, None, &RoleDefaults::default(), &app, &agents).unwrap();
            assert_eq!(
                (by_app.agent.name.as_str(), by_app.came_from),
                ("Claude Code", CameFrom::AppDefault)
            );
        }
        let by_project = resolve(Role::Developer, None, &project, &app, &agents).unwrap();
        assert_eq!(by_project.agent.name, "Strong");
        assert_eq!(by_project.came_from, CameFrom::ProjectDefault);
        let validator = resolve(Role::Validator, None, &project, &app, &agents).unwrap();
        assert_eq!(validator.came_from, CameFrom::AppDefault);

        let light = row(Role::Developer, Some("light"), picked.clone());
        let by_row = resolve(Role::Developer, Some(&light), &project, &app, &agents).unwrap();
        assert_eq!(by_row.agent.name, "Light");
        assert_eq!(by_row.came_from, CameFrom::Row { set_by: picked });
        let lead = row(Role::EpicLead, Some("strong"), RowSetBy::User);
        let by_lead = resolve(Role::EpicLead, Some(&lead), &project, &app, &agents).unwrap();
        assert_eq!(
            by_lead.came_from,
            CameFrom::Row {
                set_by: RowSetBy::User
            }
        );
        // Another role's row doesn't apply.
        let other = resolve(Role::Validator, Some(&light), &project, &app, &agents).unwrap();
        assert_eq!(other.came_from, CameFrom::AppDefault);
    }

    #[test]
    fn a_row_on_a_deleted_agent_counts_as_empty() {
        let agents = [agent("cc", "Claude Code"), agent("strong", "Strong")];
        let app = RoleDefaults::all_on("cc");
        let mut project = RoleDefaults::default();
        project.set(Role::Developer, Some("strong".into()));
        for gone in [
            row(Role::Developer, None, RowSetBy::User),
            row(Role::Developer, Some("light"), RowSetBy::User),
        ] {
            let resolved = resolve(Role::Developer, Some(&gone), &project, &app, &agents).unwrap();
            assert_eq!(resolved.came_from, CameFrom::ProjectDefault);
        }
        let mut stale = RoleDefaults::default();
        stale.set(Role::Developer, Some("light".into()));
        let resolved = resolve(Role::Developer, None, &stale, &app, &agents).unwrap();
        assert_eq!(resolved.came_from, CameFrom::AppDefault);
        assert_eq!(resolve(Role::Developer, None, &project, &app, &[]), None);
    }

    #[test]
    fn rows_lock_by_role() {
        let none = LockFacts::default();
        for role in Role::ALL.iter().copied() {
            assert_eq!(row_lock(role, none), None);
        }
        let started = LockFacts {
            developer_started: true,
            ..none
        };
        assert_eq!(row_lock(Role::Developer, started), Some(RowLock::Started));
        assert_eq!(row_lock(Role::Validator, started), None);
        let round = LockFacts {
            round_running: true,
            ..none
        };
        assert_eq!(
            row_lock(Role::Validator, round),
            Some(RowLock::RoundRunning)
        );
        let lead = LockFacts {
            lead_started: true,
            ..none
        };
        assert_eq!(row_lock(Role::EpicLead, lead), Some(RowLock::Started));
        assert_eq!(
            RowLock::Started.refusal("SHOP-19", Role::Developer),
            "SHOP-19's Developer is locked: Send back continues the same conversation"
        );
        assert_eq!(
            RowLock::RoundRunning.refusal("SHOP-29", Role::Validator),
            "SHOP-29's Validator is locked until this round ends"
        );
    }

    #[test]
    fn the_fixed_list_knows_the_tool_default_and_the_aliases() {
        let models = FixedModels::sample();
        let tool = Tool::ALL[0];
        assert_eq!(models.pick(tool, "default"), Some(AgentModel::ToolDefault));
        assert_eq!(
            models.pick(tool, "opus"),
            Some(AgentModel::Follow("opus".into()))
        );
        assert_eq!(models.pick(tool, "claude-opus-5-5"), None);
        let mut light = agent("light", "Light");
        light.model = Some("haiku".into());
        assert_eq!(
            light.model_kind(&models),
            AgentModel::Follow("haiku".into())
        );
        light.model = Some("claude-haiku-4-5".into());
        assert_eq!(
            light.model_kind(&models),
            AgentModel::Exact("claude-haiku-4-5".into())
        );
        assert_eq!(AgentModel::ToolDefault.passed(), None);
    }
}
