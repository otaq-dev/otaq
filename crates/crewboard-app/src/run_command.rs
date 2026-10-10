//! The run-command dialog (RUN-02; *13 · Run & Shell → 1 · Empty … 5 · Saved*): asks "What command runs
//! shop-web?" with an optional Open URL, offers **Ask Claude** (a click; Cancel stops it), and on Save stores both in
//! the project's settings in the app database (d0023) and starts the ticket's Run. The logic is
//! [`crate::view_models::run_command`].

use std::time::Duration;

use crewboard_core::board::settings::ProjectSettings;
use crewboard_core::protocol::{
    Event, KeyParams, ProjectParams, ProjectSettingsParams, Reply, Request,
};
use crewboard_core::terminals::run::RunInfo;
use crewboard_core::terminals::suggest::SuggestUpdate;
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, IntoElement,
    SharedString, Window, div, prelude::*, px,
};

use crate::components::button::{Button, Variant};
use crate::components::dialog::ConfirmDialog;
use crate::components::text_input::{InputEvent, TextInput};
use crate::icons::{IconName, icon};
use crate::service_link::{ServiceEvent, ServiceLink};
use crate::theme::{ActiveTheme, FONT_MONO, text};
use crate::ui::TextStyleExt;
use crate::view_models::dialog::DialogKind;
use crate::view_models::run_command::{Phase, RunCommandModel};

/// The dialog closed (saved or not).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCommandClosed;

/// How long "Saved" shows before the dialog closes.
const SAVED_FOR: Duration = Duration::from_millis(1600);

pub struct RunCommandDialog {
    link: Entity<ServiceLink>,
    model: RunCommandModel,
    command: Entity<TextInput>,
    url: Entity<TextInput>,
    focus: FocusHandle,
    /// The project's settings as loaded (Save changes only the two fields).
    settings: Option<ProjectSettings>,
    saving: bool,
}

impl EventEmitter<RunCommandClosed> for RunCommandDialog {}

impl RunCommandDialog {
    pub fn new(
        link: Entity<ServiceLink>,
        project: String,
        project_name: String,
        run: Option<RunInfo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let command = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("e.g. pnpm dev · dotnet run · cargo run -- serve · python -m app")
                .mono()
        });
        let url = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("e.g. http://localhost:$PORT")
                .mono()
        });
        for input in [&command, &url] {
            cx.subscribe(input, |dialog, _, event: &InputEvent, cx| match event {
                InputEvent::Changed => {
                    dialog.model.edited();
                    cx.notify();
                }
                InputEvent::Submit => dialog.save(cx),
                InputEvent::Cancel => dialog.dismiss(cx),
                _ => {}
            })
            .detach();
        }
        cx.subscribe(&link, |dialog, _, event: &ServiceEvent, cx| {
            if let ServiceEvent::Event(Event::RunSuggestion { project, update }) = event
                && *project == dialog.model.project
            {
                dialog.on_suggestion(update, cx);
            }
        })
        .detach();
        let focus = cx.focus_handle();
        window.focus(&command.read(cx).focus_handle(cx), cx);
        let current = run.as_ref().map(|r| r.command.clone()).unwrap_or_default();
        let model = RunCommandModel::new(&project, &project_name, run.as_ref(), &current);
        let request = link
            .read(cx)
            .send(Request::ProjectSettingsGet(ProjectParams {
                project: project.clone(),
            }));
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::ProjectSettings(reply)) = request.await {
                let _ = this.update(cx, |dialog, cx| {
                    let settings = reply.settings;
                    dialog
                        .command
                        .update(cx, |i, cx| i.set_text(&settings.run_command, cx));
                    dialog
                        .url
                        .update(cx, |i, cx| i.set_text(&settings.open_url, cx));
                    dialog.settings = Some(settings);
                    cx.notify();
                });
            }
        })
        .detach();
        Self {
            link,
            model,
            command,
            url,
            focus,
            settings: None,
            saving: false,
        }
    }

    fn command_text(&self, cx: &App) -> String {
        self.command.read(cx).text().to_owned()
    }

    /// Ask Claude (a click: it spends tokens).
    fn ask(&mut self, cx: &mut Context<Self>) {
        if !self.model.ask() {
            return;
        }
        let request = self.link.read(cx).send(Request::RunSuggest(ProjectParams {
            project: self.model.project.clone(),
        }));
        cx.spawn(async move |this, cx| {
            if let Err(e) = request.await {
                let _ = this.update(cx, |dialog, cx| {
                    let command = dialog.command_text(cx);
                    dialog.model.failed(&e.message, &command);
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }

    fn on_suggestion(&mut self, update: &SuggestUpdate, cx: &mut Context<Self>) {
        match update {
            SuggestUpdate::Progress { line } => self.model.progress(line),
            SuggestUpdate::Done { suggestion } => {
                if let Some((command, url)) = self.model.suggested(suggestion) {
                    self.command.update(cx, |i, cx| i.set_text(&command, cx));
                    self.url.update(cx, |i, cx| i.set_text(&url, cx));
                }
            }
            SuggestUpdate::Failed { message } => {
                let command = self.command_text(cx);
                self.model.failed(message, &command);
            }
        }
        cx.notify();
    }

    /// Cancel, Esc or a click outside: a running Ask Claude stops (its process is killed), and the dialog closes
    /// without saving.
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.model.thinking() {
            let command = self.command_text(cx);
            self.model.cancel_ask(&command);
            let request = self
                .link
                .read(cx)
                .send(Request::RunSuggestCancel(ProjectParams {
                    project: self.model.project.clone(),
                }));
            cx.background_executor()
                .spawn(async move {
                    let _ = request.await;
                })
                .detach();
        }
        cx.emit(RunCommandClosed);
    }

    /// Save (& start): the two fields go into the project's settings, then the ticket's Run starts.
    fn save(&mut self, cx: &mut Context<Self>) {
        let command = self.command_text(cx);
        if self.saving || !self.model.can_save(&command) {
            return;
        }
        let Some(mut settings) = self.settings.clone() else {
            return;
        };
        settings.run_command = command.trim().to_owned();
        settings.open_url = self.url.read(cx).text().trim().to_owned();
        self.saving = true;
        let link = self.link.clone();
        let set = link
            .read(cx)
            .send(Request::ProjectSettingsSet(ProjectSettingsParams {
                project: self.model.project.clone(),
                settings,
            }));
        let start = self.model.ticket.as_ref().map(|(key, _)| key.to_string());
        cx.spawn(async move |this, cx| {
            let saved = set.await;
            let _ = this.update(cx, |dialog, cx| {
                dialog.saving = false;
                match saved {
                    Ok(_) => {
                        dialog.model.saved();
                        if let Some(key) = start {
                            let run = dialog
                                .link
                                .read(cx)
                                .send(Request::RunStart(KeyParams { key }));
                            cx.background_executor()
                                .spawn(async move {
                                    let _ = run.await;
                                })
                                .detach();
                        }
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(SAVED_FOR).await;
                            let _ = this.update(cx, |_, cx| cx.emit(RunCommandClosed));
                        })
                        .detach();
                    }
                    Err(e) => dialog.model.error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for RunCommandDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let command = self.command_text(cx);
        let field = |prefix: gpui::AnyElement, input: Entity<TextInput>| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(10.))
                .py(px(6.))
                .rounded(px(6.))
                .border_1()
                .border_color(c.border_strong)
                .bg(c.bg_panel)
                .child(prefix)
                .child(div().flex_1().min_w_0().child(input))
        };
        let dollar = div()
            .font_family(FONT_MONO)
            .text_size(px(12.))
            .text_color(c.text_muted)
            .child("$")
            .into_any_element();
        let link_icon = icon(IconName::ExternalLink, px(12.))
            .text_color(c.text_muted)
            .into_any_element();
        let muted = |s: &'static str| {
            div()
                .text_style_of(text::METADATA)
                .text_color(c.text_muted)
                .child(s)
        };
        let saved = matches!(self.model.phase, Phase::Saved { .. });
        let intro = (!saved).then(|| {
            div()
                .text_style_of(text::BODY)
                .text_size(px(13.))
                .text_color(c.text_secondary)
                .child(
                    "Runs in the ticket’s worktree when you press Run. Use $PORT if it listens on a port. Saved in \
                     project settings — nothing is written to the repo.",
                )
        });
        let url_block = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.text_primary)
                            .child("Open URL"),
                    )
                    .child(muted("optional")),
            )
            .child(field(link_icon, self.url.clone()))
            .child(muted(
                "Leave empty for backends, CLIs and libraries — then there’s no Open button.",
            ));
        let box_ = |content: gpui::Div| {
            content
                .flex()
                .flex_col()
                .gap(px(4.))
                .px(px(12.))
                .py(px(10.))
                .rounded(px(6.))
                .bg(c.st_investigate_bg)
        };
        let status = match &self.model.phase {
            Phase::Thinking { lines } => Some(
                box_(div())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                icon(IconName::LoaderCircle, px(12.)).text_color(c.st_investigate),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(c.st_investigate)
                                    .child("Claude is reading the repo…"),
                            ),
                    )
                    .children(lines.iter().map(|l| {
                        div()
                            .font_family(FONT_MONO)
                            .text_size(px(11.))
                            .text_color(c.text_secondary)
                            .child(SharedString::from(format!("· {l}")))
                    }))
                    .into_any_element(),
            ),
            Phase::Suggested { why } if !why.is_empty() => Some(
                box_(div())
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(icon(IconName::Sparkles, px(13.)).text_color(c.st_investigate))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.))
                                    .text_color(c.text_primary)
                                    .child(SharedString::from(why.clone())),
                            ),
                    )
                    .into_any_element(),
            ),
            Phase::Saved { message } => Some(
                div()
                    .flex()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(6.))
                    .bg(c.st_approved_bg)
                    .child(icon(IconName::CircleCheck, px(13.)).text_color(c.st_approved))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .text_color(c.text_primary)
                            .child(SharedString::from(message.clone())),
                    )
                    .into_any_element(),
            ),
            _ => None,
        };
        let ask = self.model.offers_ask(&command).then(|| {
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    Button::new("run-command-ask", "Ask Claude")
                        .icon(IconName::Sparkles)
                        .variant(Variant::Secondary)
                        .compact()
                        .on_click(cx.listener(|dialog, _, _, cx| dialog.ask(cx))),
                )
                .child(muted("Claude looks at the repo and suggests a command."))
        });
        let error = self.model.error.clone().map(|e| {
            div()
                .px(px(10.))
                .py(px(8.))
                .rounded(px(6.))
                .bg(c.st_error_bg)
                .text_size(px(12.))
                .text_color(c.st_error)
                .child(SharedString::from(e))
        });
        let this = cx.entity().downgrade();
        let cancel = cx.entity().downgrade();
        ConfirmDialog::new(
            "run-command",
            &self.focus,
            DialogKind::Primary,
            self.model.title(),
            self.model.save_label(),
            move |_: &mut Window, cx: &mut App| {
                let _ = this.update(cx, |d, cx| d.save(cx));
            },
            move |_: &mut Window, cx: &mut App| {
                let _ = cancel.update(cx, |d, cx| d.dismiss(cx));
            },
        )
        .width(400.)
        .confirm_disabled(!self.model.can_save(&command) || self.saving)
        .children_boxes(
            intro
                .map(|d| d.into_any_element())
                .into_iter()
                .chain([field(dollar, self.command.clone()).into_any_element()])
                .chain([url_block.into_any_element()])
                .chain(ask.map(|d| d.into_any_element()))
                .chain(status)
                .chain(error.map(|d| d.into_any_element()))
                .collect(),
        )
    }
}
