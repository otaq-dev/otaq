//! The header's search and filter field (UI-02, EPC-01, d0058 ⌘K; *03 · Board & columns → Search & filter ·
//! ⌘K*). ⌘K focuses it; typing filters the board live; a finished token (`is:needs-me `) becomes a removable
//! chip; the list under the field suggests filters with their counts; Esc clears the filter and leaves.

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, FontWeight, IntoElement, SharedString, Window,
    div, prelude::*, px,
};

use crate::components::floating_shadow;
use crate::components::kbd::Kbd;
use crate::components::text_input::{InputEvent, TextInput};
use crate::icons::{IconName, icon};
use crate::keymap::{KeyAction, Platform};
use crate::theme::{ActiveTheme, FONT_MONO, radius};
use crate::view_models::filter::Token;
use crate::workspace::Workspace;

/// The field fills the header between the project and the right-hand items, up to 480 px (the 1920 frames);
/// 289 px at 1440 with every header item. It gives up its width before the drag areas beside it do, and stops
/// at 149 px, its width in *02 · App shell → 1280 × 800 · minimum*.
pub const MAX_WIDTH: f32 = 480.;
pub const MIN_WIDTH: f32 = 149.;
/// The field's shrink weight against the drag areas' 1: high enough that they keep their 16 px until the
/// field reaches `MIN_WIDTH`.
const SHRINK: f32 = 1000.;

pub struct SearchField {
    workspace: Entity<Workspace>,
    input: Entity<TextInput>,
    /// Where focus goes when Esc leaves the field.
    leave_to: Option<FocusHandle>,
}

impl SearchField {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Search  ·  is:needs-me")
                .bare()
        });
        cx.subscribe_in(
            &input,
            window,
            |field, input, event: &InputEvent, window, cx| {
                match event {
                    InputEvent::Changed => {
                        let typed = input.read(cx).text().to_owned();
                        let rest = field.workspace.update(cx, |ws, cx| {
                            let rest = ws.filter.take_input(&typed);
                            cx.notify();
                            rest
                        });
                        if rest != typed {
                            input.update(cx, |i, cx| i.set_text(&rest, cx));
                        }
                    }
                    InputEvent::Cancel => {
                        field.workspace.update(cx, |ws, cx| {
                            ws.filter.clear();
                            cx.notify();
                        });
                        input.update(cx, |i, cx| i.set_text("", cx));
                        if let Some(to) = &field.leave_to {
                            window.focus(to, cx);
                        } else {
                            window.blur(cx);
                        }
                    }
                    InputEvent::Submit => {
                        // ⌘K "cleanup" (*16 → Where Cleanup lives*).
                        if input.read(cx).text().trim().eq_ignore_ascii_case("cleanup") {
                            input.update(cx, |i, cx| i.set_text("", cx));
                            field.workspace.update(cx, |ws, cx| {
                                ws.filter.clear();
                                cx.notify();
                            });
                            cx.emit(crate::shell::ShellRequest::OpenCleanup);
                        } else {
                            field.add_first_suggestion(cx)
                        }
                    }
                    InputEvent::Blur
                    | InputEvent::ArrowUp
                    | InputEvent::ArrowDown
                    | InputEvent::AltSubmit => {}
                }
                cx.notify();
            },
        )
        .detach();
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        Self {
            workspace,
            input,
            leave_to: None,
        }
    }

    /// ⌘K: focus the field; Esc later returns focus to `back`.
    pub fn focus(&mut self, back: FocusHandle, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_to = Some(back);
        let handle = self.input.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    }

    fn add_token(&mut self, token: Token, cx: &mut Context<Self>) {
        self.workspace.update(cx, |ws, cx| {
            ws.filter.add(token);
            ws.filter.text.clear();
            cx.notify();
        });
        self.input.update(cx, |i, cx| i.set_text("", cx));
    }

    /// ↵ with a half-typed token picks the first matching suggestion.
    fn add_first_suggestion(&mut self, cx: &mut Context<Self>) {
        let ws = self.workspace.read(cx);
        if !ws.filter.text.contains(':') {
            return;
        }
        let first = ws
            .filter
            .suggestions(&ws.tickets, ws.selected.as_ref())
            .into_iter()
            .next();
        if let Some((token, _, _)) = first {
            self.add_token(token, cx);
        }
    }
}

impl gpui::EventEmitter<crate::shell::ShellRequest> for SearchField {}

impl Render for SearchField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        let ws = self.workspace.read(cx);
        let active = ws.filter.is_active();
        let results = ws.filter.visible(&ws.tickets).map(|v| v.len());
        let chips: Vec<_> = ws
            .filter
            .tokens
            .iter()
            .enumerate()
            .map(|(i, token)| {
                let remove = cx.listener(move |field, _, _, cx| {
                    field.workspace.update(cx, |ws, cx| {
                        ws.filter.remove(i);
                        cx.notify();
                    });
                });
                div()
                    .id(("search-chip", i))
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(3.))
                    .px(px(5.))
                    .rounded(px(4.))
                    .bg(c.st_attn_bg)
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .text_color(c.st_attn)
                    .child(SharedString::from(token.label()))
                    .child(
                        div()
                            .id(("search-chip-x", i))
                            .cursor_pointer()
                            .child(icon(IconName::X, px(10.)).text_color(c.st_attn))
                            .on_click(remove),
                    )
            })
            .collect();
        let suggestions = focused.then(|| ws.filter.suggestions(&ws.tickets, ws.selected.as_ref()));
        let platform = Platform::current();
        let tail = match results {
            Some(n) if active => div()
                .flex_none()
                .text_size(px(11.))
                .text_color(c.text_muted)
                .child(SharedString::from(format!(
                    "{n} result{}",
                    if n == 1 { "" } else { "s" }
                )))
                .into_any_element(),
            _ => div()
                .flex_none()
                .children(Kbd::for_action(KeyAction::Search, platform))
                .into_any_element(),
        };
        let list =
            suggestions.filter(|s| !s.is_empty()).map(|items| {
                div()
                    .absolute()
                    .top(px(32.))
                    .left_0()
                    .w(px(360.))
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
                            .child("FILTERS"),
                    )
                    .children(items.into_iter().enumerate().map(
                        |(i, (token, description, count))| {
                            let label = token.label();
                            div()
                                .id(("search-suggestion", i))
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .h(px(28.))
                                .px(px(8.))
                                .rounded(px(5.))
                                .cursor_pointer()
                                .hover(|s| s.bg(c.bg_hover))
                                .child(
                                    div()
                                        .font_family(FONT_MONO)
                                        .text_size(px(12.))
                                        .text_color(c.text_primary)
                                        .child(SharedString::from(label)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .truncate()
                                        .text_size(px(11.))
                                        .text_color(c.text_muted)
                                        .child(SharedString::from(description)),
                                )
                                .child(
                                    div()
                                        .font_family(FONT_MONO)
                                        .text_size(px(11.))
                                        .text_color(c.text_muted)
                                        .child(SharedString::from(count.to_string())),
                                )
                                // Mouse down, so the click lands before the input loses focus.
                                .on_mouse_down(
                                    gpui::MouseButton::Left,
                                    cx.listener(move |field, _, _, cx| {
                                        cx.stop_propagation();
                                        field.add_token(token.clone(), cx);
                                    }),
                                )
                        },
                    ))
            });
        div()
            .relative()
            .w(px(MAX_WIDTH))
            .min_w(px(MIN_WIDTH))
            .flex_grow_0()
            .flex_shrink(SHRINK)
            .child(
                div()
                    .id("search-field")
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .w_full()
                    .h(px(28.))
                    .pl(px(10.))
                    .pr(px(6.))
                    .rounded(px(radius::MD))
                    .bg(c.bg_column)
                    .border(if focused { px(1.5) } else { px(1.) })
                    .border_color(if focused { c.focus } else { c.border })
                    .child(icon(IconName::Search, px(13.)).text_color(c.text_muted))
                    .children(chips)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .font_weight(FontWeight::NORMAL)
                            .child(self.input.clone()),
                    )
                    .child(tail),
            )
            .children(list)
    }
}

impl Focusable for SearchField {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}
