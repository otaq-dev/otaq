//! The window around the board (UI-01; *02 · App shell & project switcher*, *03 · Board → Header, Project
//! rail*): the 44 px header that is also the title bar, the project rail (tiles below 1600 px, a 232 px
//! sidebar above), the board area and the quick switcher (⌘P). Global things (inbox, usage, limits) belong to
//! the header and are shared by every project (d0024); their slots stay hidden until their tickets wire them
//! (inbox T-0118, tokens T-0126, plan meter T-0130 (in), processes T-0136, Project terminal T-0138). The agents chip
//! and its limits popover are in (T-0082).

use std::time::{Duration, Instant};

use crewboard_core::board::settings::AgentLimits;
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, IntoElement, SharedString, Window, deferred, div, prelude::*, px,
};

use crate::components::button::{Button, IconButton};
use crate::components::floating_shadow;
use crate::components::kbd::Kbd;
use crate::components::menu::Tooltip;
use crate::components::pill::MetaChip;
use crate::components::stepper::Stepper;
use crate::components::text_input::{InputEvent, TextInput};
use crate::icons::{IconName, icon};
use crate::inbox::{InboxEvent, InboxView};
use crate::keymap::{KeyAction, Platform};
use crate::search::SearchField;
use crate::theme::{ActiveTheme, Colors, FONT_MONO, radius};
use crate::view_models::limits::{
    BACK_PRESSURE_ITEMS, MAX_AGENTS, agents_chip, running_by_project, running_total,
};
use crate::view_models::switcher::{QuickCursor, RailItem, SIDEBAR_FROM_WIDTH};
use crate::workspace::Workspace;

/// What the shell asks its owner to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellRequest {
    NewTicket,
    OpenSettings,
    AddProject,
    /// Run the tools check again (the onboarding's tools step).
    CheckTools,
    /// A project's settings, at its Agent instructions (d0040).
    ProjectInstructions(String),
    /// The current project's Project terminal (⇧⌘T, RUN-06): open it, or focus it when it is open.
    ProjectTerminal,
    /// "What command runs shop-web?" (RUN-02), for a ticket's Run.
    RunCommand {
        project: String,
        run: Option<Box<crewboard_core::terminals::run::RunInfo>>,
    },
    /// An inbox row: switch to its project if needed and open its panel on this tab (UI-06).
    OpenTicket {
        project: String,
        key: crewboard_core::domain::TicketKey,
        tab: crate::view_models::panel::Tab,
    },
    /// "Resume all (N)" in the inbox: confirm, then resume the interrupted sessions.
    ResumeAll,
    /// Resume all after the usage limit reset (the inbox's limit row, USG-06): "continue" to the waiting agents.
    ResumeAfterLimit,
    /// A button on an inbox row (Retry start, Reopen parent, Remove dependency).
    InboxAction(
        crewboard_core::domain::TicketKey,
        crate::view_models::inbox::RowAction,
    ),
    /// The next inbox item (⌘J, the inbox's Next).
    NextInboxItem,
    /// An answer stopped at unsent text in the terminal (SES-07): ask first.
    Unsent(crate::view_models::terminal_strip::UnsentPrompt),
    /// The Cleanup window (the rail's broom, Settings → Data, ⌘K "cleanup"; REC-04).
    OpenCleanup,
}

/// The broom at the bottom of the rail (*16 → Where Cleanup lives*): a neutral dot only while leftovers wait.
fn broom(id: &'static str, count: usize, c: &Colors) -> gpui::Stateful<gpui::Div> {
    let tip = if count == 0 {
        "Cleanup".to_owned()
    } else {
        format!(
            "Cleanup · {count} {}",
            if count == 1 { "leftover" } else { "leftovers" }
        )
    };
    div()
        .id(id)
        .relative()
        .flex_none()
        .size(px(36.))
        .flex()
        .justify_center()
        .items_center()
        .rounded(px(8.))
        .cursor_pointer()
        .hover(|s| s.bg(c.bg_hover))
        .tooltip(Tooltip::with_detail(
            tip,
            "Nothing is deleted without your click".to_owned(),
        ))
        .child(icon(IconName::BrushCleaning, px(16.)).text_color(c.text_muted))
        .children((count > 0).then(|| {
            div()
                .absolute()
                .top(px(6.))
                .right(px(6.))
                .size(px(7.))
                .rounded_full()
                .bg(c.text_muted)
        }))
}

/// A path with the home folder shown as `~` (the user profile folder on Windows).
pub fn tilde(path: &str) -> String {
    match std::env::home_dir().map(|h| h.display().to_string()) {
        Some(home) if !home.is_empty() && path.starts_with(&home) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_owned(),
    }
}

pub struct Header {
    workspace: Entity<Workspace>,
    pub search: Entity<SearchField>,
    /// The inbox popover, while open (UI-06).
    inbox: Option<Entity<InboxView>>,
    /// When a click outside last closed the inbox, so the same click on the button doesn't reopen it.
    inbox_closed_at: Option<Instant>,
    /// The agent limits popover is open.
    limits_open: bool,
    /// When a click outside last closed the popover, so the same click on the chip doesn't reopen it.
    limits_closed_at: Option<Instant>,
    /// The running processes menu is open (RUN-04).
    processes_open: bool,
    processes_closed_at: Option<Instant>,
}

impl EventEmitter<ShellRequest> for Header {}

impl Header {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        let search = cx.new(|cx| SearchField::new(workspace.clone(), window, cx));
        // ⌘K "cleanup" opens the Cleanup window.
        cx.subscribe(&search, |_, _, request: &ShellRequest, cx| {
            cx.emit(request.clone())
        })
        .detach();
        Self {
            workspace,
            search,
            inbox: None,
            inbox_closed_at: None,
            limits_open: false,
            limits_closed_at: None,
            processes_open: false,
            processes_closed_at: None,
        }
    }

    /// Opens or closes the inbox (⇧⌘I and the header button).
    pub fn toggle_inbox(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.inbox.is_some() {
            self.close_inbox(cx);
            return;
        }
        if self
            .inbox_closed_at
            .is_some_and(|t| t.elapsed() < Duration::from_millis(300))
        {
            return;
        }
        let view = cx.new(|cx| InboxView::new(self.workspace.clone(), cx));
        cx.subscribe(&view, |header, _, event: &InboxEvent, cx| {
            let request = match event {
                InboxEvent::Close => None,
                InboxEvent::Open { project, key, tab } => Some(ShellRequest::OpenTicket {
                    project: project.clone(),
                    key: key.clone(),
                    tab: *tab,
                }),
                InboxEvent::ResumeAll => Some(ShellRequest::ResumeAll),
                InboxEvent::ResumeAfterLimit => Some(ShellRequest::ResumeAfterLimit),
                InboxEvent::Action(key, action) => {
                    Some(ShellRequest::InboxAction(key.clone(), action.clone()))
                }
                InboxEvent::Next => Some(ShellRequest::NextInboxItem),
                InboxEvent::Unsent(prompt) => Some(ShellRequest::Unsent(prompt.clone())),
            };
            // Row buttons act in place; everything else leaves the inbox.
            if !matches!(request, Some(ShellRequest::InboxAction(..))) {
                header.close_inbox(cx);
            }
            if let Some(request) = request {
                cx.emit(request);
            }
        })
        .detach();
        window.focus(&view.focus_handle(cx), cx);
        self.inbox = Some(view);
        cx.notify();
    }

    /// Opens the inbox if it isn't open (a click on a grouped notification).
    pub fn open_inbox(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.inbox.is_none() {
            self.toggle_inbox(window, cx);
        }
    }

    pub fn close_inbox(&mut self, cx: &mut Context<Self>) {
        if self.inbox.take().is_some() {
            self.inbox_closed_at = Some(Instant::now());
            cx.notify();
        }
    }

    /// The Needs-you button with the inbox count (ATT-10: the sum of the rail badges), and the popover under it.
    fn inbox_button(&self, c: &Colors, cx: &mut Context<Self>) -> AnyElement {
        let total = self.workspace.read(cx).inbox.total();
        let open = self.inbox.is_some();
        let active = total > 0;
        let button = div()
            .id("header-inbox")
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .h(px(28.))
            .pl(px(9.))
            .pr(px(6.))
            .rounded(px(6.))
            .border_1()
            .cursor_pointer()
            .when(active, |d| d.bg(c.st_attn_bg).border_color(c.st_attn))
            .when(!active, |d| {
                d.border_color(if open { c.accent } else { c.border_strong })
                    .hover(|s| s.bg(c.bg_hover))
            })
            .tooltip(Tooltip::text(
                "Needs you",
                KeyAction::OpenInbox.keys(Platform::current()),
            ))
            .child(icon(IconName::Inbox, px(14.)).text_color(if active {
                c.st_attn
            } else {
                c.text_secondary
            }))
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if active { c.st_attn } else { c.text_secondary })
                    .child("Needs you"),
            )
            .child(
                div()
                    .px(px(5.))
                    .rounded(px(8.))
                    .bg(if active { c.st_attn } else { c.bg_column })
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if active { gpui::white() } else { c.text_muted })
                    .child(SharedString::from(total.to_string())),
            )
            .on_click(cx.listener(|h, _, window, cx| h.toggle_inbox(window, cx)));
        let popover = self.inbox.clone().map(|view| {
            deferred(div().absolute().top(px(34.)).right_0().child(view)).with_priority(1)
        });
        div()
            .relative()
            .flex_none()
            .child(button)
            .children(popover)
            .into_any_element()
    }

    /// Today's tokens across all projects (USG-01), with the dollar estimate when it is on (USG-02): *02 → Header
    /// states → Header · dollar estimate enabled*. Hidden until the first summary arrives.
    fn usage(&self, c: &Colors, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ws = self.workspace.read(cx);
        let estimate = ws
            .global_settings
            .as_ref()
            .is_some_and(|g| g.dollar_estimate);
        let view = crate::view_models::usage::header(ws.usage.as_ref()?, estimate);
        let (title, detail) = view.tooltip;
        Some(
            div()
                .id("header-usage")
                .flex()
                .flex_none()
                .items_center()
                .gap(px(5.))
                .px(px(6.))
                .text_size(px(12.))
                .tooltip(Tooltip::with_detail(title, detail))
                .child(icon(IconName::Coins, px(13.)).text_color(c.text_muted))
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_color(c.text_primary)
                        .child(SharedString::from(view.number)),
                )
                .child(
                    div()
                        .text_color(c.text_muted)
                        .child(SharedString::from(view.label)),
                )
                .into_any_element(),
        )
    }

    /// The plan meter (USG-05; *02 → Header states → Header · plan meter (Pro / Max)*): "5h · 82% · week · 40%",
    /// amber from 80 %, reset times on hover. Hidden until status-line data exists (API-key users never see it).
    fn plan_meter(&self, c: &Colors, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ws = self.workspace.read(cx);
        let plan = ws.usage.as_ref()?.plan.as_ref();
        let meter = crate::view_models::plan_meter::meter(plan, crewboard_core::store::now_ms())?;
        let cell = |cell: &crate::view_models::plan_meter::Cell| {
            [
                div()
                    .text_color(c.text_muted)
                    .child(cell.label)
                    .into_any_element(),
                div()
                    .font_family(FONT_MONO)
                    .when(cell.amber, |d| {
                        d.text_color(c.st_paused).font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!cell.amber, |d| d.text_color(c.text_primary))
                    .child(SharedString::from(cell.value.clone()))
                    .into_any_element(),
            ]
        };
        Some(
            div()
                .id("header-plan-meter")
                .flex()
                .flex_none()
                .items_center()
                .gap(px(5.))
                .h(px(26.))
                .px(px(8.))
                .rounded(px(5.))
                .border_1()
                .border_color(c.border)
                .text_size(px(12.))
                .hover(|s| s.bg(c.bg_hover))
                .tooltip(Tooltip::with_lines(
                    meter.title.clone(),
                    meter.lines.clone(),
                    Some(meter.note.clone()),
                ))
                .children(cell(&meter.five_hour))
                .child(div().text_color(c.text_muted).child("·"))
                .children(cell(&meter.week))
                .into_any_element(),
        )
    }

    /// "▶ 3 processes" across every project while something runs, and its menu: each Run with Open (when the
    /// project has an Open URL) and Stop; a row opens the ticket's Run tab (RUN-04).
    fn processes(&self, c: &Colors, cx: &mut Context<Self>) -> Option<AnyElement> {
        let runs = self.workspace.read(cx).runs.clone();
        let label = crate::view_models::processes::chip_label(&runs)?;
        let open = self.processes_open;
        let chip = div()
            .id("header-processes")
            .flex()
            .flex_none()
            .items_center()
            .gap(px(5.))
            .h(px(24.))
            .px(px(8.))
            .rounded(px(5.))
            .border_1()
            .border_color(if open { c.accent } else { c.st_approved })
            .bg(c.st_approved_bg)
            .cursor_pointer()
            .child(icon(IconName::Play, px(11.)).text_color(c.st_approved))
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(11.5))
                    .text_color(c.st_approved)
                    .child(SharedString::from(label)),
            )
            .child(icon(IconName::ChevronDown, px(11.)).text_color(c.st_approved))
            .on_click(cx.listener(|h, _, _, cx| {
                let just_closed = h
                    .processes_closed_at
                    .is_some_and(|t| t.elapsed() < Duration::from_millis(300));
                if !just_closed {
                    h.processes_open = !h.processes_open;
                }
                cx.notify();
            }));
        let menu = open.then(|| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as i64);
            let rows = crate::view_models::processes::rows(&runs, now);
            let keys: Vec<String> = rows.iter().map(|r| r.key.to_string()).collect();
            let stop_all = {
                let ws = self.workspace.clone();
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                    ws.update(cx, |ws, cx| {
                        for key in &keys {
                            ws.act(
                                crewboard_core::protocol::Request::RunStop(
                                    crewboard_core::protocol::KeyParams { key: key.clone() },
                                ),
                                cx,
                            );
                        }
                    });
                }
            };
            let row_els = rows.into_iter().enumerate().map(|(i, row)| {
                let stop_ws = self.workspace.clone();
                let stop_key = row.key.to_string();
                let open_url = row.url.clone();
                let (project, key) = (row.project.clone(), row.key.clone());
                div()
                    .id(SharedString::from(format!("process-{i}")))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(c.bg_hover))
                    .on_click(cx.listener(move |h, _, _, cx| {
                        h.processes_open = false;
                        cx.emit(ShellRequest::OpenTicket {
                            project: project.clone(),
                            key: key.clone(),
                            tab: crate::view_models::panel::Tab::Run,
                        });
                        cx.notify();
                    }))
                    .child(div().size(px(6.)).rounded_full().bg(c.st_approved))
                    .child(
                        div()
                            .px(px(5.))
                            .rounded(px(4.))
                            .bg(c.bg_column)
                            .font_family(FONT_MONO)
                            .text_size(px(10.5))
                            .text_color(c.text_secondary)
                            .child(SharedString::from(row.project_name.clone())),
                    )
                    .child(
                        div()
                            .font_family(FONT_MONO)
                            .text_size(px(11.5))
                            .text_color(c.text_secondary)
                            .child(SharedString::from(row.key.to_string())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.5))
                            .text_color(c.text_primary)
                            .child(SharedString::from(row.title.clone())),
                    )
                    .children(row.uptime.clone().map(|u| {
                        div()
                            .text_size(px(11.))
                            .text_color(c.text_muted)
                            .child(SharedString::from(u))
                    }))
                    .children(row.port.clone().map(|p| {
                        div()
                            .font_family(FONT_MONO)
                            .text_size(px(11.5))
                            .text_color(c.st_approved)
                            .child(SharedString::from(p))
                    }))
                    .children(open_url.map(|url| {
                        div()
                            .id(SharedString::from(format!("process-open-{i}")))
                            .tooltip(Tooltip::text("Open in the browser", None))
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                cx.open_url(&url);
                            })
                            .child(icon(IconName::ExternalLink, px(12.)).text_color(c.text_muted))
                    }))
                    .child(
                        div()
                            .id(SharedString::from(format!("process-stop-{i}")))
                            .tooltip(Tooltip::text("Stop", None))
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                stop_ws.update(cx, |ws, cx| {
                                    ws.act(
                                        crewboard_core::protocol::Request::RunStop(
                                            crewboard_core::protocol::KeyParams {
                                                key: stop_key.clone(),
                                            },
                                        ),
                                        cx,
                                    )
                                });
                            })
                            .child(icon(IconName::Square, px(12.)).text_color(c.st_error)),
                    )
            });
            deferred(
                div()
                    .id("processes-menu")
                    .absolute()
                    .top(px(30.))
                    .right_0()
                    .w(px(400.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .p(px(10.))
                    .bg(c.bg_raised)
                    .border_1()
                    .border_color(c.border)
                    .rounded(px(8.))
                    .shadow(floating_shadow())
                    .on_mouse_down_out(cx.listener(|h, _, _, cx| {
                        h.processes_open = false;
                        h.processes_closed_at = Some(Instant::now());
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .px(px(4.))
                            .pb(px(4.))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c.text_primary)
                                    .child("Running processes"),
                            )
                            .child(
                                div()
                                    .id("processes-stop-all")
                                    .cursor_pointer()
                                    .text_size(px(12.))
                                    .text_color(c.st_error)
                                    .on_click(stop_all)
                                    .child("Stop all"),
                            ),
                    )
                    .children(row_els),
            )
            .with_priority(1)
        });
        Some(
            div()
                .relative()
                .flex_none()
                .child(chip)
                .children(menu)
                .into_any_element(),
        )
    }

    /// The agents chip: "● 3 running" with a tooltip (limits off), or the "● 3 / 5 agents ⌄" control with its
    /// popover (limits on; SCH-03, UI-01).
    fn agents(&self, c: &Colors, cx: &mut Context<Self>) -> AnyElement {
        let ws = self.workspace.read(cx);
        let limits = ws
            .global_settings
            .as_ref()
            .map(|g| g.agent_limits)
            .unwrap_or_default();
        let projects = ws.switcher.projects();
        let chip = agents_chip(&limits, ws.scheduler.as_ref(), running_total(projects));
        let per_project = running_by_project(projects);
        let open = self.limits_open && chip.control;
        let mut el = div()
            .id("header-agents")
            .flex()
            .flex_none()
            .items_center()
            .gap(px(5.))
            .h(px(26.))
            .px(px(8.))
            .rounded(px(5.))
            .text_size(px(12.))
            .child(div().size(px(7.)).rounded_full().bg(if chip.active {
                c.st_working
            } else {
                c.text_muted
            }))
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_color(c.text_primary)
                    .child(SharedString::from(chip.count)),
            )
            .child(div().text_color(c.text_muted).child(chip.label));
        if chip.control {
            el = el
                .border_1()
                .border_color(if open { c.accent } else { c.border_strong })
                .cursor_pointer()
                .hover(|s| s.bg(c.bg_hover))
                .child(icon(IconName::ChevronDown, px(12.)).text_color(c.text_muted))
                .on_click(cx.listener(|h, _, _, cx| {
                    let just_closed = h
                        .limits_closed_at
                        .is_some_and(|t| t.elapsed() < Duration::from_millis(300));
                    if !just_closed {
                        h.limits_open = !h.limits_open;
                    }
                    cx.notify();
                }));
        } else {
            el = el.bg(c.bg_hover).tooltip(Tooltip::with_detail(
                "Agents running across all projects",
                per_project,
            ));
        }
        let popover = open.then(|| {
            let ws = self.workspace.clone();
            let save = move |limits: AgentLimits, cx: &mut App| {
                ws.update(cx, |ws, cx| ws.set_limits(limits, cx));
            };
            let row = |label: &'static str, stepper: Stepper| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .line_height(gpui::relative(1.4))
                            .text_color(c.text_secondary)
                            .child(label),
                    )
                    .child(stepper)
            };
            let max_save = save.clone();
            deferred(
                div()
                    .id("limits-popover")
                    .absolute()
                    .top(px(32.))
                    .right_0()
                    .w(px(300.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .p(px(16.))
                    .bg(c.bg_raised)
                    .border_1()
                    .border_color(c.border)
                    .rounded(px(8.))
                    .shadow(floating_shadow())
                    .on_mouse_down_out(cx.listener(|h, _, _, cx| {
                        h.limits_open = false;
                        h.limits_closed_at = Some(Instant::now());
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.text_primary)
                            .child("Agent limits"),
                    )
                    .child(row(
                        "Max agents running at once",
                        Stepper::new("popover-max-agents", limits.max_concurrency, MAX_AGENTS)
                            .on_change(move |n, _, cx| {
                                max_save(
                                    AgentLimits {
                                        max_concurrency: n,
                                        ..limits
                                    },
                                    cx,
                                )
                            }),
                    ))
                    .child(row(
                        "Pause new starts when the inbox has",
                        Stepper::new(
                            "popover-back-pressure",
                            limits.back_pressure,
                            BACK_PRESSURE_ITEMS,
                        )
                        .on_change(move |n, _, cx| {
                            save(
                                AgentLimits {
                                    back_pressure: n,
                                    ..limits
                                },
                                cx,
                            )
                        }),
                    ))
                    .child(
                        div()
                            .text_size(px(11.))
                            .line_height(gpui::relative(1.45))
                            .text_color(c.text_muted)
                            .child(
                                "Queued tickets wait for a free slot. Nothing that is already running is stopped.",
                            ),
                    ),
            )
            .with_priority(1)
        });
        div()
            .relative()
            .flex_none()
            .child(el)
            .children(popover)
            .into_any_element()
    }
}

impl Render for Header {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::window_controls::{caption_buttons, drag_area, shows_caption_buttons};
        let captions = shows_caption_buttons(window);
        let c = ActiveTheme::get(cx).colors;
        let platform = Platform::current();
        let project = self.workspace.read(cx).current().cloned();
        let has_project = project.is_some();
        let project_block = project.map(|p| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.text_primary)
                        .child(SharedString::from(p.name.clone())),
                )
                .child(MetaChip::new(IconName::GitBranch, p.base_branch.clone()))
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(tilde(&p.repo_path))),
                )
        });
        let usage = self.usage(&c, cx);
        let plan_meter = self.plan_meter(&c, cx);
        let processes = self.processes(&c, cx);
        let agents = self.agents(&c, cx);
        let inbox = self.inbox_button(&c, cx);
        let new_ticket = cx.listener(|_, _, _, cx| cx.emit(ShellRequest::NewTicket));
        let settings = cx.listener(|_, _, _, cx| cx.emit(ShellRequest::OpenSettings));
        let project_terminal = cx.listener(|_, _, _, cx| cx.emit(ShellRequest::ProjectTerminal));
        div()
            .id("header")
            .flex_none()
            .h(px(44.))
            .w_full()
            .flex()
            .items_center()
            .gap(px(8.))
            .pl(px(14.))
            .pr(px(10.))
            .bg(c.bg_panel)
            .border_b_1()
            .border_color(c.border)
            // Room for the macOS traffic lights drawn over the transparent title bar.
            .when(cfg!(target_os = "macos"), |el| {
                el.child(
                    div()
                        .w(px(58.))
                        .h_full()
                        .child(drag_area("drag-lights").size_full()),
                )
            })
            .children(project_block)
            .child(drag_area("drag-before-search"))
            .children(has_project.then(|| self.search.clone()))
            .child(drag_area("drag-after-search"))
            .children(usage)
            .children(plan_meter)
            .children(processes)
            .child(agents)
            .child(div().w(px(1.)).h(px(20.)).bg(c.border))
            .child(inbox)
            .child(
                div()
                    .id("new-ticket")
                    .tooltip(Tooltip::text(
                        "New ticket",
                        KeyAction::NewTicket.keys(platform),
                    ))
                    .child(
                        Button::new("header-new-ticket", "New ticket")
                            .icon(IconName::Plus)
                            .on_click(new_ticket),
                    ),
            )
            .children(has_project.then(|| {
                div()
                    .id("project-terminal")
                    .tooltip(Tooltip::text(
                        "Project terminal",
                        KeyAction::ProjectTerminal.keys(platform),
                    ))
                    .child(
                        IconButton::new("header-project-terminal", IconName::SquareTerminal)
                            .on_click(project_terminal),
                    )
            }))
            .child(
                div()
                    .id("settings")
                    .tooltip(Tooltip::text(
                        "Settings",
                        KeyAction::Settings.keys(platform),
                    ))
                    .child(
                        IconButton::new("header-settings", IconName::Settings).on_click(settings),
                    ),
            )
            // Windows and Linux: minimize, maximize and close at the right end (*Title bar · Windows & Linux*).
            .when(captions, |el| el.child(caption_buttons(&c)))
    }
}

/// The project rail and the board area.
pub struct ShellBody {
    workspace: Entity<Workspace>,
    board: AnyElementSlot,
}

/// The board area's content, set by the owner (the board view from T-0035).
pub enum AnyElementSlot {
    Empty,
    View(gpui::AnyView),
}

impl EventEmitter<ShellRequest> for ShellBody {}

impl ShellBody {
    pub fn new(workspace: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                let alive = this.update(cx, |body, cx| {
                    let expired = body
                        .workspace
                        .update(cx, |ws, _| ws.switcher.expire(Instant::now()));
                    if expired {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    return;
                }
            }
        })
        .detach();
        Self {
            workspace,
            board: AnyElementSlot::Empty,
        }
    }

    pub fn set_board(&mut self, view: gpui::AnyView, cx: &mut Context<Self>) {
        self.board = AnyElementSlot::View(view);
        cx.notify();
    }

    fn rail(&self, items: Vec<RailItem>, c: &Colors, cx: &mut Context<Self>) -> AnyElement {
        let tiles = items.into_iter().map(|item| {
            let key = item.key.clone();
            let ws = self.workspace.clone();
            let (fill, stroke, text) = if item.selected {
                (c.accent_soft, c.accent, c.accent)
            } else if item.idle {
                (c.bg_column, c.border, c.text_muted)
            } else {
                (c.bg_column, c.border, c.text_secondary)
            };
            let mut tile = div()
                .relative()
                .size(px(36.))
                .flex()
                .justify_center()
                .items_center()
                .rounded(px(8.))
                .bg(fill)
                .border_1()
                .border_color(stroke)
                .font_family(FONT_MONO)
                .text_size(px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(text)
                .child(SharedString::from(item.key.clone()));
            if item.ringing {
                tile = tile.shadow(crate::components::focus_ring(c.st_attn));
            }
            if item.needs_you > 0 {
                tile = tile.child(
                    div()
                        .absolute()
                        .top(px(-6.))
                        .right(px(-2.))
                        .h(px(16.))
                        .min_w(px(16.))
                        .px(px(4.))
                        .flex()
                        .justify_center()
                        .items_center()
                        .rounded(px(8.))
                        .bg(c.st_attn)
                        .border_2()
                        .border_color(c.bg_panel)
                        .text_size(px(10.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(gpui::white())
                        .child(SharedString::from(item.needs_you.to_string())),
                );
            }
            if item.agents_working > 0 {
                tile = tile.child(
                    div()
                        .absolute()
                        .left(px(-5.))
                        .bottom(px(-3.))
                        .h(px(15.))
                        .px(px(3.))
                        .flex()
                        .items_center()
                        .gap(px(2.))
                        .rounded(px(8.))
                        .bg(c.st_working_bg)
                        .border_2()
                        .border_color(c.bg_panel)
                        .text_size(px(9.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.st_working)
                        .child(icon(IconName::Activity, px(8.)).text_color(c.st_working))
                        .child(SharedString::from(item.agents_working.to_string())),
                );
            }
            let tip = format!(
                "{} · {} · {} needs you · {} running",
                item.name, item.key, item.needs_you, item.agents_working
            );
            div()
                .id(SharedString::from(format!("rail-{}", item.key)))
                .relative()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(5.))
                .cursor_pointer()
                .tooltip(Tooltip::text(tip, item.shortcut.clone()))
                .children(item.selected.then(|| {
                    div()
                        .absolute()
                        .left_0()
                        .top(px(8.))
                        .w(px(3.))
                        .h(px(20.))
                        .rounded_r(px(2.))
                        .bg(c.accent)
                }))
                .child(tile)
                .child(
                    div().w_full().px(px(4.)).flex().justify_center().child(
                        div()
                            .truncate()
                            .text_size(px(10.))
                            .font_weight(if item.selected {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(if item.selected {
                                c.text_primary
                            } else if item.idle {
                                c.text_muted
                            } else {
                                c.text_secondary
                            })
                            .child(SharedString::from(item.name.clone())),
                    ),
                )
                .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.select(&key, cx)))
                .into_any_element()
        });
        let add = cx.listener(|_, _, _, cx| cx.emit(ShellRequest::AddProject));
        div()
            .id("project-rail")
            .flex_none()
            .w(px(64.))
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.))
            .py(px(10.))
            .bg(c.bg_panel)
            .border_r_1()
            .border_color(c.border)
            .overflow_y_scroll()
            .children(tiles)
            .child(
                div()
                    .id("rail-add-project")
                    .flex_none()
                    .size(px(36.))
                    .flex()
                    .justify_center()
                    .items_center()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(c.border_strong)
                    .cursor_pointer()
                    .hover(|s| s.bg(c.bg_hover))
                    .tooltip(Tooltip::text("Add project", None))
                    .child(icon(IconName::Plus, px(16.)).text_color(c.text_muted))
                    .on_click(add),
            )
            .child(div().flex_1())
            .child(
                broom("rail-cleanup", self.workspace.read(cx).cleanup_count, c)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ShellRequest::OpenCleanup))),
            )
            .into_any_element()
    }

    fn sidebar(&self, items: Vec<RailItem>, c: &Colors, cx: &mut Context<Self>) -> AnyElement {
        let rows = items.into_iter().map(|item| {
            let key = item.key.clone();
            let ws = self.workspace.clone();
            let mut row = div()
                .id(SharedString::from(format!("sidebar-{}", item.key)))
                .w_full()
                .flex()
                .items_center()
                .gap(px(9.))
                .px(px(8.))
                .py(px(6.))
                .rounded(px(6.))
                .cursor_pointer()
                .child(
                    div()
                        .flex_none()
                        .size(px(22.))
                        .flex()
                        .justify_center()
                        .items_center()
                        .rounded(px(5.))
                        .bg(if item.selected { c.accent } else { c.bg_column })
                        .child(
                            icon(IconName::FolderGit2, px(12.)).text_color(if item.selected {
                                gpui::white()
                            } else {
                                c.text_secondary
                            }),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(if item.selected {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if item.idle {
                            c.text_muted
                        } else {
                            c.text_primary
                        })
                        .child(SharedString::from(format!("{} · {}", item.name, item.key))),
                )
                .children((item.agents_working > 0).then(|| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(3.))
                        .child(icon(IconName::Activity, px(11.)).text_color(c.st_working))
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(11.))
                                .text_color(c.st_working)
                                .child(SharedString::from(item.agents_working.to_string())),
                        )
                }))
                .children((item.needs_you > 0).then(|| {
                    div()
                        .h(px(16.))
                        .px(px(5.))
                        .flex()
                        .items_center()
                        .rounded(px(8.))
                        .bg(c.st_attn)
                        .font_family(FONT_MONO)
                        .text_size(px(10.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(gpui::white())
                        .child(SharedString::from(item.needs_you.to_string()))
                }))
                .children(item.idle.then(|| {
                    div()
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child("idle")
                }))
                .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.select(&key, cx)));
            row = if item.selected {
                row.bg(c.accent_soft)
            } else {
                row.hover(|s| s.bg(c.bg_hover))
            };
            if item.ringing {
                row = row.shadow(crate::components::focus_ring(c.st_attn));
            }
            row
        });
        let add = cx.listener(|_, _, _, cx| cx.emit(ShellRequest::AddProject));
        let platform = Platform::current();
        div()
            .id("project-sidebar")
            .flex_none()
            .w(px(232.))
            .h_full()
            .flex()
            .flex_col()
            .gap(px(2.))
            .px(px(8.))
            .py(px(12.))
            .bg(c.bg_panel)
            .border_r_1()
            .border_color(c.border)
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .items_center()
                    .px(px(8.))
                    .pt(px(4.))
                    .pb(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .font_family(FONT_MONO)
                            .text_size(px(10.))
                            .text_color(c.text_muted)
                            .child("PROJECTS"),
                    )
                    .children(Kbd::for_action(KeyAction::SwitchProject, platform)),
            )
            .children(rows)
            .child(
                div()
                    .id("sidebar-add-project")
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(c.bg_hover))
                    .child(icon(IconName::Plus, px(14.)).text_color(c.text_muted))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(c.text_muted)
                            .child("Add project"),
                    )
                    .on_click(add),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        broom("sidebar-cleanup", self.workspace.read(cx).cleanup_count, c)
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(ShellRequest::OpenCleanup)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(c.text_secondary)
                            .child("Cleanup"),
                    ),
            )
            .into_any_element()
    }
}

impl Render for ShellBody {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let items = self
            .workspace
            .read(cx)
            .switcher
            .rail(Instant::now(), Platform::current());
        let wide = f32::from(window.viewport_size().width) > SIDEBAR_FROM_WIDTH;
        let rail = if wide {
            self.sidebar(items, &c, cx)
        } else {
            self.rail(items, &c, cx)
        };
        let board = match &self.board {
            AnyElementSlot::View(view) => view.clone().into_any_element(),
            AnyElementSlot::Empty => div().flex_1().into_any_element(),
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .child(rail)
            .child(div().flex_1().min_w_0().h_full().flex().child(board))
    }
}

/// The quick switcher (⌘P): type to filter, ↑/↓ to move, ↵ to switch, Esc to close, ⌘N to add a project.
pub struct QuickSwitcher {
    workspace: Entity<Workspace>,
    input: Entity<TextInput>,
    cursor: QuickCursor,
    focus: FocusHandle,
}

/// The switcher's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitcherOutcome {
    Switched,
    AddProject,
}

impl EventEmitter<SwitcherOutcome> for QuickSwitcher {}
impl EventEmitter<DismissEvent> for QuickSwitcher {}

impl Focusable for QuickSwitcher {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

gpui::actions!(quick_switcher, [SwitcherAddProject]);

pub const SWITCHER_CONTEXT: &str = "QuickSwitcher";

impl QuickSwitcher {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Switch project…")
                .arrows_out()
                .bare()
        });
        cx.subscribe(&input, |qs, _, event: &InputEvent, cx| {
            qs.on_input(event, cx)
        })
        .detach();
        let handle = input.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        Self {
            workspace,
            input,
            cursor: QuickCursor::default(),
            focus: cx.focus_handle(),
        }
    }

    fn rows(&self, cx: &App) -> Vec<RailItem> {
        let filter = self.input.read(cx).text().to_owned();
        self.workspace
            .read(cx)
            .switcher
            .quick(&filter, Instant::now(), Platform::current())
    }

    fn on_input(&mut self, event: &InputEvent, cx: &mut Context<Self>) {
        let n = self.rows(cx).len();
        match event {
            InputEvent::ArrowDown => self.cursor.down(n),
            InputEvent::ArrowUp => self.cursor.up(n),
            InputEvent::Changed => self.cursor = QuickCursor::default(),
            InputEvent::Submit => {
                self.choose(self.cursor.index, cx);
                return;
            }
            InputEvent::Cancel => {
                cx.emit(DismissEvent);
                return;
            }
            InputEvent::Blur | InputEvent::AltSubmit => {}
        }
        cx.notify();
    }

    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.rows(cx).get(index) {
            let key = row.key.clone();
            self.workspace.update(cx, |ws, cx| ws.select(&key, cx));
            cx.emit(SwitcherOutcome::Switched);
            cx.emit(DismissEvent);
        }
    }
}

impl Render for QuickSwitcher {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let rows = self.rows(cx);
        self.cursor.clamp(rows.len());
        let platform = Platform::current();
        let list = rows.into_iter().enumerate().map(|(i, row)| {
            let mut name = format!("{} · {}", row.name, row.key);
            if row.selected {
                name.push_str(" · current");
            }
            let mut el = div()
                .id(("switcher-row", i))
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(10.))
                .py(px(7.))
                .rounded(px(6.))
                .cursor_pointer()
                .child(
                    div()
                        .flex_none()
                        .w(px(36.))
                        .h(px(22.))
                        .flex()
                        .justify_center()
                        .items_center()
                        .rounded(px(5.))
                        .bg(c.bg_column)
                        .font_family(FONT_MONO)
                        .text_size(px(8.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.text_secondary)
                        .child(SharedString::from(row.key.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .child(
                            div()
                                .truncate()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c.text_primary)
                                .child(SharedString::from(name)),
                        )
                        .child(
                            div()
                                .truncate()
                                .font_family(FONT_MONO)
                                .text_size(px(10.))
                                .text_color(c.text_muted)
                                .child(SharedString::from(tilde(&row.repo_path))),
                        ),
                )
                .children((row.agents_working > 0).then(|| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(3.))
                        .child(icon(IconName::Activity, px(11.)).text_color(c.st_working))
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(11.))
                                .text_color(c.st_working)
                                .child(SharedString::from(format!(
                                    "{} running",
                                    row.agents_working
                                ))),
                        )
                }))
                .children((row.needs_you > 0).then(|| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(3.))
                        .px(px(6.))
                        .py(px(1.))
                        .rounded(px(8.))
                        .bg(c.st_attn_bg)
                        .child(icon(IconName::Hand, px(10.)).text_color(c.st_attn))
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(11.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.st_attn)
                                .child(SharedString::from(row.needs_you.to_string())),
                        )
                }))
                .children(row.idle.then(|| {
                    div()
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child("idle")
                }))
                .children(row.shortcut.clone().map(|k| {
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(10.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(k))
                }))
                .on_click(cx.listener(move |qs, _, _, cx| qs.choose(i, cx)));
            el = if i == self.cursor.index {
                el.bg(c.accent_soft)
            } else {
                el.hover(|s| s.bg(c.bg_hover))
            };
            el
        });
        let hint = |keys: &str, label: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .child(Kbd::text(keys.to_owned()))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child(label),
                )
        };
        let add_keys = KeyAction::NewTicket.keys(platform).unwrap_or_default();
        div()
            .id("quick-switcher")
            .key_context(SWITCHER_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &SwitcherAddProject, _, cx| {
                cx.emit(SwitcherOutcome::AddProject);
                cx.emit(DismissEvent);
            }))
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
            .w(px(520.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(c.bg_raised)
            .border_1()
            .border_color(c.border)
            .rounded(px(radius::DIALOG))
            .shadow(floating_shadow())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .py(px(12.))
                    .border_b_1()
                    .border_color(c.border)
                    .child(icon(IconName::FolderGit2, px(15.)).text_color(c.text_muted))
                    .child(div().flex_1().child(self.input.clone()))
                    .child(Kbd::text("Esc")),
            )
            .child(
                div()
                    .id("switcher-list")
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .p(px(6.))
                    .children(list),
            )
            .child(
                div()
                    .flex()
                    .gap(px(14.))
                    .px(px(14.))
                    .py(px(8.))
                    .bg(c.bg_panel)
                    .border_t_1()
                    .border_color(c.border)
                    .child(hint("↑↓", "move"))
                    .child(hint("↵", "switch"))
                    .child(hint(&add_keys, "add project")),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_becomes_a_tilde() {
        let home = std::env::home_dir()
            .map(|h| h.display().to_string())
            .unwrap_or_default();
        if !home.is_empty() {
            assert_eq!(tilde(&format!("{home}/code/shop-web")), "~/code/shop-web");
        }
        assert_eq!(tilde("/opt/repo"), "/opt/repo");
    }
}
