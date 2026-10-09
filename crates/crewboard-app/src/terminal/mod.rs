//! The Terminal tab's terminal: a ticket's agent session, emulated with `alacritty_terminal` and drawn with GPUI as
//! Zed does (SES-09, d0001). It is a real terminal grid, never re-rendered as chat bubbles.
//!
//! The view attaches to the ticket's session through the service (stored scrollback first, then live output),
//! sends typed keys through the service's relay, follows the panel's size, and keeps the scrollback visible with
//! input off while the service is unreachable (SVC-04). After a reconnect it redraws from the service's stored
//! scrollback (SVC-05).

pub mod colors;
pub mod element;
pub mod keys;
pub mod model;

use alacritty_terminal::index::{Column, Line, Point as GridPoint, Side};
use alacritty_terminal::term::TermMode;
use crewboard_core::domain::TicketKey;
use crewboard_core::protocol::{
    Event, FocusParams, InputParams, Reply, Request, ResizeParams, SessionIdParams, SessionTarget,
    decode_bytes, encode_bytes,
};
use crewboard_core::session::{SessionKind, SessionState, TerminalSize};
use gpui::{
    App, ClipboardItem, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding,
    MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Point, Render, ScrollDelta,
    ScrollWheelEvent, SharedString, Styled, Window, actions, div, prelude::*, px,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::components::kbd::Kbd;
use crate::components::text_input::{InputEvent, TextInput};
use crate::icons::{IconName, icon};
use crate::keymap::{KeyAction, Platform};
use crate::service_link::{ServiceEvent, ServiceLink};
use crate::theme::{ActiveTheme, text};
use crate::ui::TextStyleExt;
use crate::view_models::clock;
use crate::view_models::connection::LinkState;
use element::TerminalElement;
use model::{Phase, TerminalModel};

actions!(
    terminal,
    [
        CopySelection,
        PasteText,
        OpenFind,
        FontBigger,
        FontSmaller,
        LeaveTerminal
    ]
);

/// What a terminal tells its host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    /// The user left the terminal (⇧⌘`, the inbox shortcut, a click outside): the host takes the keyboard.
    LeftFocus,
    /// "Instructions: shop-web" was clicked (d0040).
    OpenInstructions,
}

impl gpui::EventEmitter<TerminalEvent> for TerminalView {}

/// The terminal whose grid has keyboard focus, for the key router.
struct FocusedTerminal(Option<gpui::WeakEntity<TerminalView>>);

impl gpui::Global for FocusedTerminal {}

/// While a terminal has focus every key goes to Claude Code, Esc included, before any app binding can take it
/// (SES-10); the routing itself is [`crate::keymap::route`], one table for every platform. ⌘ shortcuts stay
/// the app's. Install once at start-up.
pub fn install_key_router(cx: &mut App) {
    cx.set_global(FocusedTerminal(None));
    cx.intercept_keystrokes(|event, window, cx| {
        let in_terminal = event
            .context_stack
            .last()
            .is_some_and(|c| c.contains(TERMINAL_CONTEXT));
        if !in_terminal {
            return;
        }
        let platform = crate::keymap::Platform::current();
        if crate::keymap::route(platform, true, &event.keystroke.modifiers)
            != crate::keymap::KeyRoute::Terminal
        {
            return;
        }
        let Some(view) = cx
            .try_global::<FocusedTerminal>()
            .and_then(|f| f.0.clone())
            .and_then(|w| w.upgrade())
        else {
            return;
        };
        if !view.read(cx).focus.is_focused(window) {
            return;
        }
        let keystroke = event.keystroke.clone();
        if view.update(cx, |view, cx| view.handle_keystroke(&keystroke, cx)) {
            cx.stop_propagation();
        }
    })
    .detach();
}

/// The key context while a terminal has focus.
pub const TERMINAL_CONTEXT: &str = "Terminal";

/// The terminal's own ⌘ keys (d0058: ⌘F finds; ⌘+ / ⌘− change the font size, per the settings frame).
pub fn bind_keys(cx: &mut App) {
    let t = Some(TERMINAL_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("secondary-c", CopySelection, t),
        KeyBinding::new("secondary-v", PasteText, t),
        KeyBinding::new("secondary-f", OpenFind, t),
        KeyBinding::new("secondary-=", FontBigger, t),
        KeyBinding::new("secondary-+", FontBigger, t),
        KeyBinding::new("secondary--", FontSmaller, t),
        KeyBinding::new("secondary-shift-`", LeaveTerminal, t),
        // The inbox shortcut leaves the terminal too; T-0119 also opens the next item with it.
        KeyBinding::new("secondary-j", LeaveTerminal, t),
    ]);
}

/// Where the grid is drawn, measured by the element, for mouse positions.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    pub origin: Point<Pixels>,
    pub cell_width: Pixels,
    pub line_height: Pixels,
}

/// The font sizes a terminal accepts (Settings → Appearance, T-0045).
const FONT_SIZES: std::ops::RangeInclusive<f32> = 8.0..=32.0;

pub struct TerminalView {
    link: Entity<ServiceLink>,
    ticket: TicketKey,
    pub model: TerminalModel,
    focus: FocusHandle,
    /// Points; the default comes from Settings → Appearance (T-0045).
    pub font_size: f32,
    /// Keystrokes and resizes, sent in order.
    sender: Option<UnboundedSender<Request>>,
    /// Only the newest attach answer counts.
    attach_generation: u64,
    connected: bool,
    pub geometry: Option<Geometry>,
    /// Where a drag started, until it moves (a plain click selects nothing).
    drag_from: Option<(GridPoint, Side)>,
    /// Wheel movement not yet a whole line.
    scroll_rest: f32,
    find_input: Option<Entity<TextInput>>,
    /// "Instructions: shop-web", shown at the end of the strip under the terminal (set by the host).
    pub instructions: Option<String>,
}

impl TerminalView {
    pub fn new(
        link: Entity<ServiceLink>,
        ticket: TicketKey,
        font_size: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        cx.on_focus(&focus, window, |view, _, cx| {
            let me = cx.entity().downgrade();
            cx.set_global(FocusedTerminal(Some(me)));
            view.report_focus(true);
            cx.notify();
        })
        .detach();
        cx.on_blur(&focus, window, |view, _, cx| {
            view.report_focus(false);
            cx.notify();
        })
        .detach();
        cx.subscribe(&link, |view, _, event: &ServiceEvent, cx| {
            view.on_service(event, cx)
        })
        .detach();
        cx.observe(&link, |view, link, cx| {
            let connected = matches!(link.read(cx).state, LinkState::Connected(_));
            if connected != view.connected {
                view.connected = connected;
                if connected {
                    view.attach(cx);
                } else {
                    view.sender = None;
                    view.model.disconnected();
                }
                cx.notify();
            }
        })
        .detach();
        cx.on_release(|view: &mut Self, cx: &mut App| view.detach(cx))
            .detach();
        let connected = matches!(link.read(cx).state, LinkState::Connected(_));
        let mut view = Self {
            link,
            ticket,
            model: TerminalModel::new(TerminalSize { cols: 80, rows: 24 }),
            focus,
            font_size,
            sender: None,
            attach_generation: 0,
            connected,
            geometry: None,
            drag_from: None,
            scroll_rest: 0.,
            find_input: None,
            instructions: None,
        };
        if connected {
            view.attach(cx);
        }
        view
    }

    /// Asks the service for the ticket's session: its stored output, then live output on this connection.
    pub fn attach(&mut self, cx: &mut Context<Self>) {
        self.attach_generation += 1;
        let generation = self.attach_generation;
        self.model.attaching();
        let link = self.link.read(cx);
        self.sender = link.ordered_sender();
        let request = link.send(Request::SessionAttach(SessionTarget {
            session: None,
            key: Some(self.ticket.to_string()),
        }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |view, cx| {
                if view.attach_generation != generation {
                    return;
                }
                match result {
                    Ok(Reply::SessionAttached(attached)) => {
                        let bytes = decode_bytes(&attached.scrollback).unwrap_or_default();
                        view.model.attach(attached.session, &bytes, attached.end);
                        // The PTY follows this terminal's size.
                        view.send_resize(view.model.size());
                    }
                    Err(e) if e.not_found => view.model.no_session(),
                    Ok(_) | Err(_) => view.model.disconnected(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn detach(&mut self, cx: &mut App) {
        if let Some(session) = self.session_id() {
            let request = self
                .link
                .read(cx)
                .send(Request::SessionDetach(SessionIdParams { session }));
            cx.background_executor()
                .spawn(async move {
                    let _ = request.await;
                })
                .detach();
        }
    }

    fn session_id(&self) -> Option<String> {
        self.model.session.as_ref().map(|s| s.id.clone())
    }

    fn on_service(&mut self, event: &ServiceEvent, cx: &mut Context<Self>) {
        match event {
            ServiceEvent::Event(Event::SessionOutput {
                session,
                offset,
                data,
            }) if Some(session) == self.session_id().as_ref() => {
                let bytes = decode_bytes(data).unwrap_or_default();
                let fed = self.model.output(*offset, &bytes);
                if !fed.replies.is_empty() {
                    self.send(Request::SessionInput(InputParams {
                        session: session.clone(),
                        data: encode_bytes(&fed.replies),
                        reply: true,
                    }));
                }
                if self.model.needs_reattach {
                    self.attach(cx);
                }
                if fed.changed {
                    cx.notify();
                }
            }
            ServiceEvent::Event(Event::SessionState { session })
                if session.kind == SessionKind::Agent
                    && session.ticket.as_ref() == Some(&self.ticket) =>
            {
                if session.state == SessionState::Ended {
                    self.model.ended(session.clone());
                } else if self.session_id().as_ref() != Some(&session.id) {
                    // A new session for this ticket (a start or a resume): follow it.
                    self.attach(cx);
                }
                cx.notify();
            }
            ServiceEvent::Resync => self.attach(cx),
            ServiceEvent::Event(_) => {}
        }
    }

    fn send(&self, request: Request) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(request);
        }
    }

    fn send_resize(&self, size: TerminalSize) {
        if let (Some(session), Phase::Live) = (self.session_id(), &self.model.phase) {
            self.send(Request::SessionResize(ResizeParams { session, size }));
        }
    }

    /// The element measured how many cells fit; the grid and the PTY follow.
    pub fn fit(&mut self, size: TerminalSize, _cx: &mut Context<Self>) {
        if self.model.resize(size) {
            self.send_resize(size);
        }
    }

    pub fn cursor_visible(&self) -> bool {
        self.model.phase == Phase::Live
    }

    /// Sends what a person typed (relayed by the service, which tracks unsent text, d0048).
    pub fn type_bytes(&mut self, bytes: &[u8]) {
        if !self.model.input_enabled() || bytes.is_empty() {
            return;
        }
        if let Some(session) = self.session_id() {
            // Typing scrolls back to the newest output.
            self.model
                .term
                .scroll_display(alacritty_terminal::grid::Scroll::Bottom);
            self.send(Request::SessionInput(InputParams {
                session,
                data: encode_bytes(bytes),
                reply: false,
            }));
        }
    }

    /// A keystroke routed to this terminal (see [`install_key_router`]). `true` when it became bytes.
    fn handle_keystroke(&mut self, keystroke: &gpui::Keystroke, cx: &mut Context<Self>) -> bool {
        match keys::to_bytes(keystroke, self.model.mode()) {
            Some(bytes) => {
                self.type_bytes(&bytes);
                cx.notify();
                true
            }
            None => false,
        }
    }

    fn report_focus(&self, focused: bool) {
        if let (Some(session), Phase::Live) = (self.session_id(), &self.model.phase) {
            self.send(Request::SessionFocus(FocusParams { session, focused }));
        }
    }

    /// ⇧⌘` (and the inbox shortcut): the keyboard goes back to the app.
    fn leave(&mut self, _: &LeaveTerminal, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_focus(window, cx);
    }

    fn leave_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus.is_focused(window) {
            window.blur(cx);
            cx.emit(TerminalEvent::LeftFocus);
            cx.notify();
        }
    }

    fn hint(&self) -> Option<(IconName, String)> {
        let up = self.model.scrolled_up();
        if up > 0 && matches!(self.model.phase, Phase::Live | Phase::Ended) {
            return Some((
                IconName::ArrowUp,
                format!("Scrolled up {} lines", thousands(up)),
            ));
        }
        match self.model.phase {
            Phase::Reconnecting => Some((
                IconName::RefreshCw,
                "Input paused while reconnecting · output will catch up".into(),
            )),
            Phase::Ended => Some((IconName::Lock, "Read-only · the session ended".into())),
            Phase::Live => Some((
                IconName::MousePointerClick,
                "Click the terminal to type to the agent".into(),
            )),
            Phase::Attaching | Phase::NoSession => None,
        }
    }

    /// "Session ended · 11:02 · 41 min" and why.
    fn end_marker(&self) -> Option<(String, String)> {
        let session = self.model.session.as_ref()?;
        let ended_at = session.ended_at?;
        let title = format!(
            "Session ended · {} · {}",
            clock::local_hhmm(ended_at),
            clock::duration(ended_at - session.started_at)
        );
        let reason = match session.end_reason.as_deref() {
            Some("exit 0") | None => "The agent's session finished.".to_owned(),
            Some(reason) => format!("The agent's session ended ({reason})."),
        };
        Some((title, reason))
    }
}

impl TerminalView {
    /// The grid cell under a window position, and which half of it.
    fn grid_point(&self, position: Point<Pixels>) -> Option<(GridPoint, Side)> {
        let g = self.geometry?;
        let x = f32::from(position.x - g.origin.x).max(0.);
        let y = f32::from(position.y - g.origin.y).max(0.);
        let (cw, lh) = (f32::from(g.cell_width), f32::from(g.line_height));
        let size = self.model.size();
        let column = ((x / cw) as usize).min(usize::from(size.cols).saturating_sub(1));
        let row = ((y / lh) as i32).min(i32::from(size.rows) - 1);
        let line = row - self.model.scrolled_up() as i32;
        let side = if x % cw < cw / 2. {
            Side::Left
        } else {
            Side::Right
        };
        Some((GridPoint::new(Line(line), Column(column)), side))
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        if let Some((point, side)) = self.grid_point(event.position) {
            if event.click_count <= 1 {
                self.model.clear_selection();
                self.drag_from = Some((point, side));
            } else {
                self.drag_from = None;
                self.model.select_start(point, side, event.click_count);
            }
        }
        cx.notify();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let Some((point, side)) = self.grid_point(event.position) else {
            return;
        };
        if let Some((from, from_side)) = self.drag_from.take() {
            self.model.select_start(from, from_side, 1);
        }
        self.model.select_to(point, side);
        cx.notify();
    }

    fn on_scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let line_height = self.geometry.map_or(18., |g| f32::from(g.line_height));
        let lines = match event.delta {
            ScrollDelta::Lines(d) => d.y,
            ScrollDelta::Pixels(d) => f32::from(d.y) / line_height,
        } + self.scroll_rest;
        let whole = lines.trunc();
        self.scroll_rest = lines - whole;
        if whole == 0. {
            return;
        }
        let mode = self.model.mode();
        if mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
            // Full-screen programs scroll themselves: send arrow keys, as other terminals do.
            let key: &[u8] = if whole > 0. { b"\x1b[A" } else { b"\x1b[B" };
            let bytes: Vec<u8> = (0..whole.abs() as usize)
                .flat_map(|_| key.iter().copied())
                .collect();
            self.type_bytes(&bytes);
        } else {
            self.model.scroll(whole as i32);
        }
        cx.notify();
    }

    fn copy(&mut self, _: &CopySelection, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.model.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn paste(&mut self, _: &PasteText, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
            let bytes = keys::paste(&text, self.model.mode());
            self.type_bytes(&bytes);
            cx.notify();
        }
    }

    fn font_bigger(&mut self, _: &FontBigger, _: &mut Window, cx: &mut Context<Self>) {
        self.set_font_size(self.font_size + 1., cx);
    }

    fn font_smaller(&mut self, _: &FontSmaller, _: &mut Window, cx: &mut Context<Self>) {
        self.set_font_size(self.font_size - 1., cx);
    }

    /// This terminal's font size (the setting is only the default for new terminals).
    pub fn set_font_size(&mut self, size: f32, cx: &mut Context<Self>) {
        self.font_size = size.clamp(*FONT_SIZES.start(), *FONT_SIZES.end());
        cx.notify();
    }

    fn open_find(&mut self, _: &OpenFind, window: &mut Window, cx: &mut Context<Self>) {
        let input = match &self.find_input {
            Some(input) => input.clone(),
            None => {
                let input = cx.new(|cx| {
                    TextInput::new(window, cx)
                        .placeholder("Find in scrollback")
                        .bare()
                });
                cx.subscribe_in(
                    &input,
                    window,
                    |view, input, event: &InputEvent, window, cx| {
                        match event {
                            InputEvent::Changed => {
                                let query = input.read(cx).text().to_owned();
                                view.model.set_query(&query);
                            }
                            InputEvent::Submit => view.model.step_find(true),
                            InputEvent::AltSubmit => view.model.step_find(false),
                            InputEvent::Cancel => {
                                view.model.close_find();
                                window.focus(&view.focus, cx);
                            }
                            _ => {}
                        }
                        cx.notify();
                    },
                )
                .detach();
                self.find_input = Some(input.clone());
                input
            }
        };
        self.model.open_find();
        window.focus(&input.read(cx).focus_handle(cx), cx);
        cx.notify();
    }

    /// Find from outside the terminal (the strip's search button).
    pub fn find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_find(&OpenFind, window, cx);
    }

    /// The program's input line as the user sees it (for the unsent-text prompt).
    pub fn typed_line(&self) -> Option<String> {
        self.model.cursor_line_text()
    }

    pub fn phase(&self) -> &Phase {
        &self.model.phase
    }

    fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model.close_find();
        window.focus(&self.focus, cx);
        cx.notify();
    }
}

/// 1320 → "1,320".
pub fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(d);
    }
    out
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let focused = self.focus.is_focused(window);
        let phase = self.model.phase.clone();
        let entity = cx.entity();
        let grid = div()
            .id("terminal-grid")
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _, _, _| view.drag_from = None),
            )
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .size_full()
            .bg(c.term_bg)
            .border_1()
            .border_color(if focused { c.accent } else { c.border })
            .rounded(px(6.))
            .px(px(14.))
            .py(px(12.))
            .when(phase == Phase::Reconnecting, |d| d.opacity(0.55))
            .child(TerminalElement::new(entity));

        let overlay = match &phase {
            Phase::Reconnecting => Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .px(px(12.))
                            .py(px(8.))
                            .bg(c.bg_raised)
                            .border_1()
                            .border_color(c.border_strong)
                            .rounded(px(6.))
                            .text_style_of(text::METADATA)
                            .text_size(px(12.))
                            .text_color(c.text_secondary)
                            .child(icon(IconName::RefreshCw, px(13.)).text_color(c.text_secondary))
                            .child("Reconnecting… agents keep running"),
                    )
                    .into_any_element(),
            ),
            Phase::NoSession => Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_style_of(text::METADATA)
                    .text_size(px(12.))
                    .text_color(c.text_muted)
                    .child(SharedString::from(format!(
                        "{} has no agent session yet. It starts when the ticket goes to Dev.",
                        self.ticket
                    )))
                    .into_any_element(),
            ),
            Phase::Ended => self.end_marker().map(|(title, reason)| {
                div()
                    .absolute()
                    .left(px(14.))
                    .right(px(14.))
                    .bottom(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(10.))
                    .bg(c.bg_column)
                    .border_1()
                    .border_color(c.text_secondary)
                    .rounded(px(6.))
                    .child(icon(IconName::CircleCheck, px(15.)).text_color(c.text_secondary))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_style_of(text::METADATA)
                                    .text_size(px(12.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(c.text_secondary)
                                    .child(SharedString::from(title)),
                            )
                            .child(
                                div()
                                    .text_style_of(text::METADATA)
                                    .text_color(c.text_secondary)
                                    .child(SharedString::from(reason)),
                            ),
                    )
                    .into_any_element()
            }),
            Phase::Live | Phase::Attaching => None,
        };

        // Scrolled up: where we are, and a pill back to the newest output.
        let up = self.model.scrolled_up();
        let position = (up > 0).then(|| {
            let (first, total) = self.model.position();
            div()
                .absolute()
                .top(px(4.))
                .left(px(14.))
                .font_family(crate::theme::FONT_MONO)
                .text_size(px(11.))
                .text_color(c.text_muted)
                .child(SharedString::from(format!(
                    "… line {} of {}",
                    thousands(first),
                    thousands(total)
                )))
        });
        let new_lines = self.model.new_lines;
        let jump = (up > 0).then(|| {
            let label = if new_lines > 0 {
                format!("{} new lines", thousands(new_lines))
            } else {
                "Jump to latest".to_owned()
            };
            div()
                .id("jump-to-latest")
                .absolute()
                .bottom(px(12.))
                .right(px(24.))
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(10.))
                .py(px(5.))
                .rounded(px(14.))
                .bg(c.accent)
                .text_color(c.text_on_accent)
                .text_style_of(text::METADATA)
                .cursor_pointer()
                .on_click(cx.listener(|view, _, _, cx| {
                    view.model.jump_to_latest();
                    cx.notify();
                }))
                .child(icon(IconName::ArrowDown, px(12.)).text_color(c.text_on_accent))
                .child(SharedString::from(label))
        });

        let find_bar =
            self.find_input
                .clone()
                .filter(|_| self.model.find.open)
                .map(|input| {
                    let label = self.model.find.label();
                    let button = |id: &'static str, name: IconName| {
                        div()
                            .id(id)
                            .size(px(22.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|d| d.bg(c.bg_hover))
                            .child(icon(name, px(13.)).text_color(c.text_secondary))
                    };
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .h(px(30.))
                        .pl(px(10.))
                        .pr(px(6.))
                        .bg(c.bg_raised)
                        .border_1()
                        .border_color(c.accent)
                        .rounded(px(6.))
                        .child(icon(IconName::Search, px(13.)).text_color(c.text_muted))
                        .child(div().flex_1().min_w_0().child(input))
                        .child(
                            div()
                                .text_style_of(text::METADATA)
                                .text_color(c.text_muted)
                                .child(SharedString::from(label)),
                        )
                        .child(
                            button("find-previous", IconName::ChevronUp).on_click(cx.listener(
                                |view, _, _, cx| {
                                    view.model.step_find(true);
                                    cx.notify();
                                },
                            )),
                        )
                        .child(
                            button("find-next", IconName::ChevronDown).on_click(cx.listener(
                                |view, _, _, cx| {
                                    view.model.step_find(false);
                                    cx.notify();
                                },
                            )),
                        )
                        .child(button("find-close", IconName::X).on_click(
                            cx.listener(|view, _, window, cx| view.close_find(window, cx)),
                        ))
                });

        let typing = focused && self.model.phase == Phase::Live && self.model.scrolled_up() == 0;
        let hint = if typing {
            // The focus ring's hint (*Answering a question · focused*).
            let leave = KeyAction::LeaveTerminal
                .keys(Platform::current())
                .unwrap_or_default();
            Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(4.))
                    .text_style_of(text::METADATA)
                    .child(icon(IconName::Keyboard, px(13.)).text_color(c.accent))
                    .child(div().text_color(c.accent).child("Typing goes to the agent"))
                    .child(div().flex_1())
                    .child(Kbd::text(leave))
                    .child(div().text_color(c.text_muted).child("leave terminal")),
            )
        } else {
            self.hint().map(|(name, label)| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(4.))
                    .text_style_of(text::METADATA)
                    .text_color(c.text_muted)
                    .child(icon(name, px(13.)).text_color(c.text_muted))
                    .child(SharedString::from(label))
            })
        };

        let instructions = self.instructions.clone().map(|label| {
            div()
                .id("terminal-instructions")
                .flex()
                .items_center()
                .gap(px(4.))
                .cursor_pointer()
                .text_style_of(text::METADATA)
                .text_color(c.accent)
                .on_click(cx.listener(|_, _, _, cx| cx.emit(TerminalEvent::OpenInstructions)))
                .child(icon(IconName::Info, px(11.)).text_color(c.accent))
                .child(SharedString::from(label))
        });
        let footer = (hint.is_some() || instructions.is_some()).then(|| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().flex_1().min_w_0().children(hint))
                .children(instructions)
        });

        div()
            .id("terminal-well")
            .track_focus(&self.focus)
            .key_context("Terminal")
            .on_action(cx.listener(Self::leave))
            .on_mouse_down_out(cx.listener(|view, _, window, cx| view.leave_focus(window, cx)))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::open_find))
            .on_action(cx.listener(Self::font_bigger))
            .on_action(cx.listener(Self::font_smaller))
            .size_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(8.))
            .children(find_bar)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(grid)
                    .children(position)
                    .children(overlay)
                    .children(jump),
            )
            .children(footer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(142), "142");
        assert_eq!(thousands(1320), "1,320");
        assert_eq!(thousands(2_140_000), "2,140,000");
    }
}
