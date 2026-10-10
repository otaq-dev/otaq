//! The main window: the header (also the title bar), the banner area under it (one banner per situation,
//! d0016), the project rail and board, the toast host bottom-right, the quick switcher and the restart
//! confirmation. While no project exists yet the first-run screens take the place of header and board.
//!
//! While the service is unreachable, or the service needs a restart before anything else works, the content
//! ignores input (SVC-04, SVC-03); the banners' own buttons stay usable.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, ClipboardItem, Context, DismissEvent, Entity, EventEmitter, ExternalPaths,
    FocusHandle, KeyBinding, SharedString, Window, actions, div, prelude::*, px,
};

use crate::add_project::{AddProjectEvent, AddProjectForm, FormMode};
use crate::board::{
    BoardView, CardPressed, ChooseApprove, ChooseClose, EditCriteria, Notice, OpenEditor, OpenPanel,
};
use crate::components::banner::{Banner, BannerAction, BannerTone};
use crate::components::button::{Button, Variant};
use crate::components::floating_shadow;
use crate::components::kbd::Kbd;
use crate::components::toast::{Toast, ToastKind};
use crate::editor::{EditorClosed, TicketEditor};
use crate::empty_states::{EmptyBoard, NoProjects};
use crate::icons::{IconName, icon};
use crate::onboarding::{Finished, Onboarding, PERMISSION_DOCS_URL, title_bar};
use crate::panel::{PanelEvent, SidePanel};
use crate::service_link::ServiceLink;
use crate::settings::{SettingsClosed, SettingsView};
use crate::shell::{
    Header, QuickSwitcher, SWITCHER_CONTEXT, ShellBody, ShellRequest, SwitcherAddProject,
    SwitcherOutcome,
};
use crate::theme::{self, ActiveTheme, radius, text};
use crate::ui::TextStyleExt;
use crate::view_models::banners::{BannerModel, BannerRow, Situation, SituationKind};
use crate::view_models::card::CardActionKind;
use crate::view_models::connection::{ConnectionViewModel, LinkState, Tone};
use crate::view_models::panel::{self as panel_vm, Tab};
use crate::view_models::restart::RestartDialog;
use crate::view_models::toasts::{ToastLevel, ToastModel};
use crate::workspace::Workspace;
use crewboard_core::client::Compatibility;
use crewboard_core::domain::TicketKey;
use crewboard_core::protocol::{OnboardingState, Reply, Request};

actions!(restart_dialog, [CancelDialog]);
actions!(
    workspace,
    [
        SwitchProject,
        NewTicket,
        StartDev,
        CancelDrag,
        Search,
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
        OpenSelected,
        ClosePanel,
        ExpandPanel,
        PreviousTicket,
        NextTicket,
        PanelTab1,
        PanelTab2,
        PanelTab3,
        PanelTab4,
        PanelTab5,
        PanelTab6,
        SendBackReview,
        ApproveReview,
        OpenInbox,
        NextInboxItem,
        OpenProjectTerminal,
    ]
);

const DIALOG_CONTEXT: &str = "RestartDialog";
const WORKSPACE_CONTEXT: &str = "Workspace";

/// Binds the window's keys from the d0058 table ([`crate::keymap`]): ⌘ on macOS, Ctrl on Windows and Linux,
/// and there the "in terminal" Ctrl+Shift column while a terminal has focus (T-0158). Call once at start-up.
pub fn bind_keys(cx: &mut App) {
    use crate::keymap::{KeyAction, Platform, bound_columns};
    let w = Some(WORKSPACE_CONTEXT);
    let mut bindings = vec![
        KeyBinding::new("escape", CancelDialog, Some(DIALOG_CONTEXT)),
        KeyBinding::new("escape", CancelDrag, w),
        KeyBinding::new("secondary-n", SwitcherAddProject, Some(SWITCHER_CONTEXT)),
        // The side panel (UI-04, d0058). Esc closes it when nothing is being dragged; a focused terminal keeps
        // Esc for Claude Code (the key router takes it first).
        KeyBinding::new("enter", OpenSelected, w),
        KeyBinding::new("escape", ClosePanel, w),
    ];
    for &(column, in_terminal) in bound_columns(Platform::current()) {
        let context = if in_terminal {
            Some(crate::terminal::TERMINAL_CONTEXT)
        } else {
            w
        };
        for action in KeyAction::all() {
            if let Some(keys) = action.keystroke(column) {
                bindings.extend(app_binding(action, &keys, context));
            }
        }
    }
    cx.bind_keys(bindings);
}

/// The window's binding for one d0058 action. Open / close panel are bound above (plain ↵ / Esc); Leave
/// terminal and Find belong to the terminal ([`crate::terminal::bind_keys`]).
fn app_binding(
    action: crate::keymap::KeyAction,
    keys: &str,
    context: Option<&str>,
) -> Option<KeyBinding> {
    use crate::keymap::KeyAction as A;
    let binding = match action {
        A::SwitchProject => KeyBinding::new(keys, SwitchProject, context),
        A::NewTicket => KeyBinding::new(keys, NewTicket, context),
        A::StartDev => KeyBinding::new(keys, StartDev, context),
        A::Search => KeyBinding::new(keys, Search, context),
        A::Settings => KeyBinding::new(keys, OpenSettings, context),
        A::Project(n) => match n {
            1 => KeyBinding::new(keys, Project1, context),
            2 => KeyBinding::new(keys, Project2, context),
            3 => KeyBinding::new(keys, Project3, context),
            4 => KeyBinding::new(keys, Project4, context),
            5 => KeyBinding::new(keys, Project5, context),
            6 => KeyBinding::new(keys, Project6, context),
            7 => KeyBinding::new(keys, Project7, context),
            8 => KeyBinding::new(keys, Project8, context),
            9 => KeyBinding::new(keys, Project9, context),
            _ => return None,
        },
        A::ExpandPanel => KeyBinding::new(keys, ExpandPanel, context),
        A::PreviousTicket => KeyBinding::new(keys, PreviousTicket, context),
        A::NextTicket => KeyBinding::new(keys, NextTicket, context),
        A::SwitchTab(n) => match n {
            1 => KeyBinding::new(keys, PanelTab1, context),
            2 => KeyBinding::new(keys, PanelTab2, context),
            3 => KeyBinding::new(keys, PanelTab3, context),
            4 => KeyBinding::new(keys, PanelTab4, context),
            5 => KeyBinding::new(keys, PanelTab5, context),
            6 => KeyBinding::new(keys, PanelTab6, context),
            _ => return None,
        },
        // d0058: ⌘⌫ (Ctrl+⌫) is Send back on a ticket in Ready for Review.
        A::SendBack => KeyBinding::new(keys, SendBackReview, context),
        // d0058: ⌘↵ (Ctrl+↵) opens Approve ▾.
        A::Approve => KeyBinding::new(keys, ApproveReview, context),
        // d0058: ⇧⌘I (Ctrl+Shift+I) opens the inbox from anywhere.
        A::OpenInbox => KeyBinding::new(keys, OpenInbox, context),
        // d0058: ⌘J (Ctrl+J) opens the next inbox item with its terminal focused, from anywhere (ATT-07).
        A::NextInboxItem => KeyBinding::new(keys, NextInboxItem, context),
        // d0058: ⇧⌘T (Ctrl+Shift+T) opens or focuses the Project terminal.
        A::ProjectTerminal => KeyBinding::new(keys, OpenProjectTerminal, context),
        A::OpenPanel | A::ClosePanel | A::LeaveTerminal | A::FindInTerminal => return None,
    };
    Some(binding)
}

/// The board area: the board, an empty state, or the link state while there is nothing to show.
struct BoardArea {
    link: Entity<ServiceLink>,
    workspace: Entity<Workspace>,
    board: Entity<BoardView>,
    /// The side panel next to the board (UI-04).
    panel: Option<Entity<SidePanel>>,
    /// The panel's width, remembered while the app runs.
    panel_width: f32,
    /// A resize drag: where it started and the width then.
    resizing: Option<(f32, f32)>,
    panel_focus: FocusHandle,
}

impl BoardArea {
    fn open_panel(&mut self, key: TicketKey, window: &mut Window, cx: &mut Context<Self>) {
        self.board.update(cx, |b, cx| b.select_key(key.clone(), cx));
        if let Some(panel) = &self.panel {
            panel.update(cx, |p, cx| {
                if p.key() != &key {
                    p.show(key, window, cx);
                }
            });
            return;
        }
        let workspace = self.workspace.clone();
        let width = self.panel_width;
        let panel = cx.new(|cx| {
            let mut panel = SidePanel::new(workspace, key, window, cx);
            panel.width = width;
            panel
        });
        cx.subscribe_in(
            &panel,
            window,
            |area, _, event: &PanelEvent, window, cx| match event {
                PanelEvent::Close => {
                    area.close_panel(cx);
                }
                PanelEvent::ToggleExpanded => area.toggle_expanded(cx),
                PanelEvent::Show(key) => area.open_panel(key.clone(), window, cx),
                PanelEvent::TakeFocus => window.focus(&area.panel_focus, cx),
                PanelEvent::Instructions(project) => {
                    cx.emit(ShellRequest::ProjectInstructions(project.clone()))
                }
                PanelEvent::EditRunCommand { project, run } => cx.emit(ShellRequest::RunCommand {
                    project: project.clone(),
                    run: run.clone(),
                }),
                PanelEvent::Toast(notice) => cx.emit(notice.clone()),
                PanelEvent::NextInboxItem => cx.emit(ShellRequest::NextInboxItem),
                PanelEvent::OpenDialog(dialog) => {
                    if let Some(panel) = &area.panel {
                        panel.update(cx, |p, cx| p.open_dialog(*dialog, window, cx));
                    }
                }
                PanelEvent::Start => {
                    let key = area.panel.as_ref().map(|p| p.read(cx).key().clone());
                    if let Some(key) = key {
                        area.board.update(cx, |b, cx| {
                            b.select_key(key, cx);
                            b.start_selected(window, cx);
                        });
                    }
                }
                PanelEvent::EditInPanel => {
                    if let Some(panel) = &area.panel {
                        panel.update(cx, |p, cx| p.open_ticket(true, false, window, cx));
                    }
                }
                PanelEvent::MenuChoice(choice) => {
                    let key = area.panel.as_ref().map(|p| p.read(cx).key().clone());
                    let ticket = key.and_then(|k| {
                        area.workspace
                            .read(cx)
                            .tickets
                            .iter()
                            .find(|t| t.key == k)
                            .cloned()
                    });
                    if let Some(ticket) = ticket {
                        // The board's dialogs need the board on screen.
                        if area.expanded(cx) {
                            area.toggle_expanded(cx);
                        }
                        area.board
                            .update(cx, |b, cx| b.on_menu(choice, &ticket, cx));
                    }
                }
            },
        )
        .detach();
        self.panel = Some(panel);
        cx.notify();
    }

    /// A card's button: Deliver now opens the panel and delivers there (with its prompt if needed).
    fn card_pressed(&mut self, pressed: &CardPressed, window: &mut Window, cx: &mut Context<Self>) {
        let CardPressed(key, kind) = pressed;
        self.open_panel(key.clone(), window, cx);
        let Some(panel) = &self.panel else {
            return;
        };
        match kind {
            CardActionKind::DeliverNow => panel.update(cx, |p, cx| p.deliver_now(None, window, cx)),
            // Retry start, and Retry on a ticket held because its base diverged (SES-02, d0059).
            CardActionKind::RetryStart | CardActionKind::Retry => {
                panel.update(cx, |p, cx| p.retry_start(cx))
            }
            CardActionKind::Reopen => panel.update(cx, |p, cx| p.reopen(cx)),
            CardActionKind::Resume => panel.update(cx, |p, cx| p.resume(false, cx)),
            // A click on an option opens the panel with it picked; sending stays one more click or ↵ (ATT-12).
            CardActionKind::PickOption(n) => {
                panel.update(cx, |p, cx| p.pick_answer(*n, window, cx))
            }
            CardActionKind::SendAnswer => panel.update(cx, |p, cx| p.send_answer(cx)),
            CardActionKind::RetryResume => panel.update(cx, |p, cx| p.resume(true, cx)),
            // REV-13, REV-14: the PR's manual outcomes and Push to PR open the panel's dialogs.
            CardActionKind::MarkMerged => panel.update(cx, |p, cx| p.mark_merged(cx)),
            CardActionKind::MarkClosed => panel.update(cx, |p, cx| p.ask_mark_closed(window, cx)),
            CardActionKind::PushToPr => panel.update(cx, |p, cx| p.open_approve(window, cx)),
            _ => {}
        }
    }

    /// The panel's Ticket tab for a ticket (double click, "Add criteria").
    fn open_ticket_tab(
        &mut self,
        key: TicketKey,
        at_criteria: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_panel(key, window, cx);
        if let Some(panel) = &self.panel {
            panel.update(cx, |p, cx| p.open_ticket(false, at_criteria, window, cx));
        }
    }

    /// `true` when there was a panel to close.
    fn close_panel(&mut self, cx: &mut Context<Self>) -> bool {
        let closed = self.panel.take().is_some();
        cx.notify();
        closed
    }

    fn toggle_expanded(&mut self, cx: &mut Context<Self>) {
        if let Some(panel) = &self.panel {
            panel.update(cx, |p, cx| {
                p.expanded = !p.expanded;
                cx.notify();
            });
            cx.notify();
        }
    }

    fn step(&mut self, step: i32, window: &mut Window, cx: &mut Context<Self>) {
        let next = self
            .panel
            .as_ref()
            .and_then(|p| p.read(cx).neighbour(step, cx));
        if let Some(key) = next {
            self.open_panel(key, window, cx);
        }
    }

    fn set_tab(&mut self, n: u8, cx: &mut Context<Self>) {
        if let (Some(panel), Some(tab)) = (&self.panel, Tab::from_number(n)) {
            panel.update(cx, |p, cx| p.set_tab(tab, cx));
        }
    }

    fn expanded(&self, cx: &App) -> bool {
        self.panel.as_ref().is_some_and(|p| p.read(cx).expanded)
    }

    /// The board and, next to it, the panel with its resize handle (or the panel alone, expanded).
    fn with_panel(
        &self,
        board: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(panel) = self.panel.clone() else {
            return board;
        };
        let c = ActiveTheme::get(cx).colors;
        let expanded = self.expanded(cx);
        let window_width = f32::from(window.viewport_size().width);
        let width = panel_vm::clamp_width(self.panel_width, window_width);
        let handle = div()
            .id("panel-resize")
            .w(px(5.))
            .h_full()
            .flex_none()
            .cursor_col_resize()
            .border_l_1()
            .border_color(c.border_strong)
            .hover(|d| d.bg(c.accent_soft))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|area, e: &gpui::MouseDownEvent, _, cx| {
                    area.resizing = Some((f32::from(e.position.x), area.panel_width));
                    cx.stop_propagation();
                }),
            );
        let panel_box = div()
            .id("panel-box")
            .track_focus(&self.panel_focus)
            .h_full()
            .min_w_0()
            .when(expanded, |d| d.flex_1())
            .when(!expanded, |d| d.w(px(width)).flex_none())
            .child(panel);
        div()
            .id("board-and-panel")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .on_mouse_move(
                cx.listener(move |area, e: &gpui::MouseMoveEvent, window, cx| {
                    if let Some((start_x, start_width)) = area.resizing {
                        if e.pressed_button != Some(gpui::MouseButton::Left) {
                            area.resizing = None;
                            return;
                        }
                        let window_width = f32::from(window.viewport_size().width);
                        area.panel_width = panel_vm::clamp_width(
                            start_width + start_x - f32::from(e.position.x),
                            window_width,
                        );
                        if let Some(panel) = &area.panel {
                            let w = area.panel_width;
                            panel.update(cx, |p, _| p.width = w);
                        }
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|area, _, _, _| area.resizing = None),
            )
            .when(!expanded, |d| {
                d.child(div().flex_1().min_w_0().h_full().flex().child(board))
                    .child(handle)
            })
            .child(panel_box)
            .into_any_element()
    }
}

impl EventEmitter<ShellRequest> for BoardArea {}
impl EventEmitter<Notice> for BoardArea {}

impl Render for BoardArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vm = ConnectionViewModel::new(&self.link.read(cx).state);
        let ws = self.workspace.read(cx);
        if vm.input_enabled && ws.loaded {
            let this = cx.entity().downgrade();
            let tools = cx.entity().downgrade();
            match ws.current() {
                None => {
                    return NoProjects::new(
                        move |_, cx| {
                            let _ = this.update(cx, |_, cx| cx.emit(ShellRequest::AddProject));
                        },
                        move |_, cx| {
                            let _ = tools.update(cx, |_, cx| cx.emit(ShellRequest::CheckTools));
                        },
                    )
                    .into_any_element();
                }
                Some(project) if ws.tickets.is_empty() => {
                    return EmptyBoard::new(&project.name, &project.repo_path, move |_, cx| {
                        let _ = this.update(cx, |_, cx| cx.emit(ShellRequest::NewTicket));
                    })
                    .into_any_element();
                }
                Some(_) => {
                    let board = self.board.clone().into_any_element();
                    return self.with_panel(board, window, cx);
                }
            }
        }
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
            .into_any_element()
    }
}

pub struct Root {
    area: Entity<BoardArea>,
    link: Entity<ServiceLink>,
    workspace: Entity<Workspace>,
    board: Entity<BoardView>,
    header: Entity<Header>,
    body: Entity<ShellBody>,
    switcher: Option<Entity<QuickSwitcher>>,
    add_project: Option<Entity<AddProjectForm>>,
    editor: Option<Entity<TicketEditor>>,
    settings: Option<Entity<SettingsView>>,
    /// ⌘N in the quick switcher asked for the add-project dialog (opened at the next render, which has the
    /// window).
    pending_add: bool,
    focus: FocusHandle,
    /// The first-run screens, while no project exists (UI-08). Asked once per launch, after connecting.
    onboarding: Option<Entity<Onboarding>>,
    first_run_asked: bool,
    banners: BannerModel,
    toasts: ToastModel,
    dialog: Option<RestartDialog>,
    dialog_focus: FocusHandle,
    was_disconnected: bool,
    /// The inbox's Resume all confirmation, while it asks (UI-06).
    resume_all: Option<crate::view_models::inbox::ResumeAllConfirm>,
    /// The Project terminal drawer (RUN-06), while shown; one per project, the shell lives in the service.
    project_terminal: Option<Entity<crate::project_terminal::ProjectTerminal>>,
    /// "What command runs shop-web?" (RUN-02), while open.
    run_command: Option<Entity<crate::run_command::RunCommandDialog>>,
    /// An inbox answer stopped at unsent text in the terminal (SES-07).
    unsent: Option<crate::view_models::terminal_strip::UnsentPrompt>,
    /// When the app opened, the service start the after-restart summary was shown for, and whether the app has
    /// connected yet (REC-06).
    opened_ms: i64,
    summary_for: Option<i64>,
    connected_once: bool,
    /// The Cleanup window while open (REC-04).
    cleanup: Option<Entity<crate::cleanup::CleanupView>>,
    /// The usage-limit banner as last raised (USG-06); recomputed every tick for its countdown.
    limit_banner: Option<crate::view_models::limit_banner::LimitBanner>,
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
            let mut ticks: u32 = 0;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                ticks = ticks.wrapping_add(1);
                let alive = this.update(cx, |root, cx| {
                    // The rail's Cleanup dot: a read-only scan once a minute.
                    if ticks.is_multiple_of(120) {
                        root.workspace.update(cx, |ws, cx| ws.refresh_cleanup(cx));
                    }
                    if root.toasts.expire(Instant::now()) {
                        cx.notify();
                    }
                    // The usage limit's countdown and its reset (USG-06).
                    root.sync_usage_banner(cx);
                    root.workspace.update(cx, |ws, cx| ws.expire_inbox(cx));
                });
                if alive.is_err() {
                    return;
                }
            }
        })
        .detach();
        let workspace = cx.new(|cx| Workspace::new(link.clone(), cx));
        cx.observe(&workspace, |root, _, cx| root.sync_usage_banner(cx))
            .detach();
        let header = cx.new(|cx| Header::new(workspace.clone(), window, cx));
        let body = cx.new(|cx| ShellBody::new(workspace.clone(), cx));
        let board = cx.new(|cx| BoardView::new(workspace.clone(), cx));
        // Tickets agents create show up live with one toast per burst (TKT-03); a ticket that waited for its
        // dependencies says when it starts (T-0083).
        cx.subscribe(
            &workspace,
            |root, _, event: &crate::workspace::WorkspaceEvent, cx| {
                if let crate::workspace::WorkspaceEvent::AgentCreated(title, text)
                | crate::workspace::WorkspaceEvent::TicketStarted(title, text) = event
                {
                    root.toasts.push(
                        ToastLevel::Success,
                        title.clone(),
                        Some(text.clone()),
                        Instant::now(),
                    );
                    cx.notify();
                }
                if let crate::workspace::WorkspaceEvent::RunStopped(message) = event {
                    root.toasts
                        .push(ToastLevel::Success, message.clone(), None, Instant::now());
                    cx.notify();
                }
                if let crate::workspace::WorkspaceEvent::PushedContentDiffers(message) = event {
                    root.toasts.push(
                        ToastLevel::Error,
                        message.clone(),
                        Some("Open the ticket to see what differs.".into()),
                        Instant::now(),
                    );
                    cx.notify();
                }
            },
        )
        .detach();
        cx.subscribe(&board, |root, _, notice: &Notice, cx| {
            let level = if notice.error {
                ToastLevel::Error
            } else {
                ToastLevel::Success
            };
            root.toasts.push(
                level,
                notice.title.clone(),
                notice.text.clone(),
                Instant::now(),
            );
            cx.notify();
        })
        .detach();
        cx.subscribe_in(
            &board,
            window,
            |root, _, edit: &EditCriteria, window, cx| {
                root.open_editor(OpenEditor::Ticket(edit.0.clone()), true, window, cx)
            },
        )
        .detach();
        cx.subscribe_in(&board, window, |root, _, open: &OpenEditor, window, cx| {
            root.open_editor(open.clone(), false, window, cx)
        })
        .detach();
        let area = cx.new(|cx| {
            cx.observe(&link, |_, _, cx| cx.notify()).detach();
            cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
            cx.subscribe(&board, |_, _, request: &ShellRequest, cx| {
                cx.emit(request.clone())
            })
            .detach();
            cx.subscribe_in(
                &board,
                window,
                |area: &mut BoardArea, _, open: &OpenPanel, window, cx| {
                    area.open_panel(open.0.clone(), window, cx)
                },
            )
            .detach();
            cx.subscribe_in(
                &board,
                window,
                |area: &mut BoardArea, _, choose: &ChooseApprove, window, cx| {
                    area.open_panel(choose.0.clone(), window, cx);
                    if let Some(panel) = &area.panel {
                        panel.update(cx, |p, cx| p.open_approve(window, cx));
                    }
                },
            )
            .detach();
            cx.subscribe_in(
                &board,
                window,
                |area: &mut BoardArea, _, choose: &ChooseClose, window, cx| {
                    area.open_panel(choose.0.clone(), window, cx);
                    if let Some(panel) = &area.panel {
                        panel.update(cx, |p, cx| {
                            p.open_dialog(crate::panel::review::PanelDialog::Close, window, cx)
                        });
                    }
                },
            )
            .detach();
            cx.subscribe_in(
                &board,
                window,
                |area: &mut BoardArea, _, pressed: &CardPressed, window, cx| {
                    area.card_pressed(pressed, window, cx)
                },
            )
            .detach();
            // A project switch closes the panel (its ticket belongs to the other board).
            cx.subscribe(
                &workspace,
                |area: &mut BoardArea, _, event: &crate::workspace::WorkspaceEvent, cx| {
                    if matches!(event, crate::workspace::WorkspaceEvent::ProjectSelected(_)) {
                        area.close_panel(cx);
                    }
                },
            )
            .detach();
            BoardArea {
                link: link.clone(),
                workspace: workspace.clone(),
                board: board.clone(),
                panel: None,
                panel_width: panel_vm::DEFAULT_WIDTH,
                resizing: None,
                panel_focus: cx.focus_handle(),
            }
        });
        cx.subscribe_in(
            &area,
            window,
            |root, _, request: &ShellRequest, window, cx| {
                root.on_shell_request(request.clone(), window, cx)
            },
        )
        .detach();
        cx.subscribe(&area, |root, _, notice: &Notice, cx| {
            let level = if notice.error {
                ToastLevel::Error
            } else {
                ToastLevel::Success
            };
            root.toasts.push(
                level,
                notice.title.clone(),
                notice.text.clone(),
                Instant::now(),
            );
            cx.notify();
        })
        .detach();
        body.update(cx, |body, cx| body.set_board(area.clone().into(), cx));
        cx.subscribe_in(
            &header,
            window,
            |root, _, request: &ShellRequest, window, cx| {
                root.on_shell_request(request.clone(), window, cx)
            },
        )
        .detach();
        cx.subscribe_in(
            &body,
            window,
            |root, _, request: &ShellRequest, window, cx| {
                root.on_shell_request(request.clone(), window, cx)
            },
        )
        .detach();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            area,
            link,
            workspace,
            board,
            header,
            body,
            switcher: None,
            add_project: None,
            editor: None,
            settings: None,
            pending_add: false,
            focus,
            onboarding: None,
            first_run_asked: false,
            banners: BannerModel::default(),
            toasts: ToastModel::default(),
            dialog: None,
            dialog_focus: cx.focus_handle(),
            was_disconnected: false,
            resume_all: None,
            run_command: None,
            project_terminal: None,
            unsent: None,
            opened_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as i64),
            summary_for: None,
            connected_once: false,
            cleanup: None,
            limit_banner: None,
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
        // After a reboot, crash or restart: the summary of what was interrupted; after a normal reopen a quiet
        // "Reconnected" (REC-06).
        let mut summarized = false;
        if let LinkState::Connected(info) = &link.state {
            use crate::view_models::after_restart::{OnConnect, on_connect};
            match on_connect(
                &info.status,
                self.opened_ms,
                !self.connected_once,
                self.summary_for,
            ) {
                OnConnect::Summary(situation) => {
                    self.summary_for = Some(info.status.started_at);
                    self.banners.raise(situation);
                    summarized = true;
                }
                OnConnect::Reconnected => {
                    self.toasts
                        .push(ToastLevel::Success, "Reconnected", None, now);
                }
                OnConnect::Nothing => {}
            }
            self.connected_once = true;
        }
        if self.was_disconnected
            && matches!(link.state, LinkState::Connected(_))
            && outcome.is_none()
            && !link.restarting
            && !summarized
        {
            self.toasts
                .push(ToastLevel::Success, "Reconnected", None, now);
        }
        self.was_disconnected = disconnected;
        match outcome {
            // The summary banner says it, with what was interrupted.
            Some(Ok(())) if summarized => {}
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

    /// Requests from the header, the rail and the board. The ticket editor (T-0040) and Settings (T-0045) hook
    /// in here.
    fn on_shell_request(
        &mut self,
        request: ShellRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match request {
            ShellRequest::NewTicket => {
                if self.onboarding.is_none() && self.workspace.read(cx).current().is_some() {
                    self.board
                        .update(cx, |b, cx| b.start_quick_create(window, cx));
                }
            }
            ShellRequest::OpenSettings => self.open_settings(window, cx),
            ShellRequest::AddProject => self.open_add_project(None, window, cx),
            ShellRequest::CheckTools => self.open_tools(cx),
            ShellRequest::ProjectTerminal => self.toggle_project_terminal(window, cx),
            ShellRequest::RunCommand { project, run } => {
                let link = self.link.clone();
                let name = run
                    .as_ref()
                    .map(|r| r.project_name.clone())
                    .or_else(|| {
                        self.workspace
                            .read(cx)
                            .switcher
                            .projects()
                            .iter()
                            .find(|p| p.key == project)
                            .map(|p| p.name.clone())
                    })
                    .unwrap_or_else(|| project.clone());
                let dialog = cx.new(|cx| {
                    crate::run_command::RunCommandDialog::new(
                        link,
                        project,
                        name,
                        run.map(|r| *r),
                        window,
                        cx,
                    )
                });
                cx.subscribe_in(
                    &dialog,
                    window,
                    |root, _, _: &crate::run_command::RunCommandClosed, window, cx| {
                        root.run_command = None;
                        window.focus(&root.focus, cx);
                        cx.notify();
                    },
                )
                .detach();
                self.run_command = Some(dialog);
                cx.notify();
            }
            ShellRequest::ProjectInstructions(project) => {
                self.open_settings(window, cx);
                if let Some(settings) = &self.settings {
                    settings.update(cx, |s, cx| s.show_project_instructions(&project, cx));
                }
            }
            ShellRequest::OpenTicket { project, key, tab } => {
                self.open_ticket(project, key, tab, window, cx)
            }
            ShellRequest::ResumeAll => {
                self.resume_all = self.workspace.read(cx).inbox.resume_all_confirm();
                if self.resume_all.is_some() {
                    window.focus(&self.dialog_focus, cx);
                }
            }
            ShellRequest::ResumeAfterLimit => self.resume_after_limit(cx),
            ShellRequest::InboxAction(key, action) => {
                use crate::view_models::inbox::RowAction;
                use crewboard_core::protocol::{DepParams, KeyParams};
                let request = match action {
                    RowAction::RetryStart => Request::TicketRetryStart(KeyParams {
                        key: key.to_string(),
                    }),
                    RowAction::ReopenParent(parent) => Request::TicketReopen(KeyParams {
                        key: parent.to_string(),
                    }),
                    RowAction::RemoveDependency(parent) => Request::DepRemove(DepParams {
                        child: key.to_string(),
                        parent: parent.to_string(),
                    }),
                    RowAction::UsageLimit => Request::UsageMarkLimit,
                    RowAction::ResumeTarget { message, .. } => {
                        Request::MessageResume(crewboard_core::protocol::IdParams { id: message })
                    }
                };
                self.workspace.update(cx, |ws, cx| ws.act(request, cx));
            }
            ShellRequest::NextInboxItem => self.next_inbox_item(window, cx),
            ShellRequest::OpenCleanup => self.open_cleanup(window, cx),
            ShellRequest::Unsent(prompt) => {
                self.unsent = Some(prompt);
                window.focus(&self.dialog_focus, cx);
            }
        }
        cx.notify();
    }

    /// The Cleanup window (REC-04): from the rail's broom, Settings → Data or ⌘K "cleanup".
    fn open_cleanup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.onboarding.is_some() || self.cleanup.is_some() {
            return;
        }
        self.settings = None;
        let view =
            cx.new(|cx| crate::cleanup::CleanupView::new(self.workspace.clone(), window, cx));
        cx.subscribe_in(
            &view,
            window,
            |root, _, event: &crate::cleanup::CleanupEvent, window, cx| match event {
                crate::cleanup::CleanupEvent::Closed => {
                    root.cleanup = None;
                    window.focus(&root.focus, cx);
                    cx.notify();
                }
                crate::cleanup::CleanupEvent::Done(title, text) => {
                    root.toasts.push(
                        ToastLevel::Success,
                        title.clone(),
                        text.clone(),
                        Instant::now(),
                    );
                    cx.notify();
                }
            },
        )
        .detach();
        self.cleanup = Some(view);
        cx.notify();
    }

    /// ⌘J (ATT-07): the next inbox item with its terminal focused; "All clear" when nothing is left. Never
    /// called by an answer: the ticket stays open until the user presses it again.
    fn next_inbox_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.onboarding.is_some() {
            return;
        }
        match self.workspace.update(cx, |ws, cx| ws.next_inbox_item(cx)) {
            Some(item) => self.open_ticket(item.project, item.key, Tab::Terminal, window, cx),
            None => {
                self.header.update(cx, |h, cx| h.close_inbox(cx));
                self.area.update(cx, |a, cx| {
                    a.close_panel(cx);
                });
                window.focus(&self.focus, cx);
            }
        }
    }

    /// "All clear" over the board after the last item (*10 → 4 · All clear*).
    fn all_clear(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (handled, minutes) = self.workspace.update(cx, |ws, _| ws.all_clear())?;
        let running =
            crate::view_models::limits::running_total(self.workspace.read(cx).switcher.projects());
        let c = ActiveTheme::get(cx).colors;
        let segments = (0..handled.clamp(1, 24))
            .map(|_| div().w(px(16.)).h(px(3.)).rounded(px(2.)).bg(c.st_approved));
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .justify_center()
                .items_center()
                .child(
                    div()
                        .w(px(320.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(12.))
                        .p(px(28.))
                        .bg(c.bg_raised)
                        .border_1()
                        .border_color(c.border)
                        .rounded(px(12.))
                        .shadow(floating_shadow())
                        .child(
                            div()
                                .size(px(44.))
                                .rounded_full()
                                .bg(c.st_approved_bg)
                                .flex()
                                .justify_center()
                                .items_center()
                                .child(icon(IconName::Check, px(20.)).text_color(c.st_approved)),
                        )
                        .child(
                            div()
                                .text_size(px(16.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(c.text_primary)
                                .child("All clear"),
                        )
                        .child(
                            div()
                                .text_center()
                                .text_size(px(12.))
                                .line_height(gpui::relative(1.45))
                                .text_color(c.text_secondary)
                                .child(SharedString::from(
                                    crate::view_models::inbox_run::all_clear_text(
                                        handled, minutes, running,
                                    ),
                                )),
                        )
                        .child(div().flex().gap(px(4.)).children(segments)),
                )
                .into_any_element(),
        )
    }

    /// ⇧⌘T and the header button: open the current project's terminal, or put the keyboard in it when it is open.
    fn toggle_project_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.workspace.read(cx).current().cloned() else {
            return;
        };
        if let Some(drawer) = &self.project_terminal
            && drawer.read(cx).project == project.key
        {
            drawer.update(cx, |d, cx| d.focus(window, cx));
            return;
        }
        let font_size = self
            .workspace
            .read(cx)
            .global_settings
            .as_ref()
            .map_or(13., |s| s.terminal_font_size as f32);
        let link = self.link.clone();
        let drawer = cx.new(|cx| {
            crate::project_terminal::ProjectTerminal::new(
                link,
                project.key.clone(),
                project.name.clone(),
                crate::shell::tilde(&project.repo_path),
                font_size,
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &drawer,
            window,
            |root, _, event: &crate::project_terminal::ProjectTerminalEvent, window, cx| {
                match event {
                    crate::project_terminal::ProjectTerminalEvent::Hide => {
                        root.project_terminal = None;
                    }
                    crate::project_terminal::ProjectTerminalEvent::LeftFocus => {}
                    crate::project_terminal::ProjectTerminalEvent::OpenTicket(key) => {
                        let project = key.project.to_string();
                        root.open_ticket(
                            project,
                            key.clone(),
                            crate::view_models::panel::Tab::Ticket,
                            window,
                            cx,
                        );
                        return;
                    }
                    crate::project_terminal::ProjectTerminalEvent::Delete(key) => {
                        let key = key.to_string();
                        root.board.update(cx, |b, cx| b.ask_delete(key, cx));
                        return;
                    }
                }
                window.focus(&root.focus, cx);
                cx.notify();
            },
        )
        .detach();
        drawer.update(cx, |d, cx| d.focus(window, cx));
        self.project_terminal = Some(drawer);
        cx.notify();
    }

    /// Opens a ticket's panel on a tab, switching to its project first if needed (inbox rows, ⌘J). The Terminal
    /// tab gets the keyboard.
    fn open_ticket(
        &mut self,
        project: String,
        key: TicketKey,
        tab: Tab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.header.update(cx, |h, cx| h.close_inbox(cx));
        let switching =
            self.workspace.read(cx).current().map(|p| p.key.as_str()) != Some(project.as_str());
        if switching {
            self.workspace.update(cx, |ws, cx| ws.select(&project, cx));
        }
        // After the switch's own events (which close the old project's panel).
        cx.defer_in(window, move |root, window, cx| {
            root.area.update(cx, |area, cx| {
                area.open_panel(key, window, cx);
                if let Some(panel) = &area.panel {
                    panel.update(cx, |p, cx| match tab {
                        Tab::Terminal => p.focus_terminal(window, cx),
                        other => p.set_tab(other, cx),
                    });
                }
            });
        });
    }

    /// The unsent-text prompt for an inbox answer: Clear it and send repeats the answer with the line cleared.
    fn unsent_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use crate::view_models::terminal_strip::Click;
        let prompt = self.unsent.clone()?;
        let this = cx.entity().downgrade();
        let keep = this.clone();
        let shown = prompt.clone();
        Some(crate::components::unsent::unsent_dialog(
            &shown,
            &self.dialog_focus,
            move |window, cx| {
                let prompt = prompt.clone();
                let _ = this.update(cx, |root, cx| {
                    root.unsent = None;
                    if let Click::Answer { choice } = &prompt.click {
                        let request =
                            crate::view_models::answer::request(&prompt.ticket, choice, true);
                        root.workspace.update(cx, |ws, cx| ws.act(request, cx));
                    }
                    window.focus(&root.focus, cx);
                    cx.notify();
                });
            },
            move |window, cx| {
                let _ = keep.update(cx, |root, cx| {
                    root.unsent = None;
                    window.focus(&root.focus, cx);
                    cx.notify();
                });
            },
            cx,
        ))
    }

    /// The Resume all confirmation (*10 → Resume all · confirm*).
    fn resume_all_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let confirm = self.resume_all.clone()?;
        let c = ActiveTheme::get(cx).colors;
        let this = cx.entity().downgrade();
        let cancel_this = this.clone();
        let rows = confirm.included.iter().map(|(key, title)| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(10.))
                .py(px(7.))
                .rounded(px(radius::MD))
                .bg(c.bg_panel)
                .text_size(px(12.))
                .child(icon(IconName::ZapOff, px(13.)).text_color(c.st_error))
                .child(
                    div()
                        .font_family(theme::FONT_MONO)
                        .text_color(c.accent)
                        .child(SharedString::from(key.to_string())),
                )
                .child(
                    div()
                        .text_color(c.text_primary)
                        .child(SharedString::from(title.clone())),
                )
        });
        let left_out = confirm.not_included.clone().map(|text| {
            div()
                .flex()
                .gap(px(8.))
                .px(px(10.))
                .py(px(7.))
                .rounded(px(radius::MD))
                .bg(c.st_error_bg)
                .text_size(px(12.))
                .line_height(gpui::relative(1.45))
                .child(icon(IconName::CircleAlert, px(13.)).text_color(c.st_error))
                .child(
                    div()
                        .flex_1()
                        .text_color(c.st_error)
                        .child(SharedString::from(text)),
                )
        });
        Some(
            crate::components::dialog::ConfirmDialog::new(
                "resume-all",
                &self.dialog_focus,
                crate::view_models::dialog::DialogKind::Primary,
                confirm.title.clone(),
                confirm.confirm.clone(),
                move |window, cx| {
                    let _ = this.update(cx, |root, cx| {
                        root.resume_all = None;
                        root.banners.clear(SituationKind::SessionsInterrupted);
                        root.workspace.update(cx, |ws, cx| {
                            ws.act(
                                Request::TicketsResumeAll(
                                    crewboard_core::protocol::ResumeAllParams::default(),
                                ),
                                cx,
                            )
                        });
                        window.focus(&root.focus, cx);
                        cx.notify();
                    });
                },
                move |window, cx| {
                    let _ = cancel_this.update(cx, |root, cx| {
                        root.resume_all = None;
                        window.focus(&root.focus, cx);
                        cx.notify();
                    });
                },
            )
            .confirm_icon(IconName::Play)
            .text(confirm.text)
            .child(div().flex().flex_col().gap(px(6.)).children(rows))
            .children_boxes(left_out.into_iter().map(|d| d.into_any_element()).collect())
            .into_any_element(),
        )
    }

    /// The ticket editor: a new ticket, or an existing one (read mode, or at "Done when" for "Add criteria").
    fn open_editor(
        &mut self,
        open: OpenEditor,
        at_criteria: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.onboarding.is_some() {
            return;
        }
        let workspace = self.workspace.clone();
        let editor = match open {
            OpenEditor::New { title, epic } => {
                cx.new(|cx| TicketEditor::create(workspace, &title, epic, window, cx))
            }
            // Existing tickets are read and edited in the side panel's Ticket tab (T-0062).
            OpenEditor::Ticket(key) => {
                self.area
                    .update(cx, |a, cx| a.open_ticket_tab(key, at_criteria, window, cx));
                return;
            }
        };
        cx.subscribe_in(&editor, window, |root, _, _: &EditorClosed, window, cx| {
            root.editor = None;
            window.focus(&root.focus, cx);
            cx.notify();
        })
        .detach();
        self.editor = Some(editor);
        cx.notify();
    }

    /// Settings (⌘,).
    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.onboarding.is_some() || self.settings.is_some() {
            return;
        }
        let workspace = self.workspace.clone();
        let view = cx.new(|cx| SettingsView::new(workspace, window, cx));
        cx.subscribe_in(&view, window, |root, _, _: &SettingsClosed, window, cx| {
            root.settings = None;
            window.focus(&root.focus, cx);
            cx.notify();
        })
        .detach();
        cx.subscribe_in(
            &view,
            window,
            |root, _, _: &crate::settings::OpenCleanup, window, cx| root.open_cleanup(window, cx),
        )
        .detach();
        self.settings = Some(view);
        cx.notify();
    }

    /// The add-project dialog, optionally with a folder already picked (dropped on the window).
    fn open_add_project(
        &mut self,
        folder: Option<std::path::PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.onboarding.is_some() {
            return;
        }
        if let (Some(form), Some(folder)) = (&self.add_project, folder.clone()) {
            form.update(cx, |f, cx| f.pick(folder, cx));
            return;
        }
        if self.add_project.is_some() {
            return;
        }
        self.switcher = None;
        let workspace = self.workspace.clone();
        let form =
            cx.new(|cx| AddProjectForm::new(workspace, FormMode::Dialog, folder, window, cx));
        cx.subscribe_in(
            &form,
            window,
            |root, _, event: &AddProjectEvent, window, cx| {
                root.add_project = None;
                window.focus(&root.focus, cx);
                if let AddProjectEvent::Added(key) = event {
                    root.toasts.push(
                        ToastLevel::Success,
                        format!("Project added · {key}"),
                        None,
                        Instant::now(),
                    );
                }
                cx.notify();
            },
        )
        .detach();
        self.add_project = Some(form);
        cx.notify();
    }

    /// "Check tools" from the empty board: the onboarding's tools step, run again.
    fn open_tools(&mut self, cx: &mut Context<Self>) {
        if self.onboarding.is_some() {
            return;
        }
        let dismissed = !self.banners.is_active(SituationKind::PermissionWarning);
        self.start_onboarding(
            OnboardingState {
                has_projects: false,
                permission_warning_dismissed: dismissed,
            },
            cx,
        );
        if let Some(onboarding) = &self.onboarding {
            onboarding.update(cx, |o, cx| o.at_tools(cx));
        }
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
                root.pending_add = true;
                cx.notify();
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
        let workspace = self.workspace.clone();
        let onboarding =
            cx.new(|_| Onboarding::new(link, workspace, state.permission_warning_dismissed));
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

    /// Raises, updates or clears the one usage-limit banner from the plan state (USG-06, P6).
    fn sync_usage_banner(&mut self, cx: &mut Context<Self>) {
        let banner = {
            let ws = self.workspace.read(cx);
            ws.usage.as_ref().and_then(|u| {
                crate::view_models::limit_banner::banner(
                    u.plan.as_ref(),
                    u.limit_waiting,
                    crewboard_core::store::now_ms(),
                )
            })
        };
        if banner == self.limit_banner {
            return;
        }
        match &banner {
            Some(b) => self.banners.raise(Situation::UsageLimit {
                title: b.title.clone(),
                text: b.text.clone(),
                resume: Some(b.resume.clone()),
            }),
            None => self.banners.clear(SituationKind::UsageLimit),
        }
        self.limit_banner = banner;
        cx.notify();
    }

    /// Resume all after the usage limit (USG-06): the user's click tells every waiting agent to continue.
    fn resume_after_limit(&mut self, cx: &mut Context<Self>) {
        let Some(task) = self.link.read(cx).spawn_request(Request::UsageResumeAll) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |root, cx| {
                let (level, title, text) = match result {
                    Ok(Ok(Reply::Resumed(keys))) => {
                        let n = keys.len();
                        let agents = if n == 1 { "agent" } else { "agents" };
                        (
                            ToastLevel::Success,
                            format!("{n} {agents} told to continue"),
                            Some(
                                "Each continues once it is idle and its input line is empty."
                                    .to_owned(),
                            ),
                        )
                    }
                    Ok(Ok(_)) => return,
                    Ok(Err(e)) => (
                        ToastLevel::Error,
                        "Resume all failed".to_owned(),
                        Some(e.to_string()),
                    ),
                    Err(_) => (ToastLevel::Error, "Resume all failed".to_owned(), None),
                };
                root.toasts.push(level, title, text, Instant::now());
                root.workspace.update(cx, |ws, cx| ws.refresh_usage(cx));
                cx.notify();
            });
        })
        .detach();
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
            SituationKind::SessionsInterrupted => {
                self.on_shell_request(ShellRequest::ResumeAll, window, cx)
            }
            SituationKind::UsageLimit => self.resume_after_limit(cx),
            SituationKind::Reconnecting => {}
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
        let second = cx.entity().downgrade();
        let dismiss = cx.entity().downgrade();
        banner_element(
            row,
            restarting,
            move |window, cx| {
                let _ = act.update(cx, |root, cx| root.on_banner_action(kind, window, cx));
            },
            move |window, cx| {
                // "Review & resume": the inbox, where each item resumes on its own.
                let _ = second.update(cx, |root, cx| {
                    root.header.update(cx, |h, cx| h.toggle_inbox(window, cx));
                });
            },
            move |_, cx| {
                let _ = dismiss.update(cx, |root, cx| root.dismiss_banner(kind, cx));
            },
        )
    }

    fn toast_host(&self, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let colors = ActiveTheme::get(cx).colors;
        toast_stack(&self.toasts, &colors, move |id, cx| {
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
    on_secondary: impl Fn(&mut Window, &mut App) + 'static,
    on_dismiss: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let (tone, icon_name) = match row.kind {
        SituationKind::ServiceStopped => (BannerTone::Error, IconName::ServerOff),
        SituationKind::SessionsInterrupted => (BannerTone::Error, IconName::ZapOff),
        SituationKind::RestartRequired => (BannerTone::Warning, IconName::ServerCog),
        SituationKind::UsageLimit => (BannerTone::Attention, IconName::Gauge),
        SituationKind::PermissionWarning => (BannerTone::Warning, IconName::ShieldAlert),
        SituationKind::ServiceUpdateReady => (BannerTone::Info, IconName::RefreshCw),
        SituationKind::Reconnecting => (BannerTone::Neutral, IconName::RefreshCw),
    };
    let kind = row.kind;
    let on_action = std::rc::Rc::new(on_action);
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
            (SituationKind::ServiceStopped | SituationKind::SessionsInterrupted, false) => {
                (label, IconName::Play)
            }
            (SituationKind::PermissionWarning, false) => (label, IconName::ExternalLink),
            (_, false) => (label, IconName::RotateCw),
        };
        let on_action = on_action.clone();
        banner = banner.action(
            BannerAction::new(label, move |_, window, cx| on_action(window, cx))
                .icon(action_icon)
                .disabled(busy),
        );
    }
    if let Some(label) = row.secondary {
        banner = banner.action(
            BannerAction::new(label, move |_, window, cx| on_secondary(window, cx))
                .icon(IconName::ArrowRight),
        );
    }
    // The usage limit's Resume all (USG-06): disabled until a known reset, highlighted after it.
    if let Some(resume) = row.resume {
        banner = banner.action(
            BannerAction::new(resume.label, move |_, window, cx| on_action(window, cx))
                .icon(IconName::Play)
                .disabled(!resume.enabled || busy)
                .primary(resume.primary),
        );
    }
    if row.dismissable {
        banner = banner.on_dismiss(move |_, window, cx| on_dismiss(window, cx));
    }
    banner.into_any_element()
}

/// The toast stack, bottom-right; the newest toast sits at the bottom.
/// "Created by agent" toasts get the sparkles in the agent colour, "started · all dependencies merged" the play
/// icon in the working colour.
pub fn toast_stack(
    model: &ToastModel,
    colors: &crate::theme::Colors,
    on_close: impl Fn(u64, &mut App) + 'static,
) -> AnyElement {
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
            if item
                .title
                .ends_with(crate::view_models::agent_activity::CREATED_BY_AGENT)
            {
                toast = toast.icon(IconName::Sparkles, Some(colors.st_investigate));
            }
            if item
                .title
                .ends_with(crate::view_models::queue::DEPENDENCIES_MERGED)
            {
                toast = toast.icon(IconName::Play, Some(colors.st_working));
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let resume_all = self.resume_all_dialog(cx);
        let run_command = self.run_command.clone();
        // The Project terminal drawer, under the board (*17 · Project terminal*: 380 px; maximized it takes most
        // of the window).
        let drawer = self
            .project_terminal
            .clone()
            .filter(|_| self.onboarding.is_none())
            .map(|d| {
                let height = if d.read(cx).maximized {
                    (f32::from(window.viewport_size().height) - 160.)
                        .max(crate::view_models::project_terminal::DRAWER_HEIGHT)
                } else {
                    crate::view_models::project_terminal::DRAWER_HEIGHT
                };
                div().flex_none().h(px(height)).child(d)
            });
        let all_clear = self.all_clear(cx);
        let unsent = self.unsent_dialog(cx);
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
        if std::mem::take(&mut self.pending_add) {
            self.open_add_project(None, window, cx);
        }
        let add_project = self.add_project.clone().map(|form| {
            div()
                .id("add-project-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .justify_center()
                .items_center()
                .bg(gpui::rgba(0x00000033))
                .child(form)
        });
        // The stored appearance (Settings → Theme) wins over the default "System".
        if let Some(settings) = &self.workspace.read(cx).global_settings {
            let choice = crate::view_models::settings::theme_choice(settings.appearance);
            if cx.global::<ActiveTheme>().choice != choice {
                ActiveTheme::update(choice, window.appearance(), cx);
            }
        }
        let cleanup_view = self.cleanup.clone().map(|view| {
            div()
                .id("cleanup-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .justify_center()
                .items_center()
                .bg(gpui::rgba(0x00000033))
                .child(view)
        });
        let settings_view = self.settings.clone().map(|view| {
            div()
                .id("settings-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .justify_center()
                .items_center()
                .bg(gpui::rgba(0x00000033))
                .child(view)
        });
        let editor = self.editor.clone().map(|editor| {
            div()
                .id("editor-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .justify_center()
                .items_start()
                .pt(px(64.))
                .bg(gpui::rgba(0x00000033))
                .child(editor)
        });
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
            .on_action(cx.listener(|root, _: &NewTicket, window, cx| {
                root.on_shell_request(ShellRequest::NewTicket, window, cx)
            }))
            .on_action(cx.listener(|root, _: &Search, window, cx| {
                if root.onboarding.is_none() {
                    let back = root.focus.clone();
                    let search = root.header.read(cx).search.clone();
                    search.update(cx, |s, cx| s.focus(back, window, cx));
                }
            }))
            .on_action(cx.listener(|root, _: &StartDev, window, cx| {
                if root.onboarding.is_none() {
                    root.board.update(cx, |b, cx| b.start_selected(window, cx));
                }
            }))
            .on_action(cx.listener(|_, _: &CancelDrag, window, cx| {
                // Esc only means "cancel" while a card is being dragged.
                if !cx.stop_active_drag(window) {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|root, _: &OpenSelected, window, cx| {
                let selected = root.board.read(cx).selected.clone();
                match selected {
                    Some(key) if root.onboarding.is_none() => {
                        root.area.update(cx, |a, cx| a.open_panel(key, window, cx))
                    }
                    _ => cx.propagate(),
                }
            }))
            .on_action(cx.listener(|root, _: &ClosePanel, _, cx| {
                if !root.area.update(cx, |a, cx| a.close_panel(cx)) {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|root, _: &ExpandPanel, _, cx| {
                root.area.update(cx, |a, cx| a.toggle_expanded(cx))
            }))
            .on_action(cx.listener(|root, _: &PreviousTicket, window, cx| {
                root.area.update(cx, |a, cx| a.step(-1, window, cx))
            }))
            .on_action(cx.listener(|root, _: &NextTicket, window, cx| {
                root.area.update(cx, |a, cx| a.step(1, window, cx))
            }))
            .on_action(cx.listener(|root, _: &ApproveReview, window, cx| {
                root.area.update(cx, |a, cx| {
                    if let Some(panel) = &a.panel {
                        panel.update(cx, |p, cx| p.open_approve(window, cx));
                    }
                })
            }))
            .on_action(cx.listener(|root, _: &SendBackReview, window, cx| {
                root.area.update(cx, |a, cx| {
                    if let Some(panel) = &a.panel {
                        panel.update(cx, |p, cx| p.open_send_back(window, cx));
                    }
                })
            }))
            .on_action(cx.listener(|root, _: &PanelTab1, _, cx| {
                root.area.update(cx, |a, cx| a.set_tab(1, cx))
            }))
            .on_action(cx.listener(|root, _: &PanelTab2, _, cx| {
                root.area.update(cx, |a, cx| a.set_tab(2, cx))
            }))
            .on_action(cx.listener(|root, _: &PanelTab3, _, cx| {
                root.area.update(cx, |a, cx| a.set_tab(3, cx))
            }))
            .on_action(cx.listener(|root, _: &PanelTab4, _, cx| {
                root.area.update(cx, |a, cx| a.set_tab(4, cx))
            }))
            .on_action(cx.listener(|root, _: &PanelTab5, _, cx| {
                root.area.update(cx, |a, cx| a.set_tab(5, cx))
            }))
            .on_action(cx.listener(|root, _: &PanelTab6, _, cx| {
                root.area.update(cx, |a, cx| a.set_tab(6, cx))
            }))
            .on_action(cx.listener(|root, _: &OpenSettings, window, cx| {
                root.on_shell_request(ShellRequest::OpenSettings, window, cx)
            }))
            .on_drop(cx.listener(|root, paths: &ExternalPaths, window, cx| {
                if let Some(path) = paths.paths().first() {
                    root.open_add_project(Some(path.clone()), window, cx);
                }
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
            .on_action(
                cx.listener(|root, _: &NextInboxItem, window, cx| root.next_inbox_item(window, cx)),
            )
            .on_action(cx.listener(|root, _: &OpenProjectTerminal, window, cx| {
                if root.onboarding.is_none() {
                    root.toggle_project_terminal(window, cx);
                }
            }))
            .on_action(cx.listener(|root, _: &OpenInbox, window, cx| {
                if root.onboarding.is_none() {
                    root.header.update(cx, |h, cx| h.toggle_inbox(window, cx));
                }
            }))
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(colors.bg_app)
            .font_family(theme::FONT_UI)
            .child(top)
            .children(rows)
            .child(body)
            .children(drawer)
            .children(switcher)
            .children(add_project)
            .children(editor)
            .children(settings_view)
            .children(cleanup_view)
            .child(toasts)
            .children(dialog)
            .children(resume_all)
            .children(run_command)
            .children(all_clear)
            .children(unsent)
    }
}
