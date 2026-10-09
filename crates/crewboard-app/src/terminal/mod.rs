//! The Terminal tab's terminal: a ticket's agent session, emulated with `alacritty_terminal` and drawn with GPUI as
//! Zed does (SES-09, d0001). It is a real terminal grid, never re-rendered as chat bubbles.
//!
//! The view attaches to the ticket's session through the service (stored scrollback first, then live output),
//! sends typed keys through the service's relay, follows the panel's size, and keeps the scrollback visible with
//! input off while the service is unreachable (SVC-04). After a reconnect it redraws from the service's stored
//! scrollback (SVC-05).
//!
//! The side panel's Terminal tab (T-0059) hosts it; until then nothing constructs it.
#![allow(dead_code)]

pub mod colors;
pub mod element;
pub mod keys;
pub mod model;

use crewboard_core::domain::TicketKey;
use crewboard_core::protocol::{
    Event, InputParams, Reply, Request, ResizeParams, SessionIdParams, SessionTarget, decode_bytes,
    encode_bytes,
};
use crewboard_core::session::{SessionKind, SessionState, TerminalSize};
use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Render, SharedString, Styled, Window, div, prelude::*, px,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::icons::{IconName, icon};
use crate::service_link::{ServiceEvent, ServiceLink};
use crate::theme::{ActiveTheme, text};
use crate::ui::TextStyleExt;
use crate::view_models::clock;
use crate::view_models::connection::LinkState;
use element::TerminalElement;
use model::{Phase, TerminalModel};

pub struct TerminalView {
    link: Entity<ServiceLink>,
    ticket: TicketKey,
    pub model: TerminalModel,
    focus: FocusHandle,
    /// Points; the default comes from Settings → Appearance (T-0045).
    pub font_size: f32,
    /// Keystrokes and resizes, sent in order.
    sender: Option<UnboundedSender<Request>>,
    /// Only the newest attach answer counts.
    attach_generation: u64,
    connected: bool,
}

impl TerminalView {
    pub fn new(
        link: Entity<ServiceLink>,
        ticket: TicketKey,
        font_size: f32,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(&link, |view, _, event: &ServiceEvent, cx| {
            view.on_service(event, cx)
        })
        .detach();
        cx.observe(&link, |view, link, cx| {
            let connected = matches!(link.read(cx).state, LinkState::Connected(_));
            if connected != view.connected {
                view.connected = connected;
                if connected {
                    view.attach(cx);
                } else {
                    view.sender = None;
                    view.model.disconnected();
                }
                cx.notify();
            }
        })
        .detach();
        cx.on_release(|view: &mut Self, cx: &mut App| view.detach(cx))
            .detach();
        let connected = matches!(link.read(cx).state, LinkState::Connected(_));
        let mut view = Self {
            link,
            ticket,
            model: TerminalModel::new(TerminalSize { cols: 80, rows: 24 }),
            focus: cx.focus_handle(),
            font_size,
            sender: None,
            attach_generation: 0,
            connected,
        };
        if connected {
            view.attach(cx);
        }
        view
    }

    pub fn ticket(&self) -> &TicketKey {
        &self.ticket
    }

    /// Asks the service for the ticket's session: its stored output, then live output on this connection.
    pub fn attach(&mut self, cx: &mut Context<Self>) {
        self.attach_generation += 1;
        let generation = self.attach_generation;
        self.model.attaching();
        let link = self.link.read(cx);
        self.sender = link.ordered_sender();
        let request = link.send(Request::SessionAttach(SessionTarget {
            session: None,
            key: Some(self.ticket.to_string()),
        }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |view, cx| {
                if view.attach_generation != generation {
                    return;
                }
                match result {
                    Ok(Reply::SessionAttached(attached)) => {
                        let bytes = decode_bytes(&attached.scrollback).unwrap_or_default();
                        view.model.attach(attached.session, &bytes, attached.end);
                        // The PTY follows this terminal's size.
                        view.send_resize(view.model.size());
                    }
                    Err(e) if e.not_found => view.model.no_session(),
                    Ok(_) | Err(_) => view.model.disconnected(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn detach(&mut self, cx: &mut App) {
        if let Some(session) = self.session_id() {
            let request = self
                .link
                .read(cx)
                .send(Request::SessionDetach(SessionIdParams { session }));
            cx.background_executor()
                .spawn(async move {
                    let _ = request.await;
                })
                .detach();
        }
    }

    fn session_id(&self) -> Option<String> {
        self.model.session.as_ref().map(|s| s.id.clone())
    }

    fn on_service(&mut self, event: &ServiceEvent, cx: &mut Context<Self>) {
        match event {
            ServiceEvent::Event(Event::SessionOutput {
                session,
                offset,
                data,
            }) if Some(session) == self.session_id().as_ref() => {
                let bytes = decode_bytes(data).unwrap_or_default();
                let fed = self.model.output(*offset, &bytes);
                if !fed.replies.is_empty() {
                    self.send(Request::SessionInput(InputParams {
                        session: session.clone(),
                        data: encode_bytes(&fed.replies),
                        reply: true,
                    }));
                }
                if self.model.needs_reattach {
                    self.attach(cx);
                }
                if fed.changed {
                    cx.notify();
                }
            }
            ServiceEvent::Event(Event::SessionState { session })
                if session.kind == SessionKind::Agent
                    && session.ticket.as_ref() == Some(&self.ticket) =>
            {
                if session.state == SessionState::Ended {
                    self.model.ended(session.clone());
                } else if self.session_id().as_ref() != Some(&session.id) {
                    // A new session for this ticket (a start or a resume): follow it.
                    self.attach(cx);
                }
                cx.notify();
            }
            ServiceEvent::Resync => self.attach(cx),
            ServiceEvent::Event(_) => {}
        }
    }

    fn send(&self, request: Request) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(request);
        }
    }

    fn send_resize(&self, size: TerminalSize) {
        if let (Some(session), Phase::Live) = (self.session_id(), &self.model.phase) {
            self.send(Request::SessionResize(ResizeParams { session, size }));
        }
    }

    /// The element measured how many cells fit; the grid and the PTY follow.
    pub fn fit(&mut self, size: TerminalSize, _cx: &mut Context<Self>) {
        if self.model.resize(size) {
            self.send_resize(size);
        }
    }

    pub fn cursor_visible(&self) -> bool {
        self.model.phase == Phase::Live
    }

    /// Sends what a person typed (relayed by the service, which tracks unsent text, d0048).
    pub fn type_bytes(&mut self, bytes: &[u8]) {
        if !self.model.input_enabled() || bytes.is_empty() {
            return;
        }
        if let Some(session) = self.session_id() {
            // Typing scrolls back to the newest output.
            self.model
                .term
                .scroll_display(alacritty_terminal::grid::Scroll::Bottom);
            self.send(Request::SessionInput(InputParams {
                session,
                data: encode_bytes(bytes),
                reply: false,
            }));
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(bytes) = keys::to_bytes(&event.keystroke, self.model.mode()) {
            self.type_bytes(&bytes);
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn hint(&self) -> Option<(IconName, String)> {
        match self.model.phase {
            Phase::Reconnecting => Some((
                IconName::RefreshCw,
                "Input paused while reconnecting · output will catch up".into(),
            )),
            Phase::Ended => Some((IconName::Lock, "Read-only · the session ended".into())),
            Phase::Live => Some((
                IconName::SquareTerminal,
                "Click the terminal to type to the agent".into(),
            )),
            Phase::Attaching | Phase::NoSession => None,
        }
    }

    /// "Session ended · 11:02 · 41 min" and why.
    fn end_marker(&self) -> Option<(String, String)> {
        let session = self.model.session.as_ref()?;
        let ended_at = session.ended_at?;
        let title = format!(
            "Session ended · {} · {}",
            clock::local_hhmm(ended_at),
            clock::duration(ended_at - session.started_at)
        );
        let reason = match session.end_reason.as_deref() {
            Some("exit 0") | None => "The agent's session finished.".to_owned(),
            Some(reason) => format!("The agent's session ended ({reason})."),
        };
        Some((title, reason))
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let phase = self.model.phase.clone();
        let entity = cx.entity();
        let grid = div()
            .size_full()
            .bg(c.term_bg)
            .border_1()
            .border_color(c.border)
            .rounded(px(6.))
            .px(px(14.))
            .py(px(12.))
            .when(phase == Phase::Reconnecting, |d| d.opacity(0.55))
            .child(TerminalElement::new(entity));

        let overlay = match &phase {
            Phase::Reconnecting => Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .px(px(12.))
                            .py(px(8.))
                            .bg(c.bg_raised)
                            .border_1()
                            .border_color(c.border_strong)
                            .rounded(px(6.))
                            .text_style_of(text::METADATA)
                            .text_size(px(12.))
                            .text_color(c.text_secondary)
                            .child(icon(IconName::RefreshCw, px(13.)).text_color(c.text_secondary))
                            .child("Reconnecting… agents keep running"),
                    )
                    .into_any_element(),
            ),
            Phase::NoSession => Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_style_of(text::METADATA)
                    .text_size(px(12.))
                    .text_color(c.text_muted)
                    .child(SharedString::from(format!(
                        "{} has no agent session yet. It starts when the ticket goes to Dev.",
                        self.ticket
                    )))
                    .into_any_element(),
            ),
            Phase::Ended => self.end_marker().map(|(title, reason)| {
                div()
                    .absolute()
                    .left(px(14.))
                    .right(px(14.))
                    .bottom(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(10.))
                    .bg(c.bg_column)
                    .border_1()
                    .border_color(c.text_secondary)
                    .rounded(px(6.))
                    .child(icon(IconName::CircleCheck, px(15.)).text_color(c.text_secondary))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_style_of(text::METADATA)
                                    .text_size(px(12.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(c.text_secondary)
                                    .child(SharedString::from(title)),
                            )
                            .child(
                                div()
                                    .text_style_of(text::METADATA)
                                    .text_color(c.text_secondary)
                                    .child(SharedString::from(reason)),
                            ),
                    )
                    .into_any_element()
            }),
            Phase::Live | Phase::Attaching => None,
        };

        let hint = self.hint().map(|(name, label)| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(4.))
                .text_style_of(text::METADATA)
                .text_color(c.text_muted)
                .child(icon(name, px(13.)).text_color(c.text_muted))
                .child(SharedString::from(label))
        });

        div()
            .id("terminal-well")
            .track_focus(&self.focus)
            .key_context("Terminal")
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, window, cx| {
                    window.focus(&view.focus, cx);
                    cx.notify();
                }),
            )
            .size_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(8.))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(grid)
                    .children(overlay),
            )
            .children(hint)
    }
}
