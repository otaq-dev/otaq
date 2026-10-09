//! Context menus and tooltips (*01 · Components → Overlays & feedback*). A menu is an entity that takes focus
//! when it opens, so ↑/↓ move, ↵ chooses and Esc closes ([`MenuModel`] holds the rules). Tooltips are always
//! dark in both themes and show the action's shortcut.

use gpui::{
    AnyView, App, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, IntoElement,
    KeyBinding, SharedString, Window, actions, div, prelude::*, px,
};

use super::floating_shadow;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_MONO, radius};
use crate::view_models::menu::{MenuItem, MenuModel};

actions!(menu, [MenuUp, MenuDown, MenuConfirm, MenuCancel]);

const CONTEXT: &str = "Menu";

pub fn bind_keys(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("up", MenuUp, c),
        KeyBinding::new("down", MenuDown, c),
        KeyBinding::new("enter", MenuConfirm, c),
        KeyBinding::new("escape", MenuCancel, c),
    ]);
}

/// The item a menu's user chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuChosen(pub SharedString);

pub struct ContextMenu {
    model: MenuModel,
    focus: FocusHandle,
    width: f32,
}

impl EventEmitter<MenuChosen> for ContextMenu {}
impl EventEmitter<DismissEvent> for ContextMenu {}

impl Focusable for ContextMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ContextMenu {
    /// A menu that takes focus at once. The owner subscribes to [`MenuChosen`] and [`DismissEvent`].
    pub fn new(items: Vec<MenuItem>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        cx.on_blur(&focus, window, |_, _, cx| cx.emit(DismissEvent))
            .detach();
        Self {
            model: MenuModel::new(items),
            focus,
            width: 260.,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    fn choose(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        if let Some(id) = self.model.activate(index) {
            cx.emit(MenuChosen(id));
            cx.emit(DismissEvent);
        }
    }
}

impl Render for ContextMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let highlighted = self.model.highlighted;
        let rows = self
            .model
            .items
            .clone()
            .into_iter()
            .enumerate()
            .map(|(i, item)| {
                let fg = if item.danger {
                    c.st_error
                } else {
                    c.text_primary
                };
                let icon_fg = if item.danger {
                    c.st_error
                } else {
                    c.text_secondary
                };
                let mut row = div()
                    .id(("menu-item", i))
                    .flex()
                    .flex_col()
                    .w_full()
                    .gap(px(1.))
                    .px(px(8.))
                    .py(px(if item.hint.is_some() { 6. } else { 0. }))
                    .min_h(px(28.))
                    .justify_center()
                    .rounded(px(5.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .children(item.icon.map(|ic| icon(ic, px(14.)).text_color(icon_fg)))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.))
                                    .text_color(fg)
                                    .child(SharedString::from(item.label.clone())),
                            )
                            .children(
                                item.checked
                                    .then(|| icon(IconName::Check, px(13.)).text_color(c.accent)),
                            )
                            .children(item.shortcut.clone().map(|k| {
                                div()
                                    .font_family(FONT_MONO)
                                    .text_size(px(11.))
                                    .text_color(c.text_muted)
                                    .child(SharedString::from(k))
                            })),
                    )
                    .children(item.hint.clone().map(|h| {
                        div()
                            .pl(px(if item.icon.is_some() { 22. } else { 0. }))
                            .text_size(px(10.5))
                            .line_height(gpui::relative(1.35))
                            .text_color(c.text_muted)
                            .child(SharedString::from(h))
                    }));
                if item.disabled {
                    row = row.opacity(0.55);
                } else {
                    row = row
                        .cursor_pointer()
                        .on_mouse_move(cx.listener(move |menu, _, _, cx| {
                            if menu.model.highlighted != Some(i) {
                                menu.model.hover(i);
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |menu, _, _, cx| menu.choose(Some(i), cx)));
                }
                if highlighted == Some(i) {
                    row = row.bg(c.accent_soft);
                }
                let separator = item
                    .separated
                    .then(|| div().my(px(4.)).h(px(1.)).w_full().bg(c.border));
                div().flex().flex_col().children(separator).child(row)
            });
        div()
            .id("context-menu")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|menu, _: &MenuUp, _, cx| {
                menu.model.previous();
                cx.notify();
            }))
            .on_action(cx.listener(|menu, _: &MenuDown, _, cx| {
                menu.model.next();
                cx.notify();
            }))
            .on_action(cx.listener(|menu, _: &MenuConfirm, _, cx| menu.choose(None, cx)))
            .on_action(cx.listener(|_, _: &MenuCancel, _, cx| cx.emit(DismissEvent)))
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
            .w(px(self.width))
            .flex()
            .flex_col()
            .p(px(4.))
            .bg(c.bg_raised)
            .border_1()
            .border_color(c.border)
            .rounded(px(radius::LG))
            .shadow(floating_shadow())
            .children(rows)
    }
}

/// A dark tooltip with an optional shortcut. Use with `.tooltip(Tooltip::text(...))`.
pub struct Tooltip {
    text: SharedString,
    keys: Option<SharedString>,
}

impl Tooltip {
    pub fn new(text: impl Into<SharedString>, keys: Option<String>) -> Self {
        Self {
            text: text.into(),
            keys: keys.map(Into::into),
        }
    }

    /// A tooltip builder for `.tooltip(...)`.
    pub fn text(
        text: impl Into<SharedString>,
        keys: Option<String>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let text = text.into();
        move |_, cx| cx.new(|_| Tooltip::new(text.clone(), keys.clone())).into()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(9.))
            .py(px(6.))
            .max_w(px(360.))
            .rounded(px(5.))
            .bg(gpui::rgb(0x1A1B1E))
            .shadow(floating_shadow())
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(gpui::rgb(0xF2F3F5))
                    .child(self.text.clone()),
            )
            .children(self.keys.clone().map(|k| {
                div()
                    .px(px(4.))
                    .rounded(px(3.))
                    .bg(gpui::rgba(0xFFFFFF1F))
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .text_color(gpui::rgb(0xC9CCD2))
                    .child(k)
            }))
    }
}
