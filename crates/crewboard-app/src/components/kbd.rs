//! Kbd (*01 · Components → Primitives → Kbd*): a shortcut hint, rendered from an action and the platform, so the
//! same action shows `⌘J` on macOS and `Ctrl+J` on Windows and Linux (d0058).

use gpui::{App, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px};

use crate::keymap::{KeyAction, Platform};
use crate::theme::{ActiveTheme, FONT_MONO, radius};

#[derive(IntoElement)]
pub struct Kbd {
    label: SharedString,
}

impl Kbd {
    /// The shortcut of `action` on `platform`; `None` when the action has none there (e.g. Open panel while
    /// the terminal has focus).
    pub fn for_action(action: KeyAction, platform: Platform) -> Option<Self> {
        action.keys(platform).map(Self::text)
    }

    /// A key hint that isn't an app action, like `esc` in a dialog.
    pub fn text(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl RenderOnce for Kbd {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        div()
            .flex()
            .flex_none()
            .justify_center()
            .items_center()
            .px(px(5.))
            .py(px(1.))
            .rounded(px(radius::SM))
            .bg(c.bg_column)
            .border_color(c.border_strong)
            .border_t(px(1.))
            .border_l(px(1.))
            .border_r(px(1.))
            .border_b(px(1.5))
            .font_family(FONT_MONO)
            .text_size(px(11.))
            .text_color(c.text_secondary)
            .child(self.label)
    }
}
