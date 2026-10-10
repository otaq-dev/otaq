//! The unsent-text prompt (SES-07, d0048; *06 → Unsent-text prompt*): "SHOP-23 has unsent text in its terminal",
//! what is typed and what the click would send, then **Keep it and cancel** / **Clear it and send**. Every click
//! that types into a session uses it: Deliver now, Send back, Send to agent to fix, an inbox or panel answer.

use gpui::{AnyElement, App, FocusHandle, SharedString, Window, div, prelude::*, px};

use super::dialog::ConfirmDialog;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_MONO};
use crate::view_models::dialog::DialogKind;
use crate::view_models::terminal_strip::UnsentPrompt;

/// The prompt as a dialog; `on_clear` is "Clear it and send", `on_keep` "Keep it and cancel" (also Esc).
pub fn unsent_dialog(
    prompt: &UnsentPrompt,
    focus: &FocusHandle,
    on_clear: impl Fn(&mut Window, &mut App) + 'static,
    on_keep: impl Fn(&mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> AnyElement {
    let c = ActiveTheme::get(cx).colors;
    let quote = |icon_name: IconName, text: String, bg, color| {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .py(px(7.))
            .rounded(px(6.))
            .bg(bg)
            .child(icon(icon_name, px(13.)).text_color(c.text_muted))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.))
                    .text_color(color)
                    .truncate()
                    .child(SharedString::from(text)),
            )
    };
    let keep = on_keep.clone();
    ConfirmDialog::new(
        "unsent-text",
        focus,
        DialogKind::Primary,
        prompt.title(),
        "Clear it and send",
        on_clear,
        on_keep,
    )
    .secondary("Keep it and cancel", keep)
    .confirm_icon(IconName::Send)
    .width(480.)
    .children_boxes(
        prompt
            .typed
            .clone()
            .map(|typed| {
                quote(IconName::Keyboard, typed, c.bg_column, c.text_primary)
                    .font_family(FONT_MONO)
                    .into_any_element()
            })
            .into_iter()
            .collect(),
    )
    .child(quote(
        IconName::MessageSquareDot,
        prompt.sending.clone(),
        c.accent_soft,
        c.text_primary,
    ))
    .text(prompt.explanation())
    .into_any_element()
}
