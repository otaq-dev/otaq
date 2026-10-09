//! The Stack list widget (DEP-05; d0006; *12 · Stack & dependencies → Stack list · detail*): the chain as an
//! indented list with statuses; the current ticket is marked; a click selects a line's ticket.

use std::rc::Rc;

use crewboard_core::domain::TicketKey;
use gpui::{App, FontWeight, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px};

use super::pill::StatusPill;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_MONO, radius};
use crate::view_models::stack::{StackRow, empty_text};

type SelectHandler = Rc<dyn Fn(TicketKey, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct StackList {
    current: TicketKey,
    rows: Vec<StackRow>,
    on_select: Option<SelectHandler>,
}

impl StackList {
    pub fn new(current: TicketKey, rows: Vec<StackRow>) -> Self {
        Self {
            current,
            rows,
            on_select: None,
        }
    }

    pub fn on_select(mut self, f: impl Fn(TicketKey, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for StackList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let empty = self.rows.is_empty().then(|| {
            div()
                .px(px(6.))
                .py(px(3.))
                .text_size(px(12.))
                .text_color(c.text_muted)
                .child(SharedString::from(empty_text(&self.current)))
        });
        let rows = self.rows.into_iter().enumerate().map(|(i, row)| {
            let select = self.on_select.clone();
            let key = row.key.clone();
            let mut line = div()
                .id(("stack-row", i))
                .flex()
                .items_center()
                .gap(px(6.))
                .py(px(3.))
                .pr(px(6.))
                .pl(px(6. + 14. * row.depth as f32))
                .rounded(px(radius::SM))
                .cursor_pointer()
                .children((row.depth > 0).then(|| {
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child("└")
                }))
                .child(
                    div()
                        .flex_none()
                        .font_family(FONT_MONO)
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if row.current {
                            c.accent
                        } else {
                            c.text_secondary
                        })
                        .child(SharedString::from(row.key.to_string())),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(12.))
                        .font_weight(if row.current {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(c.text_primary)
                        .child(SharedString::from(row.title.clone())),
                )
                .child(StatusPill::of(row.status))
                .on_click(move |_, window, cx| {
                    if let Some(f) = &select {
                        f(key.clone(), window, cx);
                    }
                });
            line = if row.current {
                line.bg(c.accent_soft)
            } else {
                line.hover(|s| s.bg(c.bg_hover))
            };
            line
        });
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .p(px(8.))
            .rounded(px(radius::LG))
            .bg(c.bg_app)
            .border_1()
            .border_color(c.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(4.))
                    .pb(px(4.))
                    .child(icon(IconName::Link2, px(12.)).text_color(c.text_muted))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.text_secondary)
                            .child("Stack"),
                    ),
            )
            .children(empty)
            .children(rows)
    }
}
