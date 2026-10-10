//! The Messages section under Stack (MSG-01, MSG-06; d0030; *05 → Messages · agent-to-agent (SHOP-40.2), Message
//! states, Dark · Messages*): questions between this ticket and its linked tickets, newest last. Read-only, except
//! answering a question that was sent to the user's inbox.

use crewboard_core::protocol::{AnswerParams, KeyParams, Reply, Request};
use gpui::{
    AnyElement, Context, Entity, Focusable, FontWeight, IntoElement, ParentElement, SharedString,
    Styled, Window, div, prelude::*, px,
};

use super::{PanelEvent, SidePanel};
use crate::components::button::{Button, Variant};
use crate::components::text_input::{InputEvent, TextInput};
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, Colors};
use crate::view_models::messages::{self as vm, Line, Thread, Tone};

impl SidePanel {
    /// Fetches the ticket's messages (with its history: on open, on every change of the ticket).
    pub(super) fn load_messages(&mut self, cx: &mut Context<Self>) {
        let key = self.key.clone();
        let request = self.link.read(cx).send(Request::TicketMessages(KeyParams {
            key: key.to_string(),
        }));
        let answers = self
            .link
            .read(cx)
            .send(Request::TicketHumanAnswers(KeyParams {
                key: key.to_string(),
            }));
        cx.spawn(async move |this, cx| {
            let messages = request.await;
            let answers = answers.await;
            let _ = this.update(cx, |panel, cx| {
                if panel.key != key {
                    return;
                }
                if let Ok(Reply::Messages(messages)) = messages {
                    panel.messages = messages;
                }
                if let Ok(Reply::HumanAnswers(answers)) = answers {
                    panel.human_answers = answers;
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens the reply box of a question sent to the inbox.
    fn answer_message(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| TextInput::new(window, cx).placeholder("Type your answer"));
        let submit_id = id.clone();
        cx.subscribe_in(
            &input,
            window,
            move |panel, _, event: &InputEvent, _, cx| match event {
                InputEvent::Submit => panel.send_message_answer(submit_id.clone(), cx),
                InputEvent::Cancel => {
                    panel.message_answer = None;
                    cx.notify();
                }
                _ => cx.notify(),
            },
        )
        .detach();
        window.focus(&input.read(cx).focus_handle(cx), cx);
        self.message_answer = Some((id, input));
        cx.notify();
    }

    fn send_message_answer(&mut self, id: String, cx: &mut Context<Self>) {
        let Some((_, input)) = &self.message_answer else {
            return;
        };
        let answer = input.read(cx).text().trim().to_owned();
        if answer.is_empty() {
            return;
        }
        let request = self
            .link
            .read(cx)
            .send(Request::MessageAnswer(AnswerParams { id, answer }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |panel, cx| {
                match result {
                    Ok(_) => panel.message_answer = None,
                    Err(e) => cx.emit(PanelEvent::Toast(crate::board::Notice {
                        error: true,
                        title: "The answer wasn\u{2019}t sent".into(),
                        text: Some(e.message),
                    })),
                }
                panel.load_messages(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// The section; `None` when the ticket has no messages.
    pub(super) fn messages_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.messages.is_empty() && self.human_answers.is_empty() {
            return None;
        }
        let c = ActiveTheme::get(cx).colors;
        let threads = vm::all_threads(
            &self.messages,
            &self.human_answers,
            &self.key,
            super::now_ms(),
        );
        let rows: Vec<AnyElement> = threads.iter().map(|t| self.thread(t, &c, cx)).collect();
        Some(
            div()
                .px(px(16.))
                .py(px(8.))
                .border_b_1()
                .border_color(c.border)
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .text_size(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.text_secondary)
                                .child(
                                    icon(IconName::MessagesSquare, px(13.))
                                        .text_color(c.text_muted),
                                )
                                .child("Messages"),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(c.text_muted)
                                .child(SharedString::from(vm::heading(&threads))),
                        ),
                )
                .children(rows)
                .into_any_element(),
        )
    }

    fn thread(&self, t: &Thread, c: &Colors, cx: &mut Context<Self>) -> AnyElement {
        let answering = self
            .message_answer
            .as_ref()
            .filter(|(id, _)| *id == t.id)
            .map(|(_, input)| input.clone());
        let id = t.id.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .p(px(8.))
            .rounded(px(6.))
            .bg(c.bg_column)
            .child(line(&t.question, c, cx))
            .children(
                t.answer
                    .as_ref()
                    .map(|a| div().pl(px(14.)).child(line(a, c, cx))),
            )
            .children(t.answerable.then(|| {
                match answering {
                    Some(input) => reply_box(input),
                    None => Button::new(SharedString::from(format!("answer-{id}")), "Answer")
                        .icon(IconName::MessageCircleReply)
                        .variant(Variant::Primary)
                        .compact()
                        .on_click(cx.listener(move |p, _, window, cx| {
                            p.answer_message(id.clone(), window, cx)
                        }))
                        .into_any_element(),
                }
            }))
            .into_any_element()
    }
}

fn reply_box(input: Entity<TextInput>) -> AnyElement {
    div().child(input).into_any_element()
}

fn line(l: &Line, c: &Colors, cx: &mut Context<SidePanel>) -> AnyElement {
    let tone = match l.tone {
        Tone::Muted => c.text_muted,
        Tone::Delivered => c.st_working,
        Tone::Done => c.st_approved,
        Tone::Attention => c.st_attn,
    };
    let jump = |key: Option<crewboard_core::domain::TicketKey>, color| match key {
        Some(key) => {
            let shown = key.clone();
            div()
                .id(SharedString::from(format!("msg-key-{key}-{}", l.time)))
                .cursor_pointer()
                .text_color(color)
                .hover(|s| s.underline())
                .child(SharedString::from(shown.to_string()))
                .on_click(cx.listener(move |_, _, _, cx| cx.emit(PanelEvent::Show(key.clone()))))
                .into_any_element()
        }
        None => div().text_color(color).child("you").into_any_element(),
    };
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(11.))
                // The sender: an agent (bot), or the user when they answered.
                .child(
                    icon(
                        if l.from.is_some() {
                            IconName::Bot
                        } else {
                            IconName::User
                        },
                        px(12.),
                    )
                    .text_color(c.text_muted),
                )
                .child(jump(l.from.clone(), c.text_primary))
                .child(icon(IconName::ArrowRight, px(11.)).text_color(c.text_muted))
                .child(jump(l.to.clone(), c.text_secondary))
                .child(div().flex_1())
                .child(
                    div()
                        .text_color(c.text_muted)
                        .child(SharedString::from(l.time.clone())),
                ),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(c.text_primary)
                .child(SharedString::from(l.text.clone())),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .text_size(px(11.))
                .text_color(tone)
                .child(icon(l.icon, px(11.)).text_color(tone))
                .child(SharedString::from(l.status.clone())),
        )
        .into_any_element()
}
