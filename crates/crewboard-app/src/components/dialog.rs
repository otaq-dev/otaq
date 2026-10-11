//! The confirm dialog (*01 · Components → Overlays & feedback → Confirm dialog*): title, body, a footer with the
//! `esc cancel` hint, Cancel and the confirm button. Esc cancels; ↵ presses the default button, which is Cancel
//! for destructive dialogs ([`crate::view_models::dialog`]). The owner keeps the dialog's focus handle and
//! focuses it when the dialog opens.
//!
//! Every modal sits on a [`backdrop`]: the dimmed layer takes all mouse events (clicks, hover, scrolling, drag
//! and drop), so nothing behind a dialog reacts while it is open. A confirm dialog covers the whole window
//! wherever its owner renders it (the board, the side panel…), and marks its window in [`OpenDialogs`] so the
//! root holds back the app's shortcuts while it is open.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Div, ElementId, FocusHandle, Global, IntoElement, KeyBinding, RenderOnce,
    SharedString, Stateful, Window, WindowId, actions, anchored, deferred, div, point, prelude::*,
    px,
};

use super::button::{Button, Variant};
use super::floating_shadow;
use super::kbd::Kbd;
use crate::icons::IconName;
use crate::theme::{ActiveTheme, radius, text};
use crate::ui::TextStyleExt;
use crate::view_models::dialog::{DialogKey, DialogKind, DialogOutcome, default_outcome, on_key};

actions!(dialog, [DialogEnter, DialogEscape]);

const CONTEXT: &str = "Dialog";

/// Paint order of a confirm dialog among the deferred layers: over popovers (1) and menus (2).
pub const DIALOG_PRIORITY: usize = 3;

/// The windows that drew a confirm dialog in their current frame. Dialogs live in many views (board, side
/// panel, settings…); this lets the root know one is open without asking each of them. The root clears its
/// window's mark at the start of every frame, before any child view renders.
#[derive(Default)]
pub struct OpenDialogs(HashSet<WindowId>);

impl Global for OpenDialogs {}

impl OpenDialogs {
    pub fn begin_frame(window: &Window, cx: &mut App) {
        let id = window.window_handle().window_id();
        cx.default_global::<Self>().0.remove(&id);
    }

    fn mark(window: &Window, cx: &mut App) {
        let id = window.window_handle().window_id();
        cx.default_global::<Self>().0.insert(id);
    }

    pub fn any_in(window: &Window, cx: &App) -> bool {
        let id = window.window_handle().window_id();
        cx.try_global::<Self>().is_some_and(|d| d.0.contains(&id))
    }
}

/// The dim over the page behind a modal.
fn scrim(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .justify_center()
        .items_center()
        .bg(gpui::rgba(0x00000033))
        // Without this the hit test goes on to whatever is behind: cards would still drag, hover and take
        // clicks made inside the dialog.
        .occlude()
}

/// The backdrop of a modal that is a direct child of the window's root: fills it, dims it and blocks the
/// mouse for everything behind it. The modal is its child, centred.
pub fn backdrop(id: impl Into<ElementId>) -> Stateful<Div> {
    scrim(id).absolute().top_0().left_0().size_full()
}

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", DialogEnter, Some(CONTEXT)),
        KeyBinding::new("escape", DialogEscape, Some(CONTEXT)),
    ]);
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct ConfirmDialog {
    id: SharedString,
    focus: FocusHandle,
    kind: DialogKind,
    title: SharedString,
    body: Vec<AnyElement>,
    confirm_label: SharedString,
    confirm_icon: Option<IconName>,
    confirm_disabled: bool,
    width: f32,
    on_confirm: Handler,
    on_cancel: Handler,
    /// A second action in place of the Cancel button (Esc still cancels): "Start anyway".
    secondary: Option<(SharedString, Handler)>,
    /// The word after the `esc` hint: "cancel", or "close" for a dialog that only shows something.
    hint: SharedString,
    /// The body scrolls when the dialog would be taller than the window (Theme credits).
    fit_window: bool,
}

impl ConfirmDialog {
    pub fn new(
        id: impl Into<SharedString>,
        focus: &FocusHandle,
        kind: DialogKind,
        title: impl Into<SharedString>,
        confirm_label: impl Into<SharedString>,
        on_confirm: impl Fn(&mut Window, &mut App) + 'static,
        on_cancel: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            focus: focus.clone(),
            kind,
            title: title.into(),
            body: Vec::new(),
            confirm_label: confirm_label.into(),
            confirm_icon: None,
            confirm_disabled: false,
            width: 440.,
            on_confirm: Rc::new(on_confirm),
            on_cancel: Rc::new(on_cancel),
            secondary: None,
            hint: "cancel".into(),
            fit_window: false,
        }
    }

    /// The word after the footer's `esc` hint (default "cancel").
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Keeps the dialog inside the window: the body scrolls instead.
    pub fn fit_window(mut self) -> Self {
        self.fit_window = true;
        self
    }

    /// Replaces the Cancel button with another action; Esc and a click outside still cancel.
    pub fn secondary(
        mut self,
        label: impl Into<SharedString>,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.secondary = Some((label.into(), Rc::new(on_click)));
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.body.push(child.into_any_element());
        self
    }

    /// Several body elements at once.
    pub fn children_boxes(mut self, children: Vec<AnyElement>) -> Self {
        self.body.extend(children);
        self
    }

    /// A body paragraph in the secondary colour.
    pub fn text(self, text: impl Into<SharedString>) -> Self {
        let text = text.into();
        self.child(Paragraph(text))
    }

    pub fn confirm_icon(mut self, icon: IconName) -> Self {
        self.confirm_icon = Some(icon);
        self
    }

    pub fn confirm_disabled(mut self, disabled: bool) -> Self {
        self.confirm_disabled = disabled;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

#[derive(IntoElement)]
struct Paragraph(SharedString);

impl RenderOnce for Paragraph {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        div()
            .text_style_of(text::BODY)
            .text_color(c.text_secondary)
            .child(self.0)
    }
}

impl RenderOnce for ConfirmDialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        OpenDialogs::mark(window, cx);
        let c = ActiveTheme::get(cx).colors;
        let kind = self.kind;
        let disabled = self.confirm_disabled;
        let run = {
            let confirm = self.on_confirm.clone();
            let cancel = self.on_cancel.clone();
            move |outcome: DialogOutcome, window: &mut Window, cx: &mut App| match outcome {
                DialogOutcome::Confirm if !disabled => confirm(window, cx),
                DialogOutcome::Confirm => {}
                DialogOutcome::Cancel => cancel(window, cx),
            }
        };
        let run = Rc::new(run);
        let (enter, escape, backdrop, cancel_click, confirm_click) =
            (run.clone(), run.clone(), run.clone(), run.clone(), run);
        let default = default_outcome(kind);
        let confirm_variant = match kind {
            DialogKind::Primary => Variant::Primary,
            DialogKind::Destructive => Variant::Destructive,
        };
        let mut confirm = Button::new(
            SharedString::from(format!("{}-confirm", self.id)),
            self.confirm_label,
        )
        .variant(confirm_variant)
        .disabled(disabled)
        .on_click(move |_, window, cx| confirm_click(DialogOutcome::Confirm, window, cx));
        if let Some(i) = self.confirm_icon {
            confirm = confirm.icon(i);
        }
        let mut cancel = Button::new(SharedString::from(format!("{}-cancel", self.id)), "Cancel")
            .variant(Variant::Ghost)
            .on_click(move |_, window, cx| cancel_click(DialogOutcome::Cancel, window, cx));
        if default == DialogOutcome::Cancel {
            cancel = cancel.variant(Variant::Secondary);
        }
        if let Some((label, handler)) = self.secondary.clone() {
            cancel = Button::new(SharedString::from(format!("{}-secondary", self.id)), label)
                .variant(Variant::Secondary)
                .on_click(move |_, window, cx| handler(window, cx));
        }
        // Drawn after everything else, at the window's origin and size, so it covers the whole window even
        // when its owner is only a part of it.
        let viewport = window.viewport_size();
        let body_id = SharedString::from(format!("{}-body", self.id));
        let body = div().flex().flex_col().gap(px(12.)).children(self.body);
        // Title, paddings and the footer take about 140 px; leave a margin around the dialog.
        let body = if self.fit_window {
            body.id(body_id)
                .max_h((viewport.height - px(200.)).max(px(120.)))
                .overflow_y_scroll()
                .into_any_element()
        } else {
            body.into_any_element()
        };
        let layer = scrim(SharedString::from(format!("{}-backdrop", self.id)))
            .w(viewport.width)
            .h(viewport.height)
            .on_click(move |_, window, cx| backdrop(DialogOutcome::Cancel, window, cx))
            .child(
                div()
                    .id(self.id.clone())
                    .key_context(CONTEXT)
                    .track_focus(&self.focus)
                    .on_action(move |_: &DialogEnter, window, cx| {
                        enter(on_key(kind, DialogKey::Enter), window, cx)
                    })
                    .on_action(move |_: &DialogEscape, window, cx| {
                        escape(on_key(kind, DialogKey::Escape), window, cx)
                    })
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .w(px(self.width))
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
                            .flex_col()
                            .gap(px(12.))
                            .pt(px(20.))
                            .px(px(20.))
                            .pb(px(16.))
                            .child(
                                div()
                                    .text_style_of(text::DISPLAY)
                                    .text_size(px(16.))
                                    .text_color(c.text_primary)
                                    .child(self.title),
                            )
                            .child(body),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .px(px(20.))
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
                                    .child(Kbd::text("esc"))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(c.text_muted)
                                            .child(self.hint),
                                    ),
                            )
                            .child(cancel)
                            .child(confirm),
                    ),
            );
        deferred(anchored().position(point(px(0.), px(0.))).child(layer))
            .with_priority(DIALOG_PRIORITY)
    }
}
