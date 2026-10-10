//! The ticket side panel (UI-04, brief 05): opens next to the board from a card click or ↵, closes with Esc or
//! its close button, resizes from its left edge, expands to the whole window (⇧⌘E), and walks the column with
//! ⌘[ / ⌘]. Its header shows key, title, status, branch (copyable), base, last signal and tokens; its tabs
//! Terminal · Diff · Run · Shell · Ticket · History carry indicators and switch with ⌘⌥1–6 (d0058, d0035).
//! The Terminal tab hosts the ticket's [`TerminalView`]; tabs whose features land later show an empty state.

use std::time::{SystemTime, UNIX_EPOCH};

use crewboard_core::board::tickets::{Base, Ticket};
use crewboard_core::domain::TicketKey;
use crewboard_core::protocol::{Event, Reply, Request, SessionListParams};
use crewboard_core::session::SessionInfo;
use gpui::{
    AnyElement, App, ClipboardItem, Context, Entity, EventEmitter, Focusable, FontWeight,
    IntoElement, ParentElement, Render, SharedString, Styled, Window, div, prelude::*, px,
};

use crate::components::button::{Button, IconButton, Variant};
use crate::components::menu::{ContextMenu, MenuChosen, Tooltip};
use crate::components::pill::StatusPill;
use crate::components::stack_list::StackList;
use crate::components::tabs::{TabIndicator, TabItem, Tabs};
use crate::diff_view::DiffView;
use crate::editor::{EditorClosed, EditorOpenTerminal, TicketEditor};
use crate::icons::{IconName, icon};
use crate::keymap::{KeyAction, Platform};
use crate::service_link::{ServiceEvent, ServiceLink};
use crate::terminal::{TerminalEvent, TerminalView};
use crate::theme::{ActiveTheme, FONT_MONO, text};
use crate::ui::TextStyleExt;
use crate::view_models::answer_options::{AnswerOptions, Pick};
use crate::view_models::card::{age_label, tokens_label};
use crate::view_models::clock;
use crate::view_models::epic_actions;
use crate::view_models::history;
use crate::view_models::panel::{self as vm, Indicator, Tab};
use crate::view_models::panel_actions::{self as actions, Action, Look};
use crate::view_models::queue;
use crate::view_models::stack::{stack_rows, summary as stack_summary};
use crate::view_models::terminal_strip::{self as strip, Choice, Tone, UnsentPrompt};
use crate::workspace::Workspace;
use crewboard_core::board::tickets::HistoryEvent;

pub mod review;
use crewboard_core::session::deliveries::Delivery;

/// What the panel asks of its host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelEvent {
    Close,
    ToggleExpanded,
    /// Show this ticket instead (⌘[ / ⌘], a Stack line).
    Show(TicketKey),
    /// The keyboard should go back to the panel (the terminal was left).
    TakeFocus,
    /// Start (to Dev), as ⇧⌘D on the board (with its empty "Done when" warning).
    Start,
    /// Edit the ticket in the Ticket tab (from the "…" menu, which has no window at hand).
    EditInPanel,
    /// A "…" menu choice the board carries out (Pin, Move to epic…, Close epic…, Delete…).
    MenuChoice(String),
    /// "Instructions: shop-web": the project's settings at Agent instructions (d0040).
    Instructions(String),
    /// A toast for the window ("Local merge done · SHOP-18").
    Toast(crate::board::Notice),
    /// A "…" menu choice that opens one of the panel's dialogs (it needs the window for focus).
    OpenDialog(review::PanelDialog),
    /// The run bar's Next (as ⌘J, ATT-07).
    NextInboxItem,
}

pub struct SidePanel {
    workspace: Entity<Workspace>,
    link: Entity<ServiceLink>,
    key: TicketKey,
    pub tab: Tab,
    pub expanded: bool,
    pub width: f32,
    /// The ticket's sessions, newest first (History, the Terminal tab's live dot).
    sessions: Vec<SessionInfo>,
    terminal: Option<Entity<TerminalView>>,
    /// The Diff tab.
    diff: Option<Entity<DiffView>>,
    /// The open "…" menu and where it opened.
    menu: Option<(Entity<ContextMenu>, gpui::Point<gpui::Pixels>)>,
    /// The Ticket tab's editor (read mode, Edit switches it to the form in place).
    editor: Option<Entity<TicketEditor>>,
    /// The ticket's events, newest first (History tab).
    history: Vec<HistoryEvent>,
    /// The Stack section is open (`None`: open when the ticket has links).
    stack_open: Option<bool>,
    /// "SHOP-23 has unsent text in its terminal" (SES-07), while it asks.
    prompt: Option<UnsentPrompt>,
    prompt_focus: gpui::FocusHandle,
    /// A Deliver now request is on its way (the button can't be pressed twice).
    delivering: bool,
    /// A Retry start request is on its way (SES-02: one click, one retry).
    retrying: bool,
    /// The setup log is shown under the Start interrupted callout.
    show_setup_log: bool,
    /// The Send back dialog while it is open (T-0094).
    send_back: Option<review::SendBackDialog>,
    /// Approve ▾ and Local merge, while open (T-0095).
    approve: Option<review::ApproveFlow>,
    /// Close ticket / Submit for review, while open (T-0106).
    lifecycle: Option<review::LifecycleDialog>,
    /// "Remove dependency" on a child of a closed parent, while it asks (T-0108).
    remove_dependency: Option<(TicketKey, gpui::FocusHandle)>,
    /// "conflicts with `main`" with its files (T-0098).
    conflict: Option<crewboard_core::board::conflicts::Conflict>,
    /// A review request is on its way (one click, one request).
    reviewing: bool,
    /// "Before the restart" for an Interrupted ticket (REC-02), once loaded; and whether it is loading.
    before_restart: Option<crewboard_core::recovery::BeforeRestart>,
    before_loading: bool,
    /// A Resume / Retry request is on its way (one click, one resume).
    resuming: bool,
    /// Asks you with answer options (ATT-12): the question's id, the list, Other…'s field, the list's keyboard,
    /// and whether an answer is on its way.
    answer: Option<(String, AnswerOptions)>,
    answer_input: Entity<crate::components::text_input::TextInput>,
    answer_focus: gpui::FocusHandle,
    answering: bool,
}

impl EventEmitter<PanelEvent> for SidePanel {}

impl SidePanel {
    pub fn new(
        workspace: Entity<Workspace>,
        key: TicketKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let link = workspace.read(cx).link.clone();
        // Read mode shows the ticket as it is now (an agent's edit appears without reloading, T-0076); a form
        // being edited is left alone.
        cx.observe_in(&workspace, window, |panel, _, window, cx| {
            let current = panel.ticket(cx);
            let stale = panel.editor.as_ref().is_some_and(|e| {
                let e = e.read(cx);
                e.is_reading() && !current.as_ref().is_some_and(|t| e.shows(t))
            });
            if stale {
                let note = panel.editor.as_ref().and_then(|e| e.read(cx).saved_note());
                panel.new_editor(false, window, cx);
                if let Some(editor) = &panel.editor {
                    editor.update(cx, |e, cx| e.set_saved_note(note, cx));
                }
            }
            cx.notify();
        })
        .detach();
        // The header's last signal ticks like the cards' (ATT-05).
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(crate::board::TICK).await;
                let alive = this.update(cx, |panel, cx| {
                    if panel.ticket(cx).is_some_and(|t| crate::board::ticks(&t)) {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    return;
                }
            }
        })
        .detach();
        cx.subscribe_in(
            &link,
            window,
            |panel, _, event: &ServiceEvent, _window, cx| match event {
                ServiceEvent::Event(Event::SessionState { session })
                    if session.ticket.as_ref() == Some(&panel.key) =>
                {
                    panel.load_sessions(cx);
                    panel.load_history(cx);
                }
                // The Ticket tab follows the workspace's copy of the ticket (see below), which is refetched
                // after this event.
                ServiceEvent::Event(Event::TicketChanged { key, .. }) if *key == panel.key => {
                    panel.load_history(cx);
                    panel.load_conflict(cx);
                }
                ServiceEvent::Resync => {
                    panel.load_sessions(cx);
                    panel.load_history(cx);
                }
                _ => {}
            },
        )
        .detach();
        let answer_input = cx.new(|cx| {
            crate::components::text_input::TextInput::new(window, cx)
                .placeholder("Type your own answer")
        });
        cx.subscribe(
            &answer_input,
            |panel, _, event: &crate::components::text_input::InputEvent, cx| match event {
                crate::components::text_input::InputEvent::Submit => panel.send_answer(cx),
                _ => cx.notify(),
            },
        )
        .detach();
        let mut panel = Self {
            workspace,
            link,
            key: key.clone(),
            tab: Tab::Terminal,
            expanded: false,
            width: vm::DEFAULT_WIDTH,
            sessions: Vec::new(),
            terminal: None,
            diff: None,
            menu: None,
            editor: None,
            history: Vec::new(),
            stack_open: None,
            prompt: None,
            prompt_focus: cx.focus_handle(),
            delivering: false,
            retrying: false,
            show_setup_log: false,
            send_back: None,
            approve: None,
            lifecycle: None,
            remove_dependency: None,
            conflict: None,
            reviewing: false,
            before_restart: None,
            before_loading: false,
            resuming: false,
            answer: None,
            answer_input,
            answer_focus: cx.focus_handle(),
            answering: false,
        };
        panel.show(key, window, cx);
        panel
    }

    pub fn key(&self) -> &TicketKey {
        &self.key
    }

    /// Shows another ticket in the same panel (same tab, width and expansion).
    pub fn show(&mut self, key: TicketKey, window: &mut Window, cx: &mut Context<Self>) {
        self.key = key.clone();
        self.sessions.clear();
        self.before_restart = None;
        let font_size = self
            .workspace
            .read(cx)
            .global_settings
            .as_ref()
            .map_or(13., |s| s.terminal_font_size as f32);
        let link = self.link.clone();
        let terminal = cx.new(|cx| TerminalView::new(link, key.clone(), font_size, window, cx));
        cx.subscribe(
            &terminal,
            |panel, _, event: &TerminalEvent, cx| match event {
                TerminalEvent::LeftFocus => cx.emit(PanelEvent::TakeFocus),
                TerminalEvent::OpenInstructions => {
                    cx.emit(PanelEvent::Instructions(panel.key.project.to_string()))
                }
            },
        )
        .detach();
        let project_name = self
            .workspace
            .read(cx)
            .current()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| self.key.project.to_string());
        terminal.update(cx, |t, _| {
            t.instructions = Some(strip::instructions_link(&project_name))
        });
        self.terminal = Some(terminal);
        let workspace = self.workspace.clone();
        let diff = cx.new(|cx| DiffView::new(workspace, key.clone(), window, cx));
        cx.observe(&diff, |_, _, cx| cx.notify()).detach();
        self.diff = Some(diff);
        self.load_sessions(cx);
        self.history.clear();
        self.load_history(cx);
        self.conflict = None;
        self.load_conflict(cx);
        self.stack_open = None;
        self.new_editor(false, window, cx);
        cx.notify();
    }

    /// A fresh editor for the Ticket tab: read mode, or the form at "Done when" (`at_criteria`).
    fn new_editor(&mut self, at_criteria: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ticket) = self.ticket(cx) else {
            self.editor = None;
            return;
        };
        let workspace = self.workspace.clone();
        let editor =
            cx.new(|cx| TicketEditor::open(workspace, &ticket, at_criteria, window, cx).embedded());
        // Esc or Cancel in the form: back to read mode with the ticket as stored.
        cx.subscribe_in(&editor, window, |panel, _, _: &EditorClosed, window, cx| {
            panel.new_editor(false, window, cx);
            cx.emit(PanelEvent::TakeFocus);
        })
        .detach();
        // "Open terminal" on the editor's saved-note banner (SES-08).
        cx.subscribe_in(
            &editor,
            window,
            |panel, _, _: &EditorOpenTerminal, window, cx| {
                panel.run(Action::OpenTerminal, window, cx);
            },
        )
        .detach();
        let live = self.has_live_agent();
        editor.update(cx, |e, cx| e.set_live_agent(live, cx));
        self.editor = Some(editor);
    }

    /// Whether the ticket's agent session runs (a save then tells the agent what changed).
    fn has_live_agent(&self) -> bool {
        self.sessions
            .iter()
            .any(|s| s.kind == crewboard_core::session::SessionKind::Agent && s.is_running())
    }

    /// The Ticket tab, in read mode or straight in the form (Edit, + Add dependency, "Add criteria").
    pub fn open_ticket(
        &mut self,
        editing: bool,
        at_criteria: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tab = Tab::Ticket;
        if at_criteria {
            self.new_editor(true, window, cx);
        } else if editing && let Some(editor) = &self.editor {
            editor.update(cx, |e, cx| e.start_editing(window, cx));
        }
        cx.notify();
    }

    fn load_history(&mut self, cx: &mut Context<Self>) {
        let key = self.key.clone();
        let request = self.link.read(cx).send(Request::TicketHistory(
            crewboard_core::protocol::KeyParams {
                key: key.to_string(),
            },
        ));
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::History(events)) = request.await {
                let _ = this.update(cx, |panel, cx| {
                    if panel.key == key {
                        panel.history = events;
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    /// The Terminal tab, with the keyboard in the terminal (Answer in terminal, the inbox, ⌘J).
    pub fn focus_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = Tab::Terminal;
        if let Some(terminal) = &self.terminal {
            let focus = terminal.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    pub fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        // Leaving the Terminal tab is leaving the terminal (ATT-03): its focus is reported lost.
        if self.tab == Tab::Terminal
            && tab != Tab::Terminal
            && let Some(terminal) = &self.terminal
        {
            terminal.update(cx, |t, _| t.hidden());
        }
        if tab == Tab::Terminal
            && self.tab != Tab::Terminal
            && let Some(terminal) = &self.terminal
        {
            terminal.update(cx, |t, _| t.shown());
        }
        self.tab = tab;
        cx.notify();
    }

    fn load_sessions(&mut self, cx: &mut Context<Self>) {
        let key = self.key.clone();
        let request = self
            .link
            .read(cx)
            .send(Request::SessionList(SessionListParams {
                key: Some(key.to_string()),
            }));
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::Sessions(list)) = request.await {
                let _ = this.update(cx, |panel, cx| {
                    if panel.key == key {
                        panel.sessions = list;
                        let live = panel.has_live_agent();
                        if let Some(editor) = &panel.editor {
                            editor.update(cx, |e, cx| e.set_live_agent(live, cx));
                        }
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn ticket(&self, cx: &App) -> Option<Ticket> {
        self.workspace
            .read(cx)
            .tickets
            .iter()
            .find(|t| t.key == self.key)
            .cloned()
    }

    /// The ticket before or after this one in its column (⌘[ / ⌘]).
    pub fn neighbour(&self, step: i32, cx: &App) -> Option<TicketKey> {
        vm::neighbour(&self.workspace.read(cx).tickets, &self.key, step)
    }

    fn header(&self, ticket: &Ticket, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let platform = Platform::current();
        let nav = |id: &'static str,
                   name: IconName,
                   tip: &'static str,
                   keys: Option<String>,
                   event: PanelEvent,
                   enabled: bool,
                   cx: &mut Context<Self>| {
            div().id(id).tooltip(Tooltip::text(tip, keys)).child(
                IconButton::new(SharedString::from(format!("{id}-button")), name)
                    .disabled(!enabled)
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(event.clone()))),
            )
        };
        let prev = self.neighbour(-1, cx);
        let next = self.neighbour(1, cx);
        let title_row = div()
            .flex()
            .items_start()
            .gap(px(8.))
            .child(
                div()
                    .pt(px(3.))
                    .font_family(FONT_MONO)
                    .text_size(px(13.))
                    .text_color(c.text_muted)
                    .child(SharedString::from(ticket.key.to_string())),
            )
            .child(
                div()
                    .id("panel-title")
                    .flex_1()
                    .min_w_0()
                    .text_size(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .line_height(px(21.))
                    .text_color(c.text_primary)
                    .line_clamp(3)
                    .tooltip(Tooltip::text(ticket.title.clone(), None))
                    .child(SharedString::from(ticket.title.clone())),
            )
            .child(nav(
                "panel-previous",
                IconName::ChevronUp,
                "Previous ticket",
                KeyAction::PreviousTicket.keys(platform),
                PanelEvent::Show(prev.clone().unwrap_or_else(|| self.key.clone())),
                prev.is_some(),
                cx,
            ))
            .child(nav(
                "panel-next",
                IconName::ChevronDown,
                "Next ticket",
                KeyAction::NextTicket.keys(platform),
                PanelEvent::Show(next.clone().unwrap_or_else(|| self.key.clone())),
                next.is_some(),
                cx,
            ))
            .child(nav(
                "panel-expand",
                if self.expanded {
                    IconName::Minimize2
                } else {
                    IconName::Maximize2
                },
                if self.expanded {
                    "Back to the board"
                } else {
                    "Expand"
                },
                KeyAction::ExpandPanel.keys(platform),
                PanelEvent::ToggleExpanded,
                true,
                cx,
            ))
            .child(nav(
                "panel-close",
                IconName::X,
                "Close",
                KeyAction::ClosePanel.keys(platform),
                PanelEvent::Close,
                true,
                cx,
            ));

        let meta = self.meta_row(ticket, cx);
        let bar = self.action_bar(ticket, cx);
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .pt(px(12.))
            .pb(px(12.))
            .pl(px(16.))
            .pr(px(14.))
            .child(title_row)
            .child(meta)
            .child(bar)
            .into_any_element()
    }

    fn meta_row(&self, ticket: &Ticket, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let mono_meta = |text: String, color| {
            div()
                .font_family(FONT_MONO)
                .text_size(px(11.))
                .text_color(color)
                .child(SharedString::from(text))
        };
        let branch = vm::branch_name(ticket).map(|name| {
            let shown = vm::middle_truncate(&name, 28);
            let copy = name.clone();
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .id("panel-branch")
                        .tooltip(Tooltip::text(name.clone(), None))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .px(px(6.))
                        .py(px(2.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(c.border)
                        .child(icon(IconName::GitBranch, px(11.)).text_color(c.text_secondary))
                        .child(mono_meta(shown, c.text_secondary)),
                )
                .child(
                    div()
                        .id("panel-copy-branch")
                        .tooltip(Tooltip::text("Copy branch name", None))
                        .cursor_pointer()
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                        })
                        .child(icon(IconName::Copy, px(12.)).text_color(c.text_muted)),
                )
        });
        let base = match &ticket.base {
            Base::Branch(b) => b.clone(),
            Base::Epic(epic) => crewboard_core::naming::branch(&epic.to_string()),
        };
        let now = now_ms();
        let signal = ticket.flags.last_signal_at.map(|at| {
            // A working agent silent for too long turns the line into a warning (ATT-05).
            let color = if crewboard_core::signals::is_stale(ticket.status, Some(at), now) {
                c.st_paused
            } else {
                c.text_muted
            };
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(icon(IconName::Activity, px(11.)).text_color(color))
                .child(mono_meta(format!("{} ago", age_label(now - at)), color))
        });
        let tokens = ticket.flags.tokens.map(|t| {
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(icon(IconName::Coins, px(11.)).text_color(c.text_muted))
                .child(mono_meta(tokens_label(t), c.text_muted))
        });
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(StatusPill::of(crate::view_models::status::shown(ticket)))
            .children(branch)
            .child(
                div()
                    .text_style_of(text::METADATA)
                    .text_color(c.text_muted)
                    .child("on"),
            )
            .child(mono_meta(base, c.text_secondary))
            .child(div().flex_1())
            .children(signal)
            .children(tokens)
            .into_any_element()
    }

    /// The state's buttons and the "…" menu (T-0060).
    fn action_bar(&self, ticket: &Ticket, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let buttons = actions::buttons(ticket).into_iter().map(|(action, look)| {
            let id = SharedString::from(format!("panel-action-{action:?}"));
            let click = cx.listener(move |panel, _, window, cx| panel.run(action, window, cx));
            let label = match action {
                Action::SendBack => crate::view_models::comments::send_back_label(
                    self.diff.as_ref().map_or(0, |d| d.read(cx).pending()),
                ),
                Action::ViewParent => ticket
                    .depends_on
                    .first()
                    .map_or_else(|| action.label().to_owned(), |p| format!("View parent {p}")),
                _ => action.label().to_owned(),
            };
            if look == Look::Attention {
                return div()
                    .id(id)
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(28.))
                    .px(px(10.))
                    .rounded(px(6.))
                    .bg(c.st_attn)
                    .text_color(c.text_on_accent)
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .on_click(click)
                    .child(icon(IconName::SquareTerminal, px(14.)).text_color(c.text_on_accent))
                    .child(SharedString::from(label))
                    .into_any_element();
            }
            let mut button = Button::new(id, label).variant(match look {
                Look::Primary => Variant::Primary,
                Look::Danger => Variant::Destructive,
                Look::Secondary | Look::Attention => Variant::Secondary,
            });
            if let Some(name) = action.icon() {
                button = button.icon(name);
            }
            if action == Action::RetryStart {
                button = button.loading(self.retrying);
            }
            button.on_click(click).into_any_element()
        });
        let more = div().id("panel-more").child(
            IconButton::new("panel-more-button", IconName::Ellipsis).on_click(cx.listener(
                |panel, e: &gpui::ClickEvent, window, cx| panel.open_menu(e.position(), window, cx),
            )),
        );
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .children(buttons)
            .child(div().flex_1())
            .child(more)
            .into_any_element()
    }

    fn run(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::Start => cx.emit(PanelEvent::Start),
            Action::Edit => self.open_ticket(true, false, window, cx),
            Action::OpenTerminal | Action::AnswerInTerminal => self.focus_terminal(window, cx),
            Action::RetryStart => self.retry_start(cx),
            Action::Resume => self.resume(false, cx),
            Action::RetryResume => self.resume(true, cx),
            Action::StartFromScratch => {
                self.open_dialog(review::PanelDialog::StartFromScratch, window, cx)
            }
            Action::ViewDiff => self.set_tab(Tab::Diff, cx),
            Action::SendBack => self.open_send_back(window, cx),
            Action::Approve => self.open_approve(window, cx),
            Action::Reopen => self.reopen(cx),
            Action::ViewParent => {
                if let Some(parent) = self.ticket(cx).and_then(|t| t.depends_on.first().cloned()) {
                    cx.emit(PanelEvent::Show(parent));
                }
            }
            // Hidden until their tickets wire them (see `Action::arrives_with`).
            _ => {}
        }
    }

    /// The answer options of the shown ticket's open question, kept in step with it (ATT-12).
    fn sync_answer(&mut self, ticket: &Ticket) {
        let question = (ticket.status == crewboard_core::domain::Status::AsksYou
            && !ticket.flags.your_turn)
            .then_some(ticket.question.as_ref())
            .flatten();
        match question {
            Some(q) if self.answer.as_ref().is_none_or(|(id, _)| id != &q.id) => {
                self.answer = AnswerOptions::of(q).map(|o| (q.id.clone(), o));
            }
            Some(_) => {}
            None => self.answer = None,
        }
    }

    /// A click on an option on the card: the list here with that option picked, and the keyboard on it.
    pub fn pick_answer(&mut self, n: u32, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ticket) = self.ticket(cx) {
            self.sync_answer(&ticket);
        }
        self.press_answer(n, window, cx);
        if self.answer.is_some()
            && !self
                .answer_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        {
            window.focus(&self.answer_focus, cx);
        }
    }

    fn press_answer(&mut self, n: u32, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, options)) = &mut self.answer else {
            return;
        };
        if options.press(n) && options.pick == Some(Pick::Other) {
            window.focus(&self.answer_input.read(cx).focus_handle(cx), cx);
        }
        cx.notify();
    }

    /// Send answer (↵ on the list, the button, or the card's Send answer): typed into the session; unsent text in
    /// the terminal asks first (d0048).
    pub fn send_answer(&mut self, cx: &mut Context<Self>) {
        if let Some(ticket) = self.ticket(cx) {
            self.sync_answer(&ticket);
        }
        let Some((_, options)) = &self.answer else {
            return;
        };
        let Some(choice) = options.choice(self.answer_input.read(cx).text()) else {
            return;
        };
        if self.answering {
            return;
        }
        self.answering = true;
        let key = self.key.clone();
        let send = self
            .link
            .read(cx)
            .send(crate::view_models::answer::request(&key, &choice, false));
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |panel, cx| {
                panel.answering = false;
                match result {
                    Ok(crewboard_core::protocol::Reply::Delivery(delivery)) => {
                        match crate::view_models::answer::outcome(&key, &choice, &delivery) {
                            crate::view_models::answer::AnswerOutcome::Sent => {
                                panel.answer = None;
                                panel.answer_input.update(cx, |i, cx| i.set_text("", cx));
                            }
                            crate::view_models::answer::AnswerOutcome::Ask(prompt) => {
                                panel.prompt = Some(prompt);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => cx.emit(PanelEvent::Toast(crate::board::Notice {
                        error: true,
                        title: format!("The answer to {key} wasn\u{2019}t sent"),
                        text: Some(e.message),
                    })),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// The question with its options above the tabs (*05 → SHOP-21 · Asks you · answer options*): 1–4 pick, ↵
    /// sends, Other… opens a free-text field, while the list has the keyboard; the terminal keeps its own keys.
    fn answer_section(&self, ticket: &Ticket, cx: &mut Context<Self>) -> Option<AnyElement> {
        use crate::components::answer_options::{
            CONTEXT, CancelPick, OptionList, PickOption1, PickOption2, PickOption3, PickOption4,
            SendPick,
        };
        let (_, options) = self.answer.as_ref()?;
        let question = ticket.question.as_ref()?;
        let c = ActiveTheme::get(cx).colors;
        let text = self.answer_input.read(cx).text().to_owned();
        let can_send = options.can_send(&text) && !self.answering;
        let other = options.pick == Some(Pick::Other);
        let this = cx.entity().downgrade();
        Some(
            div()
                .id("panel-answer-options")
                .key_context(CONTEXT)
                .track_focus(&self.answer_focus)
                .on_action(cx.listener(|p, _: &PickOption1, w, cx| p.press_answer(1, w, cx)))
                .on_action(cx.listener(|p, _: &PickOption2, w, cx| p.press_answer(2, w, cx)))
                .on_action(cx.listener(|p, _: &PickOption3, w, cx| p.press_answer(3, w, cx)))
                .on_action(cx.listener(|p, _: &PickOption4, w, cx| p.press_answer(4, w, cx)))
                .on_action(cx.listener(|p, _: &SendPick, _, cx| p.send_answer(cx)))
                .on_action(cx.listener(|p, _: &CancelPick, w, cx| p.focus_terminal(w, cx)))
                .on_click(cx.listener(|p, _, window, cx| window.focus(&p.answer_focus, cx)))
                .flex()
                .flex_none()
                .flex_col()
                .gap(px(8.))
                .mx(px(14.))
                .my(px(8.))
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(c.st_attn)
                .bg(c.st_attn_bg)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            icon(IconName::MessageCircleQuestionMark, px(14.))
                                .text_color(c.st_attn),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.text_primary)
                                .child(SharedString::from(question.text.clone())),
                        ),
                )
                .child(OptionList::new(
                    "panel-option-list",
                    options.rows(),
                    move |pick, window, cx| {
                        let _ = this.update(cx, |p, cx| {
                            let n = match pick {
                                Pick::Option(n) => n,
                                Pick::Other => {
                                    p.answer.as_ref().map_or(0, |(_, o)| o.other_number())
                                }
                            };
                            p.press_answer(n, window, cx);
                            if pick != Pick::Other {
                                window.focus(&p.answer_focus, cx);
                            }
                        });
                    },
                ))
                .when(other, |d| d.child(self.answer_input.clone()))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(11.))
                                .text_color(c.text_muted)
                                .child("1–4 pick · ↵ sends"),
                        )
                        .child(
                            Button::new("panel-send-answer", "Send answer")
                                .icon(IconName::Send)
                                .disabled(!can_send)
                                .loading(self.answering)
                                .on_click(cx.listener(|p, _, _, cx| p.send_answer(cx))),
                        ),
                )
                .into_any_element(),
        )
    }

    /// **Resume** an Interrupted ticket, or **Retry** after Resume failed (REC-01, REC-03): `claude --resume` in
    /// a new session in the same worktree. One click, one request.
    pub fn resume(&mut self, retry: bool, cx: &mut Context<Self>) {
        if self.resuming {
            return;
        }
        self.resuming = true;
        let params = crewboard_core::protocol::KeyParams {
            key: self.key.to_string(),
        };
        let request = if retry {
            Request::TicketRetryResume(params)
        } else {
            Request::TicketResume(params)
        };
        let send = self.link.read(cx).send(request);
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |panel, cx| {
                panel.resuming = false;
                panel.before_restart = None;
                if let Err(e) = result {
                    cx.emit(PanelEvent::Toast(crate::board::Notice {
                        error: true,
                        title: format!("{} can\u{2019}t be resumed", panel.key),
                        text: Some(e.message),
                    }));
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Loads "Before the restart" for an Interrupted ticket whose session is gone (REC-02).
    fn load_before_restart(&mut self, ticket: &Ticket, cx: &mut Context<Self>) {
        if self.before_restart.is_some()
            || self.before_loading
            || !crate::view_models::recovery::shows_recovery(ticket, self.has_live_agent())
        {
            return;
        }
        self.before_loading = true;
        let key = self.key.clone();
        let send = self.link.read(cx).send(Request::SessionBeforeRestart(
            crewboard_core::protocol::BeforeRestartParams {
                key: key.to_string(),
                limit: None,
            },
        ));
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |panel, cx| {
                panel.before_loading = false;
                if panel.key != key {
                    return;
                }
                if let Ok(crewboard_core::protocol::Reply::BeforeRestart(view)) = result {
                    panel.before_restart = Some(*view);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The Terminal tab of an Interrupted ticket (*06 → Interrupted · Before the restart*): the last messages and
    /// actions, read-only, with Resume; or "conversation no longer available" with Retry and Start from scratch.
    fn recovery_view(&self, ticket: &Ticket, cx: &mut Context<Self>) -> AnyElement {
        use crate::view_models::recovery::{RecoveryView, view};
        let c = ActiveTheme::get(cx).colors;
        let Some(before) = &self.before_restart else {
            return div()
                .flex_1()
                .p(px(16.))
                .text_size(px(12.))
                .text_color(c.text_muted)
                .child("Reading Claude’s saved conversation…")
                .into_any_element();
        };
        let failed = ticket.status == crewboard_core::domain::Status::ResumeFailed;
        let button =
            |id: &'static str, label: &'static str, icon_name, variant, retry: Option<bool>| {
                crate::components::button::Button::new(id, label)
                    .icon(icon_name)
                    .variant(variant)
                    .compact()
                    .loading(self.resuming && retry.is_some())
                    .on_click(cx.listener(move |panel, _, window, cx| match retry {
                        Some(retry) => panel.resume(retry, cx),
                        None => {
                            panel.open_dialog(review::PanelDialog::StartFromScratch, window, cx)
                        }
                    }))
            };
        let body = match view(ticket, before) {
            RecoveryView::BeforeRestart {
                session,
                lines,
                ended,
            } => {
                let rows = lines.into_iter().map(|line| {
                    div()
                        .flex()
                        .gap(px(10.))
                        .py(px(5.))
                        .border_b_1()
                        .border_color(c.border)
                        .font_family(FONT_MONO)
                        .text_size(px(12.))
                        .child(
                            div()
                                .w(px(40.))
                                .flex_none()
                                .text_color(c.text_muted)
                                .child(SharedString::from(line.time)),
                        )
                        .child(
                            div()
                                .w(px(52.))
                                .flex_none()
                                .text_color(if line.action {
                                    c.accent
                                } else {
                                    c.text_secondary
                                })
                                .child(SharedString::from(line.who)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_color(c.text_primary)
                                .child(SharedString::from(line.text)),
                        )
                });
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(c.text_primary)
                                    .child("Before the restart"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(11.))
                                    .text_color(c.text_muted)
                                    .child("from Claude’s saved conversation · read-only"),
                            )
                            .child(
                                div()
                                    .font_family(FONT_MONO)
                                    .text_size(px(11.))
                                    .text_color(c.text_muted)
                                    .child(SharedString::from(session)),
                            ),
                    )
                    .child(div().flex().flex_col().children(rows))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .p(px(10.))
                            .rounded(px(6.))
                            .bg(c.st_error_bg)
                            .child(icon(IconName::ZapOff, px(14.)).text_color(c.st_error))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.))
                                    .text_color(c.st_error)
                                    .child(SharedString::from(ended)),
                            )
                            .child(if failed {
                                button(
                                    "recovery-retry",
                                    "Retry",
                                    IconName::RotateCw,
                                    crate::components::button::Variant::Primary,
                                    Some(true),
                                )
                            } else {
                                button(
                                    "recovery-resume",
                                    "Resume",
                                    IconName::Play,
                                    crate::components::button::Variant::Primary,
                                    Some(false),
                                )
                            }),
                    )
            }
            RecoveryView::NotAvailable { text } => div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .p(px(12.))
                .rounded(px(6.))
                .border_1()
                .border_color(c.border)
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(10.))
                        .text_color(c.text_muted)
                        .child("CONVERSATION NO LONGER AVAILABLE"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(gpui::relative(1.45))
                        .text_color(c.text_secondary)
                        .child(SharedString::from(text)),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(button(
                            "recovery-retry",
                            "Retry",
                            IconName::RotateCw,
                            crate::components::button::Variant::Secondary,
                            Some(failed),
                        ))
                        .child(button(
                            "recovery-scratch",
                            "Start from scratch",
                            IconName::RotateCcw,
                            crate::components::button::Variant::Secondary,
                            None,
                        )),
                ),
        };
        div()
            .id("before-restart")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(body)
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .child("Read-only · the session ended"),
            )
            .into_any_element()
    }

    /// **Retry start** (the user's click, SES-02): the service puts the ticket back in Queued and runs the whole
    /// start again. Never sent twice for one click, and never without one.
    pub fn retry_start(&mut self, cx: &mut Context<Self>) {
        if self.retrying {
            return;
        }
        self.retrying = true;
        self.show_setup_log = false;
        let send = self.link.read(cx).send(Request::TicketRetryStart(
            crewboard_core::protocol::KeyParams {
                key: self.key.to_string(),
            },
        ));
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |panel, cx| {
                panel.retrying = false;
                if let Err(e) = result {
                    eprintln!("panel: retry start failed: {e}");
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// A Queued ticket's notice under the header: what it waits for, the card's text as a sentence (*05 → SHOP-20 ·
    /// Queued, waits for merge*; T-0083).
    fn queued_notice(&self, ticket: &Ticket, cx: &mut Context<Self>) -> Option<AnyElement> {
        let tickets = self.workspace.read(cx).tickets.clone();
        let text = queue::panel_notice(ticket, &tickets)?;
        let choices = crate::view_models::close::closed_parent_choices(ticket, &tickets);
        let buttons = (!choices.is_empty()).then(|| {
            div().flex().flex_wrap().gap(px(6.)).mt(px(6.)).children(
                choices.into_iter().enumerate().flat_map(|(i, choice)| {
                    let reopen = choice.parent.clone();
                    let remove = choice.parent.clone();
                    [
                        Button::new(("reopen-parent", i), choice.reopen)
                            .variant(Variant::Secondary)
                            .compact()
                            .icon(IconName::RotateCcw)
                            .on_click(cx.listener(move |panel, _, _, cx| {
                                panel.reopen_parent(reopen.clone(), cx)
                            }))
                            .into_any_element(),
                        Button::new(("remove-dependency", i), choice.remove)
                            .variant(Variant::Secondary)
                            .compact()
                            .on_click(cx.listener(move |panel, _, window, cx| {
                                panel.ask_remove_dependency(remove.clone(), window, cx)
                            }))
                            .into_any_element(),
                    ]
                }),
            )
        });
        let c = ActiveTheme::get(cx).colors;
        let (icon_name, fg, bg) = if ticket.status == crewboard_core::domain::Status::ParentClosed {
            (IconName::CircleSlash, c.st_error, c.st_error_bg)
        } else {
            (IconName::Hourglass, c.st_queued, c.st_queued_bg)
        };
        Some(
            div()
                .mx(px(16.))
                .mt(px(8.))
                .flex()
                .gap(px(8.))
                .px(px(12.))
                .py(px(9.))
                .rounded(px(6.))
                .bg(bg)
                .child(
                    div()
                        .pt(px(1.))
                        .child(icon(icon_name, px(13.)).text_color(fg)),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(12.))
                                .line_height(gpui::relative(1.45))
                                .text_color(c.text_primary)
                                .child(SharedString::from(text)),
                        )
                        .children(buttons),
                )
                .into_any_element(),
        )
    }

    /// "The start didn't finish": the failed step, its message, what Retry start does, and the setup log
    /// (T-0063).
    fn start_interrupted(&self, ticket: &Ticket, cx: &mut Context<Self>) -> Option<AnyElement> {
        if ticket.status != crewboard_core::domain::Status::StartInterrupted {
            return None;
        }
        let c = ActiveTheme::get(cx).colors;
        let (step, message) = match &ticket.start_problem {
            Some(p) => (Some(p.step), p.message.clone()),
            None => (
                None,
                format!(
                    "{} stopped before its agent session started, so there is nothing to resume.",
                    ticket.key
                ),
            ),
        };
        let heading = match step {
            Some(step) => format!("The start didn’t finish: {}", step.failure_label()),
            None => "The start didn’t finish".to_owned(),
        };
        let log_path = crewboard_core::data_dir::DataDir::from_env()
            .ok()
            .and_then(|d| d.setup_log_path(&ticket.key.to_string()).ok())
            .filter(|p| p.is_file());
        let log = (self.show_setup_log)
            .then(|| {
                log_path
                    .as_ref()
                    .and_then(|p| std::fs::read_to_string(p).ok())
            })
            .flatten();
        let bullet = |text: &'static str| {
            div()
                .flex()
                .gap(px(8.))
                .text_size(px(12.))
                .text_color(c.text_secondary)
                .child("•")
                .child(text)
        };
        let toggle = log_path.map(|_| {
            div()
                .id("setup-log-toggle")
                .text_size(px(12.))
                .text_color(c.accent)
                .cursor_pointer()
                .on_click(cx.listener(|panel, _, _, cx| {
                    panel.show_setup_log = !panel.show_setup_log;
                    cx.notify();
                }))
                .child(if self.show_setup_log {
                    "Hide the setup log"
                } else {
                    "View the setup log"
                })
        });
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .p(px(16.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .p(px(12.))
                        .rounded(px(6.))
                        .bg(c.st_error_bg)
                        .border_1()
                        .border_color(c.st_error)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(icon(IconName::CircleAlert, px(14.)).text_color(c.st_error))
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c.st_error)
                                        .child(SharedString::from(heading)),
                                ),
                        )
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(12.))
                                .text_color(c.text_primary)
                                .child(SharedString::from(message)),
                        )
                        .children(toggle),
                )
                .children(log.map(|log| {
                    div()
                        .id("setup-log")
                        .max_h(px(240.))
                        .overflow_y_scroll()
                        .p(px(10.))
                        .rounded(px(6.))
                        .bg(c.term_bg)
                        .border_1()
                        .border_color(c.border)
                        .font_family(FONT_MONO)
                        .text_size(px(11.))
                        .text_color(c.term_fg)
                        .child(SharedString::from(log))
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .p(px(12.))
                        .rounded(px(6.))
                        .bg(c.bg_column)
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(10.))
                                .text_color(c.text_muted)
                                .child("RETRY START DOES"),
                        )
                        .child(bullet("Runs the whole start again, the same way as dragging to Dev"))
                        .child(bullet("Starts a new agent session, so it uses tokens"))
                        .child(bullet(
                            "Leaves the ticket, its “Done when” list and its dependencies as they are",
                        )),
                )
                .into_any_element(),
        )
    }

    fn open_menu(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ticket) = self.ticket(cx) else {
            return;
        };
        let has_epics = !epic_actions::epic_choices(&self.workspace.read(cx).tickets).is_empty();
        let has_branch = ticket.work.as_ref().is_some_and(|w| w.branch.is_some());
        let has_session = !self.sessions.is_empty();
        let items = actions::menu(&ticket, has_epics, has_branch, has_session);
        let menu = cx.new(|cx| ContextMenu::new(items, window, cx));
        cx.subscribe(&menu, |panel, _, chosen: &MenuChosen, cx| {
            panel.menu = None;
            panel.on_menu(chosen.0.as_ref(), cx);
            cx.notify();
        })
        .detach();
        cx.subscribe(&menu, |panel, _, _: &gpui::DismissEvent, cx| {
            panel.menu = None;
            cx.notify();
        })
        .detach();
        self.menu = Some((menu, position));
        cx.notify();
    }

    fn on_menu(&mut self, choice: &str, cx: &mut Context<Self>) {
        let Some(ticket) = self.ticket(cx) else {
            return;
        };
        match choice {
            actions::EDIT_TICKET => cx.emit(PanelEvent::EditInPanel),
            actions::COPY_BRANCH => {
                if let Some(branch) = ticket.work.and_then(|w| w.branch) {
                    cx.write_to_clipboard(ClipboardItem::new_string(branch));
                }
            }
            actions::COPY_ATTACH => {
                let command = format!(
                    "{} attach {}",
                    crewboard_core::naming::COMMAND_NAME,
                    ticket.key
                );
                cx.write_to_clipboard(ClipboardItem::new_string(command));
            }
            crate::view_models::close::CLOSE_TICKET => {
                cx.emit(PanelEvent::OpenDialog(review::PanelDialog::Close))
            }
            crate::view_models::close::SUBMIT_FOR_REVIEW => {
                cx.emit(PanelEvent::OpenDialog(review::PanelDialog::Submit))
            }
            other => cx.emit(PanelEvent::MenuChoice(other.to_owned())),
        }
    }

    fn tabs(&self, ticket: &Ticket, cx: &mut Context<Self>) -> AnyElement {
        let platform = Platform::current();
        let diff = self.diff.as_ref().and_then(|d| d.read(cx).indicator());
        let items: Vec<TabItem> = vm::indicators(ticket, &self.sessions)
            .into_iter()
            .map(|(tab, indicator)| match (tab, &diff) {
                (Tab::Diff, Some(counts)) => (tab, Indicator::Text(counts.clone())),
                _ => (tab, indicator),
            })
            .map(|(tab, indicator)| {
                let item = TabItem::new(tab.icon(), tab.label());
                match indicator {
                    Indicator::None => item,
                    Indicator::Dot { attention: false } => item.indicator(TabIndicator::Live),
                    Indicator::Dot { attention: true } => {
                        item.indicator(TabIndicator::LiveAttention)
                    }
                    Indicator::Text(text) => item.indicator(TabIndicator::Note(text.into())),
                    Indicator::Count(n) => item.indicator(TabIndicator::Note(n.to_string().into())),
                }
            })
            .collect();
        let active = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
        let shortcuts = (1..=6)
            .map(|n| KeyAction::SwitchTab(n).keys(platform))
            .collect();
        let this = cx.entity().downgrade();
        div()
            .px(px(8.))
            .child(
                Tabs::new("panel-tabs", items, active)
                    .icons_only(
                        self.width < vm::ICON_TABS_BELOW && !self.expanded,
                        shortcuts,
                    )
                    .on_select(move |i, _, cx| {
                        let _ = this.update(cx, |panel, cx| panel.set_tab(Tab::ALL[i], cx));
                    }),
            )
            .into_any_element()
    }

    fn body(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        if self.tab == Tab::Terminal
            && let Some(ticket) = self.ticket(cx)
            && crate::view_models::recovery::shows_recovery(&ticket, self.has_live_agent())
        {
            return self.recovery_view(&ticket, cx);
        }
        if self.tab == Tab::Terminal
            && let Some(terminal) = &self.terminal
        {
            let strip = self.ticket(cx).map(|t| self.strip(&t, terminal, cx));
            return div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .children(strip)
                .child(div().flex_1().min_h_0().child(terminal.clone()))
                .into_any_element();
        }
        if self.tab == Tab::Ticket
            && let Some(editor) = &self.editor
        {
            return div()
                .flex_1()
                .min_h_0()
                .child(editor.clone())
                .into_any_element();
        }
        if self.tab == Tab::History {
            return self.history_view(cx);
        }
        if self.tab == Tab::Diff
            && let Some(diff) = &self.diff
        {
            let width = if self.expanded { 1200. } else { self.width };
            let read_only = self.ticket(cx).is_none_or(|t| {
                t.is_epic() || t.status != crewboard_core::domain::Status::ReadyForReview
            });
            diff.update(cx, |d, _| {
                d.width = width;
                d.read_only = read_only;
            });
            return div()
                .flex_1()
                .min_h_0()
                .child(diff.clone())
                .into_any_element();
        }
        let message = self.tab.later().unwrap_or_default();
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_style_of(text::METADATA)
            .text_size(px(12.))
            .text_color(c.text_muted)
            .child(message)
            .into_any_element()
    }
}

impl SidePanel {
    /// The Stack row between the header and the tabs (DEP-05): collapsed it summarises the links; open it shows
    /// the Stack list, whose lines open that ticket in the panel.
    fn stack_section(&self, ticket: &Ticket, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let tickets = &self.workspace.read(cx).tickets;
        let rows = stack_rows(&ticket.key, tickets);
        let linked = ticket.depends_on.len()
            + tickets
                .iter()
                .filter(|t| t.depends_on.contains(&ticket.key))
                .count();
        let open = self.stack_open.unwrap_or(linked > 0) && linked > 0;
        let summary = stack_summary(&ticket.key, tickets);
        let head = div()
            .id("stack-head")
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .on_click(cx.listener(move |panel, _, _, cx| {
                panel.stack_open = Some(!open);
                cx.notify();
            }))
            .child(
                icon(
                    if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    },
                    px(12.),
                )
                .text_color(c.text_muted),
            )
            .child(icon(IconName::Link2, px(12.)).text_color(c.text_muted))
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c.text_secondary)
                    .child("Stack"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .child(SharedString::from(summary)),
            )
            .child(
                div()
                    .id("stack-add-dependency")
                    .text_size(px(11.))
                    .text_color(c.accent)
                    .cursor_pointer()
                    .on_click(cx.listener(|panel, _, window, cx| {
                        cx.stop_propagation();
                        panel.open_ticket(true, false, window, cx)
                    }))
                    .child("+ Add dependency"),
            );
        let this = cx.entity().downgrade();
        let list = open.then(|| {
            StackList::new(ticket.key.clone(), rows).on_select(move |key, _, cx| {
                let _ = this.update(cx, |_, cx| cx.emit(PanelEvent::Show(key)));
            })
        });
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(16.))
            .py(px(7.))
            .bg(c.bg_app)
            .border_t_1()
            .border_color(c.border)
            .child(head)
            .children(list)
            .into_any_element()
    }

    /// The History tab: sessions with their Claude session IDs and the ticket's events (d0045).
    fn history_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let rows = history::rows(&self.history, &self.sessions);
        let column = |text: &'static str, width: Option<f32>| {
            let d = div()
                .font_family(FONT_MONO)
                .text_size(px(10.))
                .text_color(c.text_muted)
                .child(text);
            match width {
                Some(w) => d.w(px(w)).flex_none(),
                None => d.flex_1(),
            }
        };
        let cell = |text: String, width: f32| {
            div()
                .w(px(width))
                .flex_none()
                .font_family(FONT_MONO)
                .text_size(px(11.))
                .text_color(c.text_muted)
                .child(SharedString::from(text))
        };
        let lines = rows.into_iter().enumerate().map(|(i, row)| {
            let copy = row.claude_session_id.clone().map(|id| {
                div()
                    .id(SharedString::from(format!("history-copy-{i}")))
                    .tooltip(Tooltip::text("Copy Claude session ID", None))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(id.clone()))
                    })
                    .child(icon(IconName::Copy, px(11.)).text_color(c.text_muted))
            });
            div()
                .flex()
                .items_start()
                .gap(px(8.))
                .px(px(16.))
                .py(px(8.))
                .border_b_1()
                .border_color(c.border)
                .child(
                    icon(
                        if row.is_session {
                            IconName::SquareTerminal
                        } else {
                            IconName::History
                        },
                        px(13.),
                    )
                    .mt(px(2.))
                    .text_color(c.text_muted),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(c.text_primary)
                                .child(SharedString::from(row.title)),
                        )
                        .children(row.detail.map(|d| {
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .min_w_0()
                                        .font_family(FONT_MONO)
                                        .text_size(px(11.))
                                        .text_color(c.text_muted)
                                        .child(SharedString::from(d)),
                                )
                                .children(copy)
                        })),
                )
                .child(cell(clock::local_hhmm(row.at), 52.))
                .child(cell(row.duration, 72.))
                .child(cell(row.tokens, 52.))
        });
        div()
            .id("history")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .px(px(16.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(c.border)
                    .child(div().w(px(13.)))
                    .child(column("EVENT", None))
                    .child(column("WHEN", Some(52.)))
                    .child(column("DURATION", Some(72.)))
                    .child(column("TOKENS", Some(52.))),
            )
            .children(lines)
            .into_any_element()
    }

    /// The strip above the terminal (SES-11): state, worktree, `$PORT`, message waiting or the attach command,
    /// font size and find.
    fn strip(
        &self,
        ticket: &Ticket,
        terminal: &Entity<TerminalView>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let phase = terminal.read(cx).phase().clone();
        let (label, tone) = strip::state(&phase, ticket.status, ticket.flags.your_turn);
        let tone = match tone {
            Tone::Working => c.st_working,
            Tone::Attention => c.st_attn,
            Tone::Muted => c.text_muted,
        };
        let mono = |text: String, color| {
            div()
                .font_family(FONT_MONO)
                .text_size(px(11.))
                .text_color(color)
                .whitespace_nowrap()
                .child(SharedString::from(text))
        };
        let narrow = self.width < vm::ICON_TABS_BELOW && !self.expanded;
        let work = ticket.work.clone().unwrap_or_default();
        let path = (!narrow)
            .then(|| work.worktree.as_deref().map(strip::short_path))
            .flatten();
        let port = (!narrow)
            .then(|| work.agent_port.map(strip::agent_port))
            .flatten();
        let chip = |id: &'static str| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(5.))
                .px(px(6.))
                .py(px(2.))
                .rounded(px(4.))
                .cursor_pointer()
        };
        let right = match strip::waiting_label(ticket.flags.messages_waiting) {
            Some(waiting) => chip("strip-deliver")
                .bg(c.accent_soft)
                .on_click(cx.listener(|panel, _, window, cx| panel.deliver_now(None, window, cx)))
                .child(icon(IconName::MessageSquareDot, px(11.)).text_color(c.accent))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(c.text_primary)
                        .child(SharedString::from(waiting)),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.accent)
                        .child("Deliver now"),
                )
                .into_any_element(),
            None => {
                let command = strip::attach_command(&ticket.key);
                let copy = command.clone();
                chip("strip-attach")
                    .border_1()
                    .border_color(c.border)
                    .tooltip(Tooltip::text("Copy attach command", None))
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                    })
                    .child(icon(IconName::Copy, px(11.)).text_color(c.text_secondary))
                    .child(mono(
                        if narrow { "attach".to_owned() } else { command },
                        c.text_secondary,
                    ))
                    .into_any_element()
            }
        };
        let font = |id: &'static str, name: IconName, delta: f32, tip: &'static str| {
            let terminal = terminal.clone();
            div().id(id).tooltip(Tooltip::text(tip, None)).child(
                IconButton::new(SharedString::from(format!("{id}-button")), name).on_click(
                    move |_, _, cx| {
                        terminal.update(cx, |t, cx| {
                            let size = t.font_size + delta;
                            t.set_font_size(size, cx)
                        })
                    },
                ),
            )
        };
        let find = {
            let terminal = terminal.clone();
            div()
                .id("strip-find")
                .tooltip(Tooltip::text(
                    "Find in scrollback",
                    KeyAction::FindInTerminal.keys(Platform::current()),
                ))
                .child(
                    IconButton::new("strip-find-button", IconName::Search).on_click(
                        move |_, window, cx| terminal.update(cx, |t, cx| t.find(window, cx)),
                    ),
                )
        };
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
                    .child(div().size(px(7.)).rounded_full().bg(tone))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(tone)
                            .whitespace_nowrap()
                            .child(label),
                    ),
            )
            .children(path.map(|p| mono(p, c.text_muted)))
            .children(port.map(|p| mono(p, c.text_muted)))
            .child(div().flex_1())
            .child(right)
            .child(font(
                "strip-font-smaller",
                IconName::AArrowDown,
                -1.,
                "Smaller text",
            ))
            .child(font(
                "strip-font-bigger",
                IconName::AArrowUp,
                1.,
                "Larger text",
            ))
            .child(find)
            .into_any_element()
    }

    /// Deliver now (the user's click, SES-06), or the prompt's answer (SES-07).
    pub fn deliver_now(
        &mut self,
        choice: Option<Choice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if choice.is_some() {
            self.prompt = None;
        }
        let Some(request) = strip::deliver_now_request(&self.key, choice) else {
            cx.notify();
            return;
        };
        if self.delivering {
            return;
        }
        self.delivering = true;
        let send = self.link.read(cx).send(request);
        let key = self.key.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = send.await;
            let _ = this.update_in(cx, |panel, window, cx| {
                panel.delivering = false;
                if let Ok(Reply::Delivery(Delivery::NeedsConfirmation { .. })) = result {
                    let typed = panel
                        .terminal
                        .as_ref()
                        .and_then(|t| t.read(cx).typed_line());
                    let waiting = panel.ticket(cx).map_or(1, |t| t.flags.messages_waiting);
                    panel.prompt = Some(UnsentPrompt::deliver_now(key.clone(), typed, waiting));
                    window.focus(&panel.prompt_focus, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The unsent-text prompt's answer, for whichever click asked.
    fn answer_prompt(&mut self, choice: Choice, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prompt) = self.prompt.take() else {
            return;
        };
        match (prompt.click, choice) {
            (strip::Click::DeliverNow, choice) => self.deliver_now(Some(choice), window, cx),
            (_, Choice::Keep) => cx.notify(),
            (strip::Click::SendBack { note }, Choice::Clear) => {
                self.send_back(note, true, window, cx)
            }
            (strip::Click::SendToFix, Choice::Clear) => self.send_to_fix(true, window, cx),
            (strip::Click::Answer { choice }, Choice::Clear) => {
                let request = crate::view_models::answer::request(&self.key, &choice, true);
                self.workspace.update(cx, |ws, cx| ws.act(request, cx));
            }
        }
    }

    fn prompt_dialog(&self, prompt: &UnsentPrompt, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let clear = this.clone();
        crate::components::unsent::unsent_dialog(
            prompt,
            &self.prompt_focus,
            move |window, cx| {
                let _ = clear.update(cx, |p, cx| p.answer_prompt(Choice::Clear, window, cx));
            },
            move |window, cx| {
                let _ = this.update(cx, |p, cx| p.answer_prompt(Choice::Keep, window, cx));
            },
            cx,
        )
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl SidePanel {
    /// The ⌘J run bar on top (*10 → 1 · ⌘J from anywhere, 2 · Answered*): "Inbox · 1 of 7" with a segment per
    /// item, Next ⌘J and Exit; "Done · 6 left" once the open item is answered.
    fn run_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use crate::view_models::inbox_run::{RunBar, bar_title};
        let key = self.key.clone();
        let bar = self.workspace.update(cx, |ws, _| ws.run_bar(&key))?;
        let c = ActiveTheme::get(cx).colors;
        let (of, filled, current, done) = match bar {
            RunBar::Step { n, of, done } => (of, done, Some(n - 1), false),
            RunBar::Done { left, of } => (of, of - left, None, true),
            RunBar::AllClear { .. } => return None,
        };
        let tone = if done { c.st_approved } else { c.st_attn };
        let segments = (0..of.min(24)).map(|i| {
            div()
                .w(px(14.))
                .h(px(3.))
                .rounded(px(2.))
                .bg(if i < filled {
                    c.st_approved
                } else if Some(i) == current {
                    c.st_attn
                } else {
                    c.border_strong
                })
        });
        let next_key = crate::keymap::KeyAction::NextInboxItem
            .keys(crate::keymap::Platform::current())
            .unwrap_or_default();
        let workspace = self.workspace.clone();
        Some(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.))
                .px(px(14.))
                .h(px(32.))
                .bg(if done { c.st_approved_bg } else { c.st_attn_bg })
                .border_b_1()
                .border_color(c.border)
                .child(
                    icon(
                        if done {
                            IconName::Check
                        } else {
                            IconName::Inbox
                        },
                        px(13.),
                    )
                    .text_color(tone),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(tone)
                        .child(SharedString::from(bar_title(&bar))),
                )
                .child(div().flex().gap(px(3.)).children(segments))
                .child(div().flex_1())
                .when(done, |d| {
                    d.child(
                        div()
                            .text_size(px(11.))
                            .text_color(c.text_muted)
                            .child("next:"),
                    )
                })
                .when(!done, |d| {
                    d.child(
                        div()
                            .id("run-next")
                            .cursor_pointer()
                            .text_size(px(12.))
                            .text_color(c.text_secondary)
                            .child("Next")
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(PanelEvent::NextInboxItem)),
                            ),
                    )
                })
                .child(crate::components::kbd::Kbd::text(next_key))
                .child(
                    div()
                        .id("run-exit")
                        .cursor_pointer()
                        .text_size(px(12.))
                        .text_color(c.text_muted)
                        .child("Exit")
                        .on_click(move |_, _, cx| {
                            workspace.update(cx, |ws, cx| ws.exit_inbox_run(cx));
                        }),
                )
                .into_any_element(),
        )
    }
}

impl Render for SidePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let Some(ticket) = self.ticket(cx) else {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(c.bg_panel)
                .text_color(c.text_muted)
                .child(SharedString::from(format!("{} is gone.", self.key)))
                .into_any_element();
        };
        self.load_before_restart(&ticket, cx);
        self.sync_answer(&ticket);
        let answer = self.answer_section(&ticket, cx);
        let prompt = self.prompt.clone().map(|p| self.prompt_dialog(&p, cx));
        let send_back = self.send_back_dialog(&ticket, cx);
        let approve = self.approve_dialog(&ticket, cx);
        let lifecycle = self.lifecycle_dialog(&ticket, cx);
        let remove_dependency = self.remove_dependency_dialog(cx);
        div()
            .id("side-panel")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(c.bg_panel)
            .children(self.run_bar(cx))
            .child(self.header(&ticket, cx))
            .children(answer)
            .children(self.queued_notice(&ticket, cx))
            .children(self.conflict_strip(&ticket, cx))
            .children(self.epic_section(&ticket, cx))
            .child(self.stack_section(&ticket, cx))
            .child(self.tabs(&ticket, cx))
            .children(self.start_interrupted(&ticket, cx))
            .child(self.body(cx))
            .children(self.menu.as_ref().map(|(menu, position)| {
                gpui::deferred(
                    gpui::anchored()
                        .position(*position)
                        .snap_to_window_with_margin(px(8.))
                        .child(menu.clone()),
                )
                .with_priority(1)
            }))
            .children(send_back)
            .children(approve)
            .children(lifecycle)
            .children(remove_dependency)
            .children(prompt)
            .into_any_element()
    }
}
