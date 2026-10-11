//! The Epic Card (*01 · Components → Card component → Epic Card*): a delivery unit with its progress, branch and
//! child list. Child rows are clickable (they select the child); "Add ticket to epic" creates a child. It
//! shares the Ticket Card's tokens and interaction states.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, FontWeight, IntoElement, RenderOnce, SharedString, Window, div,
    prelude::*, px,
};

use super::button::{Button, Variant};
use super::card::ChainState;
use super::floating_shadow;
use super::pill::StatusPill;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_MONO, radius, text};
use crate::ui::TextStyleExt;
use crate::view_models::card::CardActionKind;
use crate::view_models::epic_card::{ChildState, EpicViewModel};
use crewboard_core::domain::TicketKey;

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type ChildHandler = Rc<dyn Fn(TicketKey, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(CardActionKind, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct EpicCard {
    id: ElementId,
    vm: EpicViewModel,
    width: Option<f32>,
    collapsed: bool,
    selected: bool,
    chain: ChainState,
    dragging: bool,
    on_click: Option<ClickHandler>,
    on_child: Option<ChildHandler>,
    on_add: Option<ClickHandler>,
    on_toggle: Option<ClickHandler>,
    on_action: Option<ActionHandler>,
}

impl EpicCard {
    pub fn new(id: impl Into<ElementId>, vm: EpicViewModel) -> Self {
        Self {
            id: id.into(),
            vm,
            width: None,
            collapsed: false,
            selected: false,
            chain: ChainState::None,
            dragging: false,
            on_click: None,
            on_child: None,
            on_add: None,
            on_toggle: None,
            on_action: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn chain(mut self, chain: ChainState) -> Self {
        self.chain = chain;
        self
    }

    pub fn dragging(mut self, dragging: bool) -> Self {
        self.dragging = dragging;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    /// A child row was clicked (select that ticket).
    pub fn on_child(mut self, f: impl Fn(TicketKey, &mut Window, &mut App) + 'static) -> Self {
        self.on_child = Some(Rc::new(f));
        self
    }

    pub fn on_add(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_add = Some(Rc::new(f));
        self
    }

    /// The chevron: collapse or expand the child list.
    pub fn on_toggle(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }

    pub fn on_action(
        mut self,
        f: impl Fn(CardActionKind, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for EpicCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let vm = self.vm;
        let key = vm.key.to_string();
        let (border, border_width) = if self.selected {
            (c.focus, px(2.))
        } else if self.chain == ChainState::Highlighted {
            (c.accent, px(1.5))
        } else {
            (c.border, px(1.))
        };
        let toggle = self.on_toggle.clone();
        let head = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(18.))
            .children(
                vm.pinned
                    .then(|| icon(IconName::Pin, px(11.)).text_color(c.accent)),
            )
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .child(SharedString::from(key.clone())),
            )
            .children(vm.external_id.clone().map(|id| {
                div()
                    .px(px(5.))
                    .py(px(1.))
                    .rounded(px(3.))
                    .bg(c.bg_column)
                    .border_1()
                    .border_color(c.border)
                    .font_family(FONT_MONO)
                    .text_size(px(10.5))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(id))
            }))
            .children(vm.priority.map(super::priority::PriorityChip::new))
            .child(div().flex_1())
            .child(StatusPill::legend(
                vm.pill.legend,
                Some(vm.pill.label.clone().into()),
            ))
            .child(
                div()
                    .id(SharedString::from(format!("{key}-toggle")))
                    .cursor_pointer()
                    .child(
                        icon(
                            if self.collapsed {
                                IconName::ChevronRight
                            } else {
                                IconName::ChevronDown
                            },
                            px(14.),
                        )
                        .text_color(c.text_muted),
                    )
                    .on_click(move |e, window, cx| {
                        cx.stop_propagation();
                        if let Some(t) = &toggle {
                            t(e, window, cx);
                        }
                    }),
            );

        let segment = |state: ChildState| {
            let color = match state {
                ChildState::Merged => c.st_approved,
                ChildState::Working => c.st_working,
                ChildState::Closed | ChildState::NotStarted => c.border_strong,
            };
            div().flex_1().h(px(4.)).rounded(px(2.)).bg(color)
        };
        let progress = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .gap(px(2.))
                    .children(vm.segments.iter().map(|s| segment(*s))),
            )
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(vm.progress.clone())),
            );
        let branch = div()
            .flex()
            .items_center()
            .gap(px(4.))
            .child(icon(IconName::GitBranch, px(11.)).text_color(c.text_muted))
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .truncate()
                    .child(SharedString::from(vm.branch.clone())),
            );
        let children = (!self.collapsed && !vm.children.is_empty()).then(|| {
            let on_child = self.on_child.clone();
            div()
                .flex()
                .flex_col()
                .gap(px(1.))
                .pt(px(6.))
                .border_t_1()
                .border_color(c.border)
                .children(vm.children.iter().map(|child| {
                    let finished = matches!(child.state, ChildState::Merged | ChildState::Closed);
                    let icon_color = match child.state {
                        ChildState::Merged | ChildState::Closed => c.st_done,
                        ChildState::Working => c.st_working,
                        ChildState::NotStarted => c.st_queued,
                    };
                    let on_child = on_child.clone();
                    let child_key = child.key.clone();
                    div()
                        .id(SharedString::from(format!("{key}-child-{}", child.key)))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .h(px(20.))
                        .px(px(2.))
                        .rounded(px(3.))
                        .cursor_pointer()
                        .hover(|s| s.bg(c.bg_hover))
                        .child(icon(child.state.icon(), px(11.)).text_color(icon_color))
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(10.5))
                                .text_color(c.text_muted)
                                .child(SharedString::from(child.key.to_string())),
                        )
                        .child(
                            div()
                                .flex_1()
                                .truncate()
                                .text_size(px(11.5))
                                .text_color(if finished {
                                    c.text_muted
                                } else {
                                    c.text_primary
                                })
                                .child(SharedString::from(child.title.clone())),
                        )
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(icon_color)
                                .child(child.status_label),
                        )
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            if let Some(h) = &on_child {
                                h(child_key.clone(), window, cx);
                            }
                        })
                }))
        });
        let add = (vm.can_add && !self.collapsed).then(|| {
            let on_add = self.on_add.clone();
            div()
                .id(SharedString::from(format!("{key}-add")))
                .flex()
                .items_center()
                .gap(px(4.))
                .cursor_pointer()
                .text_color(c.text_muted)
                .hover(|s| s.text_color(c.text_primary))
                .child(icon(IconName::Plus, px(11.)))
                .child(div().text_size(px(11.)).child("Add ticket to epic"))
                .on_click(move |e, window, cx| {
                    cx.stop_propagation();
                    if let Some(h) = &on_add {
                        h(e, window, cx);
                    }
                })
        });
        let actions = (!vm.actions.is_empty()).then(|| {
            let handler = self.on_action.clone();
            div()
                .flex()
                .gap(px(6.))
                .pt(px(2.))
                .children(vm.actions.iter().enumerate().map(|(i, a)| {
                    let handler = handler.clone();
                    let kind = a.kind;
                    let mut b =
                        Button::new(SharedString::from(format!("{key}-action-{i}")), a.label)
                            .compact()
                            .variant(if a.primary {
                                Variant::Primary
                            } else {
                                Variant::Secondary
                            })
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                if let Some(h) = &handler {
                                    h(kind, window, cx);
                                }
                            });
                    if let Some(i) = a.icon {
                        b = b.icon(i);
                    }
                    div().flex_1().flex().justify_center().child(b)
                }))
        });

        let mut card = div()
            .id(self.id)
            .flex()
            .flex_col()
            .gap(px(7.))
            .pt(px(9.))
            .px(px(10.))
            .pb(px(10.))
            .rounded(px(radius::MD))
            .bg(if vm.faded.is_some() {
                c.bg_column
            } else {
                c.bg_card
            })
            .border(border_width)
            .border_color(border)
            .child(head)
            .child(
                div()
                    .text_style_of(text::CARD_TITLE)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c.text_primary)
                    .line_clamp(2)
                    .child(SharedString::from(vm.title.clone())),
            )
            .child(progress)
            .child(branch)
            .children(children)
            .children(add)
            .children(actions);
        card = match self.width {
            Some(w) => card.w(px(w)),
            None => card.w_full(),
        };
        if !self.selected {
            let strong = c.border_strong;
            card = card.hover(move |s| s.border_color(strong));
        }
        if self.dragging {
            card = card.border_color(c.border_strong).shadow(floating_shadow());
        }
        if let Some(o) = vm.faded {
            card = card.opacity(o);
        }
        if self.chain == ChainState::Dimmed {
            card = card.opacity(0.35);
        }
        if let Some(on_click) = self.on_click {
            card = card
                .cursor_pointer()
                .on_click(move |e, window, cx| on_click(e, window, cx));
        }
        card
    }
}
