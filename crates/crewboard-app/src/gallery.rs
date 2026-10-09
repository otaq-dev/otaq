//! A hidden debug gallery for checking the UI by hand against `crewboard.pen` (T-0015, T-0016). It opens in its own
//! window when the app starts with `CREWBOARD_GALLERY=1`. The buttons at the top switch the theme at runtime.

use gpui::{
    AnyElement, App, Context, FontWeight, Hsla, IntoElement, SharedString, Window, div, prelude::*,
    px,
};

use crate::icons::{IconName, icon};
use crate::theme::{
    self, ActiveTheme, Colors, Status, Theme, ThemeChoice, Tier, density, radius, space, text,
};
use crate::ui::TextStyleExt;

/// The environment variable that opens the gallery: `CREWBOARD_GALLERY`.
pub fn env_var() -> String {
    crewboard_core::naming::env_var("GALLERY")
}

pub fn requested() -> bool {
    std::env::var_os(env_var()).is_some_and(|v| !v.is_empty() && v != "0")
}

pub struct Gallery;

impl Gallery {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe_window_appearance(window, |_, window, cx| {
            let choice = cx.global::<ActiveTheme>().choice;
            ActiveTheme::update(choice, window.appearance(), cx);
        })
        .detach();
        Self
    }
}

fn section(title: &str, t: &Theme, body: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(space::S3))
        .p(px(space::S5))
        .bg(t.colors.bg_panel)
        .border_1()
        .border_color(t.colors.border)
        .rounded(px(radius::DIALOG))
        .child(
            div()
                .text_style_of(text::COLUMN_TITLE)
                .text_size(px(13.))
                .text_color(t.colors.text_primary)
                .child(SharedString::from(title.to_owned())),
        )
        .child(body)
}

fn label(t: &Theme, s: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_style_of(text::CODE)
        .text_size(px(11.))
        .text_color(t.colors.text_secondary)
        .child(s.into())
}

/// A status pill as in the legend: 12 px icon + 11/500 label on the status background.
pub fn status_pill(t: &Theme, status: Status, label: Option<&str>) -> impl IntoElement {
    let style = t.status(status);
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(5.))
        .pt(px(3.))
        .pb(px(3.))
        .pl(px(6.))
        .pr(px(7.))
        .rounded(px(radius::SM))
        .bg(style.bg)
        .text_color(style.fg)
        .child(icon(style.icon, theme::ICON_SMALL).text_color(style.fg))
        .child(div().text_style_of(text::PILL).child(SharedString::from(
            label.unwrap_or(status.label()).to_owned(),
        )))
}

fn swatches(t: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .gap(px(space::S3))
        .children(Colors::NAMES.iter().map(|name| {
            let color: Hsla = t.colors.get(name).expect("token");
            div()
                .flex()
                .flex_col()
                .gap(px(space::S1))
                .w(px(112.))
                .child(
                    div()
                        .h(px(36.))
                        .rounded(px(radius::MD))
                        .border_1()
                        .border_color(t.colors.border)
                        .bg(color),
                )
                .child(label(t, *name))
        }))
}

fn legend(t: &Theme) -> impl IntoElement {
    let tiers = [
        (Tier::NeedsYou, "1 · Needs you"),
        (Tier::Review, "2 · Waiting on your review"),
        (Tier::Working, "3 · Working normally"),
        (Tier::Done, "4 · Fades back"),
    ];
    let live_label = |s: Status| match s {
        Status::Conflicts => Some("Conflicts with main"),
        Status::Running => Some("Running :4020"),
        Status::Asked => Some("Asked SHOP-40.1"),
        Status::Epic => Some("Epic · 1 / 3 merged"),
        _ => None,
    };
    div()
        .flex()
        .flex_col()
        .gap(px(space::S4))
        .children(tiers.into_iter().map(|(tier, title)| {
            div()
                .flex()
                .flex_col()
                .gap(px(space::S2))
                .child(
                    div()
                        .text_style_of(text::COLUMN_TITLE)
                        .text_size(px(13.))
                        .text_color(t.colors.text_primary)
                        .child(title),
                )
                .child(
                    div().flex().flex_wrap().gap(px(space::S2)).children(
                        Status::ALL
                            .iter()
                            .filter(|s| s.spec().0 == tier)
                            .map(|s| status_pill(t, *s, live_label(*s))),
                    ),
                )
        }))
}

fn type_scale(t: &Theme) -> impl IntoElement {
    let code_sample = format!("{}  a1b2c3d", crewboard_core::naming::branch("SHOP-18"));
    let rows = [
        (
            "Display / dialog title",
            "20 / 600 / -0.3",
            text::DISPLAY,
            "Approve SHOP-18 Checkout API",
        ),
        ("Header · project", "15 / 600", text::HEADER, "shop-web"),
        (
            "Column title",
            "12 / 600",
            text::COLUMN_TITLE,
            "Agent Working  3",
        ),
        (
            "Card title",
            "13 / 500 / 1.35",
            text::CARD_TITLE,
            "Fix flaky login redirect",
        ),
        (
            "Body",
            "13 / 400 / 1.5",
            text::BODY,
            "Colors are hard-coded in 14 files.",
        ),
        (
            "Metadata",
            "11 / 400",
            text::METADATA,
            "active 40 s ago · 182k tokens",
        ),
        ("Code / IDs", "12 mono", text::CODE, code_sample.as_str()),
        (
            "Terminal",
            "13 mono / 1.45",
            text::TERMINAL,
            "● Read(src/app/checkout/page.tsx)",
        ),
    ];
    div()
        .flex()
        .flex_col()
        .gap(px(space::S3))
        .children(rows.into_iter().map(|(name, spec, style, sample)| {
            div()
                .flex()
                .items_center()
                .gap(px(space::S4))
                .child(
                    div()
                        .w(px(180.))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_style_of(text::METADATA)
                                .text_size(px(12.))
                                .text_color(t.colors.text_secondary)
                                .child(name),
                        )
                        .child(label(t, spec)),
                )
                .child(
                    div()
                        .text_style_of(style)
                        .text_color(t.colors.text_primary)
                        .child(SharedString::from(sample.to_owned())),
                )
        }))
}

fn spacing(t: &Theme) -> impl IntoElement {
    let steps = [
        ("s-1", space::S1),
        ("s-2", space::S2),
        ("s-3", space::S3),
        ("s-4", space::S4),
        ("s-5", space::S5),
        ("s-6", space::S6),
    ];
    div()
        .flex()
        .flex_col()
        .gap(px(space::S3))
        .child(
            div().flex().items_end().gap(px(space::S4)).children(steps.into_iter().map(
                |(name, size)| {
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(space::S1))
                        .child(div().size(px(size)).bg(t.colors.accent).rounded(px(2.)))
                        .child(label(t, name))
                        .child(label(t, format!("{size} px")))
                },
            )),
        )
        .child(
            div().flex().gap(px(space::S4)).children(
                [
                    ("radius sm", radius::SM),
                    ("radius md", radius::MD),
                    ("radius lg", radius::LG),
                    ("dialog", radius::DIALOG),
                ]
                .into_iter()
                .map(|(name, r)| {
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(space::S1))
                        .child(
                            div()
                                .size(px(40.))
                                .rounded(px(r))
                                .border_1()
                                .border_color(t.colors.border_strong)
                                .bg(t.colors.bg_card),
                        )
                        .child(label(t, format!("{name} {r}")))
                }),
            ),
        )
        .child(label(
            t,
            format!(
                "Column {} px (comfortable) · {} px (compact) · Card padding {} × {} · Header {} px · Row {} px",
                density::COLUMN,
                density::COLUMN_COMPACT,
                density::CARD_PADDING.0,
                density::CARD_PADDING.1,
                density::HEADER,
                density::ROW
            ),
        ))
}

fn icons(t: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .gap(px(space::S3))
        .children(IconName::ALL.iter().map(|name| {
            div()
                .flex()
                .items_center()
                .gap(px(space::S2))
                .w(px(200.))
                .child(icon(*name, theme::ICON).text_color(t.colors.text_primary))
                .child(label(t, name.lucide_name()))
        }))
}

fn theme_button(t: &Theme, choice: ThemeChoice, current: ThemeChoice) -> AnyElement {
    let selected = choice == current;
    div()
        .id(SharedString::from(format!("theme-{choice:?}")))
        .px(px(10.))
        .py(px(4.))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(if selected {
            t.colors.accent
        } else {
            t.colors.border
        })
        .bg(if selected {
            t.colors.accent_soft
        } else {
            t.colors.bg_panel
        })
        .text_color(t.colors.text_primary)
        .text_style_of(text::METADATA)
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .child(SharedString::from(format!("{choice:?}")))
        .on_click(move |_, window, cx: &mut App| {
            ActiveTheme::update(choice, window.appearance(), cx);
        })
        .into_any_element()
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = cx.global::<ActiveTheme>();
        let t = active.theme.clone();
        let choice = active.choice;
        div()
            .id("gallery")
            .size_full()
            .overflow_y_scroll()
            .bg(t.colors.bg_app)
            .font_family(theme::FONT_UI)
            .p(px(space::S6))
            .flex()
            .flex_col()
            .gap(px(space::S5))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S3))
                    .child(
                        div()
                            .text_style_of(text::DISPLAY)
                            .text_color(t.colors.text_primary)
                            .child("Gallery · 01 · Foundations"),
                    )
                    .children(
                        [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark]
                            .into_iter()
                            .map(|c| theme_button(&t, c, choice)),
                    ),
            )
            .child(section("Color Tokens", &t, swatches(&t)))
            .child(section("Status Legend", &t, legend(&t)))
            .child(section("Type scale", &t, type_scale(&t)))
            .child(section("Spacing · radius · density", &t, spacing(&t)))
            .child(section(
                "Icons · Lucide (ISC), 14 px / 1.5 stroke",
                &t,
                icons(&t),
            ))
    }
}
