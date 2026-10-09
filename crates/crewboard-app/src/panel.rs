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
    AnyElement, App, ClipboardItem, Context, Entity, EventEmitter, FontWeight, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, div, prelude::*, px,
};

use crate::components::button::IconButton;
use crate::components::menu::Tooltip;
use crate::components::pill::StatusPill;
use crate::components::tabs::{TabIndicator, TabItem, Tabs};
use crate::icons::{IconName, icon};
use crate::keymap::{KeyAction, Platform};
use crate::service_link::{ServiceEvent, ServiceLink};
use crate::terminal::{TerminalEvent, TerminalView};
use crate::theme::{ActiveTheme, FONT_MONO, text};
use crate::ui::TextStyleExt;
use crate::view_models::card::{age_label, tokens_label};
use crate::view_models::panel::{self as vm, Indicator, Tab};
use crate::workspace::Workspace;

/// What the panel asks of its host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelEvent {
    Close,
    ToggleExpanded,
    /// Show this ticket instead (⌘[ / ⌘], a Stack line).
    Show(TicketKey),
    /// The keyboard should go back to the panel (the terminal was left).
    TakeFocus,
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
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&link, |panel, _, event: &ServiceEvent, cx| match event {
            ServiceEvent::Event(Event::SessionState { session })
                if session.ticket.as_ref() == Some(&panel.key) =>
            {
                panel.load_sessions(cx)
            }
            ServiceEvent::Resync => panel.load_sessions(cx),
            _ => {}
        })
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
        let font_size = self
            .workspace
            .read(cx)
            .global_settings
            .as_ref()
            .map_or(13., |s| s.terminal_font_size as f32);
        let link = self.link.clone();
        let terminal = cx.new(|cx| TerminalView::new(link, key, font_size, window, cx));
        cx.subscribe(&terminal, |_, _, event: &TerminalEvent, cx| match event {
            TerminalEvent::LeftFocus => cx.emit(PanelEvent::TakeFocus),
        })
        .detach();
        self.terminal = Some(terminal);
        self.load_sessions(cx);
        cx.notify();
    }

    pub fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
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
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(icon(IconName::Activity, px(11.)).text_color(c.text_muted))
                .child(mono_meta(age_label(now - at), c.text_muted))
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
            .child(StatusPill::of(ticket.status))
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

    fn tabs(&self, ticket: &Ticket, cx: &mut Context<Self>) -> AnyElement {
        let platform = Platform::current();
        let items: Vec<TabItem> = vm::indicators(ticket, &self.sessions)
            .into_iter()
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
            && let Some(terminal) = &self.terminal
        {
            return div()
                .flex_1()
                .min_h_0()
                .child(terminal.clone())
                .into_any_element();
        }
        let message = self
            .tab
            .later()
            .unwrap_or("This tab is filled in by the next tickets of this milestone.");
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

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
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
        div()
            .id("side-panel")
            .size_full()
            .flex()
            .flex_col()
            .bg(c.bg_panel)
            .child(self.header(&ticket, cx))
            .child(self.tabs(&ticket, cx))
            .child(self.body(cx))
            .into_any_element()
    }
}
