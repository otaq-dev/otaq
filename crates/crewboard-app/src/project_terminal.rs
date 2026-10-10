//! The Project terminal (RUN-06; d0019, d0031; *17 · Project terminal*): a bottom drawer (⇧⌘T, or the header's
//! button) with a plain shell in the project's own folder, one per project, owned by the service so it survives
//! closing the window. It doesn't start Claude; `crewboard` is on PATH with the user's full access, and the user
//! may run `claude` there. It ends only when the user exits the shell ("Ended", Reopen) or with a restart.

use crewboard_core::protocol::{ProjectTerminalParams, Reply, Request};
use crewboard_core::session::SessionInfo;
use gpui::{
    AnyElement, Context, Entity, EventEmitter, Focusable, FontWeight, IntoElement, ParentElement,
    Render, SharedString, Styled, Window, div, prelude::*, px,
};

use crate::components::button::{Button, IconButton, Variant};
use crate::components::menu::Tooltip;
use crate::icons::{IconName, icon};
use crate::service_link::{ServiceEvent, ServiceLink};
use crate::terminal::{TerminalEvent, TerminalTarget, TerminalView};
use crate::theme::{ActiveTheme, FONT_MONO};
use crate::view_models::project_terminal::{self as vm, State};

/// What the drawer asks of its host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectTerminalEvent {
    /// × : the drawer hides; the shell keeps running.
    Hide,
    /// The keyboard went back to the app (⇧⌘`).
    LeftFocus,
}

pub struct ProjectTerminal {
    link: Entity<ServiceLink>,
    pub project: String,
    project_name: String,
    short_repo: String,
    pub terminal: Entity<TerminalView>,
    session: Option<SessionInfo>,
    opening: bool,
    /// Someone typed into the terminal (the hint goes).
    typed: bool,
    pub maximized: bool,
}

impl EventEmitter<ProjectTerminalEvent> for ProjectTerminal {}

impl ProjectTerminal {
    pub fn new(
        link: Entity<ServiceLink>,
        project: String,
        project_name: String,
        short_repo: String,
        font_size: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let terminal = cx.new(|cx| {
            TerminalView::new(
                link.clone(),
                TerminalTarget::Project(project.clone()),
                font_size,
                window,
                cx,
            )
        });
        cx.observe(&terminal, |_, _, cx| cx.notify()).detach();
        cx.subscribe(
            &terminal,
            |drawer, _, event: &TerminalEvent, cx| match event {
                TerminalEvent::LeftFocus => cx.emit(ProjectTerminalEvent::LeftFocus),
                TerminalEvent::Typed => {
                    drawer.typed = true;
                    cx.notify();
                }
                TerminalEvent::OpenInstructions => {}
            },
        )
        .detach();
        // The shell ending (exit, restart) changes the header.
        cx.subscribe(&link, |drawer, _, event: &ServiceEvent, cx| {
            if let ServiceEvent::Event(crewboard_core::protocol::Event::SessionState { session }) =
                event
                && drawer.session.as_ref().is_some_and(|s| s.id == session.id)
            {
                drawer.session = Some(session.clone());
                cx.notify();
            }
        })
        .detach();
        let mut drawer = Self {
            link,
            project,
            project_name,
            short_repo,
            terminal,
            session: None,
            opening: false,
            typed: false,
            maximized: false,
        };
        drawer.open(false, cx);
        drawer
    }

    /// Opens the project's terminal (the one there is, or a new one); `reopen` is the user's Reopen click.
    pub fn open(&mut self, reopen: bool, cx: &mut Context<Self>) {
        if self.opening {
            return;
        }
        self.opening = true;
        let request =
            self.link
                .read(cx)
                .send(Request::ProjectTerminalOpen(ProjectTerminalParams {
                    project: self.project.clone(),
                    reopen,
                }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |drawer, cx| {
                drawer.opening = false;
                if let Ok(Reply::Session(session)) = result {
                    if reopen {
                        drawer.typed = false;
                    }
                    let live = session.is_running();
                    drawer.session = Some(*session);
                    if live {
                        drawer.terminal.update(cx, |t, cx| t.attach(cx));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Puts the keyboard in the terminal.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.terminal.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
    }

    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let state = State::of(self.session.as_ref());
        let dot = match state {
            State::Live => c.st_approved,
            State::Opening => c.text_muted,
            State::Ended | State::StoppedByRestart => c.text_muted,
        };
        let reopen = state.offers_reopen().then(|| {
            Button::new("project-terminal-reopen", "Reopen")
                .icon(IconName::RotateCw)
                .variant(Variant::Secondary)
                .compact()
                .on_click(cx.listener(|drawer, _, _, cx| drawer.open(true, cx)))
        });
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.))
            .h(px(34.))
            .px(px(14.))
            .border_b_1()
            .border_color(c.border)
            .child(icon(IconName::SquareTerminal, px(14.)).text_color(c.text_secondary))
            .child(
                div()
                    .text_size(px(12.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c.text_primary)
                    .child("Project terminal"),
            )
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(self.project_name.clone())),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .child(div().size(px(7.)).rounded_full().bg(dot))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(c.text_secondary)
                            .child(state.label()),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .child(SharedString::from(vm::path_line(&self.short_repo))),
            )
            .children(reopen)
            .child(
                div()
                    .id("project-terminal-maximize")
                    .tooltip(Tooltip::text(
                        if self.maximized {
                            "Restore"
                        } else {
                            "Maximize"
                        },
                        None,
                    ))
                    .child(
                        IconButton::new(
                            "project-terminal-maximize-button",
                            if self.maximized {
                                IconName::Minimize2
                            } else {
                                IconName::Maximize2
                            },
                        )
                        .on_click(cx.listener(|drawer, _, _, cx| {
                            drawer.maximized = !drawer.maximized;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .id("project-terminal-hide")
                    .tooltip(Tooltip::text("Hide · the shell keeps running", None))
                    .child(
                        IconButton::new("project-terminal-hide-button", IconName::X).on_click(
                            cx.listener(|_, _, _, cx| cx.emit(ProjectTerminalEvent::Hide)),
                        ),
                    ),
            )
            .into_any_element()
    }
}

impl Render for ProjectTerminal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let state = State::of(self.session.as_ref());
        let hint = vm::shows_hint(state, self.typed).then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .px(px(18.))
                .pt(px(8.))
                .font_family(FONT_MONO)
                .text_size(px(11.5))
                .text_color(c.text_muted)
                .children(
                    vm::hint(&self.project_name, &self.short_repo)
                        .into_iter()
                        .map(|line| div().child(SharedString::from(line))),
                )
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(c.bg_panel)
            .border_t_1()
            .border_color(c.border_strong)
            .child(self.header(cx))
            .children(hint)
            .child(div().flex_1().min_h_0().child(self.terminal.clone()))
    }
}
