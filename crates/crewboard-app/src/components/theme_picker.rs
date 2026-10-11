//! The theme picker (UI-11, d0069; *14 · First run & onboarding → 2 · Theme*, *15 · Settings → Global · Usage,
//! notifications, data, appearance → Dark theme, Light theme*), shared by onboarding and Settings: the mode switch
//! and the Dark theme / Light theme sections of cards. Each card's preview is a small window drawn with that
//! theme's own colours. A click hands the new [`ThemeChoice`] to the caller, which applies and saves it at once.

use std::rc::Rc;

use gpui::{App, ElementId, FontWeight, IntoElement, Window, div, prelude::*, px, relative};

use super::tabs::Segmented;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, Appearance, Colors, ThemeChoice, ThemeId};
use crate::view_models::theme_picker::{
    MODES, PickerSection, ThemeCard, mode_index, pick_mode, pick_theme, sections,
};

/// Called with the choice after a click.
pub type OnPick = Rc<dyn Fn(ThemeChoice, &mut Window, &mut App)>;

/// The Light / Dark / System switch (*Mode*).
pub fn mode_switch(id: impl Into<ElementId>, choice: ThemeChoice, on_pick: OnPick) -> Segmented {
    Segmented::new(
        id,
        MODES.iter().map(|(_, label)| (*label).into()).collect(),
        mode_index(choice.mode),
    )
    .on_select(move |i, window, cx| on_pick(pick_mode(choice, i), window, cx))
}

/// The Dark theme and Light theme sections.
#[derive(IntoElement)]
pub struct ThemeSections {
    id: ElementId,
    choice: ThemeChoice,
    os: Appearance,
    on_pick: OnPick,
    /// Between Settings rows: the row above draws the first line, and a line closes the last section.
    in_rows: bool,
}

impl ThemeSections {
    /// `os` is the window's appearance, for the "In use" tag in System mode.
    pub fn new(
        id: impl Into<ElementId>,
        choice: ThemeChoice,
        os: Appearance,
        on_pick: OnPick,
    ) -> Self {
        Self {
            id: id.into(),
            choice,
            os,
            on_pick,
            in_rows: false,
        }
    }

    /// For a list of rows that each draw a bottom line (Settings → Appearance).
    pub fn in_rows(mut self) -> Self {
        self.in_rows = true;
        self
    }
}

impl RenderOnce for ThemeSections {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let id = self.id.clone();
        let in_rows = self.in_rows;
        div()
            .id(self.id)
            .flex()
            .flex_col()
            .when(in_rows, |el| el.border_b_1().border_color(c.border))
            .children(
                sections(self.choice, self.os)
                    .into_iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let top_line = !(in_rows && i == 0);
                        let id = super::sub_id(&id, i);
                        section(id, s, self.choice, &self.on_pick, top_line, &c)
                    }),
            )
    }
}

fn section(
    id: ElementId,
    s: PickerSection,
    choice: ThemeChoice,
    on_pick: &OnPick,
    top_line: bool,
    c: &Colors,
) -> impl IntoElement {
    let in_use = s.in_use.map(|tag| {
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .py(px(3.))
            .rounded(px(4.))
            .bg(c.accent_soft)
            .child(div().size(px(6.)).rounded_full().bg(c.accent))
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c.accent)
                    .child(tag),
            )
    });
    let rows = s.rows.into_iter().enumerate().map(|(r, row)| {
        div()
            .flex()
            .gap(px(12.))
            .children(row.into_iter().enumerate().map(|(i, card)| {
                theme_card(
                    super::sub_id(&id, r * 100 + i),
                    card,
                    choice,
                    on_pick.clone(),
                    c,
                )
            }))
    });
    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .pt(px(14.))
        .pb(px(20.))
        .when(top_line, |el| el.border_t_1().border_color(c.border))
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(24.))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c.text_primary)
                                .child(s.label),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(c.text_secondary)
                                .child(s.description),
                        ),
                )
                .children(in_use),
        )
        .child(div().flex().flex_col().gap(px(18.)).children(rows))
}

fn theme_card(
    id: ElementId,
    card: ThemeCard,
    choice: ThemeChoice,
    on_pick: OnPick,
    c: &Colors,
) -> impl IntoElement {
    let theme = card.id;
    // The ring keeps 3 px around the preview: a 2 px accent border when selected, 1 px otherwise.
    let ring = div().rounded(px(8.)).map(|ring| {
        if card.selected {
            ring.border_2().border_color(c.accent).p(px(1.))
        } else {
            ring.border_1().border_color(c.border).p(px(2.))
        }
    });
    let hover = c.border_strong;
    div()
        .id(id)
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(8.))
        .cursor_pointer()
        .child(
            ring.when(!card.selected, |ring| {
                ring.group_hover("theme-card", |s| s.border_color(hover))
            })
            .child(preview(theme)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .px(px(2.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(px(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c.text_primary)
                                .child(card.name),
                        )
                        .when(card.selected, |row| {
                            row.child(icon(IconName::Check, px(12.)).text_color(c.accent))
                        }),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(11.))
                        .text_color(c.text_secondary)
                        .child(card.tagline),
                ),
        )
        .group("theme-card")
        .on_click(move |_, window, cx| on_pick(pick_theme(choice, theme), window, cx))
}

/// A small window drawn with the theme's own colours (*Preview · <Name>*): title bar, sidebar, three columns.
fn preview(id: ThemeId) -> impl IntoElement {
    let t = Colors::new(id);
    let bar = |w: f32, color| div().w(px(w)).h(px(3.)).rounded(px(1.5)).bg(color);
    let dot = |size: f32, color| div().flex_none().size(px(size)).rounded_full().bg(color);
    let mini_card = |status| {
        div()
            .h(px(12.))
            .flex()
            .items_center()
            .gap(px(2.))
            .px(px(3.))
            .rounded(px(2.))
            .bg(t.bg_card)
            .border_1()
            .border_color(t.border)
            .child(dot(3., status))
            .child(
                div()
                    .flex_1()
                    .h(px(2.))
                    .rounded(px(1.))
                    .bg(t.text_secondary),
            )
    };
    let column = |statuses: &[gpui::Hsla]| {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(div().w(px(12.)).h(px(3.)).rounded(px(1.)).bg(t.text_muted))
            .children(statuses.iter().map(|s| mini_card(*s)))
    };
    div()
        .h(px(78.))
        .flex()
        .flex_col()
        .rounded(px(5.))
        .overflow_hidden()
        .bg(t.bg_column)
        .child(
            div()
                .h(px(11.))
                .flex()
                .items_center()
                .gap(px(2.5))
                .px(px(5.))
                .bg(t.bg_app)
                .child(dot(4., t.st_error))
                .child(dot(4., t.st_paused))
                .child(dot(4., t.st_approved))
                .child(div().flex_1())
                .child(bar(18., t.border_strong)),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .child(
                    div()
                        .w(px(24.))
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .px(px(4.))
                        .py(px(6.))
                        .bg(t.bg_app)
                        .border_r_1()
                        .border_color(t.border)
                        .child(
                            div()
                                .w(relative(1.))
                                .h(px(3.))
                                .rounded(px(1.5))
                                .bg(t.accent),
                        )
                        .child(bar(12., t.border_strong))
                        .child(bar(14., t.border_strong))
                        .child(bar(10., t.border_strong)),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .gap(px(4.))
                        .p(px(5.))
                        .child(column(&[t.st_working, t.st_investigate]))
                        .child(column(&[t.st_paused, t.st_error]))
                        .child(column(&[t.st_approved])),
                ),
        )
}
