//! The ticket editor (UI-05, TKT-03; d0019, d0058; *08 · Ticket editor → Full form · filled, Validation · empty
//! form, Dark · full form*): a modal with Title, Type and Epic (when creating), What and why (Markdown, code
//! blocks kept intact), and the "Done when" checklist (add with ↵, drag to reorder, remove). An existing ticket
//! opens in read mode first; Edit switches to the form. ⌘↵ saves, Esc discards. Built as a view of its own so
//! the panel's Ticket tab can host it later (T-0062).

use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Kind, TicketKey};
use crewboard_core::protocol::{Reply, Request, TicketUpdateParams};
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
use crate::components::tabs::Segmented;
use crate::components::text_input::{InputEvent, TextInput};
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, Colors, FONT_MONO, radius};
use crate::view_models::editor::{EditorMode, EditorModel};
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
}

impl EventEmitter<EditorClosed> for TicketEditor {}

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
        let project = workspace
            .read(cx)
            .current()
            .map(|p| p.key.clone())
            .unwrap_or_default();
        let mut model = EditorModel::create(&project, title);
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
        let editor = Self::with_model(
            workspace,
            EditorModel::edit(ticket),
            !at_criteria,
            window,
            cx,
        );
        let handle = if at_criteria {
            editor.new_criterion.read(cx).focus_handle(cx)
        } else {
            editor.focus.clone()
        };
        window.focus(&handle, cx);
        editor
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
                        if creating {
                            cx.emit(EditorClosed);
                        } else {
                            ed.model = EditorModel::edit(&ticket);
                            ed.read_mode = true;
                            ed.rebuild_criteria(window, cx);
                            window.focus(&ed.focus, cx);
                        }
                    }
                    Ok(other) => ed.model.error = Some(format!("unexpected answer: {other:?}")),
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
                                        DropdownButton::new("editor-epic", epic_label)
                                            .icon(IconName::Layers),
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
        out
    }

    fn read_view(&self, c: &Colors) -> Vec<AnyElement> {
        let m = &self.model;
        let mut out = vec![
            div()
                .text_size(px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c.text_primary)
                .child(SharedString::from(m.title.clone()))
                .into_any_element(),
        ];
        out.push(Self::label("What and why", None, c));
        out.push(if m.body.trim().is_empty() {
            div()
                .text_size(px(13.))
                .text_color(c.text_muted)
                .child("Nothing written yet.")
                .into_any_element()
        } else {
            Markdown::new(m.body.clone()).into_any_element()
        });
        out.push(Self::label("Done when", None, c));
        out.push(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(m.criteria.iter().map(|crit| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
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
                                .text_size(px(13.))
                                .text_color(c.text_primary)
                                .child(SharedString::from(crit.text.clone())),
                        )
                }))
                .children(m.criteria.is_empty().then(|| {
                    div()
                        .text_size(px(13.))
                        .text_color(c.st_paused)
                        .child("No “Done when” yet")
                }))
                .into_any_element(),
        );
        out
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
        let (heading, primary) = match (&self.model.mode, self.read_mode) {
            (EditorMode::Create { .. }, _) => ("New ticket".to_owned(), "Create ticket"),
            (EditorMode::Edit { key }, true) => (key.to_string(), "Edit"),
            (EditorMode::Edit { key }, false) => (format!("Edit {key}"), "Save"),
        };
        let body: Vec<AnyElement> = if self.read_mode {
            self.read_view(&c)
        } else {
            self.form(&c, cx)
        };
        let read_mode = self.read_mode;
        let primary_click = cx.listener(move |ed, _, window, cx| {
            if read_mode {
                ed.read_mode = false;
                let handle = ed.title.read(cx).focus_handle(cx);
                window.focus(&handle, cx);
                cx.notify();
            } else {
                ed.save(window, cx);
            }
        });
        let close = cx.listener(|_, _, _, cx| cx.emit(EditorClosed));
        let epic_menu = self.epic_menu.as_ref().map(|(menu, at)| {
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
            .w(px(620.))
            .max_h(px(760.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(c.bg_raised)
            .border_1()
            .border_color(c.border)
            .rounded(px(radius::DIALOG))
            .shadow(floating_shadow())
            .child(
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
                            ),
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
