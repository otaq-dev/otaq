//! The Project terminal (RUN-06; d0019, d0031; *17 · Project terminal*): a bottom drawer (⇧⌘T, or the header's
//! button) with a plain shell in the project's own folder, one per project, owned by the service so it survives
//! closing the window. It doesn't start Claude; `crewboard` is on PATH with the user's full access, and the user
//! may run `claude` there. It ends only when the user exits the shell ("Ended", Reopen) or with a restart.
//!
//! Beside the terminal (never over its output), a summary lists the tickets created from it since it was last
//! dismissed (T-0167, *The review moment*), with a trash button on those that never went to Dev (d0060).

use crewboard_core::domain::TicketKey;
use crewboard_core::protocol::{Event, ProjectParams, ProjectTerminalParams, Reply, Request};
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
use crate::view_models::created_summary::{self, Summary};
use crate::view_models::project_terminal::{self as vm, State};

/// What the drawer asks of its host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectTerminalEvent {
    /// × : the drawer hides; the shell keeps running.
    Hide,
    /// The keyboard went back to the app (⇧⌘`).
    LeftFocus,
    /// A summary row: the ticket's panel.
    OpenTicket(TicketKey),
    /// A summary row's trash: the delete confirmation (T-0162).
    Delete(TicketKey),
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
    /// What was created here since the summary was last dismissed (T-0167).
    created: Option<Summary>,
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
        // The shell ending (exit, restart) changes the header; tickets created, changed or deleted change the
        // summary.
        cx.subscribe(&link, |drawer, _, event: &ServiceEvent, cx| match event {
            ServiceEvent::Event(Event::SessionState { session })
                if drawer.session.as_ref().is_some_and(|s| s.id == session.id) =>
            {
                drawer.session = Some(session.clone());
                cx.notify();
            }
            ServiceEvent::Event(
                Event::TicketChanged { project, .. } | Event::TicketDeleted { project, .. },
            ) if *project == drawer.project => drawer.load_created(cx),
            ServiceEvent::Resync => drawer.load_created(cx),
            _ => {}
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
            created: None,
        };
        drawer.open(false, cx);
        drawer.load_created(cx);
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
                    // Running or ended, the terminal shows it (a new one's start event may have asked first).
                    drawer.terminal.update(cx, |t, cx| t.show(&session, cx));
                    drawer.session = Some(*session);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_created(&mut self, cx: &mut Context<Self>) {
        let request = self
            .link
            .read(cx)
            .send(Request::ProjectTerminalCreated(ProjectParams {
                project: self.project.clone(),
            }));
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::CreatedHere(here)) = request.await {
                let _ = this.update(cx, |drawer, cx| {
                    drawer.created = created_summary::summary(&here.tickets);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// The summary's ×: it lists only tickets created from now on.
    fn dismiss_created(&mut self, cx: &mut Context<Self>) {
        self.created = None;
        let request = self
            .link
            .read(cx)
            .send(Request::ProjectTerminalDismiss(ProjectParams {
                project: self.project.clone(),
            }));
        cx.background_executor()
            .spawn(async move {
                let _ = request.await;
            })
            .detach();
        cx.notify();
    }

    fn summary(&self, summary: &Summary, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let rows = summary.rows.iter().enumerate().map(|(i, row)| {
            let open = row.key.clone();
            let delete = row.key.clone();
            let detail = {
                let mut parts = Vec::new();
                if !row.deps.is_empty() {
                    parts.push(format!("⛓ {}", row.deps.join(" ")));
                }
                parts.push(row.criteria.clone());
                parts.join(" · ")
            };
            div()
                .id(SharedString::from(format!("created-{i}")))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .py(px(6.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|s| s.bg(c.bg_hover))
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(ProjectTerminalEvent::OpenTicket(open.clone()))
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .flex()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .font_family(FONT_MONO)
                                        .text_size(px(11.5))
                                        .text_color(c.accent)
                                        .child(SharedString::from(row.key.to_string())),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(px(12.5))
                                        .text_color(c.text_primary)
                                        .child(SharedString::from(row.title.clone())),
                                ),
                        )
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(10.5))
                                .text_color(c.text_muted)
                                .child(SharedString::from(detail)),
                        ),
                )
                .children(row.deletable.then(|| {
                    div()
                        .id(SharedString::from(format!("created-delete-{i}")))
                        .tooltip(Tooltip::text("Delete…", None))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.stop_propagation();
                            cx.emit(ProjectTerminalEvent::Delete(delete.clone()))
                        }))
                        .child(icon(IconName::Trash2, px(13.)).text_color(c.text_muted))
                }))
        });
        div()
            .w(px(360.))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(10.))
            .border_l_1()
            .border_color(c.border)
            .bg(c.bg_column)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.text_primary)
                            .child(SharedString::from(summary.headline.clone())),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(c.text_muted)
                            .child(SharedString::from(summary.place.clone())),
                    )
                    .child(
                        div()
                            .id("created-dismiss")
                            .tooltip(Tooltip::text("Dismiss", None))
                            .cursor_pointer()
                            .on_click(cx.listener(|drawer, _, _, cx| drawer.dismiss_created(cx)))
                            .child(icon(IconName::X, px(12.)).text_color(c.text_muted)),
                    ),
            )
            .child(
                div()
                    .id("created-rows")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(rows),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .child(created_summary::FOOTER),
            )
            .into_any_element()
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
        let created = self.created.clone().map(|s| self.summary(&s, cx));
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
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .children(hint)
                            .child(div().flex_1().min_h_0().child(self.terminal.clone())),
                    )
                    .children(created),
            )
    }
}
