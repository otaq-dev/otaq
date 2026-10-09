//! Form pieces (*01 · Components → Badges & Inputs → Fields*): a labelled field with a hint or an error under
//! it, the dropdown button, and the "Done when" checklist item (drag handle, check box, text, remove).

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
            .child(icon(IconName::ChevronDown, px(14.)).text_color(c.text_muted));
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

/// One "Done when" criterion: drag handle, check box, text and (on hover) a remove button.
#[derive(IntoElement)]
pub struct CheckItem {
    id: ElementId,
    text: SharedString,
    checked: bool,
    /// Drawn while the item is being dragged to a new place.
    dragging: bool,
    marker: Option<AnyElement>,
    on_toggle: Option<ClickHandler>,
    on_remove: Option<ClickHandler>,
}

impl CheckItem {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            checked: false,
            dragging: false,
            marker: None,
            on_toggle: None,
            on_remove: None,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn dragging(mut self, dragging: bool) -> Self {
        self.dragging = dragging;
        self
    }

    /// Something after the text, e.g. the "added by agent" marker.
    pub fn marker(mut self, marker: impl IntoElement) -> Self {
        self.marker = Some(marker.into_any_element());
        self
    }

    pub fn on_toggle(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }

    pub fn on_remove(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_remove = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for CheckItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let id = self.id.clone();
        let mut check = div()
            .id(super::sub_id(&id, 1))
            .flex()
            .flex_none()
            .size(px(14.))
            .rounded(px(3.))
            .justify_center()
            .items_center();
        check = if self.checked {
            check
                .bg(c.accent)
                .child(icon(IconName::Check, px(10.)).text_color(gpui::white()))
        } else {
            check.border(px(1.5)).border_color(c.border_strong)
        };
        if let Some(toggle) = self.on_toggle {
            check = check
                .cursor_pointer()
                .on_click(move |e, window, cx| toggle(e, window, cx));
        }
        let group: SharedString = format!("check-item-{id:?}").into();
        let mut row = div()
            .id(self.id)
            .group(group.clone())
            .flex()
            .w_full()
            .gap(px(8.))
            .items_center()
            .py(px(2.))
            .rounded(px(4.))
            .child(
                div()
                    .cursor_grab()
                    .child(icon(IconName::GripVertical, px(12.)).text_color(c.text_muted)),
            )
            .child(check)
            .child(
                div()
                    .flex_1()
                    .text_size(px(13.))
                    .text_color(if self.checked {
                        c.text_secondary
                    } else {
                        c.text_primary
                    })
                    .child(self.text),
            )
            .children(self.marker);
        if let Some(remove) = self.on_remove {
            row = row.child(
                div()
                    .id(super::sub_id(&id, 2))
                    .invisible()
                    .group_hover(group, |s| s.visible())
                    .cursor_pointer()
                    .p(px(2.))
                    .rounded(px(3.))
                    .hover(|s| s.bg(c.bg_hover))
                    .child(icon(IconName::X, px(12.)).text_color(c.text_muted))
                    .on_click(move |e, window, cx| remove(e, window, cx)),
            );
        }
        if self.dragging {
            row = row.bg(c.bg_hover).opacity(0.7);
        }
        row
    }
}
