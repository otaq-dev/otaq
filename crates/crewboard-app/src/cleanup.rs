//! The Cleanup window (REC-04; d0018; *16 → Cleanup · 3 items, Cleanup · 20 items, Cleanup · empty, Delete ·
//! confirm, Success, Dark · cleanup*): leftovers no ticket owns, grouped, with checkboxes and their path, size and
//! age; nothing is checked at first; Delete selected… asks with every item named, and only Confirm deletes. The
//! logic is [`crate::view_models::cleanup`].

use crewboard_core::protocol::{CleanupDeleteParams, Reply, Request};
use crewboard_core::recovery::cleanup::CleanupGroup;
use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, KeyBinding,
    SharedString, Window, actions, div, prelude::*, px,
};

use crate::components::button::{Button, IconButton, Variant};
use crate::components::dialog::ConfirmDialog;
use crate::components::floating_shadow;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, Colors, FONT_MONO};
use crate::view_models::cleanup::{CleanupModel, CleanupRow, success};
use crate::view_models::dialog::DialogKind;
use crate::workspace::Workspace;

actions!(cleanup, [CloseCleanup]);

const CONTEXT: &str = "Cleanup";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseCleanup, Some(CONTEXT))]);
}

/// What the window tells its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupEvent {
    Closed,
    /// Deleted: the success toast's title and text.
    Done(String, Option<String>),
}

pub struct CleanupView {
    workspace: Entity<Workspace>,
    model: CleanupModel,
    loaded: bool,
    confirming: bool,
    deleting: bool,
    error: Option<String>,
    focus: FocusHandle,
    dialog_focus: FocusHandle,
}

impl EventEmitter<CleanupEvent> for CleanupView {}

impl Focusable for CleanupView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CleanupView {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let view = Self {
            workspace,
            model: CleanupModel::default(),
            loaded: false,
            confirming: false,
            deleting: false,
            error: None,
            focus,
            dialog_focus: cx.focus_handle(),
        };
        view.scan(cx);
        view
    }

    fn scan(&self, cx: &mut Context<Self>) {
        let request = self
            .workspace
            .read(cx)
            .link
            .read(cx)
            .send(Request::CleanupScan);
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |view, cx| {
                match result {
                    Ok(Reply::CleanupScan(items)) => {
                        let n = items.len();
                        view.model.set_items(items);
                        view.workspace
                            .update(cx, |ws, cx| ws.set_cleanup_count(n, cx));
                    }
                    Ok(_) => {}
                    Err(e) => view.error = Some(e.message),
                }
                view.loaded = true;
                cx.notify();
            });
        })
        .detach();
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        let targets = self.model.selected();
        if targets.is_empty() || self.deleting {
            return;
        }
        self.deleting = true;
        let bytes = self.model.selected_bytes();
        let request = self
            .workspace
            .read(cx)
            .link
            .read(cx)
            .send(Request::CleanupDelete(CleanupDeleteParams {
                items: targets,
                confirm: true,
            }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |view, cx| {
                view.deleting = false;
                view.confirming = false;
                match result {
                    Ok(Reply::CleanupOutcome(outcome)) => {
                        let left = view.model.len().saturating_sub(outcome.deleted.len());
                        let (title, text) = success(&outcome, bytes, left);
                        cx.emit(CleanupEvent::Done(title, text));
                        view.scan(cx);
                    }
                    Ok(_) => {}
                    Err(e) => view.error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn row(&self, row: &CleanupRow, c: &Colors, cx: &mut Context<Self>) -> AnyElement {
        let target = row.target.clone();
        div()
            .id(SharedString::from(format!(
                "cleanup-{:?}-{}",
                row.target.group, row.target.id
            )))
            .flex()
            .items_center()
            .gap(px(10.))
            .pl(px(30.))
            .pr(px(8.))
            .py(px(7.))
            .rounded(px(6.))
            .cursor_pointer()
            .when(row.checked, |d| d.bg(c.accent_soft))
            .when(!row.checked, |d| d.hover(|s| s.bg(c.bg_hover)))
            .on_click(cx.listener(move |view, _, _, cx| {
                view.model.toggle(&target);
                cx.notify();
            }))
            .child(checkbox(row.checked, c))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .truncate()
                            .font_family(FONT_MONO)
                            .text_size(px(12.))
                            .text_color(c.text_primary)
                            .child(SharedString::from(row.title.clone())),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(if row.warning {
                                c.st_paused
                            } else {
                                c.text_muted
                            })
                            .child(SharedString::from(row.detail.clone())),
                    ),
            )
            .child(
                div()
                    .w(px(64.))
                    .flex_none()
                    .text_right()
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(row.size.clone())),
            )
            .child(
                div()
                    .w(px(44.))
                    .flex_none()
                    .text_right()
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .child(SharedString::from(row.age.clone())),
            )
            .into_any_element()
    }

    fn confirm_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.confirming {
            return None;
        }
        let confirm = self.model.confirm()?;
        let c = ActiveTheme::get(cx).colors;
        let this = cx.entity().downgrade();
        let cancel = this.clone();
        let lines = confirm.lines.iter().map(|line| {
            div()
                .font_family(FONT_MONO)
                .text_size(px(12.))
                .text_color(c.text_primary)
                .child(SharedString::from(line.clone()))
        });
        Some(
            ConfirmDialog::new(
                "cleanup-delete",
                &self.dialog_focus,
                DialogKind::Destructive,
                confirm.title.clone(),
                confirm.confirm.clone(),
                move |_, cx| {
                    let _ = this.update(cx, |view, cx| view.delete(cx));
                },
                move |_, cx| {
                    let _ = cancel.update(cx, |view, cx| {
                        view.confirming = false;
                        cx.notify();
                    });
                },
            )
            .confirm_icon(IconName::Trash2)
            .confirm_disabled(self.deleting)
            .width(520.)
            .text(confirm.text.clone())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .p(px(10.))
                    .rounded(px(6.))
                    .bg(c.st_error_bg)
                    .children(lines),
            )
            .into_any_element(),
        )
    }
}

fn checkbox(checked: bool, c: &Colors) -> gpui::Div {
    div()
        .flex_none()
        .size(px(14.))
        .rounded(px(3.))
        .border_1()
        .border_color(if checked { c.accent } else { c.border_strong })
        .when(checked, |d| d.bg(c.accent))
        .flex()
        .justify_center()
        .items_center()
        .children(checked.then(|| icon(IconName::Check, px(10.)).text_color(c.text_on_accent)))
}

impl Render for CleanupView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The confirmation has the keyboard while it asks; the window gets it back afterwards.
        if self.confirming && !self.dialog_focus.is_focused(window) {
            window.focus(&self.dialog_focus, cx);
        } else if !self.confirming && self.dialog_focus.is_focused(window) {
            window.focus(&self.focus, cx);
        }
        let c = ActiveTheme::get(cx).colors;
        let n = self.model.len();
        let mut body: Vec<AnyElement> = Vec::new();
        for group in self.model.groups() {
            let g = group.group;
            body.push(
                div()
                    .id(SharedString::from(format!("cleanup-group-{g:?}")))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .pt(px(10.))
                    .px(px(8.))
                    .pb(px(4.))
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.model.toggle_group(g);
                        cx.notify();
                    }))
                    .child(icon(IconName::ChevronDown, px(12.)).text_color(c.text_muted))
                    .child(checkbox(group.all_checked, &c))
                    .child(icon(group.icon, px(13.)).text_color(
                        if g == CleanupGroup::KeptWorktrees {
                            c.st_paused
                        } else {
                            c.text_secondary
                        },
                    ))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.text_primary)
                            .child(group.title),
                    )
                    .child(
                        div()
                            .font_family(FONT_MONO)
                            .text_size(px(11.))
                            .text_color(c.text_muted)
                            .child(SharedString::from(group.rows.len().to_string())),
                    )
                    .into_any_element(),
            );
            for row in &group.rows {
                body.push(self.row(row, &c, cx));
            }
        }
        let empty = (self.loaded && self.model.is_empty()).then(|| {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .child(
                    div()
                        .size(px(44.))
                        .rounded_full()
                        .bg(c.st_approved_bg)
                        .flex()
                        .justify_center()
                        .items_center()
                        .child(icon(IconName::Check, px(20.)).text_color(c.st_approved)),
                )
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.text_primary)
                        .child("Nothing to clean up"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(c.text_secondary)
                        .child("Every session, worktree, branch and port belongs to a ticket."),
                )
        });
        let footer_text = self.model.footer();
        let can_delete = footer_text.is_some() && !self.deleting;
        let dialog = self.confirm_dialog(cx);
        div()
            .id("cleanup-window")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &CloseCleanup, _, cx| cx.emit(CleanupEvent::Closed)))
            .relative()
            .w(px(760.))
            .h(px(620.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(c.bg_panel)
            .border_1()
            .border_color(c.border_strong)
            .rounded(px(10.))
            .shadow(floating_shadow())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .pt(px(18.))
                    .px(px(20.))
                    .pb(px(14.))
                    .border_b_1()
                    .border_color(c.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(icon(IconName::BrushCleaning, px(16.)).text_color(c.text_secondary))
                            .child(
                                div()
                                    .text_size(px(17.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c.text_primary)
                                    .child("Cleanup"),
                            )
                            .child(
                                div()
                                    .font_family(FONT_MONO)
                                    .text_size(px(12.))
                                    .text_color(c.text_muted)
                                    .child(SharedString::from(format!(
                                        "{n} {}",
                                        if n == 1 { "item" } else { "items" }
                                    ))),
                            )
                            .child(div().flex_1())
                            .child(IconButton::new("cleanup-close", IconName::X).on_click(
                                cx.listener(|_, _, _, cx| cx.emit(CleanupEvent::Closed)),
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(c.text_secondary)
                            .child(
                                "Leftovers that no ticket owns. Review and select — nothing is deleted automatically.",
                            ),
                    ),
            )
            .child(
                div()
                    .id("cleanup-body")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .py(px(8.))
                    .px(px(12.))
                    .overflow_y_scroll()
                    .children(body)
                    .children(empty)
                    .children(self.error.clone().map(|e| {
                        div()
                            .p(px(8.))
                            .text_size(px(12.))
                            .text_color(c.st_error)
                            .child(SharedString::from(e))
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(20.))
                    .py(px(12.))
                    .border_t_1()
                    .border_color(c.border)
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .text_color(c.text_muted)
                            .child(SharedString::from(
                                footer_text.unwrap_or_else(|| "Nothing selected".to_owned()),
                            )),
                    )
                    .child(
                        Button::new("cleanup-delete-selected", "Delete selected…")
                            .icon(IconName::Trash2)
                            .variant(Variant::Destructive)
                            .disabled(!can_delete)
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.model.footer().is_some() {
                                    view.confirming = true;
                                    window.focus(&view.dialog_focus, cx);
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .children(dialog)
    }
}
