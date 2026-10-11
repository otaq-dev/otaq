//! The board (UI-02; *03 · Board — shop-web · 1440 · Light / Dark → Body → Board*, *03 · Board & columns →
//! Empty column & empty board, Very full column · 14 cards, Width strategy*): five columns (Dev with its
//! needs-you cards on top, Agent Working, then Queued), each scrolling on its own with its header pinned and a
//! "N more" chip when there is more below; Dev's fold; Done as a 34 px rail; a fade and a chip naming the columns
//! out of view when the board scrolls sideways; epic cards in Backlog. It draws
//! [`crate::view_models::board`] from the workspace, which updates cards in place from the service's events.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crewboard_core::board::epics::ClosePlan;
use crewboard_core::board::tickets::{NewTicketInput, Ticket};
use crewboard_core::domain::{Column, Priority, TicketKey};
use crewboard_core::protocol::{KeyParams, MoveParams, MoveToEpicParams, Reply, Request};
use gpui::{
    AnyElement, App, Bounds, Context, DismissEvent, DragMoveEvent, Entity, EventEmitter,
    FocusHandle, Focusable, FontWeight, Hsla, IntoElement, MouseButton, PathBuilder, Pixels, Point,
    ScrollHandle, SharedString, Window, canvas, div, linear_color_stop, linear_gradient, point,
    prelude::*, px,
};

use crate::components::card::{ChainState, TicketCard};
use crate::components::dialog::ConfirmDialog;
use crate::components::epic_card::EpicCard;
use crate::components::menu::{ContextMenu, MenuChosen};
use crate::components::priority::PriorityDropdown;
use crate::components::text_input::{InputEvent, TextInput};
use crate::delete_dialog::{DeleteDialog, DeleteOutcome};
use crate::icons::{IconName, icon};
use crate::shell::ShellRequest;
use crate::theme::{ActiveTheme, Colors, FONT_MONO, density, radius};
use crate::view_models::board::{
    BoardInput, CardEntry, ColumnView, DEV_SECTION, DevSections, HiddenColumns, build, dev_budget,
    done_collapsed, hidden_columns,
};
use crate::view_models::chain::{Chain, chain};
use crate::view_models::dialog::DialogKind;
use crate::view_models::drag::{DropAction, DropFeedback, feedback, on_drop};
use crate::view_models::{card_menu, epic_actions, priority};
use crate::workspace::Workspace;

/// A card being dragged: the ticket (for the rules) and how it looks.
#[derive(Clone)]
pub struct DraggedCard {
    pub ticket: Ticket,
    pub entry: CardEntry,
    pub width: Pixels,
}

/// The lifted card under the pointer.
struct DragPreview {
    card: DraggedCard,
}

impl Render for DragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let id = SharedString::from(format!("drag-{}", self.card.ticket.key));
        let card = match &self.card.entry {
            CardEntry::Ticket(vm) => TicketCard::new(id, vm.clone())
                .dragging(true)
                .into_any_element(),
            CardEntry::Epic(vm) => EpicCard::new(id, vm.clone())
                .dragging(true)
                .into_any_element(),
        };
        div().w(self.card.width).opacity(0.95).child(card)
    }
}

/// The epic dialogs (T-0043).
#[derive(Clone)]
enum EpicDialog {
    /// Move to epic…: the ticket and the chosen epic (an index into the choices).
    Move { ticket: Ticket, choice: usize },
    /// Close epic…: the epic and the service's plan.
    Close { epic: Ticket, plan: ClosePlan },
}

/// Bounds recorded while laying out the board.
#[derive(Default)]
struct Layout {
    cards: HashMap<TicketKey, Bounds<Pixels>>,
    columns: Vec<Bounds<Pixels>>,
}

/// The board's width as of the last frame (`None` before the first layout).
fn board_width(scroll: &ScrollHandle) -> Option<f32> {
    let width = f32::from(scroll.bounds().size.width);
    (width > 0.).then_some(width)
}

/// The columns out of the board's view, from the last frame's layout and the current scroll.
fn hidden_in(scroll: &ScrollHandle, titles: &[&str]) -> HiddenColumns {
    let view = scroll.bounds();
    let shift = f32::from(scroll.offset().x);
    let spans: Vec<(&str, f32, f32)> = titles
        .iter()
        .enumerate()
        .filter_map(|(i, title)| {
            let b = scroll.bounds_for_item(i)?;
            Some((*title, f32::from(b.left()), f32::from(b.right())))
        })
        .collect();
    hidden_columns(
        &spans,
        (
            f32::from(view.left()) - shift,
            f32::from(view.right()) - shift,
        ),
    )
}

/// The pill that says what is out of view ("6 more", "Approved · Done") with a chevron towards it (*03 · Board &
/// columns → Very full column · 14 cards*).
fn more_chip(label: String, icon_name: IconName, icon_first: bool, c: &Colors) -> gpui::Div {
    let label = SharedString::from(label);
    let glyph = icon(icon_name, px(12.)).text_color(c.text_secondary);
    let chip = div()
        .flex()
        .items_center()
        .gap(px(4.))
        .px(px(8.))
        .py(px(3.))
        .rounded(px(12.))
        .bg(c.bg_raised)
        .border_1()
        .border_color(c.border_strong)
        .text_size(px(11.))
        .text_color(c.text_secondary);
    if icon_first {
        chip.child(glyph).child(label)
    } else {
        chip.child(label).child(glyph)
    }
}

/// The fade and the chip naming the columns past one edge of the board (*02 · App shell & project switcher →
/// 1280 × 800 · minimum*), drawn like a very full column's: an 80 px fade into the background, solid from 60 %,
/// and the chip 12 px in from the edge. It only shows; scrolling stays with the board.
fn edge_hint(label: String, after: bool, c: &Colors) -> gpui::Div {
    let (angle, chevron) = if after {
        (90., IconName::ChevronRight)
    } else {
        (270., IconName::ChevronLeft)
    };
    let hint = div()
        .absolute()
        .top_0()
        .bottom_0()
        .w(px(80.))
        .flex()
        .items_center()
        .bg(linear_gradient(
            angle,
            linear_color_stop(c.bg_app.opacity(0.), 0.),
            linear_color_stop(c.bg_app, 0.6),
        ));
    let hint = if after {
        hint.right_0().justify_end().pr(px(12.))
    } else {
        hint.left_0().justify_start().pl(px(12.))
    };
    hint.child(more_chip(label, chevron, !after, c).flex_none())
}

/// An invisible element that records its own bounds (it fills its parent).
fn record_bounds(record: impl Fn(Bounds<Pixels>) + 'static) -> impl IntoElement {
    canvas(move |bounds, _, _| record(bounds), |_, _, _, _| {})
        .absolute()
        .top_0()
        .left_0()
        .size_full()
}

/// Draws one arrow from a parent card to a child card (parent → child, d0006).
fn paint_arrow(from: Bounds<Pixels>, to: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    let (start, end, ctrl_a, ctrl_b);
    if to.center().x > from.right() {
        start = point(from.right(), from.center().y);
        end = point(to.left(), to.center().y);
        let dx = (end.x - start.x) / 2.;
        (ctrl_a, ctrl_b) = (point(start.x + dx, start.y), point(end.x - dx, end.y));
    } else if to.center().x < from.left() {
        start = point(from.left(), from.center().y);
        end = point(to.right(), to.center().y);
        let dx = (start.x - end.x) / 2.;
        (ctrl_a, ctrl_b) = (point(start.x - dx, start.y), point(end.x + dx, end.y));
    } else {
        // Same column: around the left edge.
        start = point(from.left(), from.center().y);
        end = point(to.left(), to.center().y);
        (ctrl_a, ctrl_b) = (
            point(start.x - px(26.), start.y),
            point(end.x - px(26.), end.y),
        );
    }
    let mut line = PathBuilder::stroke(px(1.5));
    line.move_to(start);
    line.cubic_bezier_to(end, ctrl_a, ctrl_b);
    if let Ok(path) = line.build() {
        window.paint_path(path, color);
    }
    // The head points from the last control point into the child.
    let (dx, dy) = (f32::from(end.x - ctrl_b.x), f32::from(end.y - ctrl_b.y));
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let (ux, uy) = (dx / len, dy / len);
    let at = |along: f32, across: f32| -> Point<Pixels> {
        point(
            end.x - px(ux * along) + px(-uy * across),
            end.y - px(uy * along) + px(ux * across),
        )
    };
    let mut head = PathBuilder::fill();
    head.add_polygon(&[end, at(7., 3.5), at(7., -3.5)], true);
    if let Ok(path) = head.build() {
        window.paint_path(path, color);
    }
}

/// Something the window should tell the user (a toast).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub error: bool,
    pub title: String,
    pub text: Option<String>,
}

pub struct BoardView {
    workspace: Entity<Workspace>,
    pub selected: Option<TicketKey>,
    dev_unfolded: bool,
    /// The user opened or closed the Done rail; else it follows the board's width.
    done_choice: Option<bool>,
    collapsed_epics: HashSet<TicketKey>,
    scrolls: Vec<ScrollHandle>,
    /// The board's sideways scroll; its bounds give the board's width as of the last frame.
    board_scroll: ScrollHandle,
    /// The column under a dragged card.
    drag_over: Option<Column>,
    /// The ticket being dragged (set when the drag starts; GPUI only says whether a drag is active).
    dragging: Option<Ticket>,
    /// A Dev card dropped on Backlog: the Park confirmation (T-0107).
    park_ask: Option<Ticket>,
    /// The card under the pointer; with the selection it decides the highlighted chain.
    hovered: Option<TicketKey>,
    chain: Option<Chain>,
    /// Where cards and column bodies were laid out this frame, for the chain's arrows.
    layout: Rc<RefCell<Layout>>,
    /// The open right-click menu, where it opened, and for which ticket.
    menu: Option<(Entity<ContextMenu>, Point<Pixels>, Ticket)>,
    /// The Open URL of the Run whose chip menu is open.
    run_url: Option<String>,
    /// Quick-create at the top of Backlog (⌘N), optionally creating a child of an epic (key, title).
    quick: Option<Entity<TextInput>>,
    quick_epic: Option<(TicketKey, String)>,
    /// Quick-create's optional priority (Normal unless picked, d0065) and its open menu.
    quick_priority: Option<Priority>,
    quick_priority_menu: Option<(Entity<ContextMenu>, Point<Pixels>)>,
    epic_dialog: Option<EpicDialog>,
    /// A dialog just opened from a menu: focus it at the next render (which has the window).
    focus_dialog: bool,
    /// Delete… was chosen: the dialog opens at the next render (it needs the window).
    pending_delete: Option<String>,
    delete: Option<Entity<DeleteDialog>>,
    dialog_focus: FocusHandle,
}

impl EventEmitter<ShellRequest> for BoardView {}
impl EventEmitter<Notice> for BoardView {}

/// A Review card was dropped on Approved: open the approve choice for it (T-0095).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChooseApprove(pub TicketKey);

impl EventEmitter<ChooseApprove> for BoardView {}

/// "Close ticket…" from a card's menu: the panel's close confirmation (T-0106).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChooseClose(pub TicketKey);

impl EventEmitter<ChooseClose> for BoardView {}

/// Asks for the ticket editor: the full form for a new ticket (⇧↵ in quick-create, with the title typed so
/// far), or an existing ticket (double click).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenEditor {
    New {
        title: String,
        epic: Option<TicketKey>,
        /// Picked in quick-create; `None`: Normal, or the epic's for a child.
        priority: Option<Priority>,
    },
    Ticket(TicketKey),
}

impl EventEmitter<OpenEditor> for BoardView {}

/// A button on a card was pressed (Deliver now, Retry start…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardPressed(pub TicketKey, pub crate::view_models::card::CardActionKind);

impl EventEmitter<CardPressed> for BoardView {}

/// How often ages on working cards are redrawn.
pub const TICK: std::time::Duration = std::time::Duration::from_secs(1);

/// Whether a ticket shows a time that moves on its own: a last signal or a background run on a Dev card.
pub fn ticks(ticket: &crewboard_core::board::tickets::Ticket) -> bool {
    crewboard_core::signals::LIVE.contains(&ticket.status) && ticket.flags.last_signal_at.is_some()
}

/// Asks to show a ticket in the side panel (a click on its card, or ↵ on the selected card).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenPanel(pub TicketKey);

impl EventEmitter<OpenPanel> for BoardView {}

impl BoardView {
    pub fn new(workspace: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        // Last-signal ages ("40s", "Quiet 6 min") and background times tick without refetching (ATT-05).
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let alive = this.update(cx, |board, cx| {
                    if board.workspace.read(cx).tickets.iter().any(ticks) {
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
            selected: None,
            dev_unfolded: false,
            done_choice: None,
            collapsed_epics: HashSet::new(),
            scrolls: (0..Column::ALL.len())
                .map(|_| ScrollHandle::new())
                .collect(),
            board_scroll: ScrollHandle::new(),
            drag_over: None,
            dragging: None,
            park_ask: None,
            hovered: None,
            chain: None,
            layout: Rc::default(),
            menu: None,
            run_url: None,
            quick: None,
            quick_epic: None,
            quick_priority: None,
            quick_priority_menu: None,
            epic_dialog: None,
            focus_dialog: false,
            pending_delete: None,
            delete: None,
            dialog_focus: cx.focus_handle(),
        }
    }

    fn ticket(&self, key: &TicketKey, cx: &App) -> Option<Ticket> {
        self.workspace
            .read(cx)
            .tickets
            .iter()
            .find(|t| &t.key == key)
            .cloned()
    }

    /// ⇧⌘D: start the selected ticket, as if it was dropped on Dev.
    pub fn start_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ticket) = self.selected.as_ref().and_then(|k| self.ticket(k, cx)) else {
            return;
        };
        self.drop_on(ticket, Column::Dev, window, cx);
    }

    fn drop_on(
        &mut self,
        ticket: Ticket,
        column: Column,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.drag_over = None;
        self.dragging = None;
        match on_drop(&ticket, column) {
            DropAction::Nothing => {}
            DropAction::Move => self.send_move(ticket.key.clone(), column, cx),
            DropAction::ChooseApprove => cx.emit(ChooseApprove(ticket.key.clone())),
            DropAction::AskPark => {
                self.park_ask = Some(ticket);
                window.focus(&self.dialog_focus, cx);
            }
            DropAction::NotYet(message) => cx.emit(Notice {
                error: false,
                title: message,
                text: None,
            }),
        }
        cx.notify();
    }

    fn send_move(&mut self, key: TicketKey, column: Column, cx: &mut Context<Self>) {
        let request = self
            .workspace
            .read(cx)
            .link
            .read(cx)
            .send(Request::TicketMove(MoveParams {
                key: key.to_string(),
                target: crate::view_models::drag::target_for(column),
                confirmed: false,
            }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |board, cx| match result {
                Ok(Reply::Ticket(ticket)) => {
                    board.workspace.update(cx, |ws, cx| ws.upsert(*ticket, cx));
                }
                Ok(_) => {}
                Err(e) => cx.emit(Notice {
                    error: e.refusal.as_deref() != Some("not_available_yet"),
                    title: e.message,
                    text: None,
                }),
            });
        })
        .detach();
    }

    /// ⌘N: the inline title field at the top of Backlog. ↵ creates, ⇧↵ opens the full form with the title,
    /// Esc cancels.
    pub fn start_quick_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.quick_epic = None;
        self.open_quick_create(window, cx);
    }

    /// The quick-create title field is open.
    pub fn quick_creating(&self) -> bool {
        self.quick.is_some()
    }

    /// "Add ticket to epic": quick-create a child of the epic.
    fn start_quick_child(&mut self, epic: &Ticket, window: &mut Window, cx: &mut Context<Self>) {
        self.quick_epic = Some((epic.key.clone(), epic.title.clone()));
        self.open_quick_create(window, cx);
    }

    fn open_quick_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(quick) = &self.quick {
            let handle = quick.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
            return;
        }
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Ticket title")
                .bare()
        });
        cx.subscribe(&input, |board, input, event: &InputEvent, cx| {
            let title = input.read(cx).text().trim().to_owned();
            match event {
                InputEvent::Submit if !title.is_empty() => {
                    board.quick_create(title, cx);
                    input.update(cx, |i, cx| i.set_text("", cx));
                }
                InputEvent::AltSubmit => {
                    board.quick = None;
                    let epic = board.quick_epic.take().map(|(k, _)| k);
                    let priority = board.quick_priority.take();
                    cx.emit(OpenEditor::New {
                        title,
                        epic,
                        priority,
                    });
                }
                InputEvent::Cancel => {
                    board.quick = None;
                    board.quick_epic = None;
                    board.quick_priority = None;
                }
                _ => {}
            }
            cx.notify();
        })
        .detach();
        let handle = input.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        self.quick = Some(input);
        cx.notify();
    }

    fn quick_create(&mut self, title: String, cx: &mut Context<Self>) {
        let ws = self.workspace.read(cx);
        let Some(project) = ws.current().map(|p| p.key.clone()) else {
            return;
        };
        let send = ws.link.read(cx).send(Request::TicketCreate(NewTicketInput {
            project,
            title,
            epic: self.quick_epic.as_ref().map(|(k, _)| k.to_string()),
            // Each new ticket starts unpicked again.
            priority: self.quick_priority.take(),
            ..NewTicketInput::default()
        }));
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |board, cx| match result {
                Ok(Reply::Ticket(ticket)) => {
                    board.workspace.update(cx, |ws, cx| ws.upsert(*ticket, cx));
                }
                Ok(_) => {}
                Err(e) => cx.emit(Notice {
                    error: true,
                    title: e.message,
                    text: None,
                }),
            });
        })
        .detach();
    }

    /// Quick-create's priority: the four values; the pick is kept for the next ↵ (or ⇧↵).
    fn open_quick_priority_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self.quick_shown_priority(cx);
        let menu =
            cx.new(|cx| ContextMenu::new(priority::menu_items(current), window, cx).width(180.));
        cx.subscribe(&menu, |board, _, chosen: &MenuChosen, cx| {
            if let Some(p) = priority::from_menu_id(&chosen.0) {
                board.quick_priority = Some(p);
                cx.notify();
            }
        })
        .detach();
        // The title field gets the keyboard back.
        cx.subscribe_in(&menu, window, |board, _, _: &DismissEvent, window, cx| {
            board.quick_priority_menu = None;
            if let Some(quick) = &board.quick {
                let handle = quick.read(cx).focus_handle(cx);
                window.focus(&handle, cx);
            }
            cx.notify();
        })
        .detach();
        self.quick_priority_menu = Some((menu, position));
        cx.notify();
    }

    /// What quick-create's chip shows: the pick, else the epic's for a child, else Normal.
    fn quick_shown_priority(&self, cx: &App) -> Priority {
        self.quick_priority
            .or_else(|| {
                let (epic, _) = self.quick_epic.as_ref()?;
                self.ticket(epic, cx).map(|t| t.priority)
            })
            .unwrap_or_default()
    }

    fn quick_create_box(
        &self,
        c: &Colors,
        input: &Entity<TextInput>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hint = |keys: &'static str, label: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(crate::components::kbd::Kbd::text(keys))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child(label),
                )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(radius::MD))
            .bg(c.bg_card)
            .border(px(1.5))
            .border_color(c.focus)
            .children(
                self.quick_epic
                    .as_ref()
                    .map(|(_, title)| crate::components::card::epic_chip(title.clone().into(), c)),
            )
            .child(div().text_size(px(13.)).child(input.clone()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(hint("↵", "create"))
                    .child(hint("⇧↵", "full form"))
                    .child(div().flex_1())
                    // *Quick-create in Backlog → Priority (optional)*: P3 Normal unless picked.
                    .child(
                        div()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|board, e: &gpui::MouseDownEvent, window, cx| {
                                    cx.stop_propagation();
                                    board.open_quick_priority_menu(e.position, window, cx)
                                }),
                            )
                            .child(
                                PriorityDropdown::new(
                                    "quick-create-priority",
                                    self.quick_shown_priority(cx),
                                )
                                .compact()
                                .outlined()
                                .open(self.quick_priority_menu.is_some()),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// Opens the card's right-click menu at the pointer.
    fn open_menu(
        &mut self,
        ticket: Ticket,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let has_epics = !epic_actions::epic_choices(&self.workspace.read(cx).tickets).is_empty();
        let menu = cx.new(|cx| ContextMenu::new(card_menu::items(&ticket, has_epics), window, cx));
        cx.subscribe(&menu, |board, _, chosen: &MenuChosen, cx| {
            if let Some((_, _, ticket)) = &board.menu {
                let ticket = ticket.clone();
                board.on_menu(&chosen.0, &ticket, cx);
            }
        })
        .detach();
        cx.subscribe(&menu, |board, _, _: &DismissEvent, cx| {
            board.menu = None;
            cx.notify();
        })
        .detach();
        self.menu = Some((menu, position, ticket));
        cx.notify();
    }

    /// The delete confirmation for a ticket (T-0162), asked from elsewhere (the Project terminal's summary).
    pub fn ask_delete(&mut self, key: String, cx: &mut Context<Self>) {
        self.pending_delete = Some(key);
        cx.notify();
    }

    /// The `▶ :4020` chip's menu: Open (with an Open URL) and Stop (RUN-01, RUN-03).
    fn open_run_menu(&mut self, key: TicketKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ticket) = self.ticket(&key, cx) else {
            return;
        };
        let Some(run) = self
            .workspace
            .read(cx)
            .runs
            .iter()
            .find(|r| r.key == key)
            .cloned()
        else {
            return;
        };
        let items = crate::view_models::run::chip_menu(&run);
        if items.is_empty() {
            return;
        }
        self.run_url = run.url();
        let position = window.mouse_position();
        let menu = cx.new(|cx| ContextMenu::new(items, window, cx));
        cx.subscribe(&menu, |board, _, chosen: &MenuChosen, cx| {
            if let Some((_, _, ticket)) = &board.menu {
                let ticket = ticket.clone();
                board.on_menu(&chosen.0, &ticket, cx);
            }
        })
        .detach();
        cx.subscribe(&menu, |board, _, _: &DismissEvent, cx| {
            board.menu = None;
            cx.notify();
        })
        .detach();
        self.menu = Some((menu, position, ticket));
        cx.notify();
    }

    pub fn on_menu(&mut self, choice: &str, ticket: &Ticket, cx: &mut Context<Self>) {
        match choice {
            crate::view_models::run::MENU_OPEN => {
                if let Some(url) = self.run_url.take() {
                    cx.open_url(&url);
                }
                return;
            }
            crate::view_models::run::MENU_STOP => {
                let send =
                    self.workspace
                        .read(cx)
                        .link
                        .read(cx)
                        .send(Request::RunStop(KeyParams {
                            key: ticket.key.to_string(),
                        }));
                cx.spawn(async move |this, cx| {
                    if let Err(e) = send.await {
                        let _ = this.update(cx, |_, cx| {
                            cx.emit(Notice {
                                error: true,
                                title: e.message,
                                text: None,
                            })
                        });
                    }
                })
                .detach();
                return;
            }
            card_menu::MOVE_TO_EPIC => {
                self.epic_dialog = Some(EpicDialog::Move {
                    ticket: ticket.clone(),
                    choice: 0,
                });
                self.focus_dialog = true;
                cx.notify();
                return;
            }
            card_menu::CLOSE_EPIC => {
                self.open_close_epic(ticket.clone(), cx);
                return;
            }
            crate::view_models::close::CLOSE_TICKET => {
                cx.emit(ChooseClose(ticket.key.clone()));
                return;
            }
            crate::view_models::delete::DELETE => {
                self.pending_delete = Some(ticket.key.to_string());
                cx.notify();
                return;
            }
            _ => {}
        }
        let Some(request) = card_menu::request(choice, ticket) else {
            return;
        };
        let send = self.workspace.read(cx).link.read(cx).send(request);
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |board, cx| match result {
                Ok(Reply::Ticket(ticket)) => {
                    board.workspace.update(cx, |ws, cx| ws.upsert(*ticket, cx));
                }
                Ok(_) => {}
                Err(e) => cx.emit(Notice {
                    error: true,
                    title: e.message,
                    text: None,
                }),
            });
        })
        .detach();
    }

    fn open_close_epic(&mut self, epic: Ticket, cx: &mut Context<Self>) {
        let send = self
            .workspace
            .read(cx)
            .link
            .read(cx)
            .send(Request::EpicClosePlan(KeyParams {
                key: epic.key.to_string(),
            }));
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |board, cx| match result {
                Ok(Reply::ClosePlan(plan)) => {
                    board.epic_dialog = Some(EpicDialog::Close { epic, plan });
                    board.focus_dialog = true;
                    cx.notify();
                }
                Ok(_) => {}
                Err(e) => cx.emit(Notice {
                    error: true,
                    title: e.message,
                    text: None,
                }),
            });
        })
        .detach();
    }

    /// Sends an epic action and reports the outcome.
    fn send_epic(&mut self, request: Request, done: String, cx: &mut Context<Self>) {
        self.epic_dialog = None;
        let send = self.workspace.read(cx).link.read(cx).send(request);
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |board, cx| {
                match result {
                    Ok(Reply::Ticket(ticket)) => {
                        board.workspace.update(cx, |ws, cx| ws.upsert(*ticket, cx));
                        cx.emit(Notice {
                            error: false,
                            title: done,
                            text: None,
                        });
                    }
                    Ok(_) => cx.emit(Notice {
                        error: false,
                        title: done,
                        text: None,
                    }),
                    Err(e) => cx.emit(Notice {
                        error: true,
                        title: e.message,
                        text: None,
                    }),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn epic_dialog_view(&self, dialog: &EpicDialog, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let tickets = self.workspace.read(cx).tickets.clone();
        let cancel = {
            let this = cx.entity().downgrade();
            move |_: &mut Window, cx: &mut App| {
                let _ = this.update(cx, |board, cx| {
                    board.epic_dialog = None;
                    cx.notify();
                });
            }
        };
        match dialog {
            EpicDialog::Move { ticket, choice } => {
                let choices = epic_actions::epic_choices(&tickets);
                let Some(chosen) = choices.get(*choice).cloned() else {
                    return div().into_any_element();
                };
                let preview = epic_actions::move_preview(ticket, &chosen, &tickets);
                let confirm = {
                    let this = cx.entity().downgrade();
                    let key = ticket.key.clone();
                    let chosen = chosen.clone();
                    move |_: &mut Window, cx: &mut App| {
                        let request = Request::EpicMoveTo(MoveToEpicParams {
                            key: key.to_string(),
                            epic: chosen.key.to_string(),
                        });
                        let done = format!("Moved {key} → {}", chosen.new_key);
                        let _ = this.update(cx, |board, cx| board.send_epic(request, done, cx));
                    }
                };
                let rows = choices.iter().enumerate().map(|(i, e)| {
                    let on = i == *choice;
                    div()
                        .id(("epic-choice", i))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(radius::MD))
                        .border(if on { px(1.5) } else { px(1.) })
                        .border_color(if on { c.focus } else { c.border })
                        .cursor_pointer()
                        .child(icon(IconName::Layers, px(13.)).text_color(c.st_investigate))
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(12.))
                                .text_color(c.text_muted)
                                .child(SharedString::from(e.key.to_string())),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(13.))
                                .text_color(c.text_primary)
                                .child(SharedString::from(e.title.clone())),
                        )
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(11.))
                                .text_color(c.text_muted)
                                .child(SharedString::from(e.progress.clone())),
                        )
                        .on_click(cx.listener(move |board, _, _, cx| {
                            if let Some(EpicDialog::Move { choice, .. }) = &mut board.epic_dialog {
                                *choice = i;
                                cx.notify();
                            }
                        }))
                });
                ConfirmDialog::new(
                    "move-to-epic",
                    &self.dialog_focus,
                    DialogKind::Primary,
                    preview.title.clone(),
                    preview.confirm.clone(),
                    confirm,
                    cancel,
                )
                .confirm_icon(IconName::Layers)
                .child(div().flex().flex_col().gap(px(6.)).children(rows))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(radius::MD))
                        .bg(c.st_investigate_bg)
                        .font_family(FONT_MONO)
                        .text_size(px(12.))
                        .child(
                            div()
                                .text_color(c.text_muted)
                                .child(SharedString::from(ticket.key.to_string())),
                        )
                        .child(div().text_color(c.text_muted).child("→"))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.st_investigate)
                                .child(SharedString::from(chosen.new_key.to_string())),
                        )
                        .child(
                            div()
                                .font_family(crate::theme::FONT_UI)
                                .text_color(c.text_secondary)
                                .child(SharedString::from(preview.alias_note.clone())),
                        ),
                )
                .children_boxes(
                    preview
                        .base_note
                        .clone()
                        .into_iter()
                        .map(|note| {
                            div()
                                .flex()
                                .gap(px(8.))
                                .px(px(10.))
                                .py(px(8.))
                                .rounded(px(radius::MD))
                                .bg(c.st_paused_bg)
                                .child(
                                    icon(IconName::GitBranch, px(13.))
                                        .text_color(c.st_paused)
                                        .mt(px(2.)),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(c.text_primary)
                                        .child(SharedString::from(note)),
                                )
                                .into_any_element()
                        })
                        .collect(),
                )
                .text("No branch or worktree exists yet, so nothing on disk is renamed.")
                .width(480.)
                .into_any_element()
            }
            EpicDialog::Close { epic, plan } => {
                let text = epic_actions::close_dialog(epic, plan, &tickets);
                let confirm = {
                    let this = cx.entity().downgrade();
                    let key = epic.key.clone();
                    move |_: &mut Window, cx: &mut App| {
                        let request = Request::EpicClose(crewboard_core::protocol::CloseParams {
                            confirm: true,
                            key: key.to_string(),
                        });
                        let done = format!("Closed epic {key}");
                        let _ = this.update(cx, |board, cx| board.send_epic(request, done, cx));
                    }
                };
                let box_ = |icon_name: IconName, tint: gpui::Hsla, bg: gpui::Hsla, text: String| {
                    div()
                        .flex()
                        .gap(px(8.))
                        .px(px(10.))
                        .py(px(8.))
                        .rounded(px(radius::MD))
                        .bg(bg)
                        .child(icon(icon_name, px(13.)).text_color(tint).mt(px(2.)))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(c.text_primary)
                                .child(SharedString::from(text)),
                        )
                };
                ConfirmDialog::new(
                    "close-epic",
                    &self.dialog_focus,
                    DialogKind::Destructive,
                    text.title.clone(),
                    text.confirm.clone(),
                    confirm,
                    cancel,
                )
                .confirm_icon(IconName::CircleSlash)
                .children_boxes(
                    text.agents
                        .iter()
                        .map(|a| {
                            box_(
                                IconName::TriangleAlert,
                                c.st_paused,
                                c.st_paused_bg,
                                a.clone(),
                            )
                            .into_any_element()
                        })
                        .chain(text.also.clone().map(|a| {
                            box_(IconName::Layers, c.text_secondary, c.bg_column, a)
                                .into_any_element()
                        }))
                        .collect(),
                )
                .text(text.note)
                .into_any_element()
            }
        }
    }

    /// "Park SHOP-25?": Park sends ticket.park; Cancel leaves the card in Dev and the agent working.
    fn park_dialog(&self, ticket: &Ticket, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let text = crate::view_models::close::park_text(ticket);
        let park = {
            let key = ticket.key.clone();
            let this = cx.entity().downgrade();
            move |_: &mut Window, cx: &mut App| {
                let key = key.clone();
                let _ = this.update(cx, |board, cx| {
                    board.park_ask = None;
                    let send = board
                        .workspace
                        .read(cx)
                        .link
                        .read(cx)
                        .send(Request::TicketPark(crewboard_core::protocol::CloseParams {
                            key: key.to_string(),
                            confirm: true,
                        }));
                    cx.spawn(async move |this, cx| {
                        let result = send.await;
                        let _ = this.update(cx, |board, cx| match result {
                            Ok(Reply::Ticket(ticket)) => {
                                board.workspace.update(cx, |ws, cx| ws.upsert(*ticket, cx))
                            }
                            Ok(_) => {}
                            Err(e) => cx.emit(Notice {
                                error: true,
                                title: format!("{key} wasn\u{2019}t parked"),
                                text: Some(e.to_string()),
                            }),
                        });
                    })
                    .detach();
                    cx.notify();
                });
            }
        };
        let cancel = {
            let this = cx.entity().downgrade();
            move |_: &mut Window, cx: &mut App| {
                let _ = this.update(cx, |board, cx| {
                    board.park_ask = None;
                    cx.notify();
                });
            }
        };
        ConfirmDialog::new(
            "park",
            &self.dialog_focus,
            DialogKind::Primary,
            text.title,
            "Park",
            park,
            cancel,
        )
        .confirm_icon(IconName::SquareParking)
        .text(text.message)
        .children_boxes(
            text.kept
                .into_iter()
                .map(|(line_icon, line)| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .px(px(10.))
                        .py(px(5.))
                        .rounded(px(5.))
                        .bg(c.bg_column)
                        .font_family(crate::theme::FONT_MONO)
                        .text_size(px(11.))
                        .text_color(c.text_secondary)
                        .child(icon(line_icon, px(12.)).text_color(c.text_muted))
                        .child(SharedString::from(line))
                        .into_any_element()
                })
                .collect(),
        )
        .into_any_element()
    }

    /// Selects a ticket from outside the board (the Stack list).
    pub fn select_key(&mut self, key: TicketKey, cx: &mut Context<Self>) {
        self.select(key, cx);
    }

    fn select(&mut self, key: TicketKey, cx: &mut Context<Self>) {
        self.selected = Some(key.clone());
        self.workspace.update(cx, |ws, _| ws.selected = Some(key));
        cx.notify();
    }

    fn card(&self, entry: &CardEntry, cx: &mut Context<Self>) -> AnyElement {
        let inner = self.card_inner(entry, cx);
        let key = entry.key().clone();
        let Some(ticket) = self.ticket(&key, cx) else {
            return inner;
        };
        let lifted = self.dragging.as_ref().is_some_and(|d| d.key == key);
        let dragged = DraggedCard {
            ticket,
            entry: entry.clone(),
            width: px(240.),
        };
        let record = {
            let layout = self.layout.clone();
            let key = key.clone();
            move |bounds| {
                layout.borrow_mut().cards.insert(key.clone(), bounds);
            }
        };
        let hover_key = key.clone();
        let menu_ticket = dragged.ticket.clone();
        let mut wrapper = div()
            .id(SharedString::from(format!("drag-source-{key}")))
            .relative()
            .on_hover(cx.listener(move |board, hovered: &bool, _, cx| {
                let next = hovered.then(|| hover_key.clone());
                if *hovered || board.hovered.as_ref() == Some(&hover_key) {
                    board.hovered = next;
                    cx.notify();
                }
            }))
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(move |board, e: &gpui::MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    board.open_menu(menu_ticket.clone(), e.position, window, cx);
                }),
            )
            .child(record_bounds(record))
            .on_drag(dragged, {
                let this = cx.entity().downgrade();
                move |card, _offset, _window, cx| {
                    let card = card.clone();
                    let ticket = card.ticket.clone();
                    let _ = this.update(cx, |board, cx| {
                        board.dragging = Some(ticket);
                        cx.notify();
                    });
                    cx.new(|_| DragPreview { card })
                }
            })
            .child(inner);
        if lifted {
            wrapper = wrapper.opacity(0.4);
        }
        wrapper.into_any_element()
    }

    fn card_inner(&self, entry: &CardEntry, cx: &mut Context<Self>) -> AnyElement {
        let key = entry.key().clone();
        let selected = self.selected.as_ref() == Some(&key);
        let chain_state = match &self.chain {
            Some(chain) if chain.members.contains(&key) => ChainState::Highlighted,
            Some(_) => ChainState::Dimmed,
            None => ChainState::None,
        };
        let id = SharedString::from(format!("card-{key}"));
        let click = {
            let key = key.clone();
            cx.listener(move |board, e: &gpui::ClickEvent, _, cx| {
                board.select(key.clone(), cx);
                if e.click_count() >= 2 {
                    cx.emit(OpenEditor::Ticket(key.clone()));
                } else {
                    cx.emit(OpenPanel(key.clone()));
                }
            })
        };
        match entry {
            CardEntry::Ticket(vm) => {
                let pressed = key.clone();
                let fresh = self.workspace.read(cx).fresh.contains_key(&key);
                TicketCard::new(id, vm.clone())
                    .selected(selected)
                    .fresh(fresh)
                    .chain(chain_state)
                    .on_click(click)
                    .on_action({
                        let this = cx.entity().downgrade();
                        move |kind, window, cx| {
                            let _ = this.update(cx, |board, cx| {
                                board.select(pressed.clone(), cx);
                                if kind == crate::view_models::card::CardActionKind::RunChip {
                                    board.open_run_menu(pressed.clone(), window, cx);
                                } else {
                                    cx.emit(CardPressed(pressed.clone(), kind));
                                }
                            });
                        }
                    })
                    .into_any_element()
            }
            CardEntry::Epic(vm) => {
                let toggle_key = key.clone();
                EpicCard::new(id, vm.clone())
                    .selected(selected)
                    .chain(chain_state)
                    .collapsed(self.collapsed_epics.contains(&key))
                    .on_click(click)
                    .on_toggle(cx.listener(move |board, _, _, cx| {
                        if !board.collapsed_epics.remove(&toggle_key) {
                            board.collapsed_epics.insert(toggle_key.clone());
                        }
                        cx.notify();
                    }))
                    .on_child({
                        let this = cx.entity().downgrade();
                        move |child, _, cx| {
                            let _ = this.update(cx, |board, cx| board.select(child, cx));
                        }
                    })
                    .on_add({
                        let epic = key.clone();
                        cx.listener(move |board, _, window, cx| {
                            if let Some(epic) = board.ticket(&epic, cx) {
                                board.start_quick_child(&epic, window, cx);
                            }
                        })
                    })
                    .into_any_element()
            }
        }
    }

    fn header(&self, view: &ColumnView, c: &Colors, cx: &mut Context<Self>) -> AnyElement {
        let (fg, _) = column_icon_color(view.column, c);
        let extra = match view.column {
            Column::Backlog => Some(
                div()
                    .id("backlog-add")
                    .cursor_pointer()
                    .rounded(px(4.))
                    .hover(|s| s.bg(c.bg_hover))
                    .child(icon(IconName::Plus, px(14.)).text_color(c.text_muted))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ShellRequest::NewTicket)))
                    .into_any_element(),
            ),
            Column::Done => Some(
                div()
                    .id("done-collapse")
                    .cursor_pointer()
                    .rounded(px(4.))
                    .hover(|s| s.bg(c.bg_hover))
                    .child(icon(IconName::ChevronsLeftRight, px(13.)).text_color(c.text_muted))
                    .on_click(cx.listener(|board, _, _, cx| {
                        board.done_choice = Some(true);
                        cx.notify();
                    }))
                    .into_any_element(),
            ),
            _ => None,
        };
        let row = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .pt(px(4.))
            .px(px(4.))
            .pb(px(2.))
            .child(icon(view.icon, px(13.)).text_color(fg))
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c.text_primary)
                    .child(view.title),
            )
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .child(SharedString::from(view.count.clone())),
            )
            .child(div().flex_1())
            .children(extra);
        match view.hint {
            // Ready for Dev: "Starts when ready, in priority order" under the title (d0066).
            Some(hint) => div()
                .flex()
                .flex_col()
                .flex_none()
                .child(row)
                .child(
                    div()
                        .px(px(4.))
                        .pb(px(2.))
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child(hint),
                )
                .into_any_element(),
            None => row.into_any_element(),
        }
    }

    fn section_header(
        label: &'static str,
        count: usize,
        hint: Option<&'static str>,
        c: &Colors,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .pt(px(6.))
            .px(px(4.))
            .pb(px(2.))
            .border_t_1()
            .border_color(c.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .font_family(FONT_MONO)
                    .text_size(px(10.))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c.text_secondary)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_color(c.text_muted)
                            .child(SharedString::from(count.to_string())),
                    ),
            )
            .children(hint.map(|h| div().text_size(px(11.)).text_color(c.text_muted).child(h)))
            .into_any_element()
    }

    fn empty_column(view: &ColumnView, c: &Colors) -> AnyElement {
        let (fg, _) = column_icon_color(view.column, c);
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .px(px(14.))
            .py(px(18.))
            .rounded(px(radius::MD))
            .border_1()
            .border_dashed()
            .border_color(c.border_strong)
            .child(icon(view.icon, px(16.)).text_color(fg))
            .child(
                div()
                    .text_center()
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .child(view.empty_hint),
            )
            .into_any_element()
    }

    fn dev_body(&self, dev: &DevSections, c: &Colors, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut items: Vec<AnyElement> = dev.top.iter().map(|e| self.card(e, cx)).collect();
        items.push(Self::section_header(
            DEV_SECTION,
            dev.working_count,
            None,
            c,
        ));
        items.extend(dev.working.iter().map(|e| self.card(e, cx)));
        items
    }

    fn column(
        &self,
        i: usize,
        view: &ColumnView,
        c: &Colors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if view.collapsed {
            return div()
                .id("done-rail")
                .flex_none()
                .w(px(34.))
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .py(px(10.))
                .rounded(px(radius::LG))
                .bg(c.bg_column)
                .cursor_pointer()
                .hover(|s| s.bg(c.bg_hover))
                .child(icon(view.icon, px(13.)).text_color(c.st_done))
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(view.count.clone())),
                )
                .children(view.title.chars().map(|ch| {
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.text_secondary)
                        .child(SharedString::from(ch.to_string()))
                }))
                .on_click(cx.listener(|board, _, _, cx| {
                    board.done_choice = Some(false);
                    cx.notify();
                }))
                .into_any_element();
        }
        let body: Vec<AnyElement> = match &view.dev {
            Some(dev) => self.dev_body(dev, c, cx),
            None if view.cards.is_empty()
                && !(view.column == Column::Backlog && self.quick.is_some()) =>
            {
                vec![Self::empty_column(view, c)]
            }
            None => view.cards.iter().map(|e| self.card(e, cx)).collect(),
        };
        let mut body = body;
        if view.column == Column::Backlog
            && let Some(quick) = &self.quick
        {
            body.insert(0, self.quick_create_box(c, quick, cx));
        }
        let item_count = body.len();
        let handle = self.scrolls[i].clone();
        let hidden_below = {
            let max = handle.max_offset().y;
            let at_bottom = -handle.offset().y >= max - px(1.);
            if max > px(0.) && !at_bottom {
                item_count.saturating_sub(handle.bottom_item() + 1)
            } else {
                0
            }
        };
        let fold = view.dev.as_ref().and_then(|d| d.fold.clone());
        let chip = |label: String, icon_name: IconName| more_chip(label, icon_name, false, c);
        let footer = if let Some(label) = fold {
            Some(
                div()
                    .id(("fold", i))
                    .flex()
                    .justify_center()
                    .pb(px(2.))
                    .cursor_pointer()
                    .child(chip(label, IconName::ChevronDown))
                    .on_click(cx.listener(|board, _, _, cx| {
                        board.dev_unfolded = true;
                        cx.notify();
                    }))
                    .into_any_element(),
            )
        } else if view.column == Column::Dev && self.dev_unfolded {
            Some(
                div()
                    .id(("fold", i))
                    .flex()
                    .justify_center()
                    .pb(px(2.))
                    .cursor_pointer()
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(c.text_muted)
                            .hover(|s| s.text_color(c.text_secondary))
                            .child("Fold"),
                    )
                    .on_click(cx.listener(|board, _, _, cx| {
                        board.dev_unfolded = false;
                        cx.notify();
                    }))
                    .into_any_element(),
            )
        } else if hidden_below > 0 {
            Some(
                div()
                    .flex()
                    .justify_center()
                    .pb(px(2.))
                    .child(chip(format!("{hidden_below} more"), IconName::ChevronDown))
                    .into_any_element(),
            )
        } else {
            None
        };
        let dragging = self.dragging.clone();
        let over_here = self.drag_over == Some(view.column);
        let feedback_here = match (&dragging, over_here) {
            (Some(t), true) => Some(feedback(t, view.column)),
            _ => None,
        };
        let forbidden_box = match &feedback_here {
            Some(DropFeedback::Forbidden { title, detail }) => Some(
                div()
                    .absolute()
                    .top(px(120.))
                    .left(px(12.))
                    .right(px(12.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(6.))
                    .p(px(12.))
                    .rounded(px(radius::LG))
                    .bg(c.st_error_bg)
                    .border_1()
                    .border_color(c.st_error)
                    .child(icon(IconName::Ban, px(16.)).text_color(c.st_error))
                    .child(
                        div()
                            .text_center()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c.st_error)
                            .child(SharedString::from(title.clone())),
                    )
                    .children(detail.map(|d| {
                        div()
                            .text_center()
                            .text_size(px(11.))
                            .text_color(c.text_secondary)
                            .child(d)
                    })),
            ),
            _ => None,
        };
        let quick_add = (view.column == Column::Backlog).then(|| {
            div()
                .id("backlog-quick-add")
                .flex()
                .flex_none()
                .items_center()
                .gap(px(6.))
                .p(px(6.))
                .rounded(px(radius::MD))
                .cursor_pointer()
                .hover(|s| s.bg(c.bg_hover))
                .child(icon(IconName::Plus, px(13.)).text_color(c.text_muted))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(c.text_muted)
                        .child("Add ticket"),
                )
                .on_click(cx.listener(|_, _, _, cx| cx.emit(ShellRequest::NewTicket)))
        });
        let column = view.column;
        let mut col = div()
            .id(("column", i))
            .relative()
            .flex_1()
            .min_w(px(density::COLUMN))
            .h_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(6.))
            .rounded(px(radius::LG))
            .bg(c.bg_column)
            .border_2()
            .border_color(gpui::transparent_black())
            .on_drag_move(
                cx.listener(move |board, e: &DragMoveEvent<DraggedCard>, _, cx| {
                    let inside = e.bounds.contains(&e.event.position);
                    if inside && board.drag_over != Some(column) {
                        board.drag_over = Some(column);
                        cx.notify();
                    } else if !inside && board.drag_over == Some(column) {
                        board.drag_over = None;
                        cx.notify();
                    }
                }),
            )
            .on_drop(cx.listener(move |board, card: &DraggedCard, window, cx| {
                board.drop_on(card.ticket.clone(), column, window, cx)
            }));
        match &feedback_here {
            Some(DropFeedback::Allowed) => {
                col = col.bg(c.accent_soft).border_color(c.accent);
            }
            Some(DropFeedback::Forbidden { .. }) => {
                col = col.border_color(c.st_error);
            }
            _ => {}
        }
        let header = self.header(view, c, cx);
        col.child(header)
            .child(
                div()
                    .id(("column-body", i))
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap(px(if view.column == Column::Dev { 5. } else { 6. }))
                    .overflow_y_scroll()
                    .track_scroll(&handle)
                    .child(record_bounds({
                        let layout = self.layout.clone();
                        move |bounds| layout.borrow_mut().columns.push(bounds)
                    }))
                    .children(body)
                    .children((view.hidden > 0).then(|| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(6.))
                            .py(px(4.))
                            .child(icon(IconName::EyeOff, px(12.)).text_color(c.text_muted))
                            .child(div().text_size(px(11.)).text_color(c.text_muted).child(
                                SharedString::from(format!("{} hidden by filter", view.hidden)),
                            ))
                    }))
                    .children(quick_add),
            )
            .children(footer)
            .children(forbidden_box)
            .into_any_element()
    }
}

/// The colour of a column's header icon.
fn column_icon_color(column: Column, c: &Colors) -> (gpui::Hsla, gpui::Hsla) {
    match column {
        Column::Backlog | Column::ReadyForDev => (c.st_queued, c.st_queued_bg),
        Column::Dev => (c.st_working, c.st_working_bg),
        Column::ReadyForReview => (c.st_review, c.st_review_bg),
        Column::Approved => (c.st_approved, c.st_approved_bg),
        Column::Done => (c.st_done, c.st_queued_bg),
    }
}

impl Render for BoardView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        if !cx.has_active_drag() {
            self.drag_over = None;
            self.dragging = None;
        }
        {
            let mut layout = self.layout.borrow_mut();
            layout.cards.clear();
            layout.columns.clear();
        }
        let active = self.hovered.clone().or_else(|| self.selected.clone());
        self.chain = active.and_then(|k| chain(&k, &self.workspace.read(cx).tickets));
        let viewport = window.viewport_size();
        let ws = self.workspace.read(cx);
        let base = ws.current().map(|p| p.base_branch.clone());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or_default();
        let budget = (!self.dev_unfolded).then(|| dev_budget(f32::from(viewport.height)));
        let visible = ws.filter.visible(&ws.tickets);
        let collapsed = done_collapsed(
            self.done_choice,
            board_width(&self.board_scroll).unwrap_or(f32::from(viewport.width)),
        );
        let columns = build(&BoardInput {
            tickets: &ws.tickets,
            visible: visible.as_deref(),
            project_base: base.as_deref(),
            now_ms: now,
            dev_budget: budget,
            done_collapsed: collapsed,
        });
        let titles: Vec<&'static str> = columns.iter().map(|view| view.title).collect();
        let hidden = hidden_in(&self.board_scroll, &titles);
        let columns: Vec<AnyElement> = columns
            .iter()
            .enumerate()
            .map(|(i, view)| self.column(i, view, &c, cx))
            .collect();
        if std::mem::take(&mut self.focus_dialog) {
            window.focus(&self.dialog_focus, cx);
        }
        if let Some(key) = self.pending_delete.take() {
            let link = self.workspace.read(cx).link.clone();
            let dialog = cx.new(|cx| DeleteDialog::new(link, key, window, cx));
            cx.subscribe(&dialog, |board, _, outcome: &DeleteOutcome, cx| {
                board.delete = None;
                if let DeleteOutcome::Deleted(preview) = outcome {
                    let removed = preview.children.len() + 1;
                    cx.emit(Notice {
                        error: false,
                        title: format!(
                            "Deleted {}{}",
                            preview.key,
                            if removed > 1 {
                                format!(" and {} tickets", removed - 1)
                            } else {
                                String::new()
                            }
                        ),
                        text: None,
                    });
                }
                cx.notify();
            })
            .detach();
            self.delete = Some(dialog);
        }
        let dialog = match self.epic_dialog.clone() {
            _ if self.park_ask.is_some() => self.park_ask.clone().map(|t| self.park_dialog(&t, cx)),
            Some(d) => Some(self.epic_dialog_view(&d, cx)),
            None => self.delete.clone().map(|d| d.into_any_element()),
        };
        div()
            .relative()
            .flex_1()
            .size_full()
            .child(
                div()
                    .id("board")
                    .size_full()
                    .flex()
                    .gap(px(8.))
                    .p(px(10.))
                    .overflow_x_scroll()
                    .track_scroll(&self.board_scroll)
                    .children(columns),
            )
            .children(
                hidden
                    .before
                    .clone()
                    .map(|label| edge_hint(label, false, &c)),
            )
            .children(hidden.after.clone().map(|label| edge_hint(label, true, &c)))
            .child({
                // The width and the hidden columns above are the last frame's: render again if this frame's
                // layout changes the rail or the hint.
                let scroll = self.board_scroll.clone();
                let choice = self.done_choice;
                canvas(
                    move |_, window, _| {
                        let rail_changed = board_width(&scroll)
                            .is_some_and(|w| done_collapsed(choice, w) != collapsed);
                        if rail_changed || hidden_in(&scroll, &titles) != hidden {
                            window.request_animation_frame();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
            })
            .children(self.chain.clone().map(|chain| {
                let layout = self.layout.clone();
                let color = c.accent;
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        let layout = layout.borrow();
                        // Only cards that are actually in view (inside their column's scroll area).
                        let visible = |b: &Bounds<Pixels>| {
                            layout.columns.iter().any(|col| col.contains(&b.center()))
                        };
                        for (parent, child) in &chain.edges {
                            if let (Some(from), Some(to)) =
                                (layout.cards.get(parent), layout.cards.get(child))
                                && visible(from)
                                && visible(to)
                            {
                                paint_arrow(*from, *to, color, window);
                            }
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full()
            }))
            .children(self.quick_priority_menu.as_ref().map(|(menu, position)| {
                gpui::deferred(
                    gpui::anchored()
                        .position(*position)
                        .snap_to_window_with_margin(px(8.))
                        .child(menu.clone()),
                )
                .with_priority(1)
            }))
            .children(self.menu.as_ref().map(|(menu, position, _)| {
                gpui::deferred(
                    gpui::anchored()
                        .position(*position)
                        .snap_to_window_with_margin(px(8.))
                        .child(menu.clone()),
                )
                .with_priority(1)
            }))
            .children(dialog)
    }
}
