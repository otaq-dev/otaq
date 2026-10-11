//! Priority (*01 · Components → Primitives → Priority*, *01 · Foundations → Status Legend → 5 · Priority*; TKT-09,
//! d0065): the chip (12 px icon + mono number, ink only, never orange or red), the form's segmented control
//! (*08 · Ticket editor → Full form · filled → Priority field*) and the small dropdown trigger of the panel header,
//! the Ticket tab and quick-create (*Priority · editable*, *Quick-create in Backlog → Priority (optional)*).

use std::rc::Rc;

use crewboard_core::domain::Priority;
use gpui::{
    App, ElementId, FontWeight, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px,
};

use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, Colors, FONT_MONO, radius};
use crate::view_models::priority::{self as vm, Ink};

/// The chip's colours: (fill, stroke, icon and number).
fn chip_colors(priority: Priority, c: &Colors) -> (Option<gpui::Hsla>, gpui::Hsla, gpui::Hsla) {
    match vm::ink(priority) {
        Ink::Inverted => (Some(c.text_primary), c.text_primary, c.bg_card),
        Ink::Strong => (None, c.border_strong, c.text_primary),
        Ink::Muted => (None, c.border, c.text_muted),
    }
}

/// The word next to a chip in the panel and the legend: full ink for P1 and P2, muted for P3 and P4.
pub fn word_color(priority: Priority, c: &Colors) -> gpui::Hsla {
    match vm::ink(priority) {
        Ink::Inverted | Ink::Strong => c.text_primary,
        Ink::Muted => c.text_muted,
    }
}

/// The chip: radius 4, 1 px stroke, gap 3, padding 1/5/1/3, a 12 px icon and the number in mono 10.5/600.
#[derive(IntoElement)]
pub struct PriorityChip {
    priority: Priority,
}

impl PriorityChip {
    pub fn new(priority: Priority) -> Self {
        Self { priority }
    }
}

impl RenderOnce for PriorityChip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let (fill, stroke, ink) = chip_colors(self.priority, &c);
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(3.))
            .pt(px(1.))
            .pb(px(1.))
            .pl(px(3.))
            .pr(px(5.))
            .rounded(px(radius::SM))
            .border_1()
            .border_color(stroke)
            .when_some(fill, |d, fill| d.bg(fill))
            .child(icon(vm::icon(self.priority), px(12.)).text_color(ink))
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(10.5))
                    .line_height(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ink)
                    .whitespace_nowrap()
                    .child(SharedString::from(vm::short(self.priority))),
            )
    }
}

/// The dropdown trigger (*Priority · editable*): the chip, the word (unless `compact`) and a chevron. The owner
/// opens the menu on mouse down, as with the other dropdowns. `outlined` is quick-create's bordered variant.
#[derive(IntoElement)]
pub struct PriorityDropdown {
    id: ElementId,
    priority: Priority,
    compact: bool,
    outlined: bool,
    open: bool,
}

impl PriorityDropdown {
    pub fn new(id: impl Into<ElementId>, priority: Priority) -> Self {
        Self {
            id: id.into(),
            priority,
            compact: false,
            outlined: false,
            open: false,
        }
    }

    /// Chip and chevron only (quick-create).
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// A 1 px `border` outline (quick-create).
    pub fn outlined(mut self) -> Self {
        self.outlined = true;
        self
    }

    /// The menu is open: drawn pressed.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
}

impl RenderOnce for PriorityDropdown {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .pt(px(1.))
            .pb(px(1.))
            .pl(px(2.))
            .pr(px(4.))
            .rounded(px(radius::SM))
            .cursor_pointer()
            .when(self.outlined, |d| d.border_1().border_color(c.border))
            .when(self.open, |d| d.bg(c.bg_pressed))
            .when(!self.open, |d| d.hover(|s| s.bg(c.bg_hover)))
            .child(PriorityChip::new(self.priority))
            .when(!self.compact, |d| {
                d.child(
                    div()
                        .text_size(px(11.))
                        .text_color(word_color(self.priority, &c))
                        .whitespace_nowrap()
                        .child(self.priority.label()),
                )
            })
            .child(icon(IconName::ChevronDown, px(11.)).text_color(c.text_muted))
    }
}

type SelectHandler = Rc<dyn Fn(Priority, &mut Window, &mut App)>;

/// The form's segmented control: the four values as chip + word; the selected one raised with a strong outline.
#[derive(IntoElement)]
pub struct PrioritySegmented {
    id: ElementId,
    selected: Priority,
    on_select: Option<SelectHandler>,
}

impl PrioritySegmented {
    pub fn new(id: impl Into<ElementId>, selected: Priority) -> Self {
        Self {
            id: id.into(),
            selected,
            on_select: None,
        }
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(Priority, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PrioritySegmented {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let id = self.id.clone();
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(radius::MD))
            .bg(c.bg_column)
            .children(Priority::ALL.into_iter().enumerate().map(|(i, p)| {
                let on = p == self.selected;
                let handler = self.on_select.clone();
                div()
                    .id(super::sub_id(&id, i))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .pt(px(4.))
                    .pb(px(4.))
                    .pl(px(6.))
                    .pr(px(10.))
                    .rounded(px(radius::SM))
                    .border_1()
                    .cursor_pointer()
                    .text_size(px(12.))
                    .child(PriorityChip::new(p))
                    .child(SharedString::from(p.label()))
                    .on_click(move |_, window, cx| {
                        if let Some(h) = &handler {
                            h(p, window, cx);
                        }
                    })
                    .map(|option| {
                        if on {
                            option
                                .bg(c.bg_raised)
                                .border_color(c.border_strong)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c.text_primary)
                        } else {
                            option
                                .border_color(gpui::transparent_black())
                                .text_color(c.text_secondary)
                                .hover(|s| s.text_color(c.text_primary))
                        }
                    })
            }))
    }
}
