//! The ticket editor (UI-05, TKT-03; d0019, d0058; *08 · Ticket editor → Full form · filled, Validation · empty
//! form, Dark · full form*): a modal with Title, Type and Epic (when creating), What and why (Markdown, code
//! blocks kept intact), and the "Done when" checklist (add with ↵, drag to reorder, remove). An existing ticket
//! opens in read mode first; Edit switches to the form. ⌘↵ saves, Esc discards. Built as a view of its own so
//! the panel's Ticket tab can host it later (T-0062).

use crewboard_core::board::agent_edits::AgentEdits;
use crewboard_core::board::tickets::Field;
use crewboard_core::board::tickets::{Base, Ticket};
use crewboard_core::domain::{Kind, TicketKey};
use crewboard_core::protocol::{
    DepParams, KeyParams, ProjectParams, Reply, Request, TicketUpdateParams,
};
use crewboard_core::store::now_ms;
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, IntoElement, KeyBinding, MouseButton, Point, SharedString, Window, actions,
    anchored, deferred, div, prelude::*, px,
};

use crate::components::button::{Button, IconButton, Variant};
use crate::components::field::DropdownButton;
use crate::components::floating_shadow;
use crate::components::kbd::Kbd;
use crate::components::markdown::Markdown;
use crate::components::menu::{ContextMenu, MenuChosen};
use crate::components::pill::StatusPill;
use crate::components::tabs::Segmented;
use crate::components::text_input::{InputEvent, TextInput};
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, Colors, FONT_MONO, radius};
use crate::view_models::agent_activity::{save_hint, saved_note};
use crate::view_models::agent_edits::{changed, compare, done_when_note, origin_line};
use crate::view_models::editor::{EditorMode, EditorModel};
use crate::view_models::editor_fields::{
    BaseField, PickerRow, base_choices, base_label, picker_rows, refusal_icon, refusal_title,
    visibility,
};
use crate::view_models::menu::MenuItem;
use crate::workspace::Workspace;

actions!(ticket_editor, [SaveTicket, CloseEditor]);

const CONTEXT: &str = "TicketEditor";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-enter", SaveTicket, Some(CONTEXT)),
        KeyBinding::new("escape", CloseEditor, Some(CONTEXT)),
    ]);
}

/// The editor is done (saved, cancelled or discarded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorClosed;

/// A criterion being dragged to a new place.
#[derive(Clone)]
struct DraggedCriterion {
    index: usize,
    text: SharedString,
}

struct CriterionPreview(SharedString);

impl Render for CriterionPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(radius::SM))
            .bg(c.bg_raised)
            .border_1()
            .border_color(c.border_strong)
            .shadow(floating_shadow())
            .text_size(px(13.))
            .text_color(c.text_primary)
            .child(self.0.clone())
    }
}

pub struct TicketEditor {
    workspace: Entity<Workspace>,
    pub model: EditorModel,
    read_mode: bool,
    title: Entity<TextInput>,
    body: Entity<TextInput>,
    criteria: Vec<Entity<TextInput>>,
    new_criterion: Entity<TextInput>,
    focus: FocusHandle,
    epic_menu: Option<(Entity<ContextMenu>, Point<gpui::Pixels>)>,
    base_menu: Option<(Entity<ContextMenu>, Point<gpui::Pixels>)>,
    /// The Depends-on search field and whether its list is open.
    dep_query: Entity<TextInput>,
    picker_open: bool,
    external_id: Entity<TextInput>,
    /// The ticket as last loaded (read mode's origin line).
    ticket: Option<Ticket>,
    /// The agent's unseen edits (T-0023), and the ones Compare shows after marking them seen.
    edits: AgentEdits,
    seen_edits: AgentEdits,
    /// Compare is open for one field or all of them.
    compare: Option<Option<Field>>,
    /// Shown inside the side panel's Ticket tab (T-0062): no dialog frame or title row of its own.
    embedded: bool,
    /// The ticket's agent session runs, so a save tells the agent what changed (SES-08; the panel keeps it
    /// current).
    live_agent: bool,
    /// After a save that sends the agent a note: "Saved. SHOP-19’s agent will be told what changed: …".
    saved_note: Option<String>,
}

impl EventEmitter<EditorClosed> for TicketEditor {}

/// "Open terminal" on the saved-note banner: the panel switches to the Terminal tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorOpenTerminal;

impl EventEmitter<EditorOpenTerminal> for TicketEditor {}

impl Focusable for TicketEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TicketEditor {
    /// A new ticket in the selected project, with the title typed so far.
    pub fn create(
        workspace: Entity<Workspace>,
        title: &str,
        epic: Option<TicketKey>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (project, base) = workspace
            .read(cx)
            .current()
            .map(|p| (p.key.clone(), p.base_branch.clone()))
            .unwrap_or_default();
        let mut model = EditorModel::create(&project, title, &base);
        model.epic = epic;
        let mut editor = Self::with_model(workspace, model, false, window, cx);
        let handle = editor.title.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        editor.read_mode = false;
        editor
    }

    /// An existing ticket: read mode, or straight into the form at "Done when" (`at_criteria`).
    pub fn open(
        workspace: Entity<Workspace>,
        ticket: &Ticket,
        at_criteria: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut editor = Self::with_model(
            workspace,
            EditorModel::edit(ticket),
            !at_criteria,
            window,
            cx,
        );
        editor.ticket = Some(ticket.clone());
        editor.load_agent_edits(cx);
        let handle = if at_criteria {
            editor.new_criterion.read(cx).focus_handle(cx)
        } else {
            editor.focus.clone()
        };
        window.focus(&handle, cx);
        editor
    }

    /// The editor inside the side panel's Ticket tab: it fills the tab and the panel's header names the ticket.
    pub fn embedded(mut self) -> Self {
        self.embedded = true;
        self
    }

    /// Whether read mode shows `ticket` as it is: the fields the Ticket tab draws (signals and ages that change
    /// every few seconds don't count).
    pub fn shows(&self, ticket: &Ticket) -> bool {
        self.ticket.as_ref().is_some_and(|t| {
            (
                &t.title,
                &t.body,
                &t.done_when,
                &t.depends_on,
                &t.base,
                &t.external_id,
                &t.epic,
            ) == (
                &ticket.title,
                &ticket.body,
                &ticket.done_when,
                &ticket.depends_on,
                &ticket.base,
                &ticket.external_id,
                &ticket.epic,
            ) && (&t.created_by, &t.updated_by, t.flags.updated_by_agent)
                == (
                    &ticket.created_by,
                    &ticket.updated_by,
                    ticket.flags.updated_by_agent,
                )
        })
    }

    pub fn saved_note(&self) -> Option<String> {
        self.saved_note.clone()
    }

    /// Keeps the saved-note banner across a refresh of read mode.
    pub fn set_saved_note(&mut self, note: Option<String>, cx: &mut Context<Self>) {
        self.saved_note = note;
        cx.notify();
    }

    /// Whether the ticket's agent session runs (the panel knows its sessions).
    pub fn set_live_agent(&mut self, live: bool, cx: &mut Context<Self>) {
        if self.live_agent != live {
            self.live_agent = live;
            cx.notify();
        }
    }

    /// Switches an embedded editor from read mode to the form (the panel's Edit, + Add dependency).
    pub fn start_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.read_mode = false;
        self.saved_note = None;
        let handle = self.title.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    }

    pub fn is_reading(&self) -> bool {
        self.read_mode
    }

    fn with_model(
        workspace: Entity<Workspace>,
        model: EditorModel,
        read_mode: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| {
            let mut input = TextInput::new(window, cx).placeholder("Short, imperative title");
            input.set_text(&model.title, cx);
            input
        });
        cx.subscribe_in(
            &title,
            window,
            |ed, input, event: &InputEvent, window, cx| {
                match event {
                    InputEvent::Changed => {
                        ed.model.title = input.read(cx).text().to_owned();
                        let error = ed.model.title_error().is_some();
                        input.update(cx, |i, cx| i.set_error(error, cx));
                    }
                    InputEvent::Submit => ed.save(window, cx),
                    InputEvent::Cancel => cx.emit(EditorClosed),
                    _ => {}
                }
                cx.notify();
            },
        )
        .detach();
        let body = cx.new(|cx| {
            let mut input = TextInput::new(window, cx)
                .multiline(px(120.))
                .code_fences()
                .placeholder("Context, constraints, links…");
            input.set_text(&model.body, cx);
            input
        });
        cx.subscribe_in(
            &body,
            window,
            |ed, input, event: &InputEvent, window, cx| {
                match event {
                    InputEvent::Changed => ed.model.body = input.read(cx).text().to_owned(),
                    InputEvent::Submit => ed.save(window, cx),
                    InputEvent::Cancel => cx.emit(EditorClosed),
                    _ => {}
                }
                cx.notify();
            },
        )
        .detach();
        let new_criterion = cx.new(|cx| TextInput::new(window, cx).placeholder("Add a criterion"));
        cx.subscribe_in(
            &new_criterion,
            window,
            |ed, input, event: &InputEvent, window, cx| {
                match event {
                    InputEvent::Submit => {
                        let text = input.read(cx).text().to_owned();
                        if ed.model.add_criterion(&text) {
                            input.update(cx, |i, cx| i.set_text("", cx));
                            ed.rebuild_criteria(window, cx);
                        }
                    }
                    InputEvent::Cancel => cx.emit(EditorClosed),
                    _ => {}
                }
                cx.notify();
            },
        )
        .detach();
        let dep_query = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Search tickets by # or title")
                .bare()
        });
        cx.subscribe(&dep_query, |ed, _, event: &InputEvent, cx| {
            match event {
                InputEvent::Changed => ed.picker_open = true,
                InputEvent::Blur => ed.picker_open = false,
                InputEvent::Cancel => cx.emit(EditorClosed),
                InputEvent::Submit => {
                    let first = ed.picker_rows(cx).into_iter().find(|r| r.enabled);
                    if let Some(row) = first {
                        ed.add_dependency(row.key, cx);
                    }
                }
                _ => {}
            }
            cx.notify();
        })
        .detach();
        let external_id = cx.new(|cx| {
            let mut input = TextInput::new(window, cx)
                .mono()
                .leading_icon(IconName::Hash)
                .placeholder("e.g. PAY-123");
            input.set_text(&model.external_id, cx);
            input
        });
        cx.subscribe_in(
            &external_id,
            window,
            |ed, input, event: &InputEvent, window, cx| {
                match event {
                    InputEvent::Changed => ed.model.external_id = input.read(cx).text().to_owned(),
                    InputEvent::Submit => ed.save(window, cx),
                    InputEvent::Cancel => cx.emit(EditorClosed),
                    _ => {}
                }
                cx.notify();
            },
        )
        .detach();
        let mut editor = Self {
            workspace,
            model,
            read_mode,
            title,
            body,
            criteria: Vec::new(),
            new_criterion,
            focus: cx.focus_handle(),
            epic_menu: None,
            base_menu: None,
            dep_query,
            picker_open: false,
            external_id,
            ticket: None,
            edits: AgentEdits::default(),
            seen_edits: AgentEdits::default(),
            compare: None,
            embedded: false,
            live_agent: false,
            saved_note: None,
        };
        editor.rebuild_criteria(window, cx);
        editor
    }

    /// One input per criterion, in the model's order.
    fn rebuild_criteria(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.criteria = self
            .model
            .criteria
            .iter()
            .enumerate()
            .map(|(i, criterion)| {
                let input = cx.new(|cx| {
                    let mut input = TextInput::new(window, cx).bare();
                    input.set_text(&criterion.text, cx);
                    input
                });
                cx.subscribe(&input, move |ed, input, event: &InputEvent, cx| {
                    match event {
                        InputEvent::Changed => {
                            let text = input.read(cx).text().to_owned();
                            ed.model.edit_criterion(i, &text);
                        }
                        InputEvent::Cancel => cx.emit(EditorClosed),
                        _ => {}
                    }
                    cx.notify();
                })
                .detach();
                input
            })
            .collect();
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model.tried = true;
        let error = self.model.title_error().is_some();
        self.title.update(cx, |i, cx| i.set_error(error, cx));
        if !self.model.can_save() {
            cx.notify();
            return;
        }
        let request = match &self.model.mode {
            EditorMode::Create { .. } => self.model.create_input().map(Request::TicketCreate),
            EditorMode::Edit { key } => match self.model.update_input() {
                Some(update) => Some(Request::TicketUpdate(TicketUpdateParams {
                    key: key.to_string(),
                    update,
                })),
                None => {
                    self.read_mode = true;
                    window.focus(&self.focus, cx);
                    cx.notify();
                    return;
                }
            },
        };
        let Some(request) = request else { return };
        self.model.saving = true;
        self.model.error = None;
        cx.notify();
        let send = self.workspace.read(cx).link.read(cx).send(request);
        let creating = self.model.is_create();
        cx.spawn_in(window, async move |this, cx| {
            let result = send.await;
            let _ = this.update_in(cx, |ed, window, cx| {
                ed.model.saving = false;
                match result {
                    Ok(Reply::Ticket(ticket)) => {
                        let ticket = *ticket;
                        ed.workspace
                            .update(cx, |ws, cx| ws.upsert(ticket.clone(), cx));
                        if creating && !ed.model.deps.is_empty() {
                            let deps = ed.model.deps.clone();
                            ed.link_after_create(ticket, deps, window, cx);
                        } else if creating {
                            cx.emit(EditorClosed);
                        } else {
                            ed.saved_note = ed
                                .ticket
                                .as_ref()
                                .and_then(|before| saved_note(before, &ticket, ed.live_agent));
                            ed.model = EditorModel::edit(&ticket);
                            ed.ticket = Some(ticket);
                            ed.read_mode = true;
                            ed.rebuild_criteria(window, cx);
                            window.focus(&ed.focus, cx);
                        }
                    }
                    Ok(other) => ed.model.error = Some(format!("unexpected answer: {other:?}")),
                    Err(e)
                        if matches!(
                            e.refusal.as_deref(),
                            Some("cross_base") | Some("base_locked") | Some("base_from_epic")
                        ) =>
                    {
                        ed.model.base_error = Some(e.message)
                    }
                    Err(e) => ed.model.error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_epic_menu(
        &mut self,
        position: Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ws = self.workspace.read(cx);
        let mut items = vec![MenuItem::new("", "No epic").checked(self.model.epic.is_none())];
        let epics: Vec<(String, String)> = ws
            .tickets
            .iter()
            .filter(|t| t.kind == Kind::Epic && !t.has_started())
            .map(|t| (t.key.to_string(), t.title.clone()))
            .collect();
        for (key, title) in &epics {
            items.push(
                MenuItem::new(SharedString::from(key.clone()), format!("{key} · {title}"))
                    .icon(IconName::Layers)
                    .checked(
                        self.model.epic.as_ref().map(ToString::to_string).as_deref() == Some(key),
                    ),
            );
        }
        let menu = cx.new(|cx| ContextMenu::new(items, window, cx).width(300.));
        cx.subscribe(&menu, |ed, _, chosen: &MenuChosen, cx| {
            ed.model.epic = chosen.0.parse().ok();
            cx.notify();
        })
        .detach();
        cx.subscribe_in(&menu, window, |ed, _, _: &DismissEvent, window, cx| {
            ed.epic_menu = None;
            window.focus(&ed.focus, cx);
            cx.notify();
        })
        .detach();
        self.epic_menu = Some((menu, position));
        cx.notify();
    }

    fn load_agent_edits(&self, cx: &mut Context<Self>) {
        let EditorMode::Edit { key } = &self.model.mode else {
            return;
        };
        let send = self
            .workspace
            .read(cx)
            .link
            .read(cx)
            .send(Request::TicketAgentEdits(KeyParams {
                key: key.to_string(),
            }));
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::AgentEdits(edits)) = send.await {
                let _ = this.update(cx, |ed, cx| {
                    ed.edits = edits;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn picker_rows(&self, cx: &App) -> Vec<PickerRow> {
        let editing = match &self.model.mode {
            EditorMode::Edit { key } => Some(key.clone()),
            EditorMode::Create { .. } => None,
        };
        let query = self.dep_query.read(cx).text().to_owned();
        picker_rows(
            editing.as_ref(),
            &self.model.deps,
            &query,
            &self.workspace.read(cx).tickets,
        )
    }

    /// Picks a dependency: an existing ticket links at once (the service checks loops and bases); a new one
    /// links right after it is created.
    fn add_dependency(&mut self, parent: TicketKey, cx: &mut Context<Self>) {
        self.model.dep_error = None;
        self.dep_query.update(cx, |i, cx| i.set_text("", cx));
        match &self.model.mode {
            EditorMode::Create { .. } => {
                if !self.model.deps.contains(&parent) {
                    self.model.deps.push(parent);
                }
                cx.notify();
            }
            EditorMode::Edit { key } => {
                let request = Request::DepAdd(DepParams {
                    child: key.to_string(),
                    parent: parent.to_string(),
                });
                self.send_link(request, cx);
            }
        }
    }

    fn remove_dependency(&mut self, parent: TicketKey, cx: &mut Context<Self>) {
        self.model.dep_error = None;
        match &self.model.mode {
            EditorMode::Create { .. } => {
                self.model.deps.retain(|d| *d != parent);
                cx.notify();
            }
            EditorMode::Edit { key } => {
                let request = Request::DepRemove(DepParams {
                    child: key.to_string(),
                    parent: parent.to_string(),
                });
                self.send_link(request, cx);
            }
        }
    }

    fn send_link(&mut self, request: Request, cx: &mut Context<Self>) {
        let send = self.workspace.read(cx).link.read(cx).send(request);
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |ed, cx| {
                match result {
                    Ok(Reply::Ticket(ticket)) => {
                        ed.model.deps = ticket.depends_on.clone();
                        ed.workspace.update(cx, |ws, cx| ws.upsert(*ticket, cx));
                    }
                    Ok(_) => {}
                    Err(e) => ed.model.dep_error = Some((e.refusal, e.message)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Links a new ticket's chosen dependencies after it was created; a refusal keeps the editor open on the
    /// new ticket with the error.
    fn link_after_create(
        &mut self,
        ticket: Ticket,
        deps: Vec<TicketKey>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let link = self.workspace.read(cx).link.clone();
        let child = ticket.key.to_string();
        let sends: Vec<_> = deps
            .iter()
            .map(|parent| {
                link.read(cx).send(Request::DepAdd(DepParams {
                    child: child.clone(),
                    parent: parent.to_string(),
                }))
            })
            .collect();
        cx.spawn_in(window, async move |this, cx| {
            let mut latest = ticket;
            let mut error = None;
            for send in sends {
                match send.await {
                    Ok(Reply::Ticket(t)) => latest = *t,
                    Ok(_) => {}
                    Err(e) => {
                        error = Some((e.refusal, e.message));
                        break;
                    }
                }
            }
            let _ = this.update_in(cx, |ed, window, cx| {
                ed.workspace
                    .update(cx, |ws, cx| ws.upsert(latest.clone(), cx));
                match error {
                    None => cx.emit(EditorClosed),
                    Some(err) => {
                        ed.model = EditorModel::edit(&latest);
                        ed.model.dep_error = Some(err);
                        ed.read_mode = false;
                        ed.rebuild_criteria(window, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_base_menu(
        &mut self,
        position: Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ws = self.workspace.read(cx);
        let Some(project) = ws.current().cloned() else {
            return;
        };
        let send = ws
            .link
            .read(cx)
            .send(Request::ProjectBranches(ProjectParams {
                project: project.key.clone(),
            }));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Reply::Branches(branches)) = send.await else {
                return;
            };
            let _ = this.update_in(cx, |ed, window, cx| {
                let (local, remote) = base_choices(&branches.local, &branches.remote);
                let current = ed.model.base_branch.clone().unwrap_or_default();
                let mut items = vec![MenuItem::header("LOCAL")];
                for b in local {
                    let mut item = MenuItem::new(SharedString::from(b.clone()), b.clone())
                        .icon(IconName::GitBranch)
                        .checked(b == current);
                    if b == project.base_branch {
                        item = item.shortcut("project default");
                    }
                    items.push(item);
                }
                if !remote.is_empty() {
                    items.push(MenuItem::header("REMOTE"));
                    for (full, name) in remote {
                        items.push(
                            MenuItem::new(SharedString::from(name.clone()), full)
                                .icon(IconName::GitBranch)
                                .checked(name == current),
                        );
                    }
                }
                let menu = cx.new(|cx| ContextMenu::new(items, window, cx).width(300.));
                cx.subscribe(&menu, |ed, _, chosen: &MenuChosen, cx| {
                    ed.model.base_branch = Some(chosen.0.to_string());
                    ed.model.base_error = None;
                    cx.notify();
                })
                .detach();
                cx.subscribe_in(&menu, window, |ed, _, _: &DismissEvent, window, cx| {
                    ed.base_menu = None;
                    window.focus(&ed.focus, cx);
                    cx.notify();
                })
                .detach();
                ed.base_menu = Some((menu, position));
                cx.notify();
            });
        })
        .detach();
    }

    fn error_box(code: Option<&str>, message: &str, c: &Colors) -> AnyElement {
        let title = refusal_title(code);
        div()
            .flex()
            .gap(px(8.))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(radius::MD))
            .bg(c.st_error_bg)
            .child(
                icon(refusal_icon(code), px(13.))
                    .text_color(c.st_error)
                    .mt(px(2.)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.st_error)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(c.text_secondary)
                            .child(SharedString::from(message.to_owned())),
                    ),
            )
            .into_any_element()
    }

    fn depends_on(&self, c: &Colors, cx: &mut Context<Self>) -> AnyElement {
        let ws = self.workspace.read(cx);
        let chips = self.model.deps.iter().map(|parent| {
            let status = ws
                .tickets
                .iter()
                .find(|t| &t.key == parent)
                .map(|t| t.status);
            let remove = {
                let parent = parent.clone();
                cx.listener(move |ed, _, _, cx| ed.remove_dependency(parent.clone(), cx))
            };
            div()
                .id(SharedString::from(format!("dep-chip-{parent}")))
                .flex()
                .flex_none()
                .items_center()
                .gap(px(4.))
                .pl(px(6.))
                .pr(px(4.))
                .py(px(2.))
                .rounded(px(4.))
                .bg(c.bg_column)
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(11.))
                        .text_color(c.text_primary)
                        .child(SharedString::from(parent.to_string())),
                )
                .children(status.map(StatusPill::of))
                .child(
                    div()
                        .id(SharedString::from(format!("dep-chip-x-{parent}")))
                        .cursor_pointer()
                        .child(icon(IconName::X, px(11.)).text_color(c.text_muted))
                        .on_click(remove),
                )
        });
        let error = self.model.dep_error.clone();
        let project_names: Vec<String> = ws
            .switcher
            .projects()
            .iter()
            .filter(|p| Some(&p.key) != ws.current().map(|c| &c.key))
            .map(|p| p.name.clone())
            .collect();
        let project_label = ws
            .current()
            .map(|p| p.name.to_uppercase())
            .unwrap_or_default();
        let picker = (self.picker_open && !self.read_mode).then(|| {
            let rows = self.picker_rows(cx);
            let count = rows.len();
            div()
                .absolute()
                .top(px(36.))
                .left_0()
                .w_full()
                .flex()
                .flex_col()
                .p(px(4.))
                .bg(c.bg_raised)
                .border_1()
                .border_color(c.border)
                .rounded(px(radius::LG))
                .shadow(floating_shadow())
                .child(
                    div()
                        .px(px(8.))
                        .py(px(4.))
                        .font_family(FONT_MONO)
                        .text_size(px(10.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(format!(
                            "{project_label} · {count} MATCH{}",
                            if count == 1 { "" } else { "ES" }
                        ))),
                )
                .child(
                    div()
                        .id("dep-picker-list")
                        .max_h(px(260.))
                        .overflow_y_scroll()
                        .children(rows.into_iter().enumerate().map(|(i, row)| {
                            let key = row.key.clone();
                            let mut el = div()
                                .id(("dep-row", i))
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .px(px(8.))
                                .py(px(5.))
                                .rounded(px(5.))
                                .child(
                                    div()
                                        .w(px(64.))
                                        .font_family(FONT_MONO)
                                        .text_size(px(11.))
                                        .text_color(c.text_muted)
                                        .child(SharedString::from(row.key.to_string())),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(13.))
                                                .text_color(c.text_primary)
                                                .child(SharedString::from(row.title.clone())),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.))
                                                .text_color(c.text_muted)
                                                .child(SharedString::from(row.note.clone())),
                                        ),
                                )
                                .child(match &row.epic_progress {
                                    Some(p) => StatusPill::legend(
                                        crate::theme::Status::Epic,
                                        Some(p.clone().into()),
                                    ),
                                    None => StatusPill::of(row.status),
                                });
                            if row.enabled {
                                el = el
                                    .cursor_pointer()
                                    .hover(|s| s.bg(c.bg_hover))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |ed, _, _, cx| {
                                            cx.stop_propagation();
                                            ed.add_dependency(key.clone(), cx);
                                        }),
                                    );
                            } else {
                                el = el.opacity(0.5);
                            }
                            el
                        })),
                )
                .children((!project_names.is_empty()).then(|| {
                    div()
                        .px(px(8.))
                        .py(px(6.))
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(format!(
                            "Tickets from {} are not listed — dependencies stay inside one project.",
                            project_names.join(" and ")
                        )))
                }))
        });
        div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(5.))
            .child(Self::label("Depends on", Some("same project only"), c))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .min_h(px(32.))
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(5.))
                    .bg(c.bg_raised)
                    .border_1()
                    .border_color(if error.is_some() {
                        c.st_error
                    } else {
                        c.border_strong
                    })
                    .children(chips)
                    .child(div().flex_1().min_w(px(140.)).child(self.dep_query.clone())),
            )
            .children(error.map(|(code, message)| Self::error_box(code.as_deref(), &message, c)))
            .children(picker)
            .into_any_element()
    }

    fn base_and_external(&self, c: &Colors, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let fields = visibility(
            self.model.kind,
            self.model.epic.as_ref(),
            self.model.base_locked,
        );
        let default = self
            .workspace
            .read(cx)
            .current()
            .map(|p| p.base_branch.clone())
            .unwrap_or_default();
        let mut out = Vec::new();
        let base = self.model.base_branch.clone().unwrap_or_default();
        let base_field = match &fields.base {
            BaseField::FromEpic(epic) => div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(icon(IconName::Layers, px(13.)).text_color(c.st_investigate))
                .child(div().text_size(px(13.)).text_color(c.text_secondary).child(
                    SharedString::from(format!("From the epic’s branch ({epic})")),
                ))
                .into_any_element(),
            BaseField::Locked(_) => DropdownButton::new("editor-base", base.clone())
                .icon(IconName::GitBranch)
                .trailing(IconName::Lock)
                .mono()
                .disabled(true)
                .into_any_element(),
            BaseField::Editable => div()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|ed, e: &gpui::MouseDownEvent, window, cx| {
                        ed.open_base_menu(e.position, window, cx)
                    }),
                )
                .child(
                    DropdownButton::new("editor-base", base.clone())
                        .icon(IconName::GitBranch)
                        .mono()
                        .up_down()
                        .open(self.base_menu.is_some()),
                )
                .into_any_element(),
        };
        let note: SharedString = match &fields.base {
            BaseField::Locked(reason) => reason.clone().into(),
            BaseField::FromEpic(_) => "children always use the epic’s branch".into(),
            BaseField::Editable if base != default => {
                format!("overrides the default ({default})").into()
            }
            BaseField::Editable => {
                "project default · local and remote branches · locked once the ticket starts".into()
            }
        };
        out.push(
            div()
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(Self::label("Start from and merge into", None, c))
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(c.text_muted)
                                .child(note),
                        ),
                )
                .child(base_field)
                .children(
                    self.model
                        .base_error
                        .clone()
                        .map(|e| Self::error_box(Some("cross_base"), &e, c)),
                )
                .into_any_element(),
        );
        if fields.external_id {
            out.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(Self::label(
                        "External ID",
                        Some("optional · used in the branch and PR title"),
                        c,
                    ))
                    .child(self.external_id.clone())
                    .into_any_element(),
            );
        }
        out
    }

    fn label(text: &'static str, note: Option<&'static str>, c: &Colors) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c.text_primary)
                    .child(text),
            )
            .children(note.map(|n| div().text_size(px(11.)).text_color(c.text_muted).child(n)))
            .into_any_element()
    }

    fn criteria_list(&self, c: &Colors, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.criteria.iter().enumerate().map(|(i, input)| {
            let criterion = &self.model.criteria[i];
            let remove = cx.listener(move |ed, _, window, cx| {
                ed.model.remove_criterion(i);
                ed.rebuild_criteria(window, cx);
            });
            div()
                .id(("criterion", i))
                .group(SharedString::from(format!("criterion-{i}")))
                .flex()
                .items_center()
                .gap(px(8.))
                .py(px(2.))
                .rounded(px(4.))
                .on_drag(
                    DraggedCriterion {
                        index: i,
                        text: criterion.text.clone().into(),
                    },
                    |dragged, _, _, cx| {
                        let text = dragged.text.clone();
                        cx.new(|_| CriterionPreview(text))
                    },
                )
                .drag_over::<DraggedCriterion>(move |style, _, _, cx| {
                    style.bg(ActiveTheme::get(cx).colors.accent_soft)
                })
                .on_drop(
                    cx.listener(move |ed, dragged: &DraggedCriterion, window, cx| {
                        ed.model.move_criterion(dragged.index, i);
                        ed.rebuild_criteria(window, cx);
                    }),
                )
                .child(
                    div()
                        .cursor_grab()
                        .child(icon(IconName::GripVertical, px(12.)).text_color(c.text_muted)),
                )
                .child(
                    div()
                        .flex_none()
                        .size(px(14.))
                        .rounded(px(3.))
                        .border(px(1.5))
                        .border_color(c.border_strong),
                )
                .child(div().flex_1().child(input.clone()))
                .children(
                    criterion
                        .by_agent
                        .then(|| icon(IconName::Sparkles, px(12.)).text_color(c.st_investigate)),
                )
                .child(
                    div()
                        .id(("criterion-remove", i))
                        .invisible()
                        .group_hover(SharedString::from(format!("criterion-{i}")), |s| {
                            s.visible()
                        })
                        .cursor_pointer()
                        .p(px(2.))
                        .rounded(px(3.))
                        .hover(|s| s.bg(c.bg_hover))
                        .child(icon(IconName::X, px(12.)).text_color(c.text_muted))
                        .on_click(remove),
                )
        });
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .children(rows)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(icon(IconName::Plus, px(13.)).text_color(c.text_muted))
                    .child(div().flex_1().child(self.new_criterion.clone())),
            )
            .into_any_element()
    }

    fn form(&self, c: &Colors, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut out = Vec::new();
        out.push(
            div()
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(Self::label("Title", None, c))
                .child(self.title.clone())
                .children(
                    self.model
                        .title_error()
                        .map(|e| div().text_size(px(11.)).text_color(c.st_error).child(e)),
                )
                .into_any_element(),
        );
        if self.model.is_create() {
            let kind_index = usize::from(self.model.kind == Kind::Epic);
            let epic_label = self
                .model
                .epic
                .as_ref()
                .map_or_else(|| "No epic".to_owned(), |e| e.to_string());
            let pick_kind = cx.entity().downgrade();
            out.push(
                div()
                    .flex()
                    .gap(px(16.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(Self::label("Type", None, c))
                            .child(
                                Segmented::new(
                                    "editor-kind",
                                    vec!["Ticket".into(), "Epic".into()],
                                    kind_index,
                                )
                                .on_select(move |i, _, cx| {
                                    let _ = pick_kind.update(cx, |ed, cx| {
                                        ed.model.kind =
                                            if i == 1 { Kind::Epic } else { Kind::Ticket };
                                        if ed.model.kind == Kind::Epic {
                                            ed.model.epic = None;
                                        }
                                        cx.notify();
                                    });
                                }),
                            ),
                    )
                    .children((self.model.kind == Kind::Ticket).then(|| {
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(Self::label(
                                "Epic",
                                Some("optional · fixed once created (it sets the key)"),
                                c,
                            ))
                            .child(
                                div()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|ed, e: &gpui::MouseDownEvent, window, cx| {
                                            ed.open_epic_menu(e.position, window, cx)
                                        }),
                                    )
                                    .child(
                                        DropdownButton::new("editor-epic", epic_label).up_down(),
                                    ),
                            )
                    }))
                    .into_any_element(),
            );
        }
        out.push(
            div()
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(Self::label(
                    "What and why",
                    Some("Markdown · code blocks"),
                    c,
                ))
                .child(self.body.clone())
                .into_any_element(),
        );
        let count = self.model.criteria.len();
        let note: &'static str = match count {
            0 => "checklist the agent works towards",
            1 => "1 item",
            _ => "items",
        };
        out.push(
            div()
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(Self::label("Done when", None, c))
                        .child(div().text_size(px(11.)).text_color(c.text_muted).child(
                            if count > 1 {
                                SharedString::from(format!("{count} items"))
                            } else {
                                SharedString::from(note)
                            },
                        )),
                )
                .child(self.criteria_list(c, cx))
                .into_any_element(),
        );
        out.push(self.depends_on(c, cx));
        out.extend(self.base_and_external(c, cx));
        out
    }

    fn read_view(&self, c: &Colors, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let m = &self.model;
        let now = now_ms();
        let edits = &self.edits;
        let section = |name: &'static str,
                       field: Option<Field>,
                       note: Option<String>,
                       cx: &mut Context<Self>|
         -> AnyElement {
            let marked = field.is_some_and(|f| changed(edits, f));
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(10.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(name.to_uppercase())),
                )
                .children(note.map(|n| {
                    div()
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(n))
                }))
                .child(div().flex_1())
                .children(marked.then(|| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .px(px(6.))
                        .py(px(1.))
                        .rounded(px(4.))
                        .bg(c.st_investigate_bg)
                        .child(icon(IconName::Sparkles, px(11.)).text_color(c.st_investigate))
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(c.st_investigate)
                                .child("Updated by agent"),
                        )
                }))
                .children(marked.then(|| {
                    div()
                        .id(SharedString::from(format!("compare-{name}")))
                        .cursor_pointer()
                        .text_size(px(11.))
                        .text_color(c.accent)
                        .child("Compare")
                        .on_click(cx.listener(move |ed, _, _, cx| ed.open_compare(field, cx)))
                }))
                .into_any_element()
        };
        let mut out = Vec::new();
        if let Some(note) = &self.saved_note {
            out.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .p(px(12.))
                    .rounded(px(radius::SM))
                    .bg(c.st_working_bg)
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(icon(IconName::Info, px(14.)).text_color(c.st_working))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.))
                                    .text_color(c.text_primary)
                                    .child(SharedString::from(note.clone())),
                            ),
                    )
                    .child(
                        div().pl(px(22.)).child(
                            Button::new("saved-open-terminal", "Open terminal")
                                .icon(IconName::SquareTerminal)
                                .variant(Variant::Secondary)
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(EditorOpenTerminal))),
                        ),
                    )
                    .into_any_element(),
            );
        }
        if let Some(ticket) = &self.ticket {
            out.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        icon(
                            if ticket.created_by.is_agent() {
                                IconName::Sparkles
                            } else {
                                IconName::User
                            },
                            px(12.),
                        )
                        .text_color(if ticket.created_by.is_agent() {
                            c.st_investigate
                        } else {
                            c.text_muted
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(c.text_muted)
                            .child(SharedString::from(origin_line(ticket, edits, now))),
                    )
                    .into_any_element(),
            );
        }
        out.push(section("Title", Some(Field::Title), None, cx));
        out.push(
            div()
                .text_size(px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c.text_primary)
                .child(SharedString::from(m.title.clone()))
                .into_any_element(),
        );
        out.push(section("What and why", Some(Field::Body), None, cx));
        out.push(if m.body.trim().is_empty() {
            div()
                .text_size(px(13.))
                .text_color(c.text_muted)
                .child("Nothing written yet.")
                .into_any_element()
        } else {
            Markdown::new(m.body.clone()).into_any_element()
        });
        let note = self.ticket.as_ref().map(done_when_note);
        out.push(section("Done when", Some(Field::DoneWhen), note, cx));
        out.push(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(m.criteria.iter().map(|crit| {
                    let mut row = div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(6.))
                        .py(px(3.))
                        .rounded(px(4.))
                        .child(
                            div()
                                .flex_none()
                                .size(px(14.))
                                .rounded(px(3.))
                                .border(px(1.5))
                                .border_color(c.border_strong),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(13.))
                                .text_color(c.text_primary)
                                .child(SharedString::from(crit.text.clone())),
                        );
                    if crit.by_agent {
                        row = row
                            .bg(c.st_investigate_bg)
                            .child(icon(IconName::Sparkles, px(12.)).text_color(c.st_investigate));
                    }
                    row
                }))
                .children(m.criteria.is_empty().then(|| {
                    div()
                        .text_size(px(13.))
                        .text_color(c.st_paused)
                        .child("No “Done when” yet")
                }))
                .into_any_element(),
        );
        let line = |label: &'static str, value: String| {
            div()
                .flex()
                .gap(px(10.))
                .child(
                    div()
                        .w(px(150.))
                        .text_size(px(12.))
                        .text_color(c.text_secondary)
                        .child(label),
                )
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(12.))
                        .text_color(c.text_primary)
                        .child(SharedString::from(value)),
                )
                .into_any_element()
        };
        let deps: Vec<String> = m.deps.iter().map(ToString::to_string).collect();
        out.push(line(
            "Depends on",
            if deps.is_empty() {
                "—".into()
            } else {
                deps.join(" ")
            },
        ));
        out.push(line(
            "Start from and merge into",
            match (&m.epic, &m.base_branch) {
                (Some(epic), _) => format!("the epic’s branch ({epic})"),
                (None, Some(b)) => base_label(&Base::Branch(b.clone())),
                (None, None) => "—".into(),
            },
        ));
        if m.epic.is_none() && !m.external_id.is_empty() {
            out.push(line("External ID", m.external_id.clone()));
        }
        out
    }

    /// Opens Compare and marks the agent's edits as seen (the history stays).
    fn open_compare(&mut self, field: Option<Field>, cx: &mut Context<Self>) {
        self.compare = Some(field);
        self.seen_edits = self.edits.clone();
        if let EditorMode::Edit { key } = &self.model.mode {
            let send = self
                .workspace
                .read(cx)
                .link
                .read(cx)
                .send(Request::TicketAgentEditsSeen(KeyParams {
                    key: key.to_string(),
                }));
            cx.spawn(async move |this, cx| {
                let _ = send.await;
                let _ = this.update(cx, |ed, cx| {
                    ed.edits = AgentEdits::default();
                    cx.notify();
                });
            })
            .detach();
        }
        cx.notify();
    }

    fn compare_view(&self, field: Option<Field>, c: &Colors) -> Vec<AnyElement> {
        compare(&self.seen_edits, field, now_ms())
            .into_iter()
            .map(|entry| {
                let column = |label: &'static str, text: String, bg| {
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .p(px(12.))
                        .bg(bg)
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(10.))
                                .text_color(c.text_muted)
                                .child(label),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(c.text_primary)
                                .child(SharedString::from(text)),
                        )
                };
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.text_primary)
                            .child(SharedString::from(entry.heading)),
                    )
                    .child(
                        div()
                            .flex()
                            .rounded(px(radius::MD))
                            .overflow_hidden()
                            .border_1()
                            .border_color(c.border)
                            .child(column("BEFORE", entry.before, c.diff_del_bg))
                            .child(column("AFTER", entry.after, c.diff_add_bg)),
                    )
                    .into_any_element()
            })
            .collect()
    }
}

impl Render for TicketEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let project = self
            .workspace
            .read(cx)
            .current()
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let (heading, mut primary) = match (&self.model.mode, self.read_mode) {
            (EditorMode::Create { .. }, _) => ("New ticket".to_owned(), "Create ticket"),
            (EditorMode::Edit { key }, true) => (key.to_string(), "Edit"),
            (EditorMode::Edit { key }, false) => (format!("Edit {key}"), "Save"),
        };
        if self.compare.is_some() {
            primary = "Back";
        }
        let body: Vec<AnyElement> = match (self.compare, self.read_mode) {
            (Some(field), _) => self.compare_view(field, &c),
            (None, true) => self.read_view(&c, cx),
            (None, false) => self.form(&c, cx),
        };
        let read_mode = self.read_mode;
        let comparing = self.compare.is_some();
        // While editing a working ticket: its agent will be told what changed (SES-08).
        let save_hint = (!read_mode && !comparing)
            .then(|| {
                self.ticket
                    .as_ref()
                    .and_then(|t| save_hint(t, self.live_agent))
            })
            .flatten();
        let primary_click = cx.listener(move |ed, _, window, cx| {
            if comparing {
                ed.compare = None;
                cx.notify();
            } else if read_mode {
                ed.read_mode = false;
                ed.saved_note = None;
                let handle = ed.title.read(cx).focus_handle(cx);
                window.focus(&handle, cx);
                cx.notify();
            } else {
                ed.save(window, cx);
            }
        });
        let close = cx.listener(|_, _, _, cx| cx.emit(EditorClosed));
        let epic_menu = self
            .epic_menu
            .as_ref()
            .or(self.base_menu.as_ref())
            .map(|(menu, at)| {
                deferred(anchored().position(*at).child(menu.clone())).with_priority(2)
            });
        div()
            .id("ticket-editor")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|ed, _: &SaveTicket, window, cx| {
                if !ed.read_mode {
                    ed.save(window, cx);
                }
            }))
            .on_action(cx.listener(|_, _: &CloseEditor, _, cx| cx.emit(EditorClosed)))
            .flex()
            .flex_col()
            .overflow_hidden()
            .when(self.embedded, |d| d.size_full().bg(c.bg_panel))
            .when(!self.embedded, |d| {
                d.w(px(620.))
                    .max_h(px(760.))
                    .bg(c.bg_raised)
                    .border_1()
                    .border_color(c.border)
                    .rounded(px(radius::DIALOG))
                    .shadow(floating_shadow())
            })
            .when(!self.embedded, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .px(px(16.))
                        .py(px(14.))
                        .border_b_1()
                        .border_color(c.border)
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.text_primary)
                                .child(SharedString::from(heading)),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(c.text_muted)
                                .child("in"),
                        )
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(12.))
                                .text_color(c.text_secondary)
                                .child(SharedString::from(project)),
                        )
                        .child(div().flex_1())
                        .child(IconButton::new("editor-close", IconName::X).on_click(close)),
                )
            })
            .child(
                div()
                    .id("editor-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .p(px(16.))
                    .children(body)
                    .children(self.model.error.clone().map(|e| {
                        div()
                            .text_size(px(12.))
                            .text_color(c.st_error)
                            .child(SharedString::from(e))
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(16.))
                    .py(px(12.))
                    .bg(c.bg_panel)
                    .border_t_1()
                    .border_color(c.border)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .children((!read_mode).then(|| {
                                Kbd::text(if cfg!(target_os = "macos") {
                                    "⌘↵"
                                } else {
                                    "Ctrl+↵"
                                })
                            }))
                            .children((!read_mode).then(|| {
                                div().text_size(px(11.)).text_color(c.text_muted).child(
                                    if self.model.is_create() {
                                        "create"
                                    } else {
                                        "save"
                                    },
                                )
                            }))
                            .child(Kbd::text("Esc"))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(c.text_muted)
                                    .child(if read_mode { "close" } else { "discard" }),
                            )
                            .children(save_hint.map(|hint| {
                                div()
                                    .ml(px(8.))
                                    .text_size(px(11.))
                                    .text_color(c.st_working)
                                    .child(SharedString::from(hint))
                            })),
                    )
                    .child(
                        Button::new("editor-cancel", if read_mode { "Close" } else { "Cancel" })
                            .variant(Variant::Ghost)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(EditorClosed))),
                    )
                    .child(
                        Button::new("editor-primary", primary)
                            .icon(if read_mode {
                                IconName::Pencil
                            } else {
                                IconName::Check
                            })
                            .loading(self.model.saving)
                            .disabled(!read_mode && !self.model.can_save())
                            .on_click(primary_click),
                    ),
            )
            .children(epic_menu)
    }
}
