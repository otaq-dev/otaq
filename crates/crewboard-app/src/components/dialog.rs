//! The confirm dialog (*01 · Components → Overlays & feedback → Confirm dialog*): title, body, a footer with the
//! `esc cancel` hint, Cancel and the confirm button. Esc cancels; ↵ presses the default button, which is Cancel
//! for destructive dialogs ([`crate::view_models::dialog`]). The owner keeps the dialog's focus handle and
//! focuses it when the dialog opens.

use std::rc::Rc;

use gpui::{
    AnyElement, App, FocusHandle, IntoElement, KeyBinding, RenderOnce, SharedString, Window,
    actions, div, prelude::*, px,
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
        }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.body.push(child.into_any_element());
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
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
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
        div()
            .id(SharedString::from(format!("{}-backdrop", self.id)))
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .justify_center()
            .items_center()
            .bg(gpui::rgba(0x00000033))
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
                            .children(self.body),
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
                                            .child("cancel"),
                                    ),
                            )
                            .child(cancel)
                            .child(confirm),
                    ),
            )
    }
}
