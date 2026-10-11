//! The theme picker (UI-11, d0069, d0070; *14 · First run & onboarding → 2 · Theme*, *15 · Settings → Global ·
//! Usage, notifications, data, appearance → Dark theme, Light theme*), shared by onboarding and Settings: the 15
//! theme cards under a Dark and a Light heading. Each card's preview is a small window drawn with that theme's own
//! colours. A click hands the theme to the caller, which applies and saves it at once.

use std::rc::Rc;

use gpui::{App, ElementId, FontWeight, IntoElement, Window, div, prelude::*, px, relative};

use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, Colors, ThemeId};
use crate::view_models::theme_picker::{PickerSection, ThemeCard, sections};

/// Called with the theme after a click.
pub type OnPick = Rc<dyn Fn(ThemeId, &mut Window, &mut App)>;

/// The Dark and Light groups of cards.
#[derive(IntoElement)]
pub struct ThemeSections {
    id: ElementId,
    selected: ThemeId,
    on_pick: OnPick,
    /// Under the Settings row "Theme", which draws the line above; a line closes the picker.
    in_rows: bool,
}

impl ThemeSections {
    pub fn new(id: impl Into<ElementId>, selected: ThemeId, on_pick: OnPick) -> Self {
        Self {
            id: id.into(),
            selected,
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
                sections(self.selected)
                    .into_iter()
                    .enumerate()
                    .map(|(i, s)| {
                        // *Dark theme* has the line above it in onboarding and sits right under the row "Theme" in
                        // Settings; *Light theme* follows with no line.
                        let first = i == 0;
                        let top = match (first, in_rows) {
                            (true, false) => 16.,
                            (true, true) => 2.,
                            (false, _) => 0.,
                        };
                        let bottom = if first { 18. } else { 20. };
                        let id = super::sub_id(&id, i);
                        section(id, s, &self.on_pick, first && !in_rows, (top, bottom), &c)
                    }),
            )
    }
}

fn section(
    id: ElementId,
    s: PickerSection,
    on_pick: &OnPick,
    top_line: bool,
    (top, bottom): (f32, f32),
    c: &Colors,
) -> impl IntoElement {
    let rows = s.rows.into_iter().enumerate().map(|(r, row)| {
        div().flex().gap(px(12.)).children(
            row.into_iter().enumerate().map(|(i, card)| {
                theme_card(super::sub_id(&id, r * 100 + i), card, on_pick.clone(), c)
            }),
        )
    });
    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .pt(px(top))
        .pb(px(bottom))
        .when(top_line, |el| el.border_t_1().border_color(c.border))
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c.text_secondary)
                .child(s.label),
        )
        .child(div().flex().flex_col().gap(px(18.)).children(rows))
}

fn theme_card(id: ElementId, card: ThemeCard, on_pick: OnPick, c: &Colors) -> impl IntoElement {
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
        .on_click(move |_, window, cx| on_pick(theme, window, cx))
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
