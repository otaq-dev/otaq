//! A number stepper, `[ − 5 + ]` (*15 · Settings → Global · Agent limits ON*, *02 · Header · agent limits ON +
//! inline popover*): three 28 px segments in one strong-bordered box; − and + change the value by one and stop at
//! the range's ends.

use std::ops::RangeInclusive;
use std::rc::Rc;

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px};

use crate::components::sub_id;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_MONO};
use crate::view_models::limits::step;

type ChangeHandler = Rc<dyn Fn(u32, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Stepper {
    id: ElementId,
    value: u32,
    range: RangeInclusive<u32>,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl Stepper {
    pub fn new(id: impl Into<ElementId>, value: u32, range: RangeInclusive<u32>) -> Self {
        Self {
            id: id.into(),
            value,
            range,
            disabled: false,
            on_change: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(mut self, handler: impl Fn(u32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Stepper {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let value = self.value;
        let segment = |n: usize, child: gpui::AnyElement| {
            div()
                .id(sub_id(&self.id, n))
                .w(px(28.))
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .child(child)
        };
        let button = |n: usize, up: bool| {
            let next = step(value, up, &self.range);
            let can = !self.disabled && next != value;
            let mut el = segment(
                n,
                icon(if up { IconName::Plus } else { IconName::Minus }, px(12.))
                    .text_color(if can { c.text_secondary } else { c.text_muted })
                    .into_any_element(),
            );
            if can {
                let handler = self.on_change.clone();
                el = el.cursor_pointer().hover(|s| s.bg(c.bg_hover)).on_click(
                    move |_, window, cx| {
                        if let Some(h) = &handler {
                            h(next, window, cx);
                        }
                    },
                );
            }
            el
        };
        let mut el = div()
            .id(self.id.clone())
            .flex()
            .flex_none()
            .items_center()
            .overflow_hidden()
            .rounded(px(5.))
            .border_1()
            .border_color(c.border_strong)
            .child(button(0, false))
            .child(segment(
                1,
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(12.))
                    .text_color(c.text_primary)
                    .child(SharedString::from(value.to_string()))
                    .into_any_element(),
            ))
            .child(button(2, true));
        if self.disabled {
            el = el.opacity(0.5);
        }
        el
    }
}
