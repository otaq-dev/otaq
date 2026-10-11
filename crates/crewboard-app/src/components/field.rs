//! Form pieces (*01 · Components → Badges & Inputs → Fields*): a labelled field with a hint or an error under
//! it, and the dropdown button.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, Entity, FontWeight, IntoElement, RenderOnce,
    SharedString, Window, div, prelude::*, px,
};

use super::text_input::TextInput;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_MONO};

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A label (11/500), the control, then the error (red) or a hint (muted).
#[derive(IntoElement)]
pub struct Field {
    label: Option<SharedString>,
    control: AnyElement,
    hint: Option<SharedString>,
    error: Option<SharedString>,
}

impl Field {
    pub fn new(label: impl Into<SharedString>, control: impl IntoElement) -> Self {
        Self {
            label: Some(label.into()),
            control: control.into_any_element(),
            hint: None,
            error: None,
        }
    }

    /// A field around a text input; the input turns red while there is an error.
    pub fn input(label: impl Into<SharedString>, input: &Entity<TextInput>) -> Self {
        Self::new(label, input.clone())
    }

    pub fn unlabelled(control: impl IntoElement) -> Self {
        Self {
            label: None,
            control: control.into_any_element(),
            hint: None,
            error: None,
        }
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn error(mut self, error: Option<impl Into<SharedString>>) -> Self {
        self.error = error.map(Into::into);
        self
    }
}

impl RenderOnce for Field {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let note = match (self.error, self.hint) {
            (Some(e), _) => Some((e, c.st_error)),
            (None, Some(h)) => Some((h, c.text_muted)),
            (None, None) => None,
        };
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(5.))
            .children(self.label.map(|l| {
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c.text_secondary)
                    .child(l)
            }))
            .child(self.control)
            .children(
                note.map(|(text, color)| div().text_size(px(11.)).text_color(color).child(text)),
            )
    }
}

/// A closed dropdown: the current value with an optional icon and a chevron. Opening the list is the owner's
/// (it shows a [`super::menu::ContextMenu`] under it).
#[derive(IntoElement)]
pub struct DropdownButton {
    id: ElementId,
    icon: Option<IconName>,
    value: SharedString,
    mono: bool,
    /// A 13 px muted icon in place of the chevron-down: chevrons-up-down in the editor's fields, a lock on a
    /// locked base (*08 → Epic, Base*).
    trailing: Option<IconName>,
    disabled: bool,
    open: bool,
    on_click: Option<ClickHandler>,
}

impl DropdownButton {
    pub fn new(id: impl Into<ElementId>, value: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon: None,
            value: value.into(),
            mono: false,
            trailing: None,
            disabled: false,
            open: false,
            on_click: None,
        }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    pub fn up_down(self) -> Self {
        self.trailing(IconName::ChevronsUpDown)
    }

    pub fn trailing(mut self, icon: IconName) -> Self {
        self.trailing = Some(icon);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for DropdownButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let mut value = div()
            .text_size(px(13.))
            .text_color(c.text_primary)
            .overflow_hidden()
            .whitespace_nowrap()
            .child(self.value);
        if self.mono {
            value = value.font_family(FONT_MONO).text_size(px(12.));
        }
        let mut el = div()
            .id(self.id)
            .flex()
            .w_full()
            .h(px(30.))
            .px(px(10.))
            .justify_between()
            .items_center()
            .bg(c.bg_raised)
            .rounded(px(5.))
            .border(if self.open { px(1.5) } else { px(1.) })
            .border_color(if self.open { c.focus } else { c.border_strong })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .overflow_hidden()
                    .children(
                        self.icon
                            .map(|i| icon(i, px(13.)).text_color(c.text_primary)),
                    )
                    .child(value),
            )
            .child(match self.trailing {
                Some(name) => icon(name, px(13.)).text_color(c.text_muted),
                None => icon(IconName::ChevronDown, px(14.)).text_color(c.text_muted),
            });
        if self.disabled {
            el = el.opacity(0.6);
        } else if let Some(handler) = self.on_click {
            el = el
                .cursor_pointer()
                .on_click(move |e, window, cx| handler(e, window, cx));
        }
        el
    }
}
