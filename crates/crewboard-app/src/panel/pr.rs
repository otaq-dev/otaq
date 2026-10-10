//! The Open PR / Push to PR dialog in the side panel (REV-10, REV-14, UI-07; d0050, d0041, d0059; *07 → Open PR ·
//! skill is preparing…, confirmation, pushing, success, invalid or taken branch name, not logged in, Push to PR ·
//! confirmation, Open PR · epic SHOP-40*). The view model is `view_models::pr_dialog`; this file sends the requests
//! and draws the states. The flow's progress comes from `ticket.pr_flow`, refetched when the ticket changes and
//! every second while the skill works.

use std::time::Duration;

use crewboard_core::domain::Status;
use crewboard_core::pr::service::PrOutcome;
use crewboard_core::protocol::{
    BranchRenameParams, KeyParams, OpenPrParams, PrConfirmParams, ProjectParams, Reply, Request,
};
use gpui::{
    AnyElement, Context, Entity, FocusHandle, FontWeight, IntoElement, ParentElement, SharedString,
    Styled, Window, div, prelude::*, px,
};

use super::review::ApproveStep;
use super::{PanelEvent, SidePanel};
use crate::board::Notice;
use crate::components::dialog::ConfirmDialog;
use crate::components::text_input::{InputEvent, TextInput};
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, Colors, FONT_MONO};
use crate::view_models::approve as ap;
use crate::view_models::dialog::DialogKind;
use crate::view_models::pr_dialog::{self as pd, PrDialog, Stage};
use crate::view_models::terminal_strip::{Click, UnsentPrompt};

/// Which text field changed.
#[derive(Clone, Copy)]
enum Field {
    Branch,
    Commit(usize),
    Title,
    Description,
}

/// The dialog's state in the panel.
pub(super) struct PrFlowUi {
    pub(super) model: PrDialog,
    focus: FocusHandle,
    branch: Entity<TextInput>,
    commits: Vec<Entity<TextInput>>,
    title: Entity<TextInput>,
    description: Entity<TextInput>,
    approve_anyway: bool,
    /// A request is on its way: the buttons wait.
    busy: bool,
}

impl SidePanel {
    /// Approve ▾ → Open PR / Push to PR #N: the provider's login first (UI-07), then `ticket.open_pr`.
    pub(super) fn start_pr(
        &mut self,
        provider: Option<crewboard_core::pr::provider::PrProvider>,
        approve_anyway: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ticket) = self.ticket(cx) else {
            return;
        };
        let skill = provider
            .as_ref()
            .map(|p| p.open_pr_skill.clone())
            .unwrap_or_default();
        let model = PrDialog::opening(&ticket, &skill, provider.as_ref());
        let not_logged_in = matches!(model.stage, Stage::NotLoggedIn { .. });
        let ui = PrFlowUi {
            model,
            focus: cx.focus_handle(),
            branch: self.pr_input(Field::Branch, window, cx),
            commits: Vec::new(),
            title: self.pr_input(Field::Title, window, cx),
            description: self.pr_input(Field::Description, window, cx),
            approve_anyway,
            busy: false,
        };
        window.focus(&ui.focus, cx);
        self.pr = Some(ui);
        if !not_logged_in {
            self.send_open_pr(false, window, cx);
        }
        self.poll_pr(window, cx);
        cx.notify();
    }

    fn pr_input(
        &self,
        field: Field,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let input = cx.new(|cx| {
            let input = TextInput::new(window, cx);
            match field {
                Field::Branch | Field::Commit(_) => input.mono(),
                Field::Description => input.multiline(px(110.)),
                Field::Title => input,
            }
        });
        cx.subscribe_in(
            &input,
            window,
            move |panel, input, event: &InputEvent, window, cx| match event {
                InputEvent::Changed => {
                    let text = input.read(cx).text().to_owned();
                    panel.pr_field_changed(field, text, window, cx);
                }
                InputEvent::Cancel => panel.cancel_pr(cx),
                _ => {}
            },
        )
        .detach();
        input
    }

    fn pr_field_changed(
        &mut self,
        field: Field,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pr) = &mut self.pr else {
            return;
        };
        match field {
            Field::Branch => {
                if text == pr.model.branch {
                    return;
                }
                let generation = pr.model.edit_branch(&text);
                self.check_branch(generation, window, cx);
            }
            Field::Commit(i) => {
                if let Some(c) = pr.model.commits.get_mut(i) {
                    c.message = text;
                }
            }
            Field::Title => pr.model.title = text,
            Field::Description => pr.model.description = text,
        }
        cx.notify();
    }

    /// The live branch-name check, once the name stayed the same for a moment (REV-10).
    fn check_branch(&mut self, generation: u64, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(pd::CHECK_AFTER_MS))
                .await;
            let _ = this.update(cx, |panel, cx| panel.check_branch_now(generation, cx));
        })
        .detach();
    }

    fn check_branch_now(&mut self, generation: u64, cx: &mut Context<Self>) {
        let Some(pr) = &self.pr else {
            return;
        };
        if !pr.model.debounce.is_current(generation) {
            return;
        }
        let send = self
            .link
            .read(cx)
            .send(Request::TicketPrBranchCheck(BranchRenameParams {
                key: self.key.to_string(),
                name: pr.model.branch.clone(),
            }));
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::BranchCheck(answer)) = send.await {
                let _ = this.update(cx, |panel, cx| {
                    if let Some(pr) = &mut panel.pr {
                        pr.model.checked(generation, &answer);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn send_open_pr(&mut self, clear: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pr) = &mut self.pr else {
            return;
        };
        pr.busy = true;
        let send = self.link.read(cx).send(Request::TicketOpenPr(OpenPrParams {
            key: self.key.to_string(),
            approve_anyway: pr.approve_anyway,
            clear,
        }));
        cx.spawn_in(window, async move |this, cx| {
            let result = send.await;
            let _ = this.update_in(cx, |panel, window, cx| {
                panel.pr_answered(result, Click::OpenPr, window, cx)
            });
        })
        .detach();
    }

    /// **Push & open PR** / **Push to PR #N**.
    fn confirm_pr(&mut self, clear: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pr) = &mut self.pr else {
            return;
        };
        if !pr.model.can_confirm() || pr.busy {
            return;
        }
        pr.busy = true;
        let send = self
            .link
            .read(cx)
            .send(Request::TicketPrConfirm(PrConfirmParams {
                key: self.key.to_string(),
                proposal: pr.model.proposal(),
                clear,
            }));
        cx.spawn_in(window, async move |this, cx| {
            let result = send.await;
            let _ = this.update_in(cx, |panel, window, cx| {
                panel.pr_answered(result, Click::PrConfirm, window, cx)
            });
        })
        .detach();
        cx.notify();
    }

    /// **Retry** after the skill reported a problem; on the not-logged-in state, checks the login again first.
    fn retry_pr(&mut self, clear: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pr) = &mut self.pr else {
            return;
        };
        if matches!(pr.model.stage, Stage::NotLoggedIn { .. }) {
            let approve_anyway = pr.approve_anyway;
            let send = self
                .link
                .read(cx)
                .send(Request::ProjectPrProvider(ProjectParams {
                    project: self.key.project.to_string(),
                }));
            cx.spawn_in(window, async move |this, cx| {
                if let Ok(Reply::PrProvider(provider)) = send.await {
                    let _ = this.update_in(cx, |panel, window, cx| {
                        panel.pr = None;
                        panel.start_pr(Some(provider), approve_anyway, window, cx);
                    });
                }
            })
            .detach();
            return;
        }
        pr.busy = true;
        let send = self
            .link
            .read(cx)
            .send(Request::TicketPrRetry(OpenPrParams {
                key: self.key.to_string(),
                approve_anyway: pr.approve_anyway,
                clear,
            }));
        cx.spawn_in(window, async move |this, cx| {
            let result = send.await;
            let _ = this.update_in(cx, |panel, window, cx| {
                panel.pr_answered(result, Click::PrRetry, window, cx)
            });
        })
        .detach();
    }

    /// **Cancel** at any point before pushing: the proposal is discarded; nothing was pushed.
    pub(super) fn cancel_pr(&mut self, cx: &mut Context<Self>) {
        let Some(pr) = self.pr.take() else {
            return;
        };
        if !matches!(pr.model.stage, Stage::NotLoggedIn { .. }) {
            let send = self.link.read(cx).send(Request::TicketPrCancel(KeyParams {
                key: self.key.to_string(),
            }));
            cx.spawn(async move |_, _| {
                let _ = send.await;
            })
            .detach();
        }
        cx.emit(PanelEvent::TakeFocus);
        cx.notify();
    }

    /// The unsent-text prompt's "Clear it and send" for a PR click.
    pub(super) fn pr_clear_and_send(
        &mut self,
        click: Click,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match click {
            Click::OpenPr => self.send_open_pr(true, window, cx),
            Click::PrConfirm => self.confirm_pr(true, window, cx),
            Click::PrRetry => self.retry_pr(true, window, cx),
            _ => {}
        }
    }

    /// An answer to Open PR, confirm or Retry.
    fn pr_answered(
        &mut self,
        result: Result<Reply, crate::service_link::RequestError>,
        click: Click,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(pr) = &mut self.pr {
            pr.busy = false;
        }
        match result {
            Ok(Reply::PrOutcome(PrOutcome::Started { .. })) => {
                if click == Click::PrConfirm
                    && let Some(pr) = &mut self.pr
                {
                    pr.model.stage = Stage::Pushing;
                }
                self.refresh_pr(window, cx);
            }
            Ok(Reply::PrOutcome(PrOutcome::NeedsConfirmation { ticket })) => {
                let typed = self.terminal.as_ref().and_then(|t| t.read(cx).typed_line());
                let sending = self
                    .pr
                    .as_ref()
                    .map(|p| p.model.skill.clone())
                    .unwrap_or_default();
                self.prompt = Some(UnsentPrompt::for_click(ticket, typed, sending, click));
                window.focus(&self.prompt_focus, cx);
            }
            // Uncommitted files and "changed since you reviewed it" show in the approve flow's dialogs.
            Ok(Reply::PrOutcome(PrOutcome::RefusedUncommitted { files, message })) => {
                self.pr = None;
                self.open_approve_result(ap::MergeView::Uncommitted { message, files }, window, cx);
            }
            Ok(Reply::PrOutcome(PrOutcome::Changed { check })) => {
                self.pr = None;
                let view = ap::changed_view(&self.key, &check);
                self.open_approve_result(view, window, cx);
            }
            Ok(_) => {}
            Err(e) => {
                cx.emit(PanelEvent::Toast(Notice {
                    error: true,
                    title: format!("Open PR for {} didn\u{2019}t go through", self.key),
                    text: Some(e.message),
                }));
                if click == Click::OpenPr {
                    self.pr = None;
                }
            }
        }
        cx.notify();
    }

    /// Refetches the flow; when the ticket reached Approved · PR open after the push, closes with the success
    /// toast.
    pub(super) fn refresh_pr(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pr) = &self.pr else {
            return;
        };
        if pr.model.stage == Stage::Pushing
            && let Some(ticket) = self.ticket(cx)
            && ticket.status == Status::PrOpen
            && let Some(n) = ticket.flags.pr_number
        {
            cx.emit(PanelEvent::Toast(Notice {
                error: false,
                title: pd::success_toast(pr.model.kind, n),
                text: ticket.work.as_ref().and_then(|w| w.pr_url.clone()),
            }));
            self.pr = None;
            cx.emit(PanelEvent::TakeFocus);
            cx.notify();
            return;
        }
        let send = self.link.read(cx).send(Request::TicketPrFlow(KeyParams {
            key: self.key.to_string(),
        }));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Reply::PrFlow(view)) = send.await {
                let _ = this.update_in(cx, |panel, window, cx| panel.pr_view(&view, window, cx));
            }
        })
        .detach();
    }

    fn pr_view(
        &mut self,
        view: &crewboard_core::pr::flow::PrFlowView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pr) = &mut self.pr else {
            return;
        };
        let was = pr.model.stage.clone();
        if !view.skill.is_empty() {
            pr.model.skill = view.skill.clone();
        }
        pr.model.update(view);
        if was != Stage::Proposed && pr.model.stage == Stage::Proposed {
            // The proposal arrived: fill the fields once; later edits are the user's.
            let (branch, title, description) = (
                pr.model.branch.clone(),
                pr.model.title.clone(),
                pr.model.description.clone(),
            );
            pr.branch.update(cx, |i, cx| i.set_text(&branch, cx));
            pr.title.update(cx, |i, cx| i.set_text(&title, cx));
            pr.description
                .update(cx, |i, cx| i.set_text(&description, cx));
            let messages: Vec<String> =
                pr.model.commits.iter().map(|c| c.message.clone()).collect();
            let locked = pr.model.branch_locked;
            let generation = pr.model.debounce.edit();
            let commits: Vec<Entity<TextInput>> = messages
                .iter()
                .enumerate()
                .map(|(i, message)| {
                    let input = self.pr_input(Field::Commit(i), window, cx);
                    input.update(cx, |input, cx| input.set_text(message, cx));
                    input
                })
                .collect();
            if let Some(pr) = &mut self.pr {
                pr.commits = commits;
            }
            if !locked {
                self.check_branch(generation, window, cx);
            }
        }
        cx.notify();
    }

    /// While the skill works, the flow is refetched every second.
    fn poll_pr(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let keep = this
                    .update_in(cx, |panel, window, cx| {
                        let working = panel.pr.as_ref().is_some_and(|p| {
                            matches!(p.model.stage, Stage::Preparing | Stage::Pushing)
                        });
                        if working {
                            panel.refresh_pr(window, cx);
                        }
                        panel.pr.is_some()
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        })
        .detach();
    }

    /// The dialog for the flow's stage.
    pub(super) fn pr_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pr = self.pr.as_ref()?;
        let c = ActiveTheme::get(cx).colors;
        let m = &pr.model;
        let this = cx.entity().downgrade();
        let cancel = {
            let this = this.clone();
            move |_: &mut Window, cx: &mut gpui::App| {
                let _ = this.update(cx, |p, cx| p.cancel_pr(cx));
            }
        };
        let label = |t: String| {
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c.text_secondary)
                .child(SharedString::from(t))
                .into_any_element()
        };
        let dialog = match &m.stage {
            Stage::NotLoggedIn { cli, login } => {
                let retry = this.clone();
                ConfirmDialog::new(
                    "pr-not-logged-in",
                    &pr.focus,
                    DialogKind::Destructive,
                    format!("Can\u{2019}t open the PR \u{2014} {cli} is not logged in"),
                    "Retry",
                    move |window, cx| {
                        let _ = retry.update(cx, |p, cx| p.retry_pr(false, window, cx));
                    },
                    cancel,
                )
                .confirm_icon(IconName::RotateCw)
                .text(format!(
                    "Run this in any terminal, then click Retry. {} never logs in for you.",
                    crewboard_core::naming::PRODUCT_NAME
                ))
                .child(code_box(&format!("$ {login}"), &c))
                .child(code_box(
                    "Azure DevOps: az login && az extension add --name azure-devops",
                    &c,
                ))
            }
            Stage::Preparing => ConfirmDialog::new(
                "pr-preparing",
                &pr.focus,
                DialogKind::Primary,
                m.heading(),
                m.confirm_label(),
                |_, _| {},
                cancel,
            )
            .confirm_icon(IconName::Upload)
            .confirm_disabled(true)
            .width(520.)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .p(px(12.))
                    .rounded(px(6.))
                    .bg(c.bg_column)
                    .child(icon(IconName::LoaderCircle, px(14.)).text_color(c.text_muted))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().text_size(px(13.)).text_color(c.text_primary).child(
                                match m.kind {
                                    crewboard_core::pr::flow::FlowKind::Push => {
                                        "Push skill is preparing\u{2026}"
                                    }
                                    crewboard_core::pr::flow::FlowKind::Open => {
                                        "Open PR skill is preparing\u{2026}"
                                    }
                                },
                            ))
                            .child(
                                div()
                                    .font_family(FONT_MONO)
                                    .text_size(px(11.))
                                    .text_color(c.text_muted)
                                    .child(SharedString::from(format!(
                                        "{} \u{b7} reading the commits",
                                        m.skill
                                    ))),
                            ),
                    ),
            ),
            Stage::Proposed => {
                let confirm = this.clone();
                let (hint, bad) = m.branch_hint();
                let mut body: Vec<AnyElement> = Vec::new();
                body.push(
                    div()
                        .text_size(px(12.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(format!(
                            "Proposed by {} \u{b7} edit anything",
                            m.skill
                        )))
                        .into_any_element(),
                );
                if let Some(provider) = &m.provider {
                    body.push(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(10.))
                            .py(px(6.))
                            .rounded(px(6.))
                            .bg(c.bg_column)
                            .text_size(px(12.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .text_color(c.text_secondary)
                                    .child(icon(IconName::GitPullRequest, px(12.)))
                                    .child(SharedString::from(provider.clone())),
                            )
                            .children(m.login.clone().map(|login| {
                                div()
                                    .text_color(c.text_muted)
                                    .child(SharedString::from(login))
                            }))
                            .into_any_element(),
                    );
                }
                body.push(label("Branch name".to_owned()));
                if m.branch_locked {
                    body.push(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(10.))
                            .py(px(6.))
                            .rounded(px(5.))
                            .bg(c.bg_column)
                            .font_family(FONT_MONO)
                            .text_size(px(12.))
                            .text_color(c.text_secondary)
                            .child(icon(IconName::GitBranch, px(12.)))
                            .child(div().flex_1().child(SharedString::from(m.branch.clone())))
                            .child(icon(IconName::Lock, px(12.)))
                            .into_any_element(),
                    );
                } else {
                    pr.branch.update(cx, |i, cx| i.set_error(bad, cx));
                    body.push(pr.branch.clone().into_any_element());
                }
                body.push(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .text_size(px(11.))
                        .text_color(if bad { c.st_error } else { c.text_muted })
                        .child(SharedString::from(hint))
                        .into_any_element(),
                );
                body.push(label(m.commits_label()));
                for input in &pr.commits {
                    body.push(input.clone().into_any_element());
                }
                if m.kind == crewboard_core::pr::flow::FlowKind::Open {
                    body.push(label("PR title".to_owned()));
                    body.push(pr.title.clone().into_any_element());
                    body.push(label("Description".to_owned()));
                    body.push(pr.description.clone().into_any_element());
                }
                body.push(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(12.))
                        .text_color(c.text_secondary)
                        .child(icon(IconName::GitMerge, px(12.)))
                        .child(SharedString::from(match (m.kind, m.pr_number) {
                            (crewboard_core::pr::flow::FlowKind::Push, Some(n)) => format!(
                                "PR #{n} \u{b7} into {}{}",
                                m.target,
                                m.provider
                                    .as_ref()
                                    .map(|p| format!(
                                        " \u{b7} {}",
                                        p.split(" \u{b7} ").next().unwrap_or(p)
                                    ))
                                    .unwrap_or_default()
                            ),
                            _ => m.target_line(),
                        }))
                        .into_any_element(),
                );
                ConfirmDialog::new(
                    "pr-confirm",
                    &pr.focus,
                    DialogKind::Primary,
                    m.heading(),
                    m.confirm_label(),
                    move |window, cx| {
                        let _ = confirm.update(cx, |p, cx| p.confirm_pr(false, window, cx));
                    },
                    cancel,
                )
                .confirm_icon(IconName::Upload)
                .confirm_disabled(!m.can_confirm() || pr.busy)
                .width(560.)
                .children_boxes(body)
            }
            Stage::Pushing => ConfirmDialog::new(
                "pr-pushing",
                &pr.focus,
                DialogKind::Primary,
                format!("Pushing {}\u{2026}", m.key),
                "Pushing\u{2026}",
                |_, _| {},
                |_, _| {},
            )
            .confirm_disabled(true)
            .width(480.)
            .children_boxes(
                m.pushing_steps()
                    .into_iter()
                    .enumerate()
                    .map(|(i, step)| {
                        let done = i + 2 < m.pushing_steps().len();
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .text_size(px(12.))
                            .text_color(if done {
                                c.text_primary
                            } else {
                                c.text_secondary
                            })
                            .child(
                                icon(
                                    if done {
                                        IconName::CircleCheck
                                    } else {
                                        IconName::LoaderCircle
                                    },
                                    px(12.),
                                )
                                .text_color(if done {
                                    c.st_approved
                                } else {
                                    c.text_muted
                                }),
                            )
                            .child(SharedString::from(step))
                            .into_any_element()
                    })
                    .collect(),
            ),
            Stage::Reported { message } => {
                let retry = this.clone();
                ConfirmDialog::new(
                    "pr-reported",
                    &pr.focus,
                    DialogKind::Destructive,
                    format!("The skill stopped: {} wasn\u{2019}t pushed", m.key),
                    "Retry",
                    move |window, cx| {
                        let _ = retry.update(cx, |p, cx| p.retry_pr(false, window, cx));
                    },
                    cancel,
                )
                .confirm_icon(IconName::RotateCw)
                .child(code_box(message, &c))
                .text("Fix it (for example log in), then Retry. Cancel discards the PR.")
            }
        };
        Some(dialog.into_any_element())
    }

    /// Opens the approve flow on one of its result dialogs (uncommitted files, changed since review).
    fn open_approve_result(
        &mut self,
        view: ap::MergeView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = matches!(view, ap::MergeView::Changed { .. });
        self.open_approve(window, cx);
        if let Some(flow) = &mut self.approve {
            flow.set_step(ApproveStep::Result(view), changed);
        }
        if changed {
            self.load_new_diffs(cx);
        }
    }
}

fn code_box(text: &str, c: &Colors) -> AnyElement {
    div()
        .px(px(10.))
        .py(px(8.))
        .rounded(px(6.))
        .bg(c.bg_column)
        .font_family(FONT_MONO)
        .text_size(px(12.))
        .text_color(c.text_primary)
        .child(SharedString::from(text.to_owned()))
        .into_any_element()
}
