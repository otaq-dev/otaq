//! The numbered answer options (ATT-12; d0061; *10 → Answer · inline*, *04 → Row · answer options*, *05 → SHOP-21 ·
//! Asks you · answer options*): one list for the inbox, the card and the panel. The recommended option has the
//! "Recommended" badge ("Rec." on a card); the picked one is outlined; each row shows the number that picks it;
//! Other… is the last. Keys (1–4, ↵, Esc) are bound by the owner on [`CONTEXT`].

use std::rc::Rc;

use gpui::{
    App, ElementId, FontWeight, IntoElement, KeyBinding, RenderOnce, SharedString, Window, actions,
    div, prelude::*, px,
};

use super::kbd::Kbd;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_MONO};
use crate::view_models::answer_options::{OptionRow, Pick};

actions!(
    answer_options,
    [
        PickOption1,
        PickOption2,
        PickOption3,
        PickOption4,
        SendPick,
        CancelPick
    ]
);

/// The key context of a focused option list.
pub const CONTEXT: &str = "AnswerOptions";

pub fn bind_keys(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("1", PickOption1, c),
        KeyBinding::new("2", PickOption2, c),
        KeyBinding::new("3", PickOption3, c),
        KeyBinding::new("4", PickOption4, c),
        KeyBinding::new("enter", SendPick, c),
        KeyBinding::new("escape", CancelPick, c),
    ]);
}

type OnPick = Rc<dyn Fn(Pick, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct OptionList {
    id: ElementId,
    rows: Vec<OptionRow>,
    compact: bool,
    on_pick: OnPick,
}

impl OptionList {
    pub fn new(
        id: impl Into<ElementId>,
        rows: Vec<OptionRow>,
        on_pick: impl Fn(Pick, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            rows,
            compact: false,
            on_pick: Rc::new(on_pick),
        }
    }
}

impl RenderOnce for OptionList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let compact = self.compact;
        let list_id = format!("{:?}", self.id);
        let rows = self.rows.into_iter().map(|row| {
            let pick = if row.other {
                Pick::Other
            } else {
                Pick::Option(row.number)
            };
            let on_pick = self.on_pick.clone();
            let badge = row.recommended.then(|| {
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(3.))
                    .px(px(5.))
                    .rounded(px(4.))
                    .bg(c.st_approved_bg)
                    .text_size(px(10.))
                    .text_color(c.st_approved)
                    .child(icon(IconName::Sparkles, px(10.)).text_color(c.st_approved))
                    .child(if compact { "Rec." } else { "Recommended" })
            });
            let number = if compact {
                div()
                    .flex_none()
                    .size(px(16.))
                    .flex()
                    .justify_center()
                    .items_center()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(c.border_strong)
                    .font_family(FONT_MONO)
                    .text_size(px(10.))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(row.number.to_string()))
                    .into_any_element()
            } else {
                div()
                    .flex_none()
                    .size(px(14.))
                    .rounded_full()
                    .border_1()
                    .border_color(if row.picked {
                        c.accent
                    } else {
                        c.border_strong
                    })
                    .when(row.picked, |d| d.border(px(4.)))
                    .into_any_element()
            };
            div()
                .id(SharedString::from(format!("{list_id}-{}", row.number)))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(if compact { 6. } else { 10. }))
                .py(px(if compact { 3. } else { 7. }))
                .rounded(px(6.))
                .border_1()
                .border_color(if row.picked { c.accent } else { c.border })
                .when(row.picked, |d| d.bg(c.accent_soft))
                .when(!row.picked, |d| {
                    d.bg(c.bg_raised).hover(|s| s.bg(c.bg_hover))
                })
                .cursor_pointer()
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    on_pick(pick, window, cx)
                })
                .child(number)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(if compact { 12. } else { 13. }))
                        .when(row.picked && !row.other, |d| {
                            d.font_weight(FontWeight::SEMIBOLD)
                        })
                        .text_color(if row.other {
                            c.text_muted
                        } else {
                            c.text_primary
                        })
                        .child(SharedString::from(if compact && row.other {
                            "Other…".to_owned()
                        } else {
                            row.label.clone()
                        })),
                )
                .children(badge)
                .when(!compact, |d| {
                    d.child(Kbd::text(SharedString::from(row.number.to_string())))
                })
        });
        div()
            .id(self.id)
            .flex()
            .flex_col()
            .gap(px(if compact { 4. } else { 6. }))
            .children(rows)
    }
}
