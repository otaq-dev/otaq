//! The first-run screens (UI-08, UI-11, SET-02; *14 · First run & onboarding*): 1 · Welcome, 2 · Theme ("Pick a
//! look", d0069) and 3 · Tools. The view reads [`OnboardingModel`]; the service does the checking (`tools.check`).
//! Commands are shown for copying only: nothing here installs a tool (d0021, d0011, d0023). The Theme step's
//! clicks are applied and saved at once, like Settings → Appearance.

use crewboard_core::naming;
use crewboard_core::protocol::{Reply, Request};
use gpui::{
    AnyElement, ClipboardItem, Context, Entity, EventEmitter, Hsla, IntoElement, SharedString,
    Window, div, prelude::*, px,
};

use crate::add_project::{AddProjectEvent, AddProjectForm, FormMode};
use crate::components::button::{Button, Variant};
use crate::components::theme_picker::{OnPick, ThemeSections, mode_switch};
use crate::icons::{IconName, icon};
use crate::keymap::{KeyAction, Platform};
use crate::service_link::ServiceLink;
use crate::theme::{ActiveTheme, Appearance, Colors, FONT_MONO, radius};
use crate::view_models::onboarding::{OnboardingModel, RowIcon, STEPS, Step, ToolRow};
use crate::workspace::Workspace;

/// Where the Claude Code docs on permission modes (allowlists, auto mode) live.
pub const PERMISSION_DOCS_URL: &str = "https://code.claude.com/docs/en/permission-modes";

/// Sent when the user continues past the tools step.
pub struct Finished {
    /// The permission-mode warning still applies (prompting, not dismissed): the board shows it as a banner.
    pub permission_warning: bool,
}

pub struct Onboarding {
    link: Entity<ServiceLink>,
    workspace: Entity<Workspace>,
    model: OnboardingModel,
    /// Step 3's form, made when the user continues past Tools.
    form: Option<Entity<AddProjectForm>>,
}

impl EventEmitter<Finished> for Onboarding {}

impl Onboarding {
    pub fn new(
        link: Entity<ServiceLink>,
        workspace: Entity<Workspace>,
        permission_dismissed: bool,
    ) -> Self {
        Self {
            link,
            workspace,
            model: OnboardingModel::new(permission_dismissed),
            form: None,
        }
    }

    /// Opens on the tools step and runs the check ("Check tools" on the empty board).
    pub fn at_tools(&mut self, cx: &mut Context<Self>) {
        self.check(cx);
    }

    fn check(&mut self, cx: &mut Context<Self>) {
        self.model.start_check();
        cx.notify();
        let request = self.link.read(cx).spawn_request(Request::ToolsCheck);
        cx.spawn(async move |this, cx| {
            let result = match request {
                Some(task) => match task.await {
                    Ok(Ok(Reply::ToolsCheck(report))) => Ok(report),
                    Ok(Ok(other)) => Err(format!("unexpected answer: {other:?}")),
                    Ok(Err(e)) => Err(e.to_string()),
                    Err(e) => Err(e.to_string()),
                },
                None => Err("not connected to the background service".to_owned()),
            };
            let _ = this.update(cx, |o, cx| {
                o.model.finish_check(result);
                cx.notify();
            });
        })
        .detach();
    }

    fn dismiss_permission_warning(&mut self, cx: &mut Context<Self>) {
        self.model.permission_dismissed = true;
        cx.notify();
        if let Some(task) = self
            .link
            .read(cx)
            .spawn_request(Request::DismissPermissionWarning)
        {
            cx.background_spawn(async move {
                let _ = task.await;
            })
            .detach();
        }
    }

    /// Continue past Tools: step 3, add the first project. A user who already has projects is done here.
    fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.model.continue_state().enabled {
            return;
        }
        if !self.workspace.read(cx).switcher.projects().is_empty() {
            cx.emit(Finished {
                permission_warning: self.model.show_permission_warning(),
            });
            return;
        }
        self.model.step = Step::Project;
        let workspace = self.workspace.clone();
        let form =
            cx.new(|cx| AddProjectForm::new(workspace, FormMode::FirstRun, None, window, cx));
        cx.subscribe(&form, |o, _, event: &AddProjectEvent, cx| {
            if let AddProjectEvent::Added(_) = event {
                cx.emit(Finished {
                    permission_warning: o.model.show_permission_warning(),
                });
            }
        })
        .detach();
        self.form = Some(form);
        cx.notify();
    }

    fn choose_theme(&mut self, cx: &mut Context<Self>) {
        self.model.choose_theme();
        cx.notify();
    }

    /// 2 · Theme: the mode switch and the two rows of theme cards; a click recolors every window and is saved.
    fn theme(&self, c: &Colors, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let choice = cx.global::<ActiveTheme>().choice;
        let os = Appearance::of_os(window.appearance());
        let workspace = self.workspace.clone();
        let pick: OnPick = std::rc::Rc::new(move |choice, window, cx| {
            ActiveTheme::update(choice, window.appearance(), cx);
            workspace.update(cx, |ws, cx| ws.set_theme_choice(choice, cx));
        });
        let this = cx.entity().downgrade();
        let connected = self.link.read(cx).client.is_some();
        Self::card(c)
            .w(px(810.))
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap(px(24.))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(24.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(c.text_primary)
                                    .child("Pick a look"),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .line_height(gpui::relative(1.5))
                                    .text_color(c.text_secondary)
                                    .child(SharedString::from(format!(
                                        "Choose how {} follows your OS, and which theme to use in dark and light. The board, panels and terminal all recolor.",
                                        naming::PRODUCT_NAME
                                    ))),
                            ),
                    )
                    .child(mode_switch("onboarding-mode", choice, pick.clone())),
            )
            .child(ThemeSections::new("onboarding-themes", choice, os, pick))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .text_color(c.text_muted)
                            .child("Change it anytime in Settings → Appearance."),
                    )
                    .child(
                        Button::new("check-tools", "Check my tools")
                            .icon(IconName::ArrowRight)
                            .disabled(!connected)
                            .on_click(move |_, _, cx| {
                                let _ = this.update(cx, |o, cx| o.check(cx));
                            }),
                    ),
            )
    }

    fn card(c: &Colors) -> gpui::Div {
        div()
            .w(px(600.))
            .flex()
            .flex_col()
            .gap(px(20.))
            .p(px(32.))
            .bg(c.bg_panel)
            .border_1()
            .border_color(c.border)
            .rounded(px(12.))
    }

    fn welcome(&self, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let attach = format!(
            "Each agent is the normal claude CLI. Attach from any terminal with {} attach SHOP-18.",
            naming::COMMAND_NAME
        );
        let inbox_key = KeyAction::NextInboxItem
            .keys(Platform::current())
            .unwrap_or_default();
        let points = [
            (IconName::SquareTerminal, "Real sessions", attach),
            (
                IconName::Inbox,
                "One inbox",
                format!(
                    "Permission prompts, questions and reviews from every project in one list. {inbox_key} opens the next."
                ),
            ),
            (
                IconName::ShieldCheck,
                "You stay in control",
                "Nothing starts, retries or deletes without a click. Your repos are never written to by settings."
                    .to_owned(),
            ),
        ];
        let this = cx.entity().downgrade();
        let connected = self.link.read(cx).client.is_some();
        Self::card(c)
            .child(
                div()
                    .text_size(px(24.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(c.text_primary)
                    .child("Run Claude Code agents from a board"),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(gpui::relative(1.5))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(format!(
                        "{} turns tickets into real claude sessions — one per ticket, each in its own git worktree. You watch and type into them from an embedded terminal, review the diff, and merge or open a PR.",
                        naming::PRODUCT_NAME
                    ))),
            )
            .child(div().flex().flex_col().gap(px(12.)).children(points.into_iter().map(
                |(name, title, text)| {
                    div()
                        .flex()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex_none()
                                .size(px(32.))
                                .rounded(px(radius::LG))
                                .bg(c.accent_soft)
                                .flex()
                                .justify_center()
                                .items_center()
                                .child(icon(name, px(16.)).text_color(c.accent)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(c.text_primary)
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .line_height(gpui::relative(1.45))
                                        .text_color(c.text_secondary)
                                        .child(SharedString::from(text)),
                                ),
                        )
                },
            )))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .text_color(c.text_muted)
                            .child("Takes about a minute"),
                    )
                    .child(
                        Button::new("choose-theme", "Choose a theme")
                            .icon(IconName::ArrowRight)
                            .disabled(!connected)
                            .on_click(move |_, _, cx| {
                                let _ = this.update(cx, |o, cx| o.choose_theme(cx));
                            }),
                    ),
            )
    }

    fn permission_warning(&self, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        div()
            .flex()
            .gap(px(10.))
            .p(px(12.))
            .rounded(px(radius::LG))
            .bg(c.st_paused_bg)
            .border_1()
            .border_color(c.st_paused)
            .child(icon(IconName::ShieldAlert, px(15.)).text_color(c.st_paused))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(c.text_primary)
                            .child("Your Claude Code settings ask for permission prompts"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .text_size(px(12.))
                            .text_color(c.text_secondary)
                            .child("With 5–10 agents, prompts will stop agents often. Consider allowlists or auto mode (")
                            .child(
                                div()
                                    .id("permission-docs")
                                    .cursor_pointer()
                                    .text_color(c.accent)
                                    .child("Claude Code docs ↗")
                                    .on_click(|_, _, cx| cx.open_url(PERMISSION_DOCS_URL)),
                            )
                            .child(SharedString::from(format!(
                                "). {} won’t change your settings.",
                                naming::PRODUCT_NAME
                            ))),
                    ),
            )
            .child(
                div()
                    .id("dismiss-permission")
                    .cursor_pointer()
                    .child(icon(IconName::X, px(13.)).text_color(c.text_muted))
                    .on_click(move |_, _, cx| {
                        let _ = this.update(cx, |o, cx| o.dismiss_permission_warning(cx));
                    }),
            )
    }

    fn row(row: ToolRow, index: usize, c: &Colors) -> AnyElement {
        let (row_icon, icon_color): (IconName, Hsla) = match row.icon {
            RowIcon::Found => (IconName::CircleCheck, c.st_approved),
            RowIcon::MissingRequired => (IconName::CircleX, c.st_error),
            RowIcon::MissingOptional => (IconName::CircleMinus, c.text_muted),
        };
        let status_color = if row.icon == RowIcon::MissingRequired {
            c.st_error
        } else {
            c.text_secondary
        };
        let mut el = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .px(px(14.))
            .py(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(icon(row_icon, px(16.)).text_color(icon_color))
                    .child(
                        div()
                            .font_family(FONT_MONO)
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(c.text_primary)
                            .child(row.name),
                    )
                    .child(
                        div()
                            .px(px(6.))
                            .py(px(1.))
                            .rounded(px(radius::SM))
                            .bg(c.bg_column)
                            .text_size(px(10.))
                            .text_color(c.text_muted)
                            .child(if row.required { "required" } else { "optional" }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .justify_end()
                            .text_size(px(12.))
                            .text_color(status_color)
                            .child(SharedString::from(row.status)),
                    ),
            )
            .children(row.explanation.map(|text| {
                div()
                    .text_size(px(12.))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(text))
            }))
            .children(row.command.map(|command| {
                let copy = command.clone();
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(5.))
                    .bg(c.term_bg)
                    .border_1()
                    .border_color(c.border)
                    .child(
                        // Long commands (Linux's az line) wrap inside the box (*gh and az missing (Windows)*).
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .font_family(FONT_MONO)
                            .text_size(px(12.))
                            .text_color(c.term_fg)
                            .child(SharedString::from(command)),
                    )
                    .child(
                        div()
                            .id(("copy", index))
                            .cursor_pointer()
                            .child(icon(IconName::Copy, px(12.)).text_color(c.text_muted))
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                            }),
                    )
            }));
        if row.icon == RowIcon::MissingRequired {
            el = el.bg(c.st_error_bg);
        }
        if index > 0 {
            el = el.border_t(px(1.)).border_color(c.border);
        }
        el.into_any_element()
    }

    fn tools(&self, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let rows: Vec<AnyElement> = self
            .model
            .rows()
            .into_iter()
            .enumerate()
            .map(|(i, r)| Self::row(r, i, c))
            .collect();
        let list = if self.model.report.is_none() {
            div()
                .p(px(14.))
                .text_size(px(12.))
                .text_color(c.text_secondary)
                .child(if self.model.checking {
                    "Checking your tools…"
                } else {
                    "No result yet"
                })
                .into_any_element()
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let cont = self.model.continue_state();
        let recheck = cx.entity().downgrade();
        let next = cx.entity().downgrade();
        let warning = self
            .model
            .show_permission_warning()
            .then(|| self.permission_warning(c, cx));
        Self::card(c)
            .child(
                div()
                    .text_size(px(22.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(c.text_primary)
                    .child("Tools"),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(format!(
                        "{} uses the tools already on your machine. It never installs or updates them.",
                        naming::PRODUCT_NAME
                    ))),
            )
            .children(warning)
            .child(
                div()
                    .border_1()
                    .border_color(c.border)
                    .rounded(px(radius::LG))
                    .overflow_hidden()
                    .child(list),
            )
            .children(self.model.error.clone().map(|e| {
                div()
                    .text_size(px(12.))
                    .text_color(c.st_error)
                    .child(SharedString::from(format!("Check failed: {e}")))
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Button::new("recheck", "Re-check")
                            .variant(Variant::Secondary)
                            .icon(IconName::RotateCcw)
                            .loading(self.model.checking)
                            .on_click(move |_, _, cx| {
                                let _ = recheck.update(cx, |o, cx| o.check(cx));
                            }),
                    )
                    .child(div().flex_1())
                    .children(cont.reason.map(|reason| {
                        div()
                            .text_size(px(12.))
                            .text_color(c.st_error)
                            .child(SharedString::from(reason))
                    }))
                    .child(
                        Button::new("continue", "Continue")
                            .icon(IconName::ArrowRight)
                            .disabled(!cont.enabled)
                            .on_click(move |_, window, cx| {
                                let _ = next.update(cx, |o, cx| o.finish(window, cx));
                            }),
                    ),
            )
    }

    pub fn step(&self) -> Step {
        self.model.step
    }
}

/// The window's 44 px title bar: the product name next to the macOS traffic lights and, during onboarding, the
/// step indicator (Welcome · Theme · Tools · Project · Done).
pub fn title_bar(step: Option<Step>, c: &Colors, captions: bool) -> impl IntoElement {
    use crate::window_controls::{caption_buttons, drag_area};
    let current = step.map(Step::index);
    let mut steps: Vec<AnyElement> = Vec::new();
    for (i, name) in STEPS.iter().enumerate().filter(|_| current.is_some()) {
        let current = current.unwrap_or_default();
        if i > 0 {
            steps.push(
                div()
                    .w(px(20.))
                    .h(px(1.))
                    .bg(c.border_strong)
                    .into_any_element(),
            );
        }
        let (badge_bg, badge_fg, label_fg, weight) = if i == current {
            (
                c.accent,
                c.text_on_accent,
                c.text_primary,
                gpui::FontWeight::SEMIBOLD,
            )
        } else {
            (
                c.bg_column,
                c.text_muted,
                c.text_muted,
                gpui::FontWeight::NORMAL,
            )
        };
        let badge = div()
            .size(px(18.))
            .rounded(px(9.))
            .bg(badge_bg)
            .flex()
            .justify_center()
            .items_center();
        let badge = if i < current {
            badge.child(icon(IconName::Check, px(11.)).text_color(c.st_approved))
        } else {
            badge
                .text_size(px(10.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(badge_fg)
                .child(SharedString::from((i + 1).to_string()))
        };
        steps.push(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(badge)
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(weight)
                        .text_color(label_fg)
                        .child(*name),
                )
                .into_any_element(),
        );
    }
    div()
        .flex_none()
        .h(px(44.))
        .w_full()
        .flex()
        .items_center()
        .px(px(14.))
        .bg(c.bg_panel)
        .border_b(px(1.))
        .border_color(c.border)
        // Room for the macOS traffic lights drawn over the transparent title bar.
        .when(cfg!(target_os = "macos"), |el| el.child(div().w(px(72.))))
        .child(
            div()
                .text_size(px(13.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(c.text_primary)
                .child(naming::PRODUCT_NAME),
        )
        .child(drag_area("drag-before-steps"))
        .child(div().flex().items_center().gap(px(6.)).children(steps))
        .child(drag_area("drag-after-steps"))
        .map(|el| {
            if captions {
                el.child(caption_buttons(c))
            } else {
                el.child(div().w(px(160.)))
            }
        })
}

impl Render for Onboarding {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let body = match self.model.step {
            Step::Welcome => self.welcome(&c, cx).into_any_element(),
            Step::Theme => self.theme(&c, window, cx).into_any_element(),
            Step::Tools => self.tools(&c, cx).into_any_element(),
            Step::Project => match &self.form {
                Some(form) => form.clone().into_any_element(),
                None => div().into_any_element(),
            },
        };
        div().size_full().flex().flex_col().bg(c.bg_app).child(
            div()
                .id("onboarding-body")
                .flex_1()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .justify_center()
                .items_center()
                .p(px(32.))
                .child(body),
        )
    }
}
