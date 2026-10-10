//! The panel's review actions (REV-04; d0034, d0048, d0058): the Send back dialog (⌘⌫ or the button) and the
//! request behind it, with the unsent-text prompt when the session's input line holds text.

use crewboard_core::board::merge::MergeProposal;
use crewboard_core::board::tickets::Ticket;
use crewboard_core::protocol::{KeyParams, LocalMergeParams, Reply, Request};
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
        });
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
        flow.step = ApproveStep::Merging;
        window.focus(&flow.focus, cx);
        let send = self
            .link
            .read(cx)
            .send(Request::TicketLocalMerge(LocalMergeParams {
                key: self.key.to_string(),
                message,
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
                    flow.step = ApproveStep::Result(view);
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
                let options = ap::options(ticket, &target, &tickets);
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
                .text("Both open a confirmation step before anything runs.")
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
            ApproveStep::Result(MergeView::Merged { .. }) => return None,
        };
        Some(dialog.into_any_element())
    }
}
