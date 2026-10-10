//! Tabs with indicators, the segmented control and the toggle (*01 · Components → Badges & Inputs → Tabs &
//! segmented*). Active tab: 2 px accent underline and 600 weight; indicators are a live dot or a mono note
//! (diff counts, port, run count).

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, FontWeight, IntoElement, RenderOnce, SharedString, Window, div,
    prelude::*, px,
};

use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_MONO, FONT_UI, radius};

type Handler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type ChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// What a tab shows after its label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabIndicator {
    /// A small dot in the working colour (live terminal).
    Live,
    /// The live dot in the attention colour (the agent waits for you).
    LiveAttention,
    /// A short mono note: `+412 −38`, `:4020`, `3`.
    Note(SharedString),
    /// A red dot: something needs fixing (Run failed to start).
    Problem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabItem {
    pub icon: IconName,
    pub label: SharedString,
    pub indicator: Option<TabIndicator>,
}

impl TabItem {
    pub fn new(icon: IconName, label: impl Into<SharedString>) -> Self {
        Self {
            icon,
            label: label.into(),
            indicator: None,
        }
    }

    pub fn indicator(mut self, indicator: TabIndicator) -> Self {
        self.indicator = Some(indicator);
        self
    }
}

#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    items: Vec<TabItem>,
    active: usize,
    on_select: Option<Handler>,
    icons_only: bool,
    /// Shortcut text per tab, shown in the icon-only tooltip (⌘⌥1…).
    shortcuts: Vec<Option<String>>,
}

impl Tabs {
    pub fn new(id: impl Into<ElementId>, items: Vec<TabItem>, active: usize) -> Self {
        Self {
            id: id.into(),
            items,
            active,
            on_select: None,
            icons_only: false,
            shortcuts: Vec::new(),
        }
    }

    /// Narrow panels: icons only, the name (and shortcut) in a tooltip on hover.
    pub fn icons_only(mut self, icons_only: bool, shortcuts: Vec<Option<String>>) -> Self {
        self.icons_only = icons_only;
        self.shortcuts = shortcuts;
        self
    }

    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let id = self.id.clone();
        div()
            .id(self.id)
            .flex()
            .w_full()
            .border_b_1()
            .border_color(c.border)
            .children(self.items.into_iter().enumerate().map(|(i, item)| {
                let active = i == self.active;
                let handler = self.on_select.clone();
                let icons_only = self.icons_only;
                let tip = (icons_only).then(|| {
                    crate::components::menu::Tooltip::text(
                        item.label.clone(),
                        self.shortcuts.get(i).cloned().flatten(),
                    )
                });
                let mut tab = div()
                    .id(super::sub_id(&id, i))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .p(px(8.))
                    .mb(px(-1.))
                    .cursor_pointer()
                    .child(icon(item.icon, px(13.)).text_color(if active {
                        c.text_primary
                    } else {
                        c.text_muted
                    }))
                    .when(!icons_only, |tab| {
                        tab.child(
                            div()
                                .font_family(FONT_UI)
                                .text_size(px(12.))
                                .font_weight(if active {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .text_color(if active {
                                    c.text_primary
                                } else {
                                    c.text_secondary
                                })
                                .child(item.label),
                        )
                    })
                    .children(item.indicator.map(|ind| {
                        match ind {
                            TabIndicator::Live => div()
                                .size(px(6.))
                                .rounded_full()
                                .bg(c.st_working)
                                .into_any_element(),
                            TabIndicator::LiveAttention => div()
                                .size(px(6.))
                                .rounded_full()
                                .bg(c.st_attn)
                                .into_any_element(),
                            TabIndicator::Problem => div()
                                .size(px(6.))
                                .rounded_full()
                                .bg(c.st_error)
                                .into_any_element(),
                            TabIndicator::Note(note) => div()
                                .font_family(FONT_MONO)
                                .text_size(px(10.))
                                .text_color(c.text_muted)
                                .child(note)
                                .into_any_element(),
                        }
                    }))
                    .on_click(move |_: &ClickEvent, window, cx| {
                        if let Some(h) = &handler {
                            h(i, window, cx);
                        }
                    });
                if let Some(tip) = tip {
                    tab = tab.tooltip(tip);
                }
                if active {
                    tab = tab.border_b_2().border_color(c.accent);
                } else {
                    tab = tab.hover(|s| s.bg(c.bg_hover));
                }
                tab
            }))
    }
}

/// Two or more options, one chosen (Unified / Side-by-side).
#[derive(IntoElement)]
pub struct Segmented {
    id: ElementId,
    options: Vec<SharedString>,
    selected: usize,
    on_select: Option<Handler>,
}

impl Segmented {
    pub fn new(id: impl Into<ElementId>, options: Vec<SharedString>, selected: usize) -> Self {
        Self {
            id: id.into(),
            options,
            selected,
            on_select: None,
        }
    }

    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Segmented {
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
            .children(self.options.into_iter().enumerate().map(|(i, label)| {
                let on = i == self.selected;
                let handler = self.on_select.clone();
                let mut option = div()
                    .id(super::sub_id(&id, i))
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(radius::SM))
                    .border_1()
                    .border_color(gpui::transparent_black())
                    .text_size(px(12.))
                    .cursor_pointer()
                    .child(label)
                    .on_click(move |_, window, cx| {
                        if let Some(h) = &handler {
                            h(i, window, cx);
                        }
                    });
                option = if on {
                    option
                        .bg(c.bg_raised)
                        .border_color(c.border)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c.text_primary)
                } else {
                    option
                        .text_color(c.text_secondary)
                        .hover(|s| s.text_color(c.text_primary))
                };
                option
            }))
    }
}

/// An on/off switch with a label.
#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    on: bool,
    label: Option<SharedString>,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl Toggle {
    pub fn new(id: impl Into<ElementId>, on: bool) -> Self {
        Self {
            id: id.into(),
            on,
            label: None,
            disabled: false,
            on_change: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let on = self.on;
        let handler = self.on_change.clone();
        let disabled = self.disabled;
        let mut track = div()
            .flex()
            .flex_none()
            .w(px(28.))
            .h(px(16.))
            .p(px(2.))
            .rounded(px(8.))
            .bg(if on { c.accent } else { c.border_strong })
            .child(div().size(px(12.)).rounded_full().bg(gpui::white()));
        if on {
            track = track.justify_end();
        }
        let mut el = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(8.))
            .child(track)
            .children(self.label.map(|l| {
                div()
                    .text_size(px(12.))
                    .text_color(c.text_secondary)
                    .child(l)
            }));
        if disabled {
            el = el.opacity(0.5);
        } else {
            el = el.cursor_pointer().on_click(move |_, window, cx| {
                if let Some(h) = &handler {
                    h(!on, window, cx);
                }
            });
        }
        el
    }
}
