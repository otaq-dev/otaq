//! The panel's review actions (REV-04; d0034, d0048, d0058): the Send back dialog (⌘⌫ or the button) and the
//! request behind it, with the unsent-text prompt when the session's input line holds text.

use crewboard_core::board::merge::MergeProposal;
use crewboard_core::board::tickets::Ticket;
use crewboard_core::protocol::{KeyParams, LocalMergeParams, ProjectParams, Reply, Request};
use crewboard_core::review::ReturnOutcome;
use gpui::{
    AnyElement, Context, Entity, FocusHandle, Focusable, FontWeight, IntoElement, ParentElement,
    SharedString, Styled, Window, div, prelude::*, px,
};

use super::SidePanel;
use crate::board::Notice;
use crate::components::dialog::ConfirmDialog;
use crate::components::text_input::TextInput;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_MONO};
use crate::view_models::approve::{self as ap, ApproveChoice, MergeView};
use crate::view_models::dialog::DialogKind;
use crate::view_models::panel::Tab;
use crate::view_models::send_back as sb;
use crate::view_models::terminal_strip::{Click, UnsentPrompt};

/// Approve ▾ and Local merge while open: the choice, the confirmation, Merging…, then the result.
pub(super) struct ApproveFlow {
    step: ApproveStep,
    /// The target and proposed message, once the service answered.
    proposal: Option<MergeProposal>,
    message: Entity<TextInput>,
    focus: FocusHandle,
    /// The user chose Approve anyway after "changed since you reviewed it" (REV-06).
    approve_anyway: bool,
    /// The change was found before the choice (not as a merge's answer): Approve anyway continues to it.
    checked_first: bool,
    /// The project's PR provider, for Open PR (REV-11).
    pr: Option<crewboard_core::pr::provider::PrProvider>,
    /// The new commits' file diffs, for "changed since you reviewed it".
    new_diffs: Vec<crewboard_core::git::diff::FileDiff>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ApproveStep {
    Menu,
    Confirm,
    Merging,
    Result(MergeView),
}

/// The open Send back dialog.
pub(super) struct SendBackDialog {
    note: Entity<TextInput>,
    focus: FocusHandle,
}

impl SidePanel {
    /// ⌘⌫ or the Send back button: the dialog, on a ticket (not an epic) in Ready for Review.
    pub fn open_send_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ticket(cx).is_some_and(|t| sb::available(&t)) {
            return;
        }
        let note = cx.new(|cx| {
            TextInput::new(window, cx)
                .multiline(px(56.))
                .placeholder("Anything that isn't tied to a line")
        });
        let focus = cx.focus_handle();
        window.focus(&note.read(cx).focus_handle(cx), cx);
        self.send_back = Some(SendBackDialog { note, focus });
        cx.notify();
    }

    fn close_send_back(&mut self, cx: &mut Context<Self>) {
        self.send_back = None;
        cx.emit(super::PanelEvent::TakeFocus);
        cx.notify();
    }

    /// Sends the review; on "needs confirmation" the unsent-text prompt asks first (d0048).
    pub(super) fn send_back(
        &mut self,
        note: String,
        clear_first: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.reviewing {
            return;
        }
        self.reviewing = true;
        let comments = self.diff.as_ref().map_or(0, |d| d.read(cx).pending());
        let send = self
            .link
            .read(cx)
            .send(sb::request(&self.key, &note, clear_first));
        let key = self.key.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = send.await;
            let _ = this.update_in(cx, |panel, window, cx| {
                panel.reviewing = false;
                match result {
                    Ok(Reply::Returned(ReturnOutcome::Done { .. })) => {
                        panel.send_back = None;
                        // The agent works on it now: show its terminal.
                        panel.tab = Tab::Terminal;
                    }
                    Ok(Reply::Returned(ReturnOutcome::NeedsConfirmation { .. })) => {
                        panel.send_back = None;
                        let typed = panel
                            .terminal
                            .as_ref()
                            .and_then(|t| t.read(cx).typed_line());
                        panel.prompt = Some(UnsentPrompt::for_click(
                            key.clone(),
                            typed,
                            sb::sending(comments),
                            Click::SendBack { note },
                        ));
                        window.focus(&panel.prompt_focus, cx);
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("panel: send back failed: {e}"),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Send to agent to fix (T-0095 shows the button); the same prompt rule applies.
    pub(super) fn send_to_fix(
        &mut self,
        clear_first: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.reviewing {
            return;
        }
        self.reviewing = true;
        let send = self
            .link
            .read(cx)
            .send(crewboard_core::protocol::Request::TicketSendToFix(
                crewboard_core::protocol::SendToFixParams {
                    key: self.key.to_string(),
                    clear_first,
                },
            ));
        let key = self.key.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = send.await;
            let _ = this.update_in(cx, |panel, window, cx| {
                panel.reviewing = false;
                match result {
                    Ok(Reply::Returned(ReturnOutcome::Done { .. })) => panel.tab = Tab::Terminal,
                    Ok(Reply::Returned(ReturnOutcome::NeedsConfirmation { .. })) => {
                        let typed = panel
                            .terminal
                            .as_ref()
                            .and_then(|t| t.read(cx).typed_line());
                        panel.prompt = Some(UnsentPrompt::for_click(
                            key.clone(),
                            typed,
                            "The restack skill (rebase and resolve the conflicts)".to_owned(),
                            Click::SendToFix,
                        ));
                        window.focus(&panel.prompt_focus, cx);
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("panel: send to fix failed: {e}"),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn send_back_dialog(
        &self,
        ticket: &Ticket,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let dialog = self.send_back.as_ref()?;
        let c = ActiveTheme::get(cx).colors;
        let pending = self
            .diff
            .as_ref()
            .map(|d| d.read(cx).pending_comments())
            .unwrap_or_default();
        let tickets = self.workspace.read(cx).tickets.clone();
        let plan = sb::plan(ticket, &pending, &tickets);
        let label = |text: &'static str| {
            div()
                .text_size(px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c.text_muted)
                .child(text)
        };
        let comments: Vec<AnyElement> = if plan.comments.is_empty() {
            vec![
                div()
                    .text_size(px(12.))
                    .text_color(c.text_muted)
                    .child("No line comments: the agent gets your note.")
                    .into_any_element(),
            ]
        } else {
            plan.comments
                .iter()
                .map(|(at, body)| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .p(px(10.))
                        .rounded(px(6.))
                        .bg(c.bg_app)
                        .border_1()
                        .border_color(c.border)
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(11.))
                                .text_color(c.text_muted)
                                .child(SharedString::from(at.clone())),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(c.text_primary)
                                .child(SharedString::from(body.clone())),
                        )
                        .into_any_element()
                })
                .collect()
        };
        let waiting = (!plan.waiting.is_empty()).then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .p(px(10.))
                .rounded(px(6.))
                .bg(c.bg_column)
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(c.text_secondary)
                        .child(sb::waiting_title(&ticket.key)),
                )
                .children(plan.waiting.iter().map(|(key, text)| {
                    div()
                        .flex()
                        .gap(px(8.))
                        .pl(px(11.))
                        .text_size(px(12.))
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_color(c.text_primary)
                                .child(SharedString::from(key.to_string())),
                        )
                        .child(
                            div()
                                .text_color(c.text_secondary)
                                .child(SharedString::from(text.clone())),
                        )
                }))
                .into_any_element()
        });
        let note = dialog.note.clone();
        let send = cx.entity().downgrade();
        let cancel = send.clone();
        let mut body: Vec<AnyElement> = vec![
            div()
                .p(px(10.))
                .rounded(px(6.))
                .bg(c.st_review_bg)
                .text_size(px(12.))
                .text_color(c.text_primary)
                .child(SharedString::from(plan.intro.clone()))
                .into_any_element(),
            label("THE AGENT WILL RECEIVE").into_any_element(),
        ];
        body.extend(comments);
        body.push(
            div()
                .text_size(px(11.))
                .text_color(c.text_secondary)
                .child("General note (optional)")
                .into_any_element(),
        );
        body.push(div().child(note.clone()).into_any_element());
        body.extend(waiting);
        Some(
            ConfirmDialog::new(
                "send-back",
                &dialog.focus,
                DialogKind::Primary,
                plan.title,
                "Send back",
                move |window, cx| {
                    let text = note.read(cx).text().to_owned();
                    let _ = send.update(cx, |p, cx| p.send_back(text, false, window, cx));
                },
                move |_, cx| {
                    let _ = cancel.update(cx, |p, cx| p.close_send_back(cx));
                },
            )
            .confirm_icon(IconName::Undo2)
            .confirm_disabled(self.reviewing)
            .width(520.)
            .children_boxes(body)
            .into_any_element(),
        )
    }
}

impl SidePanel {
    /// ⌘↵, the Approve ▾ button, or a drop on Approved: the choice (Local merge; Open PR from M10).
    pub fn open_approve(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ticket) = self.ticket(cx) else {
            return;
        };
        if ticket.status != crewboard_core::domain::Status::ReadyForReview {
            return;
        }
        let message = cx.new(|cx| TextInput::new(window, cx).multiline(px(110.)).mono());
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.approve = Some(ApproveFlow {
            step: ApproveStep::Menu,
            proposal: None,
            message,
            focus,
            approve_anyway: false,
            checked_first: false,
            pr: None,
            new_diffs: Vec::new(),
        });
        let provider = self
            .link
            .read(cx)
            .send(Request::ProjectPrProvider(ProjectParams {
                project: self.key.project.to_string(),
            }));
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::PrProvider(provider)) = provider.await {
                let _ = this.update(cx, |panel, cx| {
                    if let Some(flow) = &mut panel.approve {
                        flow.pr = Some(provider);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
        // Before any approve: new commits since the review? (REV-06)
        let check = self
            .link
            .read(cx)
            .send(Request::TicketApproveCheck(KeyParams {
                key: self.key.to_string(),
            }));
        let key = self.key.clone();
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::ApproveCheck(check)) = check.await
                && check != crewboard_core::board::review::ApproveCheck::Unchanged
            {
                let _ = this.update(cx, |panel, cx| {
                    if let Some(flow) = &mut panel.approve
                        && flow.step == ApproveStep::Menu
                    {
                        flow.step = ApproveStep::Result(ap::changed_view(&key, &check));
                        flow.checked_first = true;
                        panel.load_new_diffs(cx);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
        let send = self
            .link
            .read(cx)
            .send(Request::TicketMergeProposal(KeyParams {
                key: self.key.to_string(),
            }));
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::MergeProposal(proposal)) = send.await {
                let _ = this.update(cx, |panel, cx| {
                    if let Some(flow) = &mut panel.approve {
                        flow.message
                            .update(cx, |m, cx| m.set_text(&proposal.message, cx));
                        flow.proposal = Some(proposal);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
        cx.notify();
    }

    fn close_approve(&mut self, cx: &mut Context<Self>) {
        self.approve = None;
        cx.emit(super::PanelEvent::TakeFocus);
        cx.notify();
    }

    fn choose(&mut self, choice: ApproveChoice, window: &mut Window, cx: &mut Context<Self>) {
        if choice != ApproveChoice::LocalMerge {
            return;
        }
        if let Some(flow) = &mut self.approve {
            flow.step = ApproveStep::Confirm;
            let focus = flow.message.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    /// Squash & merge: the Local merge request with the confirmed message.
    fn merge(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(flow) = &mut self.approve else {
            return;
        };
        if flow.step == ApproveStep::Merging {
            return;
        }
        let message = flow.message.read(cx).text().to_owned();
        let approve_anyway = flow.approve_anyway;
        flow.step = ApproveStep::Merging;
        window.focus(&flow.focus, cx);
        let send = self
            .link
            .read(cx)
            .send(Request::TicketLocalMerge(LocalMergeParams {
                key: self.key.to_string(),
                message,
                approve_anyway,
            }));
        let key = self.key.clone();
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |panel, cx| {
                let view = match result {
                    Ok(Reply::LocalMerge(outcome)) => ap::view(&key, &outcome),
                    Ok(_) => return,
                    Err(e) => {
                        cx.emit(super::PanelEvent::Toast(Notice {
                            error: true,
                            title: format!("Local merge of {key} failed"),
                            text: Some(e.to_string()),
                        }));
                        panel.approve = None;
                        cx.notify();
                        return;
                    }
                };
                if let MergeView::Merged {
                    title,
                    text,
                    started,
                } = &view
                {
                    cx.emit(super::PanelEvent::Toast(Notice {
                        error: false,
                        title: title.clone(),
                        text: Some(text.clone()),
                    }));
                    if let Some(started) = started {
                        cx.emit(super::PanelEvent::Toast(Notice {
                            error: false,
                            title: started.clone(),
                            text: None,
                        }));
                    }
                    panel.approve = None;
                } else if let Some(flow) = &mut panel.approve {
                    let changed = matches!(view, MergeView::Changed { .. });
                    flow.step = ApproveStep::Result(view);
                    if changed {
                        panel.load_new_diffs(cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn approve_dialog(
        &self,
        ticket: &Ticket,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let flow = self.approve.as_ref()?;
        let c = ActiveTheme::get(cx).colors;
        let tickets = self.workspace.read(cx).tickets.clone();
        let target = flow
            .proposal
            .as_ref()
            .map_or_else(|| "…".to_owned(), |p| p.target.clone());
        let into_epic = flow.proposal.as_ref().is_some_and(|p| p.into_epic);
        let this = cx.entity().downgrade();
        let close = {
            let this = this.clone();
            move |_: &mut Window, cx: &mut gpui::App| {
                let _ = this.update(cx, |p, cx| p.close_approve(cx));
            }
        };
        let text = |t: String| {
            div()
                .text_size(px(12.))
                .text_color(c.text_secondary)
                .child(SharedString::from(t))
                .into_any_element()
        };
        let file_rows = |files: &[String]| -> Vec<AnyElement> {
            files
                .iter()
                .map(|f| {
                    div()
                        .px(px(10.))
                        .py(px(4.))
                        .rounded(px(4.))
                        .bg(c.bg_column)
                        .font_family(FONT_MONO)
                        .text_size(px(11.))
                        .text_color(c.text_primary)
                        .child(SharedString::from(f.clone()))
                        .into_any_element()
                })
                .collect()
        };
        let dialog = match &flow.step {
            ApproveStep::Menu => {
                let options = ap::options(ticket, &target, &tickets, flow.pr.as_ref());
                let rows: Vec<AnyElement> = options
                    .into_iter()
                    .enumerate()
                    .map(|(i, option)| {
                        let pick = this.clone();
                        let choice = option.choice;
                        div()
                            .id(("approve-option", i))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .p(px(10.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c.border)
                            .when(option.enabled, |d| {
                                d.cursor_pointer().hover(|s| s.bg(c.bg_hover)).on_click(
                                    move |_, window, cx| {
                                        let _ =
                                            pick.update(cx, |p, cx| p.choose(choice, window, cx));
                                    },
                                )
                            })
                            .when(!option.enabled, |d| d.opacity(0.5))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(c.text_primary)
                                            .child(SharedString::from(option.title)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(c.text_muted)
                                            .child(SharedString::from(option.detail)),
                                    ),
                            )
                            .child(
                                div()
                                    .font_family(FONT_MONO)
                                    .text_size(px(11.))
                                    .text_color(c.text_muted)
                                    .child(SharedString::from((i + 1).to_string())),
                            )
                            .into_any_element()
                    })
                    .collect();
                let local = this.clone();
                ConfirmDialog::new(
                    "approve-menu",
                    &flow.focus,
                    DialogKind::Primary,
                    ap::menu_title(ticket),
                    "Local merge",
                    move |window, cx| {
                        let _ = local
                            .update(cx, |p, cx| p.choose(ApproveChoice::LocalMerge, window, cx));
                    },
                    close,
                )
                .children_boxes(rows)
                .text(ap::menu_note(ticket))
            }
            ApproveStep::Confirm => {
                let merge = this.clone();
                let repo = self
                    .workspace
                    .read(cx)
                    .current()
                    .map(|p| p.repo_path.clone())
                    .unwrap_or_default();
                ConfirmDialog::new(
                    "approve-confirm",
                    &flow.focus,
                    DialogKind::Primary,
                    ap::confirm_title(ticket),
                    "Squash & merge",
                    move |window, cx| {
                        let _ = merge.update(cx, |p, cx| p.merge(window, cx));
                    },
                    close,
                )
                .confirm_icon(IconName::GitMerge)
                .confirm_disabled(flow.proposal.is_none())
                .width(480.)
                .child(text(ap::into_label(ticket)))
                .child(
                    div()
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(6.))
                        .bg(c.bg_column)
                        .font_family(FONT_MONO)
                        .text_size(px(12.))
                        .text_color(c.text_primary)
                        .child(SharedString::from(format!("{target}  \u{b7}  {repo}"))),
                )
                .child(text("Squash commit message".to_owned()))
                .child(flow.message.clone())
                .child(text(ap::confirm_note(ticket, &target, &tickets)))
            }
            ApproveStep::Merging => ConfirmDialog::new(
                "approve-merging",
                &flow.focus,
                DialogKind::Primary,
                ap::confirm_title(ticket),
                "Merging…",
                |_, _| {},
                |_, _| {},
            )
            .confirm_disabled(true)
            .children_boxes(
                ap::merging_steps(&target, into_epic)
                    .into_iter()
                    .map(|step| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .text_size(px(12.))
                            .text_color(c.text_secondary)
                            .child(icon(IconName::LoaderCircle, px(12.)).text_color(c.text_muted))
                            .child(SharedString::from(step))
                            .into_any_element()
                    })
                    .collect(),
            )
            .text(format!(
                "Runs in {}\u{2019}s own scratch worktree \u{2014} your checkout is never touched.",
                crewboard_core::naming::PRODUCT_NAME
            )),
            ApproveStep::Result(MergeView::Conflict {
                title,
                text: body,
                files,
            }) => {
                let fix = this.clone();
                let worktree = ticket
                    .work
                    .as_ref()
                    .and_then(|w| w.worktree.clone())
                    .unwrap_or_default();
                ConfirmDialog::new(
                    "approve-conflict",
                    &flow.focus,
                    DialogKind::Primary,
                    title.clone(),
                    "Send to agent to fix",
                    move |window, cx| {
                        let _ = fix.update(cx, |p, cx| {
                            p.approve = None;
                            p.send_to_fix(false, window, cx);
                        });
                    },
                    close.clone(),
                )
                .secondary("Close", close)
                .confirm_icon(IconName::Undo2)
                .width(480.)
                .child(text(body.clone()))
                .children_boxes(file_rows(files))
                .child(text(ap::conflict_help(&ticket.key, &target)))
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(worktree)),
                )
            }
            ApproveStep::Result(MergeView::DirtyCheckout { message })
            | ApproveStep::Result(MergeView::Diverged { message }) => {
                let retry = this.clone();
                ConfirmDialog::new(
                    "approve-refused",
                    &flow.focus,
                    DialogKind::Primary,
                    format!("Can\u{2019}t merge into {target} yet"),
                    "Retry",
                    move |window, cx| {
                        let _ = retry.update(cx, |p, cx| p.merge(window, cx));
                    },
                    close.clone(),
                )
                .secondary("Close", close)
                .confirm_icon(IconName::RotateCw)
                .text(message.clone())
            }
            ApproveStep::Result(MergeView::Uncommitted { message, files }) => {
                let back = this.clone();
                let view = this.clone();
                ConfirmDialog::new(
                    "approve-uncommitted",
                    &flow.focus,
                    DialogKind::Primary,
                    message.clone(),
                    "Send back",
                    move |window, cx| {
                        let _ = back.update(cx, |p, cx| {
                            p.approve = None;
                            p.open_send_back(window, cx);
                        });
                    },
                    close,
                )
                .secondary("View", move |_, cx| {
                    let _ = view.update(cx, |p, cx| {
                        p.approve = None;
                        p.set_tab(Tab::Diff, cx);
                    });
                })
                .confirm_icon(IconName::Undo2)
                .children_boxes(file_rows(files))
                .text("Send it back so the agent commits them, or look at them first.")
            }
            ApproveStep::Result(MergeView::Changed { title, commits, .. }) => {
                let anyway = this.clone();
                let back = this.clone();
                let mut rows: Vec<AnyElement> = commits
                    .iter()
                    .map(|(sha, subject)| {
                        div()
                            .flex()
                            .gap(px(8.))
                            .text_size(px(12.))
                            .child(
                                div()
                                    .font_family(FONT_MONO)
                                    .text_color(c.text_muted)
                                    .child(SharedString::from(sha.clone())),
                            )
                            .child(
                                div()
                                    .text_color(c.text_primary)
                                    .child(SharedString::from(subject.clone())),
                            )
                            .into_any_element()
                    })
                    .collect();
                for diff in &flow.new_diffs {
                    rows.push(new_commit_diff(diff, &c));
                }
                ConfirmDialog::new(
                    "approve-changed",
                    &flow.focus,
                    DialogKind::Primary,
                    title.clone(),
                    "Approve anyway",
                    move |window, cx| {
                        let _ = anyway.update(cx, |p, cx| {
                            let Some(flow) = &mut p.approve else {
                                return;
                            };
                            flow.approve_anyway = true;
                            if flow.checked_first {
                                flow.step = ApproveStep::Menu;
                                cx.notify();
                            } else {
                                p.merge(window, cx);
                            }
                        });
                    },
                    close,
                )
                .secondary("Send back", move |window, cx| {
                    let _ = back.update(cx, |p, cx| {
                        p.approve = None;
                        p.open_send_back(window, cx);
                    });
                })
                .width(560.)
                .text("Only the new commits are shown. Approve anyway merges them too.")
                .children_boxes(rows)
            }
            ApproveStep::Result(MergeView::Merged { .. }) => return None,
        };
        Some(dialog.into_any_element())
    }
}

impl SidePanel {
    pub(super) fn load_conflict(&mut self, cx: &mut Context<Self>) {
        let key = self.key.clone();
        let send = self.link.read(cx).send(Request::TicketConflict(KeyParams {
            key: key.to_string(),
        }));
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::Conflict(conflict)) = send.await {
                let _ = this.update(cx, |panel, cx| {
                    if panel.key == key {
                        panel.conflict = conflict;
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    /// The header's amber strip while the ticket conflicts with its target, with Send to agent to fix.
    pub(super) fn conflict_strip(
        &self,
        ticket: &Ticket,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let conflict = self.conflict.as_ref()?;
        ticket.flags.conflicts_with.as_ref()?;
        let c = ActiveTheme::get(cx).colors;
        let fix = ap::offers_fix(ticket, Some(conflict)).then(|| {
            crate::components::button::Button::new("panel-send-to-fix", "Send to agent to fix")
                .variant(crate::components::button::Variant::Secondary)
                .compact()
                .icon(IconName::Undo2)
                .loading(self.reviewing)
                .on_click(cx.listener(|panel, _, window, cx| panel.send_to_fix(false, window, cx)))
        });
        Some(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .mx(px(12.))
                .mb(px(8.))
                .px(px(10.))
                .py(px(6.))
                .rounded(px(6.))
                .bg(c.st_paused_bg)
                .child(icon(IconName::TriangleAlert, px(13.)).text_color(c.st_paused))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(12.))
                        .text_color(c.text_primary)
                        .child(SharedString::from(ap::conflict_line(conflict))),
                )
                .children(fix)
                .into_any_element(),
        )
    }
}

impl SidePanel {
    /// Fetches the file diffs of only the new commits (reviewed head → head), for the changed-since-review state.
    fn load_new_diffs(&mut self, cx: &mut Context<Self>) {
        let Some(flow) = &self.approve else {
            return;
        };
        let ApproveStep::Result(MergeView::Changed {
            files, reviewed, ..
        }) = &flow.step
        else {
            return;
        };
        let requests: Vec<_> = files
            .iter()
            .take(10)
            .map(|path| {
                self.link.read(cx).send(Request::DiffFile(
                    crewboard_core::protocol::DiffFileParams {
                        key: self.key.to_string(),
                        path: path.clone(),
                        child: None,
                        since: Some(reviewed.clone()),
                    },
                ))
            })
            .collect();
        cx.spawn(async move |this, cx| {
            let mut diffs = Vec::new();
            for request in requests {
                if let Ok(Reply::FileDiff(diff)) = request.await {
                    diffs.push(*diff);
                }
            }
            let _ = this.update(cx, |panel, cx| {
                if let Some(flow) = &mut panel.approve {
                    flow.new_diffs = diffs;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

/// One file of the new commits: its name and counts, then its lines.
fn new_commit_diff(
    diff: &crewboard_core::git::diff::FileDiff,
    c: &crate::theme::Colors,
) -> AnyElement {
    use crate::diff_view::{code, hunk_header, number, sign};
    let mut rows: Vec<AnyElement> = vec![
        div()
            .flex()
            .gap(px(8.))
            .px(px(8.))
            .py(px(4.))
            .bg(c.bg_app)
            .font_family(FONT_MONO)
            .text_size(px(11.))
            .child(
                div()
                    .flex_1()
                    .text_color(c.text_primary)
                    .child(SharedString::from(diff.file.path.clone())),
            )
            .child(
                div()
                    .text_color(c.text_muted)
                    .child(crate::view_models::diff::counts(
                        diff.file.added,
                        diff.file.removed,
                    )),
            )
            .into_any_element(),
    ];
    for hunk in &diff.hunks {
        rows.push(hunk_header(
            &format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
            ),
            c,
        ));
        for line in &hunk.lines {
            let bg = match line.kind {
                crewboard_core::git::diff::LineKind::Added => Some(c.diff_add_bg),
                crewboard_core::git::diff::LineKind::Removed => Some(c.diff_del_bg),
                crewboard_core::git::diff::LineKind::Context => None,
            };
            rows.push(
                div()
                    .h(px(18.))
                    .flex()
                    .items_center()
                    .when_some(bg, |d, bg| d.bg(bg))
                    .child(number(line.new.or(line.old), c))
                    .child(sign(line.kind, c))
                    .child(code(&line.text, None, c))
                    .into_any_element(),
            );
        }
    }
    div()
        .flex()
        .flex_col()
        .rounded(px(6.))
        .border_1()
        .border_color(c.border)
        .overflow_hidden()
        .children(rows)
        .into_any_element()
}

/// The panel's lifecycle dialogs (T-0106).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelDialog {
    Close,
    Submit,
    /// After a failed resume, or with the conversation gone (REC-03).
    StartFromScratch,
}

impl SidePanel {
    /// "Reopen parent" (CLS-04): the closed parent goes back to Backlog (T-0101).
    pub(super) fn reopen_parent(
        &mut self,
        parent: crewboard_core::domain::TicketKey,
        cx: &mut Context<Self>,
    ) {
        let send = self.link.read(cx).send(Request::TicketReopen(KeyParams {
            key: parent.to_string(),
        }));
        cx.spawn(async move |this, cx| {
            if let Err(e) = send.await {
                let _ = this.update(cx, |_, cx| {
                    cx.emit(super::PanelEvent::Toast(Notice {
                        error: true,
                        title: format!("{parent} can\u{2019}t be reopened"),
                        text: Some(e.to_string()),
                    }))
                });
            }
        })
        .detach();
    }

    /// "Remove dependency" asks once, naming the link.
    pub(super) fn ask_remove_dependency(
        &mut self,
        parent: crewboard_core::domain::TicketKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.remove_dependency = Some((parent, focus));
        cx.notify();
    }

    fn remove_dependency(&mut self, cx: &mut Context<Self>) {
        let Some((parent, _)) = self.remove_dependency.take() else {
            return;
        };
        let send =
            self.link
                .read(cx)
                .send(Request::DepRemove(crewboard_core::protocol::DepParams {
                    child: self.key.to_string(),
                    parent: parent.to_string(),
                }));
        cx.spawn(async move |this, cx| {
            if let Err(e) = send.await {
                let _ = this.update(cx, |_, cx| {
                    cx.emit(super::PanelEvent::Toast(Notice {
                        error: true,
                        title: "The dependency wasn\u{2019}t removed".to_owned(),
                        text: Some(e.to_string()),
                    }))
                });
            }
        })
        .detach();
        cx.notify();
    }

    pub(super) fn remove_dependency_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (parent, focus) = self.remove_dependency.as_ref()?;
        let (title, body) = crate::view_models::close::remove_dependency_text(&self.key, parent);
        let this = cx.entity().downgrade();
        let cancel = this.clone();
        Some(
            ConfirmDialog::new(
                "remove-dependency",
                focus,
                DialogKind::Primary,
                title,
                "Remove dependency",
                move |_, cx| {
                    let _ = this.update(cx, |p, cx| p.remove_dependency(cx));
                },
                move |_, cx| {
                    let _ = cancel.update(cx, |p, cx| {
                        p.remove_dependency = None;
                        cx.notify();
                    });
                },
            )
            .text(body)
            .into_any_element(),
        )
    }
}

/// Close ticket or Submit for review while open.
pub(super) struct LifecycleDialog {
    kind: PanelDialog,
    focus: FocusHandle,
    note: Entity<TextInput>,
    /// The service's refusal, shown inline ("2 files are not committed…").
    error: Option<String>,
}

impl SidePanel {
    pub fn open_dialog(&mut self, kind: PanelDialog, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ticket) = self.ticket(cx) else {
            return;
        };
        let allowed = match kind {
            PanelDialog::Close => crate::view_models::close::can_close(&ticket),
            PanelDialog::Submit => {
                crate::view_models::close::submit_item(&ticket).is_some_and(|item| !item.disabled)
            }
            PanelDialog::StartFromScratch => matches!(
                ticket.status,
                crewboard_core::domain::Status::ResumeFailed
                    | crewboard_core::domain::Status::Interrupted
            ),
        };
        if !allowed {
            return;
        }
        let note = cx.new(|cx| {
            TextInput::new(window, cx)
                .multiline(px(56.))
                .placeholder("What the reviewer should know")
        });
        let focus = cx.focus_handle();
        match kind {
            PanelDialog::Submit => window.focus(&note.read(cx).focus_handle(cx), cx),
            PanelDialog::Close | PanelDialog::StartFromScratch => window.focus(&focus, cx),
        }
        self.lifecycle = Some(LifecycleDialog {
            kind,
            focus,
            note,
            error: None,
        });
        cx.notify();
    }

    fn close_lifecycle(&mut self, cx: &mut Context<Self>) {
        self.lifecycle = None;
        cx.emit(super::PanelEvent::TakeFocus);
        cx.notify();
    }

    /// Reopen (the card's or the panel's button): Done · Closed → Backlog.
    pub fn reopen(&mut self, cx: &mut Context<Self>) {
        let send = self.link.read(cx).send(Request::TicketReopen(KeyParams {
            key: self.key.to_string(),
        }));
        let key = self.key.clone();
        cx.spawn(async move |this, cx| {
            if let Err(e) = send.await {
                let _ = this.update(cx, |_, cx| {
                    cx.emit(super::PanelEvent::Toast(Notice {
                        error: true,
                        title: format!("{key} can\u{2019}t be reopened"),
                        text: Some(e.to_string()),
                    }))
                });
            }
        })
        .detach();
    }

    fn confirm_lifecycle(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &self.lifecycle else {
            return;
        };
        if self.reviewing {
            return;
        }
        let key = self.key.to_string();
        let request = match dialog.kind {
            PanelDialog::Close => {
                Request::TicketClose(crewboard_core::protocol::CloseParams { key, confirm: true })
            }
            PanelDialog::Submit => {
                Request::TicketSubmitByUser(crewboard_core::protocol::SubmitByUserParams {
                    key,
                    note: dialog.note.read(cx).text().to_owned(),
                })
            }
            PanelDialog::StartFromScratch => {
                Request::TicketStartFromScratch(crewboard_core::protocol::CloseParams {
                    key,
                    confirm: true,
                })
            }
        };
        self.reviewing = true;
        let send = self.link.read(cx).send(request);
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |panel, cx| {
                panel.reviewing = false;
                match result {
                    Ok(_) => panel.lifecycle = None,
                    Err(e) => {
                        if let Some(dialog) = &mut panel.lifecycle {
                            dialog.error = Some(e.to_string());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn lifecycle_dialog(
        &self,
        ticket: &Ticket,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let dialog = self.lifecycle.as_ref()?;
        let c = ActiveTheme::get(cx).colors;
        let this = cx.entity().downgrade();
        let confirm = this.clone();
        let cancel = this.clone();
        let error = dialog.error.clone().map(|e| {
            div()
                .p(px(8.))
                .rounded(px(6.))
                .bg(c.st_error_bg)
                .text_size(px(12.))
                .text_color(c.st_error)
                .child(SharedString::from(e))
                .into_any_element()
        });
        let built = match dialog.kind {
            PanelDialog::StartFromScratch => {
                self.scratch_dialog(ticket, dialog, error, confirm, cancel, cx)
            }
            PanelDialog::Close => {
                let running = self.has_live_agent();
                let tickets = self.workspace.read(cx).tickets.clone();
                let text = crate::view_models::close::close_text(ticket, &tickets, running);
                ConfirmDialog::new(
                    "close-ticket",
                    &dialog.focus,
                    DialogKind::Destructive,
                    text.title,
                    text.confirm,
                    move |_, cx| {
                        let _ = confirm.update(cx, |p, cx| p.confirm_lifecycle(cx));
                    },
                    move |_, cx| {
                        let _ = cancel.update(cx, |p, cx| p.close_lifecycle(cx));
                    },
                )
                .confirm_icon(IconName::CircleSlash)
                .confirm_disabled(self.reviewing)
                .width(480.)
                .children_boxes(
                    text.lines
                        .into_iter()
                        .map(|line| {
                            div()
                                .p(px(8.))
                                .rounded(px(6.))
                                .bg(c.bg_column)
                                .text_size(px(12.))
                                .text_color(c.text_primary)
                                .child(SharedString::from(line))
                                .into_any_element()
                        })
                        .collect(),
                )
                .text(text.footer)
                .children_boxes(error.into_iter().collect())
            }
            PanelDialog::Submit => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as i64);
                let text = crate::view_models::close::submit_text(ticket, now);
                ConfirmDialog::new(
                    "submit-for-review",
                    &dialog.focus,
                    DialogKind::Primary,
                    text.title,
                    "Submit for review",
                    move |_, cx| {
                        let _ = confirm.update(cx, |p, cx| p.confirm_lifecycle(cx));
                    },
                    move |_, cx| {
                        let _ = cancel.update(cx, |p, cx| p.close_lifecycle(cx));
                    },
                )
                .confirm_icon(IconName::Send)
                .confirm_disabled(self.reviewing)
                .width(480.)
                .text(text.intro)
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(c.text_secondary)
                        .child("Note for the review summary (optional)"),
                )
                .child(dialog.note.clone())
                .children_boxes(error.into_iter().collect())
                .text(text.footer)
            }
        };
        Some(built.into_any_element())
    }

    /// *16 → Resume failed · Start from scratch*: what is kept, what is lost.
    fn scratch_dialog(
        &self,
        ticket: &Ticket,
        dialog: &LifecycleDialog,
        error: Option<AnyElement>,
        confirm: gpui::WeakEntity<Self>,
        cancel: gpui::WeakEntity<Self>,
        cx: &mut Context<Self>,
    ) -> ConfirmDialog {
        let c = ActiveTheme::get(cx).colors;
        let text = crate::view_models::recovery::scratch_confirm(ticket);
        let list = |title: &'static str, items: Vec<String>, good: bool| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .p(px(10.))
                .rounded(px(6.))
                .bg(if good {
                    c.st_approved_bg
                } else {
                    c.st_error_bg
                })
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(c.text_primary)
                        .child(title),
                )
                .children(items.into_iter().map(move |item| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(12.))
                        .text_color(c.text_primary)
                        .child(
                            icon(if good { IconName::Check } else { IconName::X }, px(12.))
                                .text_color(if good { c.st_approved } else { c.st_error }),
                        )
                        .child(SharedString::from(item))
                }))
                .into_any_element()
        };
        ConfirmDialog::new(
            "start-from-scratch",
            &dialog.focus,
            DialogKind::Primary,
            text.title,
            "Start from scratch",
            move |_, cx| {
                let _ = confirm.update(cx, |p, cx| p.confirm_lifecycle(cx));
            },
            move |_, cx| {
                let _ = cancel.update(cx, |p, cx| p.close_lifecycle(cx));
            },
        )
        .confirm_icon(IconName::RotateCcw)
        .confirm_disabled(self.reviewing)
        .width(480.)
        .text(text.text)
        .child(list("Kept", text.kept, true))
        .child(list("Lost", vec![text.lost.to_owned()], false))
        .child(
            div()
                .text_size(px(11.))
                .text_color(c.text_muted)
                .child(text.note),
        )
        .children_boxes(error.into_iter().collect())
    }
}

impl SidePanel {
    /// An epic's section above the tabs (*Epic panel · SHOP-40 Wishlist*): External ID, progress, its local
    /// branch, the children (a click opens one) and the note about reviewing it whole.
    pub(super) fn epic_section(
        &self,
        ticket: &Ticket,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !ticket.is_epic() {
            return None;
        }
        let c = ActiveTheme::get(cx).colors;
        let tickets = self.workspace.read(cx).tickets.clone();
        let h = crate::view_models::epic_review::header(ticket, &tickets);
        let chip = |text: String, color| {
            div()
                .px(px(6.))
                .py(px(1.))
                .rounded(px(4.))
                .bg(c.bg_column)
                .font_family(FONT_MONO)
                .text_size(px(11.))
                .text_color(color)
                .child(SharedString::from(text))
        };
        let rows: Vec<AnyElement> =
            h.children
                .iter()
                .map(|child| {
                    let key = child.key.clone();
                    div()
                        .id(SharedString::from(format!("epic-child-{}", child.key)))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(8.))
                        .py(px(5.))
                        .rounded(px(5.))
                        .cursor_pointer()
                        .hover(|s| s.bg(c.bg_hover))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(super::PanelEvent::Show(key.clone()))
                        }))
                        .child(
                            div()
                                .w(px(76.))
                                .flex_none()
                                .font_family(FONT_MONO)
                                .text_size(px(11.))
                                .text_color(c.text_muted)
                                .child(SharedString::from(child.key.to_string())),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(12.))
                                .text_color(c.text_primary)
                                .child(SharedString::from(child.title.clone())),
                        )
                        .children(child.deps.clone().map(|d| {
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(10.))
                                .text_color(c.text_muted)
                                .child(SharedString::from(d))
                        }))
                        .child(crate::components::pill::StatusPill::of(child.status))
                        .into_any_element()
                })
                .collect();
        Some(
            div()
                .mx(px(16.))
                .mt(px(8.))
                .mb(px(4.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .children(h.external_id.map(|id| chip(id, c.accent)))
                        .child(chip(h.progress, c.text_secondary))
                        .child(chip(h.branch, c.text_muted)),
                )
                .child(
                    div()
                        .text_size(px(10.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.text_muted)
                        .child(SharedString::from(format!(
                            "TICKETS \u{b7} {}",
                            h.children.len()
                        ))),
                )
                .children(rows)
                .children(h.next_key.map(|n| {
                    div()
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(n))
                }))
                .child(
                    div()
                        .p(px(8.))
                        .rounded(px(6.))
                        .bg(c.bg_column)
                        .text_size(px(11.))
                        .text_color(c.text_secondary)
                        .child(SharedString::from(h.note)),
                )
                .into_any_element(),
        )
    }
}
