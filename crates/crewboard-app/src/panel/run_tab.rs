//! The panel's Run tab (RUN-01, RUN-03; *13 · Run & Shell*): the Run port, Run / Stop, Open when the project has
//! an Open URL, and the live output below (the Run's session, read-only, in the terminal view so colours and long
//! output behave as in a terminal; it follows the end unless scrolled up). Nothing starts without a click.

use crewboard_core::domain::TicketKey;
use crewboard_core::protocol::{Event, KeyParams, Reply, Request};
use crewboard_core::terminals::run::RunInfo;
use gpui::{
    AnyElement, ClipboardItem, Context, Entity, EventEmitter, FontWeight, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, div, prelude::*, px,
};

use crate::components::button::{Button, Variant};
use crate::icons::{IconName, icon};
use crate::service_link::{ServiceEvent, ServiceLink};
use crate::terminal::{TerminalTarget, TerminalView};
use crate::theme::{ActiveTheme, FONT_MONO, text};
use crate::ui::TextStyleExt;
use crate::view_models::clock;
use crate::view_models::run::{self as vm, RunButton, Tone};

/// What the Run tab asks of the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunTabEvent {
    /// The project has no run command yet, or the user wants to fix it (RUN-02).
    EditCommand { project: String },
    /// Something went wrong: a toast.
    Error(String),
}

pub struct RunTab {
    link: Entity<ServiceLink>,
    key: TicketKey,
    /// The Run as the service last said.
    pub run: Option<RunInfo>,
    output: Entity<TerminalView>,
    /// A Run / Stop request is on its way (one click, one request).
    busy: bool,
}

impl EventEmitter<RunTabEvent> for RunTab {}

impl RunTab {
    pub fn new(
        link: Entity<ServiceLink>,
        key: TicketKey,
        font_size: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let output = cx.new(|cx| {
            TerminalView::new(
                link.clone(),
                TerminalTarget::Run(key.clone()),
                font_size,
                window,
                cx,
            )
        });
        cx.subscribe(&link, |tab, _, event: &ServiceEvent, cx| match event {
            ServiceEvent::Event(Event::RunChanged { key, .. }) if *key == tab.key => tab.load(cx),
            ServiceEvent::Resync => tab.load(cx),
            _ => {}
        })
        .detach();
        let mut tab = Self {
            link,
            key,
            run: None,
            output,
            busy: false,
        };
        tab.load(cx);
        tab
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let request = self.link.read(cx).send(Request::RunStatus(KeyParams {
            key: self.key.to_string(),
        }));
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::Run(run)) = request.await {
                let _ = this.update(cx, |tab, cx| {
                    if run.key == tab.key {
                        tab.run = Some(*run);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    /// Run (also Retry): starts the Run; without a run command the panel asks for one.
    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.send(
            Request::RunStart(KeyParams {
                key: self.key.to_string(),
            }),
            cx,
        );
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.send(
            Request::RunStop(KeyParams {
                key: self.key.to_string(),
            }),
            cx,
        );
    }

    fn send(&mut self, request: Request, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        let send = self.link.read(cx).send(request);
        let project = self.key.project.to_string();
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |tab, cx| {
                tab.busy = false;
                match result {
                    Ok(Reply::Run(run)) => tab.run = Some(*run),
                    Ok(_) => {}
                    Err(e) if e.refusal.as_deref() == Some("no_run_command") => {
                        cx.emit(RunTabEvent::EditCommand { project })
                    }
                    Err(e) => cx.emit(RunTabEvent::Error(e.message)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn press(&mut self, button: &RunButton, cx: &mut Context<Self>) {
        match button {
            RunButton::Run | RunButton::Retry => self.start(cx),
            RunButton::Stop => self.stop(cx),
            RunButton::Open { url, .. } => cx.open_url(url),
            RunButton::EditCommand => cx.emit(RunTabEvent::EditCommand {
                project: self.key.project.to_string(),
            }),
            RunButton::CopyLsof => {
                if let Some(port) = self.run.as_ref().and_then(|r| r.port) {
                    cx.write_to_clipboard(ClipboardItem::new_string(vm::lsof(&port.to_string())));
                }
            }
        }
    }

    fn card(&self, run: &RunInfo, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let card = vm::card(run, clock::local_hhmm);
        let (fg, bg, border, state_icon) = match card.tone {
            Tone::Neutral => (c.text_primary, c.bg_column, c.border, IconName::Square),
            Tone::Progress => (c.accent, c.accent_soft, c.accent, IconName::LoaderCircle),
            Tone::Success => (
                c.st_approved,
                c.st_approved_bg,
                c.st_approved,
                IconName::CircleCheck,
            ),
            Tone::Danger => (
                c.st_error,
                c.st_error_bg,
                c.st_error,
                IconName::TriangleAlert,
            ),
        };
        let state_icon = if run.state == crewboard_core::terminals::run::RunState::NotStarted {
            IconName::Play
        } else {
            state_icon
        };
        let buttons = card.buttons.iter().enumerate().map(|(i, b)| {
            let id = SharedString::from(format!("run-button-{i}"));
            let (label, icon_name, variant, disabled) = match b {
                RunButton::Run => ("Run".to_owned(), IconName::Play, Variant::Primary, false),
                RunButton::Stop => (
                    "Stop".to_owned(),
                    IconName::Square,
                    Variant::Secondary,
                    false,
                ),
                RunButton::Open { label, enabled, .. } => (
                    label.clone(),
                    IconName::ExternalLink,
                    Variant::Primary,
                    !enabled,
                ),
                RunButton::EditCommand => (
                    "Edit command".to_owned(),
                    IconName::Pencil,
                    Variant::Primary,
                    false,
                ),
                RunButton::Retry => (
                    "Retry".to_owned(),
                    IconName::RotateCw,
                    if card.buttons.contains(&RunButton::EditCommand) {
                        Variant::Secondary
                    } else {
                        Variant::Primary
                    },
                    false,
                ),
                RunButton::CopyLsof => (
                    "Copy lsof command".to_owned(),
                    IconName::Copy,
                    Variant::Secondary,
                    false,
                ),
            };
            let pressed = b.clone();
            let busy =
                self.busy && matches!(b, RunButton::Run | RunButton::Retry | RunButton::Stop);
            Button::new(id, label)
                .icon(icon_name)
                .variant(variant)
                .compact()
                .disabled(disabled)
                .loading(busy)
                .on_click(cx.listener(move |tab, _, _, cx| tab.press(&pressed, cx)))
        });
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            .rounded(px(8.))
            .bg(bg)
            .border_1()
            .border_color(border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(icon(state_icon, px(14.)).text_color(fg))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(fg)
                            .child(SharedString::from(card.title.clone())),
                    )
                    .children(card.port.clone().map(|port| {
                        div()
                            .font_family(FONT_MONO)
                            .text_size(px(11.))
                            .text_color(fg)
                            .child(SharedString::from(port))
                    })),
            )
            .children(card.command.clone().map(|command| {
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(11.5))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(command))
            }))
            .children(card.message.clone().map(|message| {
                div()
                    .text_style_of(text::BODY)
                    .text_size(px(12.))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(message))
            }))
            .child(div().flex().flex_wrap().gap(px(8.)).children(buttons))
            .into_any_element()
    }
}

impl Render for RunTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let card = self.run.clone().map(|run| self.card(&run, cx));
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(12.))
            .children(card)
            .child(
                div()
                    .px(px(4.))
                    .font_family(FONT_MONO)
                    .text_size(px(10.))
                    .text_color(c.text_muted)
                    .child("OUTPUT"),
            )
            .child(div().flex_1().min_h_0().child(self.output.clone()))
    }
}
