//! A hidden debug gallery for checking the UI by hand against `crewboard.pen` (T-0015, T-0016). It opens in its own
//! window when the app starts with `CREWBOARD_GALLERY=1`. The buttons at the top switch the mode and show any of the
//! 15 bundled themes (to compare with *01 · Theme palettes*) at runtime.

use std::time::Instant;

use gpui::{
    AnyElement, App, Context, FontWeight, Hsla, IntoElement, SharedString, Window, div, prelude::*,
    px,
};

use crate::components::banner::{Banner, BannerAction, BannerTone};
use crate::components::button::{ActionButtonState, Button, IconButton, Shown, Variant};
use crate::components::kbd::Kbd;
use crate::components::toast::{Toast, ToastKind};
use crate::icons::{IconName, icon};
use crate::keymap::{KeyAction, Platform};
use crate::root;
use crate::theme::{
    self, ActiveTheme, Colors, Mode, Status, Theme, ThemeChoice, ThemeId, Tier, density, radius,
    space, text,
};
use crate::ui::TextStyleExt;
use crate::view_models::banners::{BannerModel, Situation};
use crate::view_models::toasts::{ToastLevel, ToastModel};

/// The environment variable that opens the gallery: `CREWBOARD_GALLERY`.
pub fn env_var() -> String {
    crewboard_core::naming::env_var("GALLERY")
}

pub fn requested() -> bool {
    std::env::var_os(env_var()).is_some_and(|v| !v.is_empty() && v != "0")
}

/// The banner-area and toast demo: raise situations and push toasts to check stacking by hand.
struct StackDemo {
    banners: BannerModel,
    toasts: ToastModel,
}

pub struct Gallery {
    stack: StackDemo,
    components: gpui::Entity<crate::gallery_components::ComponentsDemo>,
    /// The live "Approve" demo: a press turns on loading for two seconds; presses meanwhile are ignored.
    demo: ActionButtonState,
    demo_runs: u32,
}

impl Gallery {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe_window_appearance(window, |_, window, cx| {
            let choice = cx.global::<ActiveTheme>().choice;
            ActiveTheme::update(choice, window.appearance(), cx);
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                let alive = this.update(cx, |g, cx| {
                    if g.stack.toasts.expire(Instant::now()) {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    return;
                }
            }
        })
        .detach();
        Self {
            components: crate::gallery_components::new(window, cx),
            stack: StackDemo {
                banners: BannerModel::default(),
                toasts: ToastModel::default(),
            },
            demo: ActionButtonState::default(),
            demo_runs: 0,
        }
    }

    fn press_demo(&mut self, cx: &mut Context<Self>) {
        if !self.demo.press() {
            return;
        }
        self.demo_runs += 1;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            let _ = this.update(cx, |gallery, cx| {
                gallery.demo.finish();
                cx.notify();
            });
        })
        .detach();
    }

    fn stack_demo(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let situations: [(&str, Situation); 6] = [
            ("Service stopped", Situation::ServiceStopped),
            (
                "Restart required",
                Situation::RestartRequired {
                    working: 3,
                    waiting: 2,
                },
            ),
            (
                "Usage limit",
                Situation::UsageLimit {
                    title: "Session limit reached".into(),
                    text: "Resets at 3:45 pm · 8 agents stopped · no new tickets will start".into(),
                    resume: Some(crate::view_models::limit_banner::ResumeAll {
                        label: "Resume all".into(),
                        enabled: false,
                        primary: false,
                    }),
                },
            ),
            ("Permission warning", Situation::PermissionWarning),
            (
                "Update ready",
                Situation::ServiceUpdateReady {
                    working: 3,
                    waiting: 2,
                },
            ),
            ("Reconnecting", Situation::Reconnecting),
        ];
        let toggles = situations
            .into_iter()
            .enumerate()
            .map(|(i, (name, situation))| {
                let kind = situation.kind();
                let on = self.stack.banners.is_active(kind);
                let this = cx.entity().downgrade();
                Button::new(("situation", i), name)
                    .variant(if on {
                        Variant::Primary
                    } else {
                        Variant::Secondary
                    })
                    .on_click(move |_, _, cx| {
                        let _ = this.update(cx, |g, cx| {
                            if g.stack.banners.is_active(kind) {
                                g.stack.banners.clear(kind);
                            } else {
                                g.stack.banners.raise(situation.clone());
                            }
                            cx.notify();
                        });
                    })
                    .into_any_element()
            });
        let ok = cx.entity().downgrade();
        let err = cx.entity().downgrade();
        let rows: Vec<AnyElement> = self
            .stack
            .banners
            .rows()
            .into_iter()
            .map(|row| {
                let kind = row.kind;
                let this = cx.entity().downgrade();
                root::banner_element(
                    row,
                    false,
                    |_, _| {},
                    |_, _| {},
                    move |_, cx| {
                        let _ = this.update(cx, |g, cx| {
                            g.stack.banners.dismiss(kind);
                            cx.notify();
                        });
                    },
                )
            })
            .collect();
        let close = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .gap(px(space::S3))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(space::S2))
                    .children(toggles),
            )
            .child(
                div()
                    .flex()
                    .gap(px(space::S2))
                    .child(
                        Button::new("push-ok", "Push success toast")
                            .variant(Variant::Secondary)
                            .on_click(move |_, _, cx| {
                                let _ = ok.update(cx, |g, cx| {
                                    g.stack.toasts.push(
                                        ToastLevel::Success,
                                        "Local merge done · SHOP-18",
                                        Some("Squashed into main as 9f3e2c1".into()),
                                        Instant::now(),
                                    );
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        Button::new("push-err", "Push error toast")
                            .variant(Variant::Secondary)
                            .on_click(move |_, _, cx| {
                                let _ = err.update(cx, |g, cx| {
                                    g.stack.toasts.push_with_copy(
                                        "gh is not logged in",
                                        Some("Run gh auth login, then retry Open PR.".into()),
                                        "Copy command",
                                        "gh auth login",
                                        Instant::now(),
                                    );
                                    cx.notify();
                                });
                            }),
                    ),
            )
            .child(
                div()
                    .relative()
                    .h(px(260.))
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(t.colors.border_strong)
                    .bg(t.colors.bg_app)
                    .children(rows)
                    .child(root::toast_stack(
                        &self.stack.toasts,
                        &ActiveTheme::get(cx).colors,
                        move |id, cx| {
                            let _ = close.update(cx, |g, cx| {
                                g.stack.toasts.dismiss(id);
                                cx.notify();
                            });
                        },
                    )),
            )
    }

    fn demo_row(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        div()
            .flex()
            .items_center()
            .gap(px(space::S3))
            .child(
                Button::new(
                    "demo",
                    if self.demo.loading { "Merging…" } else { "Approve" },
                )
                .icon(IconName::Check)
                .loading(self.demo.loading)
                .on_click(move |_, _, cx| {
                    let _ = this.update(cx, |gallery, cx| gallery.press_demo(cx));
                }),
            )
            .child(label(
                t,
                format!(
                    "Live: click (or double-click) — the action ran {} time(s); clicks while loading are ignored",
                    self.demo_runs
                ),
            ))
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
    let ansi = t
        .ansi
        .iter()
        .enumerate()
        .map(|(i, rgb)| (format!("ansi-{i}"), Hsla::from(gpui::rgb(*rgb))));
    let tokens = Colors::NAMES
        .iter()
        .map(|name| (name.to_string(), t.colors.get(name).expect("token")));
    div()
        .flex()
        .flex_wrap()
        .gap(px(space::S3))
        .children(tokens.chain(ansi).map(|(name, color)| {
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
                .child(label(t, name))
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

fn theme_button(
    t: &Theme,
    id: &str,
    label: SharedString,
    selected: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(format!("theme-{id}")))
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
        .child(label)
}

/// The mode buttons: System, Light, Dark.
fn mode_button(t: &Theme, mode: Mode, current: ThemeChoice) -> AnyElement {
    let label = match mode {
        Mode::System => "System",
        Mode::Light => "Light",
        Mode::Dark => "Dark",
    };
    theme_button(t, label, label.into(), current.mode == mode)
        .on_click(move |_, window, cx: &mut App| {
            let choice = ThemeChoice {
                mode,
                ..cx.global::<ActiveTheme>().choice
            };
            ActiveTheme::update(choice, window.appearance(), cx);
        })
        .into_any_element()
}

/// The palette switch: shows one theme, in the mode of its kind, to compare with *01 · Theme palettes*.
fn palette_button(t: &Theme, id: ThemeId) -> AnyElement {
    theme_button(t, &format!("{id:?}"), id.name(), t.id == id)
        .on_click(move |_, window, cx: &mut App| {
            let mode = match id.appearance() {
                theme::Appearance::Dark => Mode::Dark,
                theme::Appearance::Light => Mode::Light,
            };
            let choice = ThemeChoice {
                mode,
                ..cx.global::<ActiveTheme>().choice.with_theme(id)
            };
            ActiveTheme::update(choice, window.appearance(), cx);
        })
        .into_any_element()
}

#[derive(Clone, Copy)]
enum Cell {
    Default,
    Hover,
    Pressed,
    Focused,
    Disabled,
    Loading,
}

const CELLS: [(Cell, &str); 6] = [
    (Cell::Default, "DEFAULT"),
    (Cell::Hover, "HOVER"),
    (Cell::Pressed, "PRESSED"),
    (Cell::Focused, "FOCUSED"),
    (Cell::Disabled, "DISABLED"),
    (Cell::Loading, "LOADING"),
];

fn matrix_button(variant: Variant, cell: Cell, row: usize, col: usize) -> AnyElement {
    let (label, loading_label, icon_name) = match variant {
        Variant::Primary => ("Approve", "Merging…", IconName::Check),
        Variant::Secondary => ("Send back", "Working…", IconName::Undo2),
        Variant::Destructive => ("Mark closed", "Working…", IconName::X),
        Variant::Ghost => ("View parent", "Working…", IconName::ArrowUpRight),
    };
    let loading = matches!(cell, Cell::Loading);
    let mut b = Button::new(
        SharedString::from(format!("m-{row}-{col}")),
        if loading { loading_label } else { label },
    )
    .variant(variant)
    .icon(icon_name)
    .loading(loading)
    .disabled(matches!(cell, Cell::Disabled));
    b = match cell {
        Cell::Hover => b.shown(Shown::Hover),
        Cell::Pressed => b.shown(Shown::Pressed),
        Cell::Focused => b.shown(Shown::Focused),
        _ => b,
    };
    b.into_any_element()
}

fn matrix_icon_button(cell: Cell, col: usize) -> AnyElement {
    let mut b = IconButton::new(
        SharedString::from(format!("mi-{col}")),
        IconName::SquareTerminal,
    )
    .loading(matches!(cell, Cell::Loading))
    .disabled(matches!(cell, Cell::Disabled))
    .on_click(|_, _, _| {});
    b = match cell {
        Cell::Hover => b.shown(Shown::Hover),
        Cell::Pressed => b.shown(Shown::Pressed),
        Cell::Focused => b.shown(Shown::Focused),
        _ => b,
    };
    b.into_any_element()
}

fn button_matrix(t: &Theme) -> impl IntoElement {
    let cell_w = px(150.);
    let head = div().flex().child(div().w(px(110.))).children(
        CELLS
            .iter()
            .map(|(_, name)| div().w(cell_w).child(label(t, *name))),
    );
    let variants = [
        (Variant::Primary, "Primary"),
        (Variant::Secondary, "Secondary"),
        (Variant::Destructive, "Destructive"),
        (Variant::Ghost, "Ghost"),
    ];
    let rows = variants
        .into_iter()
        .enumerate()
        .map(|(row, (variant, name))| {
            div()
                .flex()
                .items_center()
                .h(px(44.))
                .child(div().w(px(110.)).child(label(t, name)))
                .children(CELLS.iter().enumerate().map(|(col, (cell, _))| {
                    div()
                        .w(cell_w)
                        .child(matrix_button(variant, *cell, row, col))
                }))
        });
    let icon_row = div()
        .flex()
        .items_center()
        .h(px(44.))
        .child(div().w(px(110.)).child(label(t, "Icon button")))
        .children(
            CELLS
                .iter()
                .enumerate()
                .map(|(col, (cell, _))| div().w(cell_w).child(matrix_icon_button(*cell, col))),
        );
    div()
        .flex()
        .flex_col()
        .child(head)
        .children(rows)
        .child(icon_row)
}

/// The whole keyboard map of d0058, one row per action, the current platform's column first.
fn kbds(t: &Theme) -> impl IntoElement {
    let actions = KeyAction::all();
    let mut platforms = [
        (Platform::Mac, "macOS"),
        (Platform::WinLinux, "Windows / Linux"),
        (Platform::WinLinuxTerminal, "In terminal (Win/Linux)"),
    ];
    platforms.sort_by_key(|(p, _)| *p != Platform::current());
    let head = div()
        .flex()
        .child(div().w(px(200.)).child(label(t, "Action")))
        .children(
            platforms
                .iter()
                .map(|(_, name)| div().w(px(200.)).child(label(t, *name))),
        );
    div()
        .flex()
        .flex_col()
        .gap(px(space::S1))
        .child(head)
        .children(actions.into_iter().map(|action| {
            div()
                .flex()
                .items_center()
                .child(div().w(px(200.)).child(label(t, format!("{action:?}"))))
                .children(platforms.iter().map(|(platform, _)| {
                    div()
                        .w(px(200.))
                        .children(Kbd::for_action(action, *platform))
                }))
        }))
}

fn banners() -> impl IntoElement {
    let noop = |_: &gpui::ClickEvent, _: &mut Window, _: &mut App| {};
    div()
        .flex()
        .flex_col()
        .gap(px(space::S3))
        .child(
            Banner::new(
                "b-usage",
                BannerTone::Attention,
                IconName::Gauge,
                "Usage limit reached",
                "Resets at 3:00 pm (in 1 h 12 min) · 8 agents stopped · no new tickets will start",
            )
            .action(BannerAction::new("Resume all", noop).icon(IconName::Play).disabled(true))
            .action(BannerAction::new("Retry", noop).icon(IconName::RotateCcw))
            .on_dismiss(noop),
        )
        .child(
            Banner::new(
                "b-permission",
                BannerTone::Warning,
                IconName::ShieldAlert,
                "Permission prompts are on",
                "With 5–10 agents, permission prompts will stop agents often. Read about allowlists and auto mode →",
            )
            .on_dismiss(noop),
        )
        .child(
            Banner::new(
                "b-update",
                BannerTone::Info,
                IconName::RefreshCw,
                "Service update ready",
                "Restart now (3 agents working, 2 waiting) · or later, when it suits you",
            )
            .action(BannerAction::new("Restart now", noop).icon(IconName::RotateCw))
            .on_dismiss(noop),
        )
        .child(
            Banner::new(
                "b-required",
                BannerTone::Warning,
                IconName::ServerCog,
                "Restart required",
                "This app can’t talk to the running service (older version) · 3 agents working, 2 waiting",
            )
            .action(BannerAction::new("Restart now", noop).icon(IconName::RotateCw)),
        )
}

fn toasts(t: &Theme) -> impl IntoElement {
    let noop = |_: &gpui::ClickEvent, _: &mut Window, _: &mut App| {};
    div()
        .flex()
        .flex_wrap()
        .gap(px(space::S4))
        .child(
            Toast::new("t-pr", ToastKind::Success, "PR #481 opened on GitHub")
                .icon(IconName::GitPullRequest, Some(t.colors.st_review))
                .text("SHOP-26 Add sitemap → main · Open in browser")
                .on_close(noop),
        )
        .child(
            Toast::new("t-merge", ToastKind::Success, "Local merge done · SHOP-18")
                .icon(IconName::GitMerge, None)
                .text("Squashed into main as 9f3e2c1")
                .on_close(noop),
        )
        .child(
            Toast::new("t-agent", ToastKind::Success, "Ticket created by agent")
                .icon(IconName::Sparkles, Some(t.colors.st_investigate))
                .text("SHOP-29 Add cart analytics → Backlog")
                .on_close(noop),
        )
        .child(
            Toast::new("t-gh", ToastKind::Error, "gh is not logged in")
                .text("Run gh auth login, then retry Open PR.")
                .action("Copy command", noop)
                .on_close(noop),
        )
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
                        [Mode::System, Mode::Light, Mode::Dark]
                            .into_iter()
                            .map(|m| mode_button(&t, m, choice)),
                    ),
            )
            .child(
                div().flex().flex_wrap().gap(px(space::S2)).children(
                    ThemeId::DARK
                        .into_iter()
                        .chain(ThemeId::LIGHT)
                        .map(|id| palette_button(&t, id)),
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
            .child(
                div()
                    .text_style_of(text::DISPLAY)
                    .text_color(t.colors.text_primary)
                    .child("01 · Components"),
            )
            .child(section(
                "Buttons · Button matrix",
                &t,
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::S4))
                    .child(button_matrix(&t))
                    .child(self.demo_row(&t, cx)),
            ))
            .child(section(
                "Primitives · Kbd · Keyboard map (d0058)",
                &t,
                kbds(&t),
            ))
            .child(section("Banners", &t, banners()))
            .child(section(
                "Banner area · stacking (max 2, reconnecting folds in) · toast host",
                &t,
                self.stack_demo(&t, cx),
            ))
            .child(section("Overlays & feedback · Toasts", &t, toasts(&t)))
            .child(self.components.clone())
    }
}
