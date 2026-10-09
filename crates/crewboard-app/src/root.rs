//! The main window: the header (also the title bar), the banner area under it (one banner per situation,
//! d0016), the project rail and board, the toast host bottom-right, the quick switcher and the restart
//! confirmation. While no project exists yet the first-run screens take the place of header and board.
//!
//! While the service is unreachable, or the service needs a restart before anything else works, the content
//! ignores input (SVC-04, SVC-03); the banners' own buttons stay usable.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, ClipboardItem, Context, DismissEvent, Entity, FocusHandle, KeyBinding,
    SharedString, Window, actions, div, prelude::*, px,
};

use crate::components::banner::{Banner, BannerAction, BannerTone};
use crate::components::button::{Button, Variant};
use crate::components::floating_shadow;
use crate::components::kbd::Kbd;
use crate::components::toast::{Toast, ToastKind};
use crate::icons::{IconName, icon};
use crate::onboarding::{Finished, Onboarding, PERMISSION_DOCS_URL, title_bar};
use crate::service_link::ServiceLink;
use crate::shell::{
    Header, QuickSwitcher, SWITCHER_CONTEXT, ShellBody, ShellRequest, SwitcherAddProject,
    SwitcherOutcome,
};
use crate::theme::{self, ActiveTheme, radius, text};
use crate::ui::TextStyleExt;
use crate::view_models::banners::{BannerModel, BannerRow, Situation, SituationKind};
use crate::view_models::connection::{ConnectionViewModel, LinkState, Tone};
use crate::view_models::restart::RestartDialog;
use crate::view_models::toasts::{ToastLevel, ToastModel};
use crate::workspace::Workspace;
use crewboard_core::client::Compatibility;
use crewboard_core::protocol::{OnboardingState, Reply, Request};

actions!(restart_dialog, [CancelDialog]);
actions!(
    workspace,
    [
        SwitchProject,
        NewTicket,
        OpenSettings,
        Project1,
        Project2,
        Project3,
        Project4,
        Project5,
        Project6,
        Project7,
        Project8,
        Project9,
    ]
);

const DIALOG_CONTEXT: &str = "RestartDialog";
const WORKSPACE_CONTEXT: &str = "Workspace";

/// Binds the window's keys (d0058: ⌘ on macOS, Ctrl on Windows and Linux). Call once at start-up.
pub fn bind_keys(cx: &mut App) {
    let w = Some(WORKSPACE_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("escape", CancelDialog, Some(DIALOG_CONTEXT)),
        KeyBinding::new("secondary-p", SwitchProject, w),
        KeyBinding::new("secondary-n", NewTicket, w),
        KeyBinding::new("secondary-,", OpenSettings, w),
        KeyBinding::new("secondary-1", Project1, w),
        KeyBinding::new("secondary-2", Project2, w),
        KeyBinding::new("secondary-3", Project3, w),
        KeyBinding::new("secondary-4", Project4, w),
        KeyBinding::new("secondary-5", Project5, w),
        KeyBinding::new("secondary-6", Project6, w),
        KeyBinding::new("secondary-7", Project7, w),
        KeyBinding::new("secondary-8", Project8, w),
        KeyBinding::new("secondary-9", Project9, w),
        KeyBinding::new("secondary-n", SwitcherAddProject, Some(SWITCHER_CONTEXT)),
    ]);
}

/// Shows the link state where the board will be, until a board exists.
struct BoardPlaceholder {
    link: Entity<ServiceLink>,
}

impl Render for BoardPlaceholder {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vm = ConnectionViewModel::new(&self.link.read(cx).state);
        let colors = ActiveTheme::get(cx).colors;
        let tone = match vm.tone {
            Tone::Calm => colors.text_secondary,
            Tone::Attention => colors.st_attn,
            Tone::Error => colors.st_error,
        };
        div()
            .flex_1()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .gap_1()
            .child(
                div()
                    .text_style_of(text::HEADER)
                    .text_color(tone)
                    .child(SharedString::from(vm.label)),
            )
            .children(vm.detail.map(|d| {
                div()
                    .text_style_of(text::METADATA)
                    .text_color(colors.text_muted)
                    .child(SharedString::from(d))
            }))
    }
}

pub struct Root {
    link: Entity<ServiceLink>,
    workspace: Entity<Workspace>,
    header: Entity<Header>,
    body: Entity<ShellBody>,
    switcher: Option<Entity<QuickSwitcher>>,
    focus: FocusHandle,
    /// The first-run screens, while no project exists (UI-08). Asked once per launch, after connecting.
    onboarding: Option<Entity<Onboarding>>,
    first_run_asked: bool,
    banners: BannerModel,
    toasts: ToastModel,
    dialog: Option<RestartDialog>,
    dialog_focus: FocusHandle,
    was_disconnected: bool,
}

impl Root {
    pub fn new(link: Entity<ServiceLink>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&link, |root, link, cx| {
            root.sync(&link, cx);
            cx.notify();
        })
        .detach();
        cx.observe_window_appearance(window, |_, window, cx| {
            let choice = cx.global::<ActiveTheme>().choice;
            ActiveTheme::update(choice, window.appearance(), cx);
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                let alive = this.update(cx, |root, cx| {
                    if root.toasts.expire(Instant::now()) {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    return;
                }
            }
        })
        .detach();
        let workspace = cx.new(|cx| Workspace::new(link.clone(), cx));
        let header = cx.new(|cx| Header::new(workspace.clone(), cx));
        let body = cx.new(|cx| ShellBody::new(workspace.clone(), cx));
        let placeholder = cx.new(|cx| {
            cx.observe(&link, |_, _, cx| cx.notify()).detach();
            BoardPlaceholder { link: link.clone() }
        });
        body.update(cx, |body, cx| body.set_board(placeholder.into(), cx));
        cx.subscribe(&header, |root, _, request: &ShellRequest, cx| {
            root.on_shell_request(request.clone(), cx)
        })
        .detach();
        cx.subscribe(&body, |root, _, request: &ShellRequest, cx| {
            root.on_shell_request(request.clone(), cx)
        })
        .detach();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            link,
            workspace,
            header,
            body,
            switcher: None,
            focus,
            onboarding: None,
            first_run_asked: false,
            banners: BannerModel::default(),
            toasts: ToastModel::default(),
            dialog: None,
            dialog_focus: cx.focus_handle(),
            was_disconnected: false,
        }
    }

    /// Raises and clears the service banners from the link state; toasts the reconnect and restart outcomes.
    fn sync(&mut self, link: &Entity<ServiceLink>, cx: &mut Context<Self>) {
        let outcome = link.update(cx, |link, _| link.restart_outcome.take());
        let link = link.read(cx);
        let now = Instant::now();
        let service_kinds = [
            SituationKind::ServiceStopped,
            SituationKind::RestartRequired,
            SituationKind::ServiceUpdateReady,
            SituationKind::Reconnecting,
        ];
        let keep = |state: &LinkState| -> Vec<Situation> {
            match state {
                LinkState::Connecting => Vec::new(),
                LinkState::Connected(info) => {
                    let (working, waiting) =
                        (info.status.agents_working, info.status.agents_waiting);
                    match info.compatibility {
                        Compatibility::Same => Vec::new(),
                        Compatibility::ServiceOlder => {
                            vec![Situation::ServiceUpdateReady { working, waiting }]
                        }
                        Compatibility::Incompatible => {
                            vec![Situation::RestartRequired { working, waiting }]
                        }
                    }
                }
                LinkState::Reconnecting { .. } => vec![Situation::Reconnecting],
                LinkState::Stopped | LinkState::Failed(_) => vec![Situation::ServiceStopped],
            }
        };
        let situations = keep(&link.state);
        for kind in service_kinds {
            if !situations.iter().any(|s| s.kind() == kind) {
                self.banners.clear(kind);
            }
        }
        for situation in situations {
            self.banners.raise(situation);
        }

        if !self.first_run_asked && matches!(link.state, LinkState::Connected(_)) {
            self.first_run_asked = true;
            let request = link.spawn_request(Request::OnboardingState);
            cx.spawn(async move |this, cx| {
                let Some(task) = request else { return };
                if let Ok(Ok(Reply::OnboardingState(state))) = task.await {
                    let _ = this.update(cx, |root, cx| root.start_onboarding(state, cx));
                }
            })
            .detach();
        }

        let disconnected = matches!(
            link.state,
            LinkState::Reconnecting { .. } | LinkState::Stopped | LinkState::Failed(_)
        );
        if self.was_disconnected
            && matches!(link.state, LinkState::Connected(_))
            && outcome.is_none()
            && !link.restarting
        {
            self.toasts
                .push(ToastLevel::Success, "Reconnected", None, now);
        }
        self.was_disconnected = disconnected;
        match outcome {
            Some(Ok(())) => {
                self.toasts.push(
                    ToastLevel::Success,
                    "Background service restarted",
                    Some("Connected to the new service".into()),
                    now,
                );
            }
            Some(Err(e)) => {
                self.toasts.push(
                    ToastLevel::Error,
                    "Couldn’t restart the background service",
                    Some(e),
                    now,
                );
            }
            None => {}
        }
    }

    /// Requests from the header and the rail. The ticket editor (T-0040), Settings (T-0045) and the add-project
    /// flow (T-0034) hook in here.
    fn on_shell_request(&mut self, request: ShellRequest, cx: &mut Context<Self>) {
        match request {
            ShellRequest::NewTicket | ShellRequest::OpenSettings | ShellRequest::AddProject => {}
        }
        cx.notify();
    }

    fn open_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.onboarding.is_some() || self.switcher.is_some() {
            return;
        }
        let workspace = self.workspace.clone();
        let switcher = cx.new(|cx| QuickSwitcher::new(workspace, window, cx));
        cx.subscribe_in(
            &switcher,
            window,
            |root, _, _: &DismissEvent, window, cx| {
                root.switcher = None;
                window.focus(&root.focus, cx);
                cx.notify();
            },
        )
        .detach();
        cx.subscribe(&switcher, |root, _, outcome: &SwitcherOutcome, cx| {
            if *outcome == SwitcherOutcome::AddProject {
                root.on_shell_request(ShellRequest::AddProject, cx);
            }
        })
        .detach();
        self.switcher = Some(switcher);
        cx.notify();
    }

    fn activate_project(&mut self, digit: u8, cx: &mut Context<Self>) {
        let key = self
            .workspace
            .read(cx)
            .switcher
            .project_for_digit(digit)
            .map(str::to_owned);
        if let Some(key) = key {
            self.workspace.update(cx, |ws, cx| ws.select(&key, cx));
        }
    }

    fn start_onboarding(&mut self, state: OnboardingState, cx: &mut Context<Self>) {
        if state.has_projects {
            return;
        }
        let link = self.link.clone();
        let onboarding = cx.new(|_| Onboarding::new(link, state.permission_warning_dismissed));
        cx.subscribe(&onboarding, |root, _, finished: &Finished, cx| {
            root.onboarding = None;
            if finished.permission_warning {
                root.banners.raise(Situation::PermissionWarning);
            }
            cx.notify();
        })
        .detach();
        self.onboarding = Some(onboarding);
        cx.notify();
    }

    /// Closing the permission-mode warning is remembered in the app's database (SET-01).
    fn dismiss_banner(&mut self, kind: SituationKind, cx: &mut Context<Self>) {
        self.banners.dismiss(kind);
        if kind == SituationKind::PermissionWarning
            && let Some(task) = self
                .link
                .read(cx)
                .spawn_request(Request::DismissPermissionWarning)
        {
            cx.background_spawn(async move {
                let _ = task.await;
            })
            .detach();
        }
        cx.notify();
    }

    fn on_banner_action(
        &mut self,
        kind: SituationKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match kind {
            SituationKind::ServiceUpdateReady | SituationKind::RestartRequired => {
                let (working, waiting) = match &self.link.read(cx).state {
                    LinkState::Connected(info) => {
                        (info.status.agents_working, info.status.agents_waiting)
                    }
                    _ => (0, 0),
                };
                self.dialog = Some(RestartDialog { working, waiting });
                window.focus(&self.dialog_focus, cx);
                cx.notify();
            }
            // Nothing runs any more, so starting interrupts no one: no confirmation.
            SituationKind::ServiceStopped => self.link.update(cx, |link, cx| link.restart(cx)),
            SituationKind::PermissionWarning => cx.open_url(PERMISSION_DOCS_URL),
            SituationKind::UsageLimit | SituationKind::Reconnecting => {}
        }
    }

    fn confirm_restart(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        self.link.update(cx, |link, cx| link.restart(cx));
        cx.notify();
    }

    fn cancel_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        cx.notify();
    }

    fn banner(&self, row: BannerRow, restarting: bool, cx: &mut Context<Self>) -> AnyElement {
        let kind = row.kind;
        let act = cx.entity().downgrade();
        let dismiss = cx.entity().downgrade();
        banner_element(
            row,
            restarting,
            move |window, cx| {
                let _ = act.update(cx, |root, cx| root.on_banner_action(kind, window, cx));
            },
            move |_, cx| {
                let _ = dismiss.update(cx, |root, cx| root.dismiss_banner(kind, cx));
            },
        )
    }

    fn toast_host(&self, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        toast_stack(&self.toasts, move |id, cx| {
            let _ = this.update(cx, |root, cx| {
                root.toasts.dismiss(id);
                cx.notify();
            });
        })
    }

    fn dialog(&self, dialog: RestartDialog, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let cancel = cx.entity().downgrade();
        let confirm = cx.entity().downgrade();
        let backdrop = cx.entity().downgrade();
        div()
            .id("dialog-backdrop")
            .absolute()
            .size_full()
            .flex()
            .justify_center()
            .items_center()
            .bg(gpui::rgba(0x00000033))
            .on_click(move |_, _, cx| {
                let _ = backdrop.update(cx, |root, cx| root.cancel_dialog(cx));
            })
            .child(
                div()
                    .id("restart-dialog")
                    .key_context(DIALOG_CONTEXT)
                    .track_focus(&self.dialog_focus)
                    .on_action(cx.listener(|root, _: &CancelDialog, _, cx| root.cancel_dialog(cx)))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .w(px(480.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .p(px(20.))
                    .bg(c.bg_raised)
                    .border_1()
                    .border_color(c.border)
                    .rounded(px(radius::DIALOG))
                    .shadow(floating_shadow())
                    .child(
                        div()
                            .text_style_of(text::DISPLAY)
                            .text_size(px(16.))
                            .text_color(c.text_primary)
                            .child(RestartDialog::TITLE),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                icon(IconName::Activity, px(14.))
                                    .text_color(c.st_paused)
                                    .mt(px(2.)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_style_of(text::BODY)
                                    .text_color(c.text_primary)
                                    .child(SharedString::from(dialog.message())),
                            ),
                    )
                    .child(
                        div()
                            .text_style_of(text::BODY)
                            .text_size(px(12.))
                            .text_color(c.text_secondary)
                            .child(RestartDialog::DETAIL),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(Kbd::text("Esc"))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(c.text_muted)
                                    .child("cancel"),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new("dialog-cancel", "Cancel")
                                    .variant(Variant::Ghost)
                                    .on_click(move |_, _, cx| {
                                        let _ =
                                            cancel.update(cx, |root, cx| root.cancel_dialog(cx));
                                    }),
                            )
                            .child(
                                Button::new("dialog-confirm", RestartDialog::CONFIRM)
                                    .icon(IconName::RotateCw)
                                    .on_click(move |_, _, cx| {
                                        let _ =
                                            confirm.update(cx, |root, cx| root.confirm_restart(cx));
                                    }),
                            ),
                    ),
            )
            .into_any_element()
    }
}

/// One banner row as drawn: tone and icon by situation; the action button shows loading while `busy`.
pub fn banner_element(
    row: BannerRow,
    busy: bool,
    on_action: impl Fn(&mut Window, &mut App) + 'static,
    on_dismiss: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let (tone, icon_name) = match row.kind {
        SituationKind::ServiceStopped => (BannerTone::Error, IconName::ServerOff),
        SituationKind::RestartRequired => (BannerTone::Warning, IconName::ServerCog),
        SituationKind::UsageLimit => (BannerTone::Attention, IconName::Gauge),
        SituationKind::PermissionWarning => (BannerTone::Warning, IconName::ShieldAlert),
        SituationKind::ServiceUpdateReady => (BannerTone::Info, IconName::RefreshCw),
        SituationKind::Reconnecting => (BannerTone::Neutral, IconName::RefreshCw),
    };
    let kind = row.kind;
    let mut banner = Banner::new(
        SharedString::from(format!("banner-{kind:?}")),
        tone,
        icon_name,
        row.title,
        row.text,
    );
    if let Some(label) = row.action {
        let (label, action_icon) = match (kind, busy) {
            (SituationKind::ServiceStopped, true) => ("Starting…", IconName::LoaderCircle),
            (_, true) => ("Restarting…", IconName::LoaderCircle),
            (SituationKind::ServiceStopped, false) => (label, IconName::Play),
            (SituationKind::PermissionWarning, false) => (label, IconName::ExternalLink),
            (_, false) => (label, IconName::RotateCw),
        };
        banner = banner.action(
            BannerAction::new(label, move |_, window, cx| on_action(window, cx))
                .icon(action_icon)
                .disabled(busy),
        );
    }
    if row.dismissable {
        banner = banner.on_dismiss(move |_, window, cx| on_dismiss(window, cx));
    }
    banner.into_any_element()
}

/// The toast stack, bottom-right; the newest toast sits at the bottom.
pub fn toast_stack(model: &ToastModel, on_close: impl Fn(u64, &mut App) + 'static) -> AnyElement {
    let on_close = std::rc::Rc::new(on_close);
    let items: Vec<AnyElement> = model
        .items()
        .iter()
        .map(|item| {
            let id = item.id;
            let close = on_close.clone();
            let kind = match item.level {
                ToastLevel::Success => ToastKind::Success,
                ToastLevel::Error => ToastKind::Error,
            };
            let mut toast = Toast::new(("toast", id as usize), kind, item.title.clone())
                .on_close(move |_, _, cx| close(id, cx));
            if item.title == RECONNECTED {
                toast = toast.icon(IconName::RefreshCw, None);
            }
            if let Some(text) = &item.text {
                toast = toast.text(text.clone());
            }
            if let Some((label, command)) = &item.copy_action {
                let command = command.clone();
                toast = toast.action(label.clone(), move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(command.clone()));
                });
            }
            toast.into_any_element()
        })
        .collect();
    div()
        .absolute()
        .bottom(px(16.))
        .right(px(16.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .children(items)
        .into_any_element()
}

/// The quiet toast after the connection came back.
pub const RECONNECTED: &str = "Reconnected";

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let link = self.link.read(cx);
        let vm = ConnectionViewModel::new(&link.state);
        let restarting = link.restarting;
        let input_enabled = vm.input_enabled && !self.banners.only_restart_allowed();
        let colors = ActiveTheme::get(cx).colors;
        let rows: Vec<AnyElement> = self
            .banners
            .rows()
            .into_iter()
            .map(|row| self.banner(row, restarting, cx))
            .collect();
        let toasts = self.toast_host(cx);
        let dialog = self.dialog.map(|d| self.dialog(d, cx));
        let step = self.onboarding.as_ref().map(|o| o.read(cx).step());
        let (top, body) = match &self.onboarding {
            Some(onboarding) => {
                let mut body = div().id("onboarding").flex_1().child(onboarding.clone());
                if !input_enabled {
                    body = body.occlude().opacity(0.6);
                }
                (
                    title_bar(step, &colors).into_any_element(),
                    body.into_any_element(),
                )
            }
            None => {
                let mut body = div()
                    .id("content")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.body.clone());
                if !input_enabled {
                    // Swallow clicks and scrolling while the service can't take them.
                    body = body.occlude().opacity(0.6);
                }
                (
                    self.header.clone().into_any_element(),
                    body.into_any_element(),
                )
            }
        };
        let switcher = self.switcher.clone().map(|s| {
            div()
                .absolute()
                .top(px(72.))
                .left_0()
                .w_full()
                .flex()
                .justify_center()
                .child(s)
        });
        div()
            .id("root")
            .key_context(WORKSPACE_CONTEXT)
            .track_focus(&self.focus)
            .on_action(
                cx.listener(|root, _: &SwitchProject, window, cx| root.open_switcher(window, cx)),
            )
            .on_action(cx.listener(|root, _: &NewTicket, _, cx| {
                root.on_shell_request(ShellRequest::NewTicket, cx)
            }))
            .on_action(cx.listener(|root, _: &OpenSettings, _, cx| {
                root.on_shell_request(ShellRequest::OpenSettings, cx)
            }))
            .on_action(cx.listener(|root, _: &Project1, _, cx| root.activate_project(1, cx)))
            .on_action(cx.listener(|root, _: &Project2, _, cx| root.activate_project(2, cx)))
            .on_action(cx.listener(|root, _: &Project3, _, cx| root.activate_project(3, cx)))
            .on_action(cx.listener(|root, _: &Project4, _, cx| root.activate_project(4, cx)))
            .on_action(cx.listener(|root, _: &Project5, _, cx| root.activate_project(5, cx)))
            .on_action(cx.listener(|root, _: &Project6, _, cx| root.activate_project(6, cx)))
            .on_action(cx.listener(|root, _: &Project7, _, cx| root.activate_project(7, cx)))
            .on_action(cx.listener(|root, _: &Project8, _, cx| root.activate_project(8, cx)))
            .on_action(cx.listener(|root, _: &Project9, _, cx| root.activate_project(9, cx)))
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(colors.bg_app)
            .font_family(theme::FONT_UI)
            .child(top)
            .children(rows)
            .child(body)
            .children(switcher)
            .child(toasts)
            .children(dialog)
    }
}
