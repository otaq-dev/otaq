//! The panel's Shell tab (RUN-05; *13 · Run & Shell → Shell tab · BILL-4*): a plain terminal in the ticket's
//! worktree with `$PORT` = the Run port. It is not Claude, so its strip says so and uses neutral colours. The
//! shell starts the first time the tab opens (the service keeps it when the window closes) and closes when the
//! ticket is Done. A shell a restart stopped stays stopped until Reopen.

use crewboard_core::domain::TicketKey;
use crewboard_core::protocol::{Reply, Request, ShellOpenParams};
use crewboard_core::session::SessionInfo;
use gpui::{
    AnyElement, Context, Entity, EventEmitter, FontWeight, IntoElement, ParentElement, Render,
    SharedString, Styled, Window, div, prelude::*, px,
};

use crate::components::button::{Button, Variant};
use crate::icons::{IconName, icon};
use crate::service_link::ServiceLink;
use crate::terminal::{TerminalEvent, TerminalTarget, TerminalView};
use crate::theme::{ActiveTheme, FONT_MONO, text};
use crate::ui::TextStyleExt;
use crate::view_models::shell as vm;

/// What the Shell tab asks of the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellTabEvent {
    /// The keyboard went back to the panel (⇧⌘`).
    LeftFocus,
}

pub struct ShellTab {
    link: Entity<ServiceLink>,
    key: TicketKey,
    pub terminal: Entity<TerminalView>,
    /// The shell as the service last said (`None` until the tab first opens).
    session: Option<SessionInfo>,
    /// Why there is no shell (no worktree), as the service said.
    problem: Option<String>,
    /// The worktree (shortened) and the Run port, for the strip.
    path: Option<String>,
    port: Option<u16>,
    opening: bool,
}

impl EventEmitter<ShellTabEvent> for ShellTab {}

impl ShellTab {
    pub fn new(
        link: Entity<ServiceLink>,
        key: TicketKey,
        font_size: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let terminal = cx.new(|cx| {
            TerminalView::new(
                link.clone(),
                TerminalTarget::Shell(key.clone()),
                font_size,
                window,
                cx,
            )
        });
        cx.observe(&terminal, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&terminal, |_, _, event: &TerminalEvent, cx| {
            if *event == TerminalEvent::LeftFocus {
                cx.emit(ShellTabEvent::LeftFocus);
            }
        })
        .detach();
        Self {
            link,
            key,
            terminal,
            session: None,
            problem: None,
            opening: false,
            path: None,
            port: None,
        }
    }

    /// The ticket as the board has it: its worktree and Run port for the strip.
    pub fn set_ticket(&mut self, ticket: &crewboard_core::board::tickets::Ticket) {
        let work = ticket.work.clone().unwrap_or_default();
        self.path = work
            .worktree
            .as_deref()
            .map(crate::view_models::terminal_strip::short_path);
        self.port = work.run_port;
    }

    /// The tab is shown: the first time, the shell opens (or the one there is shows). `reopen` is the user's
    /// Reopen click after the shell ended.
    pub fn open(&mut self, reopen: bool, cx: &mut Context<Self>) {
        if self.opening || (self.session.is_some() && !reopen) {
            return;
        }
        self.opening = true;
        let request = self.link.read(cx).send(Request::ShellOpen(ShellOpenParams {
            key: self.key.to_string(),
            reopen,
        }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |tab, cx| {
                tab.opening = false;
                match result {
                    Ok(Reply::Session(session)) => {
                        tab.problem = None;
                        let attach = session.is_running();
                        tab.session = Some(*session);
                        if attach {
                            tab.terminal.update(cx, |t, cx| t.attach(cx));
                        }
                    }
                    Ok(_) => {}
                    Err(e) => tab.problem = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn strip(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let mono = |text: String| {
            div()
                .font_family(FONT_MONO)
                .text_size(px(11.))
                .text_color(c.text_muted)
                .whitespace_nowrap()
                .child(SharedString::from(text))
        };
        let live = self.session.as_ref().is_some_and(SessionInfo::is_running);
        let ended = self.session.as_ref().is_some_and(|s| !s.is_running());
        let reopen = ended.then(|| {
            Button::new("shell-reopen", "Reopen")
                .icon(IconName::RotateCw)
                .variant(Variant::Secondary)
                .compact()
                .on_click(cx.listener(|tab, _, _, cx| tab.open(true, cx)))
        });
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(34.))
            .pl(px(16.))
            .pr(px(12.))
            .border_b_1()
            .border_color(c.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(div().size(px(7.)).rounded_full().bg(if live {
                        c.text_secondary
                    } else {
                        c.text_muted
                    }))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.text_secondary)
                            .whitespace_nowrap()
                            .child(vm::SHELL_LABEL),
                    ),
            )
            .children(self.path.clone().map(mono))
            .children(self.port.map(|p| mono(vm::port_label(p))))
            .child(div().flex_1())
            .children(reopen)
            .into_any_element()
    }
}

impl Render for ShellTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let stopped = self
            .session
            .as_ref()
            .and_then(|s| vm::stopped_note(s, crate::view_models::clock::local_hhmm));
        let body = if let Some(problem) = self.problem.clone() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .px(px(24.))
                .text_style_of(text::METADATA)
                .text_size(px(12.))
                .text_color(c.text_muted)
                .child(SharedString::from(problem))
                .into_any_element()
        } else if let Some((title, note)) = stopped
            .filter(|_| self.terminal.read(cx).phase() == &crate::terminal::model::Phase::NoSession)
        {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .px(px(24.))
                .child(icon(IconName::Square, px(16.)).text_color(c.text_muted))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.text_primary)
                        .child(SharedString::from(title)),
                )
                .child(
                    div()
                        .text_style_of(text::METADATA)
                        .text_size(px(12.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(note)),
                )
                .child(
                    Button::new("shell-reopen-stopped", "Reopen")
                        .icon(IconName::RotateCw)
                        .variant(Variant::Primary)
                        .on_click(cx.listener(|tab, _, _, cx| tab.open(true, cx))),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .child(self.terminal.clone())
                .into_any_element()
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(self.strip(cx))
            .child(body)
    }
}
