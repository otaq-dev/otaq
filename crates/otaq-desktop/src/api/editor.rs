//! `editor.*`: the ticket editor and the Ticket tab (UI-05, TKT-03, TKT-05, TKT-09, DEP-01, DEP-02, DEP-04, EPC-07,
//! SES-08; d0019, d0051, d0059, d0065, d0067, d0048; *08 · Ticket editor*). One view per ticket (or new ticket):
//! the form's labels, hints and first values, which fields it shows (`editor_fields.rs`), and for an existing
//! ticket its read mode with the agent-edit markers (`agent_edits.rs`). The page keeps what is typed and sends the
//! whole form back on save; the rules (title required, only what changed, priority only once picked, deps linked
//! after create) are `editor.rs`'s, and the service's refusals come back worded for the field they belong to.

use std::time::{SystemTime, UNIX_EPOCH};

use otaq_core::board::agent_edits::AgentEdits;
use otaq_core::board::settings::GlobalSettings;
use otaq_core::board::snapshot::ProjectInfo;
use otaq_core::board::tickets::{Base, Field, Ticket};
use otaq_core::domain::{Actor, Kind, Priority, TicketKey};
use otaq_core::agent_tools::Tool;
use otaq_core::agent_tools::models::ModelList;
use otaq_core::named_agents::rows::TicketAgents;
use otaq_core::protocol::{
    AgentRowParams, DepParams, KeyParams, ModelsParams, ProjectParams, Reply, Request, SessionListParams,
    TicketAgentsParams, TicketUpdateParams,
};
use otaq_core::session::{SessionInfo, SessionKind};
use otaq_view::icons::IconName;
use otaq_view::view_models::agent_activity;
use otaq_view::view_models::agent_rows::{self, Block};
use otaq_view::view_models::tool_problems::Tools;
use otaq_view::view_models::named_agents::RoleId;
use otaq_view::view_models::agent_edits::{changed, compare, origin_line};
use otaq_view::view_models::editor::{EditorModel, TITLE_REQUIRED};
use otaq_view::view_models::editor_fields::{
    BaseField, base_choices, picker_rows, refusal_icon, refusal_title, visibility,
};
use otaq_view::view_models::menu::MenuItem;
use otaq_view::view_models::priority::{self, FORM_HINT, PriorityView};
use otaq_view::view_models::status::PillView;
use otaq_view::view_models::validation_panel;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "editor.open" => reply(&open(app, parse(params)?).await?),
        "editor.save" => reply(&save(app, parse(params)?).await?),
        "editor.picker" => reply(&picker(app, parse(params)?).await?),
        "editor.link" => reply(&link(app, parse(params)?).await?),
        "editor.branches" => reply(&branches(app, parse(params)?).await?),
        "editor.compare" => reply(&compare_view(app, parse(params)?).await?),
        "editor.agent_row" => reply(&agent_row(app, parse(params)?).await?),
        "editor.source_mode" => {
            set_source_mode(app, parse(params)?).await?;
            reply(&Value::Null)
        }
        _ => Err(unknown(method)),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

fn unexpected(what: &str, other: &Reply) -> RequestError {
    RequestError::plain(format!("unexpected answer to {what}: {other:?}"))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn key_of(text: &str) -> Result<TicketKey, RequestError> {
    text.parse()
        .map_err(|_| RequestError::plain(format!("not a ticket key: {text}")))
}

// ---------------------------------------------------------------------------------------------------------------
// What the page sends

/// Which editor: a new ticket in a project (inside an epic when given), or an existing ticket.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[ts(rename = "EditorTarget")]
pub enum Target {
    Create {
        project: Option<String>,
        epic: Option<String>,
    },
    Edit {
        key: String,
    },
}

/// The form as typed: what the page keeps and sends back on save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "EditorForm")]
pub struct Form {
    pub title: String,
    pub body: String,
    /// 1–4 once picked; `null`: Normal for a new ticket, the epic's for a child.
    pub priority: Option<u8>,
    pub epic_kind: bool,
    /// Create only: the epic the new ticket goes into.
    pub epic: Option<String>,
    /// Create only: linked right after the ticket is created (an existing ticket links on each pick).
    pub deps: Vec<String>,
    pub base_branch: Option<String>,
    pub external_id: String,
    /// Create only: the agent rows picked in the form (*set by you*), set right after the ticket is created (an
    /// existing ticket sets each pick with `editor.agent_row`).
    pub agents: Vec<AgentPick>,
}

/// A row picked in the ⌘N form: the role and the named agent's id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "EditorAgentPick")]
pub struct AgentPick {
    pub role: RoleId,
    pub agent: String,
}

// ---------------------------------------------------------------------------------------------------------------
// What the page gets

/// A dependency chip: the key with its live status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EditorDepChip")]
pub struct DepChip {
    pub key: String,
    pub pill: PillView,
}

/// The "Start from and merge into" field: a dropdown, fixed after start, or the epic's branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "EditorBaseField")]
pub enum BaseView {
    Editable,
    Locked { text: String },
    FromEpic { text: String },
}

/// The form's words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EditorTexts")]
pub struct Texts {
    pub title: &'static str,
    pub title_placeholder: &'static str,
    pub title_required: &'static str,
    pub priority: &'static str,
    pub priority_hint: &'static str,
    pub kind: &'static str,
    pub kind_ticket: &'static str,
    pub kind_epic: &'static str,
    pub epic: &'static str,
    pub epic_hint: &'static str,
    pub epic_none: &'static str,
    pub body: &'static str,
    pub body_hint: &'static str,
    pub body_placeholder: &'static str,
    pub deps: &'static str,
    pub deps_hint: &'static str,
    pub deps_placeholder: &'static str,
    pub deps_none: &'static str,
    pub base: &'static str,
    pub base_hint: &'static str,
    pub external_id: &'static str,
    pub external_id_hint: &'static str,
    pub external_id_placeholder: &'static str,
    /// "⌘↵ create" / "save", "Esc discard" / "close".
    pub save_word: &'static str,
    pub discard_word: &'static str,
    pub close_word: &'static str,
    pub cancel: &'static str,
    pub close: &'static str,
    pub primary: &'static str,
    pub edit: &'static str,
    pub back: &'static str,
    pub in_word: &'static str,
    /// Quick-create: "↵ create · ⇧↵ full form".
    pub quick_create: &'static str,
    pub quick_full: &'static str,
    pub open_terminal: &'static str,
    pub marker: &'static str,
    pub compare: &'static str,
    pub before: &'static str,
    pub after: &'static str,
    pub nothing_written: &'static str,
    /// The "What and why" label row's icons (d0078 D7): their tooltips, and "Copied" after a copy.
    pub copy_markdown: &'static str,
    pub copied: &'static str,
    pub expand: &'static str,
    /// The full-window view (d0078 D11): its Collapse tooltip, the header's "Editing", and the footer's "Esc collapse
    /// (after closing a popover)".
    pub collapse: &'static str,
    pub editing: &'static str,
    pub collapse_word: &'static str,
    /// Compare's Rendered / Markdown toggle on the description (d0078): its name and its two choices.
    pub compare_view: &'static str,
    pub rendered: &'static str,
    pub markdown: &'static str,
}

/// The Ticket tab's read mode (*Ticket tab · read mode, with agent edits*).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "EditorReadView")]
pub struct ReadView {
    /// "Created by you  ·  edited by the agent · 4m ago".
    pub origin: String,
    pub origin_agent: bool,
    pub title: String,
    /// "What and why" exactly as stored: the page renders it (read-only editor) and Copy as Markdown copies it.
    pub body_markdown: String,
    /// Nothing but whitespace: the page shows `nothing_written` instead.
    pub body_empty: bool,
    pub priority: PriorityView,
    /// The Priority dropdown: `panel.priority` items.
    pub priority_menu: Vec<MenuItem>,
    /// "follows SHOP-40" for an epic child without its own priority.
    pub follows: Option<String>,
    pub deps: Vec<DepChip>,
    /// "main", or "the epic’s branch (SHOP-40)".
    pub base: String,
    pub external_id: Option<String>,
    /// The fields an agent changed since the user last looked ("Updated by agent").
    pub marked: Vec<EditorField>,
}

/// A field with an agent-edit marker, and the Compare filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EditorField {
    Title,
    Body,
    Priority,
    BaseBranch,
    ExternalId,
}

impl EditorField {
    fn of(field: Field) -> Self {
        match field {
            Field::Title => Self::Title,
            Field::Body => Self::Body,
            Field::Priority => Self::Priority,
            Field::BaseBranch => Self::BaseBranch,
            Field::ExternalId => Self::ExternalId,
        }
    }

    fn core(self) -> Field {
        match self {
            Self::Title => Field::Title,
            Self::Body => Field::Body,
            Self::Priority => Field::Priority,
            Self::BaseBranch => Field::BaseBranch,
            Self::ExternalId => Field::ExternalId,
        }
    }
}

/// One priority in the form's segmented control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EditorPriorityOption")]
pub struct PriorityOption {
    pub number: u8,
    pub view: PriorityView,
}

/// The editor for one ticket or a new one.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "EditorView")]
pub struct View {
    pub create: bool,
    /// The ticket's key; `null` while creating.
    pub key: Option<String>,
    pub project: String,
    pub project_name: String,
    /// "New ticket", "Edit SHOP-12".
    pub heading: String,
    pub texts: Texts,
    pub form: Form,
    pub priorities: Vec<PriorityOption>,
    /// The priority shown while none is picked: Normal, or the epic's.
    pub shown_priority: u8,
    pub follows: Option<String>,
    /// Create only: "No epic" (id "") and the open epics (a new ticket can go into one; fixed once created).
    pub epic_menu: Vec<MenuItem>,
    pub base: BaseView,
    /// Standalone tickets and epics; not epic children.
    pub external_id_shown: bool,
    /// Epics have no Depends on of their own in the form (*Full form · filled*).
    pub deps_shown: bool,
    /// The chips of an existing ticket's links.
    pub deps: Vec<DepChip>,
    /// "SHOP-19’s agent will be told what changed." while its agent runs (SES-08).
    pub save_hint: Option<String>,
    /// While it validates (VAL-08): "SHOP-29’s validation will be cancelled and its agent told what changed."
    pub save_warning: Option<String>,
    /// While it validates, beside the description once it changed: "unsaved · saving cancels the validation".
    pub unsaved_warning: Option<&'static str>,
    pub read: Option<ReadView>,
    /// The description opens as its raw Markdown source, the mode last chosen with the "Markdown" toggle (d0078 D9).
    pub source_mode: bool,
    /// The "Agents" block (AGT-04): a ticket's Developer and Validator rows, an epic's Epic lead row.
    pub agents: Option<Block>,
    /// Create only: the block once Type is Epic (one Epic lead row).
    pub epic_agents: Option<Block>,
}

fn texts(create: bool) -> Texts {
    Texts {
        title: "Title",
        title_placeholder: "Short, imperative title",
        title_required: TITLE_REQUIRED,
        priority: "Priority",
        priority_hint: FORM_HINT,
        kind: "Type",
        kind_ticket: "Ticket",
        kind_epic: "Epic",
        epic: "Epic",
        epic_hint: "optional \u{b7} fixed once created (it sets the key)",
        epic_none: "No epic",
        body: "What and why",
        body_hint: "Markdown \u{b7} paste keeps formatting",
        body_placeholder: "Context, constraints, links\u{2026}",
        deps: "Depends on",
        deps_hint: "same project only",
        deps_placeholder: "Search tickets by # or title",
        deps_none: "None",
        base: "Start from and merge into",
        base_hint: "project default \u{b7} local and remote branches \u{b7} locked once the ticket starts",
        external_id: "External ID",
        external_id_hint: "optional \u{b7} standalone tickets only \u{b7} used in the branch and PR title",
        external_id_placeholder: "e.g. PAY-123",
        save_word: if create { "create" } else { "save" },
        discard_word: "discard",
        close_word: "close",
        cancel: "Cancel",
        close: "Close",
        primary: if create { "Create ticket" } else { "Save" },
        edit: "Edit",
        back: "Back",
        in_word: "in",
        quick_create: "create",
        quick_full: "full form",
        open_terminal: "Open terminal",
        marker: "Updated by agent",
        compare: "Compare",
        before: "BEFORE",
        after: "AFTER",
        nothing_written: "Nothing written yet.",
        copy_markdown: "Copy as Markdown",
        copied: "Copied",
        expand: "Expand",
        collapse: "Collapse",
        editing: "Editing",
        collapse_word: "collapse (after closing a popover)",
        compare_view: "Show the description",
        rendered: "Rendered",
        markdown: "Markdown",
    }
}

fn priorities() -> Vec<PriorityOption> {
    Priority::ALL
        .into_iter()
        .map(|p| PriorityOption {
            number: p.number(),
            view: priority::view(p),
        })
        .collect()
}

fn chips(keys: &[TicketKey], tickets: &[Ticket]) -> Vec<DepChip> {
    keys.iter()
        .map(|k| DepChip {
            key: k.to_string(),
            pill: PillView::of(
                tickets
                    .iter()
                    .find(|t| &t.key == k)
                    .map_or(otaq_core::domain::Status::Backlog, |t| t.status),
            ),
        })
        .collect()
}

fn base_view(field: BaseField) -> BaseView {
    match field {
        BaseField::Editable => BaseView::Editable,
        BaseField::Locked(text) => BaseView::Locked { text },
        BaseField::FromEpic(epic) => BaseView::FromEpic {
            text: format!("From the epic\u{2019}s branch ({epic})"),
        },
    }
}

fn form_of(model: &EditorModel) -> Form {
    Form {
        title: model.title.clone(),
        body: model.body.clone(),
        priority: model.priority.map(Priority::number),
        epic_kind: model.kind == Kind::Epic,
        epic: model.epic.as_ref().map(ToString::to_string),
        deps: model.deps.iter().map(ToString::to_string).collect(),
        base_branch: model.base_branch.clone(),
        external_id: model.external_id.clone(),
        agents: Vec::new(),
    }
}

fn epic_priority(epic: Option<&TicketKey>, tickets: &[Ticket]) -> Option<Priority> {
    let epic = epic?;
    tickets.iter().find(|t| &t.key == epic).map(|t| t.priority)
}

/// The editor for a new ticket in `project` (inside `epic` when given).
pub fn create_view(project: &ProjectInfo, epic: Option<TicketKey>, tickets: &[Ticket]) -> View {
    let mut model = EditorModel::create(&project.key, "", &project.base_branch);
    model.epic = epic;
    let shown = model.shown_priority(epic_priority(model.epic.as_ref(), tickets));
    let fields = visibility(model.kind, model.epic.as_ref(), false);
    View {
        create: true,
        key: None,
        project: project.key.clone(),
        project_name: project.name.clone(),
        heading: "New ticket".to_owned(),
        texts: texts(true),
        form: form_of(&model),
        priorities: priorities(),
        shown_priority: shown.number(),
        follows: model.follows_epic().map(|e| format!("follows {e}")),
        epic_menu: std::iter::once(MenuItem::new("", "No epic"))
            .chain(
                otaq_view::view_models::epic_actions::epic_choices(tickets)
                    .into_iter()
                    .map(|c| {
                        MenuItem::new(c.key.to_string(), format!("{} {}", c.key, c.title))
                            .icon(IconName::Layers)
                            .hint(c.progress)
                    }),
            )
            .collect(),
        base: base_view(fields.base),
        external_id_shown: fields.external_id,
        deps_shown: true,
        deps: Vec::new(),
        save_hint: None,
        save_warning: None,
        unsaved_warning: None,
        read: None,
        source_mode: false,
        agents: None,
        epic_agents: None,
    }
}

/// The editor of an existing ticket, with its read mode.
pub fn edit_view(
    ticket: &Ticket,
    project_name: &str,
    tickets: &[Ticket],
    edits: &AgentEdits,
    live_agent: bool,
    now: i64,
) -> View {
    let model = EditorModel::edit(ticket);
    let shown = model.shown_priority(epic_priority(model.epic.as_ref(), tickets));
    let fields = visibility(model.kind, model.epic.as_ref(), model.base_locked);
    let follows = model.follows_epic().map(|e| format!("follows {e}"));
    let deps = chips(&model.deps, tickets);
    let marked = [
        Field::Title,
        Field::Body,
        Field::Priority,
        Field::BaseBranch,
        Field::ExternalId,
    ]
    .into_iter()
    .filter(|&f| changed(edits, f))
    .map(EditorField::of)
    .collect();
    View {
        create: false,
        key: Some(ticket.key.to_string()),
        project: ticket.key.project.to_string(),
        project_name: project_name.to_owned(),
        heading: format!("Edit {}", ticket.key),
        texts: texts(false),
        form: form_of(&model),
        priorities: priorities(),
        shown_priority: shown.number(),
        follows: follows.clone(),
        epic_menu: Vec::new(),
        base: base_view(fields.base),
        external_id_shown: fields.external_id,
        deps_shown: ticket.kind == Kind::Ticket,
        deps: deps.clone(),
        save_hint: agent_activity::save_hint(ticket, live_agent),
        save_warning: validation_panel::editor_warning(ticket),
        unsaved_warning: validation_panel::editor_warning(ticket)
            .map(|_| validation_panel::UNSAVED_CANCELS),
        read: Some(ReadView {
            origin: origin_line(ticket, edits, now),
            origin_agent: matches!(ticket.created_by, Actor::Agent(_)),
            title: ticket.title.clone(),
            body_markdown: ticket.body.clone(),
            body_empty: ticket.body.trim().is_empty(),
            priority: priority::view(shown),
            priority_menu: priority::menu_items(shown),
            follows,
            deps,
            base: match &ticket.base {
                Base::Epic(epic) => format!("the epic\u{2019}s branch ({epic})"),
                Base::Branch(b) => b.clone(),
            },
            external_id: ticket.external_id.clone().filter(|_| ticket.epic.is_none()),
            marked,
        }),
        source_mode: false,
        agents: None,
        epic_agents: None,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Service helpers

async fn ticket(app: &App, key: &str) -> Result<Option<Ticket>, RequestError> {
    match app
        .service
        .send(Request::TicketGet(KeyParams { key: key.to_owned() }))
        .await
    {
        Ok(Reply::Ticket(t)) => Ok(Some(*t)),
        Ok(other) => Err(unexpected("ticket.get", &other)),
        Err(e) if e.not_found => Ok(None),
        Err(e) => Err(e),
    }
}

async fn found(app: &App, key: &str) -> Result<Ticket, RequestError> {
    ticket(app, key)
        .await?
        .ok_or_else(|| RequestError::plain(format!("{key} is gone.")))
}

async fn tickets(app: &App, project: &str) -> Result<Vec<Ticket>, RequestError> {
    match app
        .service
        .send(Request::TicketList(ProjectParams {
            project: project.to_owned(),
        }))
        .await?
    {
        Reply::Tickets(list) => Ok(list),
        other => Err(unexpected("ticket.list", &other)),
    }
}

async fn projects(app: &App) -> Result<Vec<ProjectInfo>, RequestError> {
    match app.service.send(Request::ProjectList).await? {
        Reply::Projects(list) => Ok(list),
        other => Err(unexpected("project.list", &other)),
    }
}

async fn project(app: &App, key: &str) -> Result<ProjectInfo, RequestError> {
    projects(app)
        .await?
        .into_iter()
        .find(|p| p.key == key)
        .ok_or_else(|| RequestError::plain(format!("project {key} is gone.")))
}

async fn live_agent(app: &App, key: &str) -> bool {
    match app
        .service
        .send(Request::SessionList(SessionListParams {
            key: Some(key.to_owned()),
        }))
        .await
    {
        Ok(Reply::Sessions(list)) => running(&list),
        _ => false,
    }
}

fn running(sessions: &[SessionInfo]) -> bool {
    sessions
        .iter()
        .any(|s| s.kind == SessionKind::Agent && s.is_running())
}

async fn agent_edits(app: &App, key: &str) -> AgentEdits {
    match app
        .service
        .send(Request::TicketAgentEdits(KeyParams { key: key.to_owned() }))
        .await
    {
        Ok(Reply::AgentEdits(edits)) => edits,
        _ => AgentEdits::default(),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// editor.open

pub async fn open(app: &App, target: Target) -> Result<View, RequestError> {
    let source_mode = global_settings(app).await?.description_source_mode;
    let view = match target {
        Target::Create { project: key, epic } => {
            let epic = epic.as_deref().map(key_of).transpose()?;
            let key = match (&epic, key) {
                (Some(e), _) => e.project.to_string(),
                (None, Some(k)) => k,
                (None, None) => projects(app)
                    .await?
                    .into_iter()
                    .next()
                    .map(|p| p.key)
                    .ok_or_else(|| RequestError::plain("Add a project first."))?,
            };
            let info = project(app, &key).await?;
            let all = tickets(app, &key).await?;
            let tools = agent_tools(app).await;
            let new = |epic| TicketAgentsParams {
                key: None,
                project: Some(key.clone()),
                new_epic: epic,
            };
            View {
                agents: agent_block(app, new(false), &tools).await,
                epic_agents: agent_block(app, new(true), &tools).await,
                ..create_view(&info, epic, &all)
            }
        }
        Target::Edit { key } => {
            let ticket = found(app, &key).await?;
            let project_key = ticket.key.project.to_string();
            let info = project(app, &project_key).await?;
            let all = tickets(app, &project_key).await?;
            let edits = agent_edits(app, &key).await;
            let live = live_agent(app, &key).await;
            let tools = agent_tools(app).await;
            View {
                agents: agent_block(app, of_ticket(&key), &tools).await,
                ..edit_view(&ticket, &info.name, &all, &edits, live, now_ms())
            }
        }
    };
    Ok(View {
        source_mode,
        ..view
    })
}

async fn global_settings(app: &App) -> Result<GlobalSettings, RequestError> {
    match app.service.send(Request::SettingsGet).await? {
        Reply::GlobalSettings(r) => Ok(r.settings),
        other => Err(unexpected("settings.get", &other)),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// editor.source_mode

/// What `editor.source_mode` takes: the "Markdown" toggle's new state.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[ts(rename = "EditorSourceModeParams")]
pub struct SourceModeParams {
    pub on: bool,
}

/// Remembers the toggle for every editor opened after it (d0078 D9), in the global settings (d0023).
pub async fn set_source_mode(app: &App, params: SourceModeParams) -> Result<(), RequestError> {
    let mut settings = global_settings(app).await?;
    if settings.description_source_mode != params.on {
        settings.description_source_mode = params.on;
        app.service.send(Request::SettingsSet(settings)).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// editor.save

/// What `editor.save` takes.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "EditorSaveParams")]
pub struct SaveParams {
    pub target: Target,
    pub form: Form,
}

/// Which field a refusal belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "EditorRefusalField")]
pub enum RefusalField {
    Deps,
    Base,
    Form,
    /// An agent row (a locked row, AGT-04).
    Agents,
}

/// A refusal as the editor shows it (*08 → Cycle prevention, Depends on · different base rejected*): a title by
/// its code, then the service's exact message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EditorRefusal")]
pub struct Refusal {
    pub field: RefusalField,
    pub title: &'static str,
    pub message: String,
    pub icon: IconName,
}

impl Refusal {
    pub fn of(error: &RequestError, field: RefusalField) -> Self {
        let code = error.refusal.as_deref();
        let field = match code {
            Some("cross_base" | "base_locked" | "base_from_epic") if field == RefusalField::Form => {
                RefusalField::Base
            }
            _ => field,
        };
        Self {
            field,
            title: refusal_title(code),
            message: error.message.clone(),
            icon: refusal_icon(code),
        }
    }
}

/// How a save went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "outcome", rename_all = "snake_case")]
#[ts(rename = "EditorSaved")]
pub enum SaveOutcome {
    /// Created, or saved (an existing ticket goes back to read mode; `note` says what its agent will be told).
    Saved { key: String, note: Option<String> },
    /// No title: the field shows `TITLE_REQUIRED`.
    TitleMissing,
    /// Refused; `created` names a new ticket whose dependencies couldn't all be linked (the editor stays open on
    /// it).
    Refused {
        refusal: Refusal,
        created: Option<String>,
    },
}

/// The model the form stands for, with the page's values on top of the target's.
pub fn model_of(base: EditorModel, form: &Form) -> EditorModel {
    let mut model = base;
    model.title = form.title.clone();
    model.body = form.body.clone();
    model.priority = form
        .priority
        .and_then(|n| Priority::from_number(i64::from(n)));
    model.external_id = form.external_id.clone();
    if model.is_create() {
        model.kind = if form.epic_kind {
            Kind::Epic
        } else {
            Kind::Ticket
        };
        model.epic = form.epic.as_deref().and_then(|e| e.parse().ok());
        model.deps = form.deps.iter().filter_map(|d| d.parse().ok()).collect();
    }
    if model.epic.is_none() && !model.base_locked {
        model.base_branch = form.base_branch.clone().or(model.base_branch);
    }
    model.tried = true;
    model
}

pub async fn save(app: &App, params: SaveParams) -> Result<SaveOutcome, RequestError> {
    match params.target {
        Target::Create { project, epic } => {
            let epic_key = epic.or_else(|| params.form.epic.clone());
            let project = match (&epic_key, project) {
                (Some(e), _) => key_of(e)?.project.to_string(),
                (None, Some(p)) => p,
                (None, None) => return Err(RequestError::plain("Pick a project first.")),
            };
            let info = project_base(app, &project).await?;
            let mut form = params.form;
            form.epic = epic_key;
            let model = model_of(EditorModel::create(&project, "", &info), &form);
            if model.title_error().is_some() {
                return Ok(SaveOutcome::TitleMissing);
            }
            let Some(input) = model.create_input() else {
                return Ok(SaveOutcome::TitleMissing);
            };
            let created = match app.service.send(Request::TicketCreate(input)).await {
                Ok(Reply::Ticket(t)) => *t,
                Ok(other) => return Err(unexpected("ticket.create", &other)),
                Err(e) => {
                    return Ok(SaveOutcome::Refused {
                        refusal: Refusal::of(&e, RefusalField::Form),
                        created: None,
                    });
                }
            };
            let key = created.key.to_string();
            for pick in &form.agents {
                if let Err(e) = set_row(app, &key, pick.role, Some(pick.agent.clone())).await {
                    return Ok(SaveOutcome::Refused {
                        refusal: Refusal::of(&e, RefusalField::Agents),
                        created: Some(key),
                    });
                }
            }
            for parent in &model.deps {
                if let Err(e) = app
                    .service
                    .send(Request::DepAdd(DepParams {
                        child: key.clone(),
                        parent: parent.to_string(),
                    }))
                    .await
                {
                    return Ok(SaveOutcome::Refused {
                        refusal: Refusal::of(&e, RefusalField::Deps),
                        created: Some(key),
                    });
                }
            }
            Ok(SaveOutcome::Saved { key, note: None })
        }
        Target::Edit { key } => {
            let before = found(app, &key).await?;
            let model = model_of(EditorModel::edit(&before), &params.form);
            if model.title_error().is_some() {
                return Ok(SaveOutcome::TitleMissing);
            }
            let Some(update) = model.update_input() else {
                return Ok(SaveOutcome::Saved { key, note: None });
            };
            let live = live_agent(app, &key).await;
            match app
                .service
                .send(Request::TicketUpdate(TicketUpdateParams {
                    key: key.clone(),
                    update,
                }))
                .await
            {
                Ok(Reply::Ticket(after)) => Ok(SaveOutcome::Saved {
                    note: agent_activity::saved_note(&before, &after, live),
                    key,
                }),
                Ok(other) => Err(unexpected("ticket.update", &other)),
                Err(e) => Ok(SaveOutcome::Refused {
                    refusal: Refusal::of(&e, RefusalField::Form),
                    created: None,
                }),
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The Agents block and editor.agent_row (AGT-04)

/// `ticket.agents` of an existing ticket.
pub(super) fn of_ticket(key: &str) -> TicketAgentsParams {
    TicketAgentsParams {
        key: Some(key.to_owned()),
        project: None,
        new_epic: false,
    }
}

/// The facts behind a ticket's rows; `None` when the service can't say (the block is then left out).
pub(super) async fn agent_facts(app: &App, params: TicketAgentsParams) -> Option<TicketAgents> {
    match app.service.send(Request::TicketAgents(params)).await {
        Ok(Reply::TicketAgents(t)) => Some(*t),
        _ => None,
    }
}

/// `tool`'s cached model list (the display names of "Opus · always the newest", what a model resolves to).
async fn model_list(app: &App, tool: Tool) -> Option<ModelList> {
    match app
        .service
        .send(Request::AgentsModels(ModelsParams { tool }))
        .await
    {
        Ok(Reply::Models(copy)) => copy.map(|c| c.list),
        _ => None,
    }
}

/// The agent tools as the rows, the panel, the pane and recovery read them (M23): every tool's state
/// (`agent_tools.list`), every tool's active limit (`usage.activity`), and each connected tool's cached model list. When the
/// service can't list the tools, the first tool counts as the one connected, as before M23.
pub(super) async fn agent_tools(app: &App) -> Tools {
    let mut tools = tool_states(app).await;
    // Every tool's active limit (USG-10), from Activity's per-tool data.
    if let Ok(Reply::Activity(activity)) = app.service.send(Request::UsageActivity).await {
        tools.limits = activity.tools.into_iter().filter_map(|t| t.limit).collect();
    }
    for &tool in &tools.connected.clone() {
        if let Some(list) = model_list(app, tool).await {
            tools.lists.push((tool, list));
        }
    }
    tools.now = now_ms();
    tools
}

/// Every tool's state alone (`agent_tools.list`), for what needs only which tools are connected.
pub(super) async fn tool_states(app: &App) -> Tools {
    match app.service.send(Request::AgentToolsList).await {
        Ok(Reply::AgentTools(list)) => Tools::from_statuses(list.tools),
        _ => Tools::one(None),
    }
}

async fn agent_block(app: &App, params: TicketAgentsParams, tools: &Tools) -> Option<Block> {
    agent_facts(app, params)
        .await
        .map(|t| agent_rows::block(&t, tools))
}

async fn set_row(
    app: &App,
    key: &str,
    role: RoleId,
    agent: Option<String>,
) -> Result<(), RequestError> {
    app.service
        .send(Request::TicketAgentRow(AgentRowParams {
            key: key.to_owned(),
            role: role.into(),
            agent,
        }))
        .await
        .map(|_| ())
}

/// What `editor.agent_row` (and `panel.agent_row`) take: a pick in a row's picker (*set by you*), or `agent`
/// `null` for "Project default (…)" and Let agents pick again.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "AgentRowParams")]
pub struct AgentRowRequest {
    pub key: String,
    pub role: RoleId,
    pub agent: Option<String>,
}

/// How a pick went: the block as it is now, or the refusal (a row that locked meanwhile).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "outcome", rename_all = "snake_case")]
#[ts(rename = "AgentRowSet")]
pub enum AgentRowOutcome {
    Set { agents: Option<Block> },
    Refused { refusal: Refusal },
}

/// Sets one row through `ticket.agent_row`, answering the refreshed block.
pub async fn agent_row(app: &App, params: AgentRowRequest) -> Result<AgentRowOutcome, RequestError> {
    if let Err(e) = set_row(app, &params.key, params.role, params.agent).await {
        return Ok(AgentRowOutcome::Refused {
            refusal: Refusal::of(&e, RefusalField::Agents),
        });
    }
    let tools = agent_tools(app).await;
    Ok(AgentRowOutcome::Set {
        agents: agent_block(app, of_ticket(&params.key), &tools).await,
    })
}

async fn project_base(app: &App, project: &str) -> Result<String, RequestError> {
    Ok(self::project(app, project).await?.base_branch)
}

// ---------------------------------------------------------------------------------------------------------------
// editor.picker and editor.link (DEP-01, DEP-02, DEP-04)

/// What `editor.picker` takes: the project, the ticket being edited (greyed out), the links already made and
/// what is typed.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "EditorPickerParams")]
pub struct PickerParams {
    pub project: String,
    pub editing: Option<String>,
    pub chosen: Vec<String>,
    pub query: String,
}

/// One row of the Depends-on picker (*08 → Depends on · picker open*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EditorPickerRow")]
pub struct PickerRowView {
    pub key: String,
    pub title: String,
    pub pill: PillView,
    /// "this ticket will wait for it", "already merged — nothing to wait for"…
    pub note: String,
    pub enabled: bool,
    /// "Epic · 1 / 3".
    pub epic_progress: Option<String>,
}

pub async fn picker(app: &App, params: PickerParams) -> Result<Vec<PickerRowView>, RequestError> {
    let all = tickets(app, &params.project).await?;
    let editing = params.editing.as_deref().map(key_of).transpose()?;
    let chosen: Vec<TicketKey> = params
        .chosen
        .iter()
        .filter_map(|k| k.parse().ok())
        .collect();
    Ok(picker_rows(editing.as_ref(), &chosen, &params.query, &all)
        .into_iter()
        .map(|r| PickerRowView {
            key: r.key.to_string(),
            title: r.title,
            pill: PillView::of(r.status),
            note: r.note,
            enabled: r.enabled,
            epic_progress: r.epic_progress,
        })
        .collect())
}

/// What `editor.link` takes: an existing ticket's link to add or remove (the service checks loops and bases).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "EditorLinkParams")]
pub struct LinkParams {
    pub key: String,
    pub parent: String,
    pub add: bool,
}

/// The ticket's links after the change, or the refusal under Depends on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "outcome", rename_all = "snake_case")]
#[ts(rename = "EditorLinked")]
pub enum Linked {
    Linked { deps: Vec<DepChip> },
    Refused { refusal: Refusal },
}

pub async fn link(app: &App, params: LinkParams) -> Result<Linked, RequestError> {
    let dep = DepParams {
        child: params.key.clone(),
        parent: params.parent,
    };
    let request = if params.add {
        Request::DepAdd(dep)
    } else {
        Request::DepRemove(dep)
    };
    if let Err(e) = app.service.send(request).await {
        return Ok(Linked::Refused {
            refusal: Refusal::of(&e, RefusalField::Deps),
        });
    }
    let ticket = found(app, &params.key).await?;
    let all = tickets(app, &ticket.key.project.to_string()).await?;
    Ok(Linked::Linked {
        deps: chips(&ticket.depends_on, &all),
    })
}

// ---------------------------------------------------------------------------------------------------------------
// editor.branches (DEP-04, d0051)

/// The base dropdown (*08 → Row · base branch per ticket*): LOCAL branches, the project default marked, then
/// REMOTE ones by their short name. An item's id is what the form stores ("release/2.3", "origin/release/2.2").
pub fn branches_of(local: &[String], remote: &[String], default: &str) -> Vec<MenuItem> {
    let (local, remote) = base_choices(local, remote);
    let mut items = vec![MenuItem::header("LOCAL")];
    items.extend(local.into_iter().map(|b| {
        let item = MenuItem::new(b.clone(), b.clone()).icon(IconName::GitBranch);
        if b == default {
            item.shortcut("project default")
        } else {
            item
        }
    }));
    if !remote.is_empty() {
        items.push(MenuItem::header("REMOTE"));
        items.extend(
            remote
                .into_iter()
                .map(|(full, name)| MenuItem::new(full, name).icon(IconName::GitBranch)),
        );
    }
    items
}

/// What `editor.branches` takes.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "EditorBranchesParams")]
pub struct BranchesParams {
    pub project: String,
}

pub async fn branches(app: &App, params: BranchesParams) -> Result<Vec<MenuItem>, RequestError> {
    let default = project_base(app, &params.project).await?;
    match app
        .service
        .send(Request::ProjectBranches(ProjectParams {
            project: params.project,
        }))
        .await?
    {
        Reply::Branches(b) => Ok(branches_of(&b.local, &b.remote, &default)),
        other => Err(unexpected("project.branches", &other)),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// editor.compare (TKT-05)

/// What `editor.compare` takes: the ticket and, from a field's marker, that field only.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "EditorCompareParams")]
pub struct CompareParams {
    pub key: String,
    pub field: Option<EditorField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EditorCompareEntry")]
pub struct CompareEntry {
    /// The field it changed: the description renders as Markdown, the others stay plain text (d0078).
    pub field: EditorField,
    pub heading: String,
    pub before: String,
    pub after: String,
}

/// The agent's edits before / after, newest first; reading them marks them seen (the history stays).
pub async fn compare_view(
    app: &App,
    params: CompareParams,
) -> Result<Vec<CompareEntry>, RequestError> {
    let edits = agent_edits(app, &params.key).await;
    let entries = compare_entries(&edits, params.field, now_ms());
    app.service
        .send(Request::TicketAgentEditsSeen(KeyParams { key: params.key }))
        .await?;
    Ok(entries)
}

fn compare_entries(edits: &AgentEdits, only: Option<EditorField>, now_ms: i64) -> Vec<CompareEntry> {
    compare(edits, only.map(EditorField::core), now_ms)
        .into_iter()
        .map(|e| CompareEntry {
            field: EditorField::of(e.field),
            heading: e.heading,
            before: e.before,
            after: e.after,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use otaq_core::board::projects::AddProject;
    use otaq_core::domain::Status;
    use otaq_core::git::testing::TempRepos;
    use otaq_view::view_models::card_fixtures::ticket as fixture;
    use serde_json::json;

    use super::*;
    use crate::api::request as table;
    use crate::service::testing::{Harness, connected};

    fn info() -> ProjectInfo {
        ProjectInfo {
            key: "SHOP".into(),
            name: "shop-web".into(),
            repo_path: "/tmp/shop-web".into(),
            base_branch: "main".into(),
            agents_working: 0,
            needs_you: 0,
        }
    }

    fn form(title: &str) -> Form {
        Form {
            title: title.into(),
            body: String::new(),
            priority: None,
            epic_kind: false,
            epic: None,
            deps: Vec::new(),
            base_branch: None,
            external_id: String::new(),
            agents: Vec::new(),
        }
    }

    #[test]
    fn a_new_ticket_starts_normal_on_the_project_base() {
        let mut epic = fixture("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        epic.priority = Priority::High;
        let view = create_view(&info(), None, std::slice::from_ref(&epic));
        assert!(view.create);
        assert_eq!(view.texts.primary, "Create ticket");
        assert_eq!(view.texts.body_hint, "Markdown \u{b7} paste keeps formatting");
        assert_eq!(
            view.texts.collapse_word,
            "collapse (after closing a popover)"
        );
        assert_eq!(view.form.base_branch.as_deref(), Some("main"));
        assert_eq!(view.shown_priority, 3);
        assert_eq!(view.base, BaseView::Editable);
        assert_eq!(view.epic_menu[0].label, "No epic");
        assert_eq!(view.epic_menu[1].id, "SHOP-40");

        let child = create_view(&info(), Some("SHOP-40".parse().unwrap()), &[epic]);
        assert_eq!(child.shown_priority, 2, "a child shows its epic's priority");
        assert_eq!(child.follows.as_deref(), Some("follows SHOP-40"));
        assert!(!child.external_id_shown);
        assert!(matches!(child.base, BaseView::FromEpic { .. }));
    }

    #[test]
    fn read_mode_marks_the_agents_edits_and_locks_the_base_after_start() {
        let mut t = fixture("SHOP-19", "Price table", Status::Working);
        t.base_locked = true;
        t.body = "Toggle.\n\n## Done when\n\n- Works".into();
        let view = edit_view(&t, "shop-web", &[t.clone()], &AgentEdits::default(), true, 0);
        let read = view.read.unwrap();
        assert_eq!(read.body_markdown, t.body, "the stored Markdown, untouched");
        assert!(!read.body_empty);
        assert!(read.marked.is_empty());
        assert_eq!(
            view.save_hint.as_deref(),
            Some("SHOP-19\u{2019}s agent will be told what changed.")
        );
        assert_eq!(
            view.base,
            BaseView::Locked {
                text: "locked \u{b7} the ticket has started".into()
            }
        );
    }

    /// *08 → 7 · Updated by agent · compare*: each entry names its field, so the page renders the description's
    /// before / after as Markdown and keeps the other fields plain; the Markdown arrives as stored.
    #[test]
    fn compare_entries_name_their_field() {
        use otaq_core::board::agent_edits::AgentChange;
        use otaq_core::board::tickets::FieldChange;
        let change = |field, before: &str, after: &str, at| AgentChange {
            actor: Actor::Agent("SHOP-12".parse().unwrap()),
            at,
            change: FieldChange {
                field,
                before: Some(before.into()),
                after: Some(after.into()),
            },
        };
        let edits = AgentEdits {
            updated_by_agent: true,
            last_agent: None,
            last_at: Some(2_000),
            changes: vec![
                change(Field::Title, "Dark mode", "Add dark mode toggle", 2_000),
                change(Field::Body, "Toggle.\n\n- [x] a", "Toggle.\n\n- [x] a\n- [ ] b", 1_000),
            ],
        };
        let all = compare_entries(&edits, None, 60_000);
        let fields: Vec<EditorField> = all.iter().map(|e| e.field).collect();
        assert_eq!(fields, [EditorField::Title, EditorField::Body]);
        assert_eq!(all[1].after, "Toggle.\n\n- [x] a\n- [ ] b");
        let body = compare_entries(&edits, Some(EditorField::Body), 60_000);
        assert_eq!(body.len(), 1);
        assert_eq!(serde_json::to_value(&body[0]).unwrap()["field"], "body");
    }

    /// *05 → Ticket tab · rich Markdown read mode*: the page gets the description as stored, byte for byte (Copy as
    /// Markdown copies it), and an empty one shows "Nothing written yet.".
    #[test]
    fn read_mode_sends_the_stored_markdown() {
        let mut t = fixture("SHOP-47", "Checkout address autocomplete", Status::Backlog);
        t.body = "## Why\r\nUsers mistype **addresses**  \r\n* [x] 3 chars\n\n| a |\n|:-|\n| \\* |".into();
        let read = edit_view(&t, "shop-web", &[t.clone()], &AgentEdits::default(), false, 0)
            .read
            .unwrap();
        assert_eq!(read.body_markdown, t.body);
        assert!(!read.body_empty);

        for empty in ["", " \n\n\t"] {
            t.body = empty.into();
            let read = edit_view(&t, "shop-web", &[t.clone()], &AgentEdits::default(), false, 0)
                .read
                .unwrap();
            assert!(read.body_empty, "{empty:?}");
            assert_eq!(read.body_markdown, empty);
        }
    }

    /// *08 → Editing while it validates (SHOP-29)*.
    #[test]
    fn editing_while_it_validates_warns_first() {
        let mut t = fixture("SHOP-29", "Coupon field", Status::Checking);
        t.base_locked = true;
        let view = edit_view(&t, "shop-web", &[t.clone()], &AgentEdits::default(), true, 0);
        assert_eq!(
            view.save_warning.as_deref(),
            Some("SHOP-29\u{2019}s validation will be cancelled and its agent told what changed.")
        );
        assert_eq!(
            view.unsaved_warning,
            Some("unsaved \u{b7} saving cancels the validation")
        );
        t.status = Status::Working;
        let view = edit_view(&t, "shop-web", &[t.clone()], &AgentEdits::default(), true, 0);
        assert_eq!((view.save_warning, view.unsaved_warning), (None, None));
    }

    #[test]
    fn the_form_becomes_the_model_and_refusals_find_their_field() {
        let mut f = form("  ");
        f.priority = Some(2);
        f.deps = vec!["SHOP-18".into()];
        let model = model_of(EditorModel::create("SHOP", "", "main"), &f);
        assert_eq!(model.title_error(), Some(TITLE_REQUIRED));
        assert_eq!(model.priority, Some(Priority::High));
        assert_eq!(model.deps.len(), 1);

        let cross = RequestError {
            message: "SHOP-31 starts from release/2.3".into(),
            refusal: Some("cross_base".into()),
            not_found: false,
        };
        assert_eq!(Refusal::of(&cross, RefusalField::Form).field, RefusalField::Base);
        assert_eq!(Refusal::of(&cross, RefusalField::Deps).field, RefusalField::Deps);
        assert_eq!(
            Refusal::of(&cross, RefusalField::Deps).title,
            "Different base branches"
        );
        let local = branches_of(
            &["main".into(), "release/2.3".into()],
            &["origin/main".into(), "origin/release/2.2".into()],
            "main",
        );
        let labels: Vec<_> = local.iter().map(|i| (i.id.as_str(), i.label.as_str())).collect();
        assert_eq!(
            labels,
            [
                ("", "LOCAL"),
                ("main", "main"),
                ("release/2.3", "release/2.3"),
                ("", "REMOTE"),
                ("origin/release/2.2", "release/2.2")
            ]
        );
        assert_eq!(local[1].shortcut.as_deref(), Some("project default"));
    }

    #[tokio::test]
    async fn the_editor_creates_edits_and_links_real_tickets() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let path = repos.repo("shop-web", 1);
        let project = match h
            .app
            .service
            .send(Request::ProjectAdd(AddProject {
                path: path.display().to_string(),
                key: None,
                base_branch: None,
            }))
            .await
            .unwrap()
        {
            Reply::ProjectAdded { project: p, .. } => p.key,
            other => panic!("{other:?}"),
        };
        let create = json!({ "mode": "create", "project": project, "epic": null });
        let view = table(&h.app, "editor.open", create.clone()).await.unwrap();
        assert_eq!(view["heading"], "New ticket");

        let missing = table(&h.app, "editor.save", json!({ "target": create, "form": form("") }))
            .await
            .unwrap();
        assert_eq!(missing["outcome"], "title_missing");

        let saved = table(
            &h.app,
            "editor.save",
            json!({ "target": create, "form": form("Checkout API") }),
        )
        .await
        .unwrap();
        assert_eq!(saved["outcome"], "saved");
        let parent = saved["key"].as_str().unwrap().to_owned();

        let mut child_form = form("Checkout page");
        child_form.deps = vec![parent.clone()];
        let saved = table(
            &h.app,
            "editor.save",
            json!({ "target": create, "form": child_form }),
        )
        .await
        .unwrap();
        let child = saved["key"].as_str().unwrap().to_owned();

        let edit = json!({ "mode": "edit", "key": child });
        let view = table(&h.app, "editor.open", edit.clone()).await.unwrap();
        assert_eq!(view["deps"][0]["key"], parent.as_str());
        assert_eq!(view["read"]["origin"], "Created by you");

        // A loop is refused with the frame's title, under Depends on.
        let refused = table(
            &h.app,
            "editor.link",
            json!({ "key": parent, "parent": child, "add": true }),
        )
        .await
        .unwrap();
        assert_eq!(refused["outcome"], "refused");
        assert_eq!(refused["refusal"]["title"], "That would create a loop");

        let rows = table(
            &h.app,
            "editor.picker",
            json!({ "project": project, "editing": child, "chosen": [parent], "query": "" }),
        )
        .await
        .unwrap();
        assert_eq!(rows[0]["key"], child.as_str());
        assert_eq!(rows[0]["enabled"], false);

        let mut changed = view["form"].clone();
        changed["title"] = json!("Checkout page v2");
        let saved = table(&h.app, "editor.save", json!({ "target": edit, "form": changed }))
            .await
            .unwrap();
        assert_eq!(saved["outcome"], "saved");
        let view = table(&h.app, "editor.open", edit).await.unwrap();
        assert_eq!(view["read"]["title"], "Checkout page v2");

        let compare = table(&h.app, "editor.compare", json!({ "key": child, "field": null }))
            .await
            .unwrap();
        assert_eq!(compare, json!([]), "the user's own edits aren't the agent's");
        let branches = table(&h.app, "editor.branches", json!({ "project": project }))
            .await
            .unwrap();
        assert_eq!(branches[1]["shortcut"], "project default");

        // The Markdown toggle (d0078 D9): off at first, then every editor opens in the mode last chosen.
        assert_eq!(view["source_mode"], false);
        table(&h.app, "editor.source_mode", json!({ "on": true })).await.unwrap();
        let view = table(&h.app, "editor.open", create.clone()).await.unwrap();
        assert_eq!(view["source_mode"], true);
        let view = table(&h.app, "editor.open", json!({ "mode": "edit", "key": parent }))
            .await
            .unwrap();
        assert_eq!(view["source_mode"], true);
        table(&h.app, "editor.source_mode", json!({ "on": false })).await.unwrap();
        let view = table(&h.app, "editor.open", create).await.unwrap();
        assert_eq!(view["source_mode"], false);
        h.service.stop_all().await;
    }

    /// The Agents block through a real service (AGT-04): one agent reads only; with two, ⌘N's pick is set on the
    /// new ticket (*set by you*), `editor.agent_row` changes a row, and the panel shows the same rows, the open
    /// request's `focus_agent_row` and, once History has the row's change, its line.
    #[tokio::test]
    async fn the_agents_block_reads_and_sets_rows() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let path = repos.repo("shop-web", 1);
        let project = match h
            .app
            .service
            .send(Request::ProjectAdd(AddProject {
                path: path.display().to_string(),
                key: None,
                base_branch: None,
            }))
            .await
            .unwrap()
        {
            Reply::ProjectAdded { project: p, .. } => p.key,
            other => panic!("{other:?}"),
        };
        let ask = async |request: Value| {
            h.app
                .service
                .send(serde_json::from_value(request).unwrap())
                .await
                .unwrap()
        };
        let add = async |name: &str| match ask(
            json!({ "method": "agents.add", "params": { "name": name, "tool": "claude" } }),
        )
        .await
        {
            Reply::Agent(a) => a.id,
            other => panic!("{other:?}"),
        };
        let cc = add("Claude Code").await;
        for role in ["developer", "validator", "lead", "project_agent"] {
            ask(json!({ "method": "agents.set_default", "params": { "role": role, "agent": cc } }))
                .await;
        }
        let create = json!({ "mode": "create", "project": project, "epic": null });
        let view = table(&h.app, "editor.open", create.clone()).await.unwrap();
        let block = &view["agents"];
        assert_eq!(block["hint"], "who writes the code and who checks it");
        assert_eq!(block["rows"][0]["value"], "Claude Code");
        assert_eq!(block["rows"][0]["picker"], Value::Null, "one agent: read-only");
        assert_eq!(block["note"], agent_rows::ONLY_AGENT);
        assert_eq!(view["epic_agents"]["rows"][0]["label"], "Epic lead");

        let strong = add("Strong").await;
        let mut f = form("Price table");
        f.agents = vec![AgentPick {
            role: RoleId::Developer,
            agent: strong.clone(),
        }];
        let saved = table(&h.app, "editor.save", json!({ "target": create, "form": f }))
            .await
            .unwrap();
        let key = saved["key"].as_str().unwrap().to_owned();
        let view = table(&h.app, "editor.open", json!({ "mode": "edit", "key": key }))
            .await
            .unwrap();
        let dev = &view["agents"]["rows"][0];
        assert_eq!(dev["value"], "Strong");
        assert_eq!(dev["set_by"]["text"], "set by you");
        assert_eq!(dev["let_agents_pick"], "Let agents pick again");
        assert_eq!(view["agents"]["note"], agent_rows::NOT_STARTED);
        assert_eq!(view["agents"]["rows"][1]["value"], "Project default (Claude Code)");

        let set = table(
            &h.app,
            "editor.agent_row",
            json!({ "key": key, "role": "developer", "agent": null }),
        )
        .await
        .unwrap();
        assert_eq!(set["outcome"], "set");
        assert_eq!(set["agents"]["rows"][0]["value"], "Project default (Claude Code)");
        let set = table(
            &h.app,
            "panel.agent_row",
            json!({ "key": key, "role": "validator", "agent": strong }),
        )
        .await
        .unwrap();
        assert_eq!(set["agents"]["rows"][1]["value"], "Strong");
        let refused = table(
            &h.app,
            "panel.agent_row",
            json!({ "key": key, "role": "lead", "agent": strong }),
        )
        .await
        .unwrap();
        assert_eq!(refused["outcome"], "refused");
        assert_eq!(refused["refusal"]["field"], "agents");

        let load = table(
            &h.app,
            "panel.view",
            json!({ "key": key, "focus_agent_row": "validator" }),
        )
        .await
        .unwrap();
        assert_eq!(load["view"]["focus_agent_row"], "validator");
        assert_eq!(load["view"]["agents"]["rows"][1]["set_by"]["text"], "set by you");
        assert_eq!(load["view"]["token_split"], Value::Null, "nothing ran yet");
        let load = table(&h.app, "panel.view", json!({ "key": key })).await.unwrap();
        assert_eq!(load["view"]["focus_agent_row"], Value::Null);

        let history = table(&h.app, "sections.history", json!({ "key": key })).await.unwrap();
        let titles: Vec<&str> = history["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|r| r["title"].as_str())
            .collect();
        assert!(titles.contains(&"Developer: Strong (set by you)"), "{titles:?}");
        assert!(titles.contains(&"Validator: Strong (set by you)"), "{titles:?}");
        h.service.stop_all().await;
    }
}
