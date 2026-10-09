//! Button and Icon Button (*01 · Components → Buttons → Button matrix*): primary, secondary, destructive and ghost
//! variants in the states default, hover, pressed, focused, disabled and loading.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, Hsla, IntoElement, Pixels, RenderOnce, SharedString, Window, div,
    prelude::*, px, transparent_black,
};

use super::focus_ring;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, Theme, radius, text};
use crate::ui::TextStyleExt;

/// Whether a click reaches the button's action. A disabled button ignores clicks; so does a loading one, so a
/// double press can't start an action twice.
pub fn accepts_click(disabled: bool, loading: bool) -> bool {
    !disabled && !loading
}

/// State for a button whose action takes a while (Merging…, Working…): a press that gets through turns on
/// loading until [`ActionButtonState::finish`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActionButtonState {
    pub disabled: bool,
    pub loading: bool,
}

impl ActionButtonState {
    /// Returns whether the action should run.
    pub fn press(&mut self) -> bool {
        let accepted = accepts_click(self.disabled, self.loading);
        if accepted {
            self.loading = true;
        }
        accepted
    }

    pub fn finish(&mut self) {
        self.loading = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Primary,
    Secondary,
    Destructive,
    Ghost,
}

/// A state drawn on purpose, for the gallery. Real buttons get hover and pressed from the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    Hover,
    Pressed,
    Focused,
}

struct Look {
    fill: Hsla,
    hover: Hsla,
    pressed: Hsla,
    border: Option<(Hsla, f32)>,
    hover_border: Option<f32>,
    content: Hsla,
}

fn look(variant: Variant, t: &Theme) -> Look {
    let c = &t.colors;
    match variant {
        Variant::Primary => Look {
            fill: c.accent,
            hover: c.accent_hover,
            pressed: c.accent_hover,
            border: None,
            hover_border: None,
            content: c.text_on_accent,
        },
        Variant::Secondary => Look {
            fill: c.bg_raised,
            hover: c.bg_hover,
            pressed: c.bg_pressed,
            border: Some((c.border_strong, 1.)),
            hover_border: None,
            content: c.text_primary,
        },
        Variant::Destructive => Look {
            fill: c.st_error_bg,
            hover: c.st_error_bg,
            pressed: c.st_error_bg,
            border: Some((c.st_error, 1.)),
            hover_border: Some(1.5),
            content: c.st_error,
        },
        Variant::Ghost => Look {
            fill: transparent_black(),
            hover: c.bg_hover,
            pressed: c.bg_pressed,
            border: None,
            hover_border: None,
            content: c.text_secondary,
        },
    }
}

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    icon: Option<IconName>,
    variant: Variant,
    height: Pixels,
    disabled: bool,
    loading: bool,
    shown: Option<Shown>,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            variant: Variant::Primary,
            height: px(28.),
            disabled: false,
            loading: false,
            shown: None,
            on_click: None,
        }
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 24 px, as inside banners.
    pub fn compact(mut self) -> Self {
        self.height = px(24.);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Shows the loader icon and ignores clicks; the caller sets the label ("Merging…").
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn shown(mut self, shown: Shown) -> Self {
        self.shown = Some(shown);
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = ActiveTheme::get(cx);
        let l = look(self.variant, t);
        let fill = match self.shown {
            Some(Shown::Hover) => l.hover,
            Some(Shown::Pressed) => l.pressed,
            _ => l.fill,
        };
        let icon_name = if self.loading {
            Some(IconName::LoaderCircle)
        } else {
            self.icon
        };
        let mut el = div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .h(self.height)
            .px(px(10.))
            .rounded(px(radius::MD))
            .bg(fill)
            .text_color(l.content)
            .children(icon_name.map(|i| icon(i, px(14.)).text_color(l.content)))
            .child(
                div()
                    .text_style_of(text::PILL)
                    .text_size(px(12.))
                    .child(self.label),
            );
        if let Some((color, width)) = l.border {
            let width = match (self.shown, l.hover_border) {
                (Some(Shown::Hover), Some(hover)) => hover,
                _ => width,
            };
            el = el.border_color(color).border(px(width));
        }
        if self.shown == Some(Shown::Focused) {
            el = el.shadow(focus_ring(t.colors.focus));
        }
        if self.disabled {
            return el.opacity(0.4);
        }
        if accepts_click(self.disabled, self.loading) {
            let (hover, pressed, hover_border) = (l.hover, l.pressed, l.hover_border);
            el = el
                .cursor_pointer()
                .hover(move |s| {
                    let s = s.bg(hover);
                    match hover_border {
                        Some(w) => s.border(px(w)),
                        None => s,
                    }
                })
                .active(move |s| s.bg(pressed));
            if let Some(handler) = self.on_click {
                el = el.on_click(move |event, window, cx| handler(event, window, cx));
            }
        }
        el
    }
}

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: IconName,
    disabled: bool,
    loading: bool,
    shown: Option<Shown>,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon: IconName) -> Self {
        Self {
            id: id.into(),
            icon,
            disabled: false,
            loading: false,
            shown: None,
            on_click: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn shown(mut self, shown: Shown) -> Self {
        self.shown = Some(shown);
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let fill = match self.shown {
            Some(Shown::Hover) => c.bg_hover,
            Some(Shown::Pressed) => c.bg_pressed,
            _ => transparent_black(),
        };
        let name = if self.loading {
            IconName::LoaderCircle
        } else {
            self.icon
        };
        let mut el = div()
            .id(self.id)
            .flex()
            .flex_none()
            .justify_center()
            .items_center()
            .size(px(28.))
            .rounded(px(radius::MD))
            .bg(fill)
            .child(icon(name, px(15.)).text_color(c.text_secondary));
        if self.shown == Some(Shown::Focused) {
            el = el.shadow(focus_ring(c.focus));
        }
        if self.disabled {
            return el.opacity(0.4);
        }
        if accepts_click(self.disabled, self.loading) {
            el = el
                .cursor_pointer()
                .hover(move |s| s.bg(c.bg_hover))
                .active(move |s| s.bg(c.bg_pressed));
            if let Some(handler) = self.on_click {
                el = el.on_click(move |event, window, cx| handler(event, window, cx));
            }
        }
        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_ignores_clicks() {
        let mut state = ActionButtonState {
            disabled: true,
            loading: false,
        };
        assert!(!state.press());
        assert!(!state.loading);
        assert!(!accepts_click(true, false));
    }

    #[test]
    fn loading_blocks_a_double_press() {
        let mut state = ActionButtonState::default();
        assert!(state.press(), "first press runs the action");
        assert!(state.loading);
        assert!(!state.press(), "second press while loading is ignored");
        state.finish();
        assert!(
            state.press(),
            "after the action finished, a press works again"
        );
        assert!(!accepts_click(false, true));
        assert!(accepts_click(false, false));
    }
}
