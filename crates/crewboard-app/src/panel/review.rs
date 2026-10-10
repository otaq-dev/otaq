//! The panel's review actions (REV-04; d0034, d0048, d0058): the Send back dialog (⌘⌫ or the button) and the
//! request behind it, with the unsent-text prompt when the session's input line holds text.

use crewboard_core::board::tickets::Ticket;
use crewboard_core::protocol::Reply;
use crewboard_core::review::ReturnOutcome;
use gpui::{
    AnyElement, Context, Entity, FocusHandle, Focusable, FontWeight, IntoElement, ParentElement,
    SharedString, Styled, Window, div, prelude::*, px,
};

use super::SidePanel;
use crate::components::dialog::ConfirmDialog;
use crate::components::text_input::TextInput;
use crate::icons::IconName;
use crate::theme::{ActiveTheme, FONT_MONO};
use crate::view_models::dialog::DialogKind;
use crate::view_models::panel::Tab;
use crate::view_models::send_back as sb;
use crate::view_models::terminal_strip::{Click, UnsentPrompt};

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
