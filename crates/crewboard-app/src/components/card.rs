//! The Ticket Card (*01 · Components → Card component*, *04 · Ticket card variants*): key (+ pin), title (two
//! lines, the full title in a tooltip), the pill, an orange border and callout for needs-you states, bars for
//! messages waiting and background work, the meta row, an actions row, and hover quick actions. It draws a
//! [`CardViewModel`]; the board decides the interaction state (selected, chain-highlighted, dimmed, dragging).

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, Hsla, IntoElement, RenderOnce,
    SharedString, Window, div, prelude::*, px, relative,
};

use super::button::{Button, Variant};
use super::floating_shadow;
use super::menu::Tooltip;
use super::pill::StatusPill;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, Colors, FONT_MONO, radius, text};
use crate::ui::TextStyleExt;
use crate::view_models::card::{Bar, CardActionKind, CardViewModel, Outline, Tone};

/// How a card relates to the chain being highlighted (T-0037).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChainState {
    #[default]
    None,
    Highlighted,
    Dimmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    #[default]
    Comfortable,
    /// One-line title; key, pill and signal on one row; the callout shrinks to its icon in the pill.
    Compact,
}

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(CardActionKind, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct TicketCard {
    id: ElementId,
    vm: CardViewModel,
    width: Option<f32>,
    density: Density,
    selected: bool,
    chain: ChainState,
    dragging: bool,
    /// Draws the hovered look on purpose (the gallery).
    show_hover: bool,
    on_click: Option<ClickHandler>,
    on_action: Option<ActionHandler>,
    on_open_panel: Option<ClickHandler>,
}

impl TicketCard {
    pub fn new(id: impl Into<ElementId>, vm: CardViewModel) -> Self {
        Self {
            id: id.into(),
            vm,
            width: None,
            density: Density::Comfortable,
            selected: false,
            chain: ChainState::None,
            dragging: false,
            show_hover: false,
            on_click: None,
            on_action: None,
            on_open_panel: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn density(mut self, density: Density) -> Self {
        self.density = density;
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

    pub fn show_hover(mut self, show: bool) -> Self {
        self.show_hover = show;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    pub fn on_action(
        mut self,
        f: impl Fn(CardActionKind, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }

    pub fn on_open_panel(
        mut self,
        f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_panel = Some(Rc::new(f));
        self
    }
}

/// Foreground and background of a tone.
pub fn tone_colors(tone: Tone, c: &Colors) -> (Hsla, Hsla) {
    match tone {
        Tone::Muted => (c.text_muted, c.bg_column),
        Tone::Attention => (c.st_attn, c.st_attn_bg),
        Tone::Error => (c.st_error, c.st_error_bg),
        Tone::Warning => (c.st_paused, c.st_paused_bg),
        Tone::Working => (c.st_working, c.st_working_bg),
        Tone::Review => (c.st_review, c.st_review_bg),
        Tone::Running => (c.st_approved, c.st_approved_bg),
        Tone::Epic => (c.st_investigate, c.st_investigate_bg),
        Tone::Accent => (c.accent, c.accent_soft),
    }
}

/// The violet epic chip on a child card.
pub fn epic_chip(title: SharedString, c: &Colors) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(3.))
        .px(px(5.))
        .py(px(1.))
        .rounded(px(3.))
        .bg(c.st_investigate_bg)
        .text_color(c.st_investigate)
        .child(icon(IconName::Layers, px(10.)).text_color(c.st_investigate))
        .child(
            div()
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .child(title),
        )
        .into_any_element()
}

impl RenderOnce for TicketCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let vm = self.vm;
        let compact = self.density == Density::Compact;
        let hover_group: SharedString = format!("card-{}", vm.key).into();

        let (mut border, mut border_width) = match vm.outline {
            Outline::Attention => (c.st_attn, px(1.)),
            Outline::Error => (c.st_error, px(1.)),
            Outline::Normal => (c.border, px(1.)),
        };
        if self.chain == ChainState::Highlighted {
            (border, border_width) = (c.accent, px(1.5));
        }
        if self.selected {
            (border, border_width) = (c.focus, px(2.));
        }

        // Head: key, pin, spacer, pill (compact: + signal).
        let callout_icon = compact
            .then(|| vm.callout.as_ref().map(|co| co.icon))
            .flatten();
        let mut pill = vm.pill.clone().map(|p| {
            div()
                .flex()
                .items_center()
                .gap(px(3.))
                .children(callout_icon.map(|i| {
                    let (fg, _) =
                        tone_colors(vm.callout.as_ref().map_or(Tone::Muted, |c| c.tone), &c);
                    icon(i, px(12.)).text_color(fg)
                }))
                .child(StatusPill::legend(p.legend, Some(p.label.into())))
                .into_any_element()
        });
        if pill.is_none() && callout_icon.is_some() {
            let (fg, _) = tone_colors(vm.callout.as_ref().map_or(Tone::Muted, |c| c.tone), &c);
            pill = callout_icon.map(|i| icon(i, px(12.)).text_color(fg).into_any_element());
        }
        let quick = (self.on_open_panel.is_some() || self.show_hover).then(|| {
            let open = self.on_open_panel.clone();
            let mut q =
                div()
                    .flex()
                    .gap(px(2.))
                    .p(px(2.))
                    .rounded(px(5.))
                    .bg(c.bg_raised)
                    .border_1()
                    .border_color(c.border)
                    .shadow(floating_shadow())
                    .child(div().p(px(3.)).child(
                        icon(IconName::SquareTerminal, px(13.)).text_color(c.text_secondary),
                    ))
                    .child(
                        div()
                            .id(SharedString::from(format!("{}-open", vm.key)))
                            .p(px(3.))
                            .rounded(px(3.))
                            .hover(|s| s.bg(c.bg_hover))
                            .cursor_pointer()
                            .tooltip(Tooltip::text("Open panel", Some("↵".into())))
                            .child(icon(IconName::PanelRight, px(13.)).text_color(c.text_secondary))
                            .on_click(move |e, window, cx| {
                                cx.stop_propagation();
                                if let Some(open) = &open {
                                    open(e, window, cx);
                                }
                            }),
                    );
            if !self.show_hover {
                q = q
                    .invisible()
                    .group_hover(hover_group.clone(), |s| s.visible());
            }
            div().absolute().top(px(6.)).right(px(6.)).child(q)
        });
        let signal = compact
            .then(|| {
                vm.meta
                    .iter()
                    .find(|m| m.icon == IconName::Activity)
                    .map(|m| meta_item(m, &c))
            })
            .flatten();
        let head = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(18.))
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .whitespace_nowrap()
                    .child(SharedString::from(vm.key_label.clone())),
            )
            .children(
                vm.pinned
                    .then(|| icon(IconName::Pin, px(11.)).text_color(c.accent)),
            )
            .child(div().flex_1())
            .children(signal)
            .children(pill);

        let title: SharedString = vm.title.clone().into();
        let mut title_el = div()
            .id(SharedString::from(format!("{}-title", vm.key)))
            .text_style_of(text::CARD_TITLE)
            .text_color(c.text_primary)
            .tooltip(Tooltip::text(title.clone(), None))
            .child(title);
        title_el = if compact {
            title_el.truncate()
        } else {
            title_el.line_clamp(2)
        };

        let callout = (!compact).then(|| vm.callout.clone()).flatten().map(|co| {
            let (fg, bg) = tone_colors(co.tone, &c);
            let mut t = div()
                .flex_1()
                .text_size(px(11.))
                .line_height(relative(1.35))
                .text_color(c.text_primary)
                .child(SharedString::from(co.text));
            if co.mono {
                t = t.font_family(FONT_MONO);
            }
            div()
                .flex()
                .gap(px(6.))
                .px(px(7.))
                .py(px(5.))
                .rounded(px(radius::SM))
                .bg(bg)
                .child(icon(co.icon, px(12.)).text_color(fg).mt(px(1.)))
                .child(t)
        });

        let option_list = (!compact)
            .then(|| vm.options.clone())
            .flatten()
            .map(|options| {
                let handler = self.on_action.clone();
                let other = options.other_number();
                crate::components::answer_options::OptionList::new(
                    SharedString::from(format!("{}-options", vm.key)),
                    options.rows(),
                    move |pick, window, cx| {
                        let n = match pick {
                            crate::view_models::answer_options::Pick::Option(n) => n,
                            crate::view_models::answer_options::Pick::Other => other,
                        };
                        if let Some(h) = &handler {
                            h(CardActionKind::PickOption(n), window, cx);
                        }
                    },
                )
                .compact()
            });

        let deliver = self.on_action.clone();
        let deliver_id = SharedString::from(format!("{}-deliver-now", vm.key));
        let bars = vm.bars.iter().map(|bar| match bar {
            Bar::MessagesWaiting(n) => div()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(7.))
                .py(px(4.))
                .rounded(px(radius::SM))
                .bg(c.accent_soft)
                .child(icon(IconName::MessageSquareDot, px(12.)).text_color(c.accent))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c.text_primary)
                        .child(SharedString::from(format!(
                            "{n} message{} waiting",
                            if *n == 1 { "" } else { "s" }
                        ))),
                )
                .child({
                    let deliver = deliver.clone();
                    div()
                        .id(deliver_id.clone())
                        .text_size(px(11.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.accent)
                        .cursor_pointer()
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            if let Some(h) = &deliver {
                                h(CardActionKind::DeliverNow, window, cx);
                            }
                        })
                        .child("Deliver now")
                })
                .into_any_element(),
            Bar::Background { text, wakeup } => div()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(7.))
                .py(px(4.))
                .rounded(px(radius::SM))
                .bg(c.bg_column)
                .child(
                    icon(
                        if *wakeup {
                            IconName::AlarmClock
                        } else {
                            IconName::Cpu
                        },
                        px(12.),
                    )
                    .text_color(c.text_secondary),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.5))
                        .text_color(c.text_secondary)
                        .child(SharedString::from(text.clone())),
                )
                .into_any_element(),
        });

        let meta_items: Vec<AnyElement> = vm
            .meta
            .iter()
            .filter(|m| !(compact && m.icon == IconName::Activity))
            .map(|m| meta_item(m, &c))
            .collect();
        let has_meta = vm.epic.is_some() || !meta_items.is_empty();
        let meta = has_meta.then(|| {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_x(px(8.))
                .gap_y(px(4.))
                .children(vm.epic.clone().map(|e| epic_chip(e.into(), &c)))
                .children(meta_items)
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
                    let mut b = Button::new(
                        SharedString::from(format!("{}-action-{i}", vm.key)),
                        a.label,
                    )
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
            .group(hover_group)
            .relative()
            .flex()
            .flex_col()
            .gap(px(if compact { 4. } else { 6. }))
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
            .child(title_el)
            .children(callout)
            .children(option_list)
            .children(bars)
            .children(meta)
            .children(actions)
            .children(quick);
        if let Some(w) = self.width {
            card = card.w(px(w));
        } else {
            card = card.w_full();
        }
        if !self.selected && self.chain != ChainState::Highlighted {
            let hover_border = c.border_strong;
            card = card.hover(move |s| s.bg(c.bg_raised).border_color(hover_border));
        }
        if self.show_hover {
            card = card.bg(c.bg_raised).border_color(c.border_strong);
        }
        if self.dragging {
            card = card.border_color(c.border_strong).shadow(floating_shadow());
        }
        if let Some(opacity) = vm.faded {
            card = card.opacity(opacity);
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

fn meta_item(item: &crate::view_models::card::MetaItem, c: &Colors) -> AnyElement {
    let fg = match item.tone {
        Tone::Muted => c.text_muted,
        other => tone_colors(other, c).0,
    };
    let (icon_name, text) = (item.icon, item.text.as_str());
    let el = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(3.))
        .child(icon(icon_name, px(11.)).text_color(fg))
        .child(
            div()
                .font_family(FONT_MONO)
                .text_size(px(11.))
                .text_color(fg)
                .whitespace_nowrap()
                .child(SharedString::from(text.to_owned())),
        );
    match item.tooltip {
        Some(tip) => el
            .id(SharedString::from(format!("meta-tip-{text}")))
            .tooltip(crate::components::menu::Tooltip::text(tip, None))
            .into_any_element(),
        None => el.into_any_element(),
    }
}
