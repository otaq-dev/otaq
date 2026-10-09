//! The board (UI-02; *03 · Board — shop-web · 1440 · Light / Dark → Body → Board*, *03 · Board & columns →
//! Empty column & empty board, Very full column · 14 cards, Width strategy*): five columns (Dev with its
//! needs-you cards on top, Agent Working, then Queued), each scrolling on its own with its header pinned and a
//! "N more" chip when there is more below; Dev's fold; Done as a 34 px rail; epic cards in Backlog. It draws
//! [`crate::view_models::board`] from the workspace, which updates cards in place from the service's events.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crewboard_core::board::moves::DropTarget;
use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Column, TicketKey};
use crewboard_core::protocol::{MoveParams, Reply, Request};
use gpui::{
    AnyElement, App, Bounds, Context, DismissEvent, DragMoveEvent, Entity, EventEmitter,
    FocusHandle, FontWeight, Hsla, IntoElement, PathBuilder, Pixels, Point, ScrollHandle,
    SharedString, Window, canvas, div, point, prelude::*, px,
};

use crate::components::card::{ChainState, TicketCard};
use crate::components::dialog::ConfirmDialog;
use crate::components::epic_card::EpicCard;
use crate::components::menu::{ContextMenu, MenuChosen};
use crate::icons::{IconName, icon};
use crate::shell::ShellRequest;
use crate::theme::{ActiveTheme, Colors, FONT_MONO, density, radius};
use crate::view_models::board::{
    BoardInput, CardEntry, ColumnView, DevSections, QUEUED_HINT, build, dev_budget, done_collapsed,
};
use crate::view_models::card_menu;
use crate::view_models::chain::{Chain, chain};
use crate::view_models::dialog::DialogKind;
use crate::view_models::drag::{
    DropAction, DropFeedback, empty_done_when_title, feedback, on_drop,
};
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

/// Bounds recorded while laying out the board.
#[derive(Default)]
struct Layout {
    cards: HashMap<TicketKey, Bounds<Pixels>>,
    columns: Vec<Bounds<Pixels>>,
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
    /// The user opened or closed the Done rail; else it follows the window width.
    done_choice: Option<bool>,
    collapsed_epics: HashSet<TicketKey>,
    scrolls: Vec<ScrollHandle>,
    /// The column under a dragged card.
    drag_over: Option<Column>,
    /// The ticket being dragged (set when the drag starts; GPUI only says whether a drag is active).
    dragging: Option<Ticket>,
    /// The empty "Done when" warning for this ticket.
    warn_empty: Option<Ticket>,
    /// The card under the pointer; with the selection it decides the highlighted chain.
    hovered: Option<TicketKey>,
    chain: Option<Chain>,
    /// Where cards and column bodies were laid out this frame, for the chain's arrows.
    layout: Rc<RefCell<Layout>>,
    /// The open right-click menu, where it opened, and for which ticket.
    menu: Option<(Entity<ContextMenu>, Point<Pixels>, Ticket)>,
    dialog_focus: FocusHandle,
}

impl EventEmitter<ShellRequest> for BoardView {}
impl EventEmitter<Notice> for BoardView {}

/// Asks to open the ticket editor at its "Done when" list ("Add criteria", T-0040).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditCriteria(pub TicketKey);

impl EventEmitter<EditCriteria> for BoardView {}

impl BoardView {
    pub fn new(workspace: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        Self {
            workspace,
            selected: None,
            dev_unfolded: false,
            done_choice: None,
            collapsed_epics: HashSet::new(),
            scrolls: (0..Column::ALL.len())
                .map(|_| ScrollHandle::new())
                .collect(),
            drag_over: None,
            dragging: None,
            warn_empty: None,
            hovered: None,
            chain: None,
            layout: Rc::default(),
            menu: None,
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
            DropAction::Move => self.send_move(ticket.key.clone(), column, false, cx),
            DropAction::AskEmptyDoneWhen => {
                self.warn_empty = Some(ticket);
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

    fn send_move(
        &mut self,
        key: TicketKey,
        column: Column,
        confirmed: bool,
        cx: &mut Context<Self>,
    ) {
        let request = self
            .workspace
            .read(cx)
            .link
            .read(cx)
            .send(Request::TicketMove(MoveParams {
                key: key.to_string(),
                target: match column {
                    Column::Dev => DropTarget::DevQueued,
                    other => crate::view_models::drag::target_for(other),
                },
                confirmed,
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

    /// Opens the card's right-click menu at the pointer.
    fn open_menu(
        &mut self,
        ticket: Ticket,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu = cx.new(|cx| ContextMenu::new(card_menu::items(&ticket), window, cx));
        cx.subscribe(&menu, |board, _, chosen: &MenuChosen, cx| {
            if let Some((_, _, ticket)) = &board.menu {
                let ticket = ticket.clone();
                board.on_menu(chosen.0, &ticket, cx);
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

    fn on_menu(&mut self, choice: &str, ticket: &Ticket, cx: &mut Context<Self>) {
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

    fn warning_dialog(&self, ticket: &Ticket, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let start = {
            let key = ticket.key.clone();
            let this = cx.entity().downgrade();
            move |_: &mut Window, cx: &mut App| {
                let key = key.clone();
                let _ = this.update(cx, |board, cx| {
                    board.warn_empty = None;
                    board.send_move(key, Column::Dev, true, cx);
                    cx.notify();
                });
            }
        };
        let criteria = {
            let key = ticket.key.clone();
            let this = cx.entity().downgrade();
            move |_: &mut Window, cx: &mut App| {
                let _ = this.update(cx, |board, cx| {
                    board.warn_empty = None;
                    cx.emit(EditCriteria(key.clone()));
                    cx.notify();
                });
            }
        };
        let cancel = {
            let this = cx.entity().downgrade();
            move |_: &mut Window, cx: &mut App| {
                let _ = this.update(cx, |board, cx| {
                    board.warn_empty = None;
                    cx.notify();
                });
            }
        };
        let example = |text: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .size(px(12.))
                        .rounded(px(3.))
                        .border_1()
                        .border_color(c.border_strong),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(c.text_secondary)
                        .child(text),
                )
        };
        ConfirmDialog::new(
            "empty-done-when",
            &self.dialog_focus,
            DialogKind::Primary,
            empty_done_when_title(ticket),
            "Add criteria",
            criteria,
            cancel,
        )
        .confirm_icon(IconName::ListChecks)
        .secondary("Start anyway", start)
        .text("Without criteria the agent decides on its own when it’s finished, and Review has nothing to check against. You can start anyway.")
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(10.))
                .rounded(px(radius::MD))
                .bg(c.bg_app)
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(10.))
                        .text_color(c.text_muted)
                        .child("EXAMPLE"),
                )
                .child(example("The page loads without errors"))
                .child(example("Tests cover the new behaviour")),
        )
        .into_any_element()
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
            cx.listener(move |board, _, _, cx| board.select(key.clone(), cx))
        };
        match entry {
            CardEntry::Ticket(vm) => TicketCard::new(id, vm.clone())
                .selected(selected)
                .chain(chain_state)
                .on_click(click)
                .into_any_element(),
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
                    .on_add(cx.listener(|_, _, _, cx| cx.emit(ShellRequest::NewTicket)))
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
        div()
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
            .children(extra)
            .into_any_element()
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
            "AGENT WORKING",
            dev.working_count,
            None,
            c,
        ));
        items.extend(dev.working.iter().map(|e| self.card(e, cx)));
        items.push(Self::section_header(
            "QUEUED",
            dev.queued_count,
            Some(QUEUED_HINT),
            c,
        ));
        items.extend(dev.queued.iter().map(|e| self.card(e, cx)));
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
            None if view.cards.is_empty() => vec![Self::empty_column(view, c)],
            None => view.cards.iter().map(|e| self.card(e, cx)).collect(),
        };
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
        let chip = |label: String, icon_name: IconName| {
            div()
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
                .text_color(c.text_secondary)
                .child(SharedString::from(label))
                .child(icon(icon_name, px(12.)).text_color(c.text_secondary))
        };
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
        Column::Backlog => (c.st_queued, c.st_queued_bg),
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
        let columns = build(&BoardInput {
            tickets: &ws.tickets,
            visible: visible.as_deref(),
            project_base: base.as_deref(),
            now_ms: now,
            dev_budget: budget,
            done_collapsed: done_collapsed(self.done_choice, f32::from(viewport.width)),
        });
        let columns: Vec<AnyElement> = columns
            .iter()
            .enumerate()
            .map(|(i, view)| self.column(i, view, &c, cx))
            .collect();
        let dialog = self.warn_empty.clone().map(|t| self.warning_dialog(&t, cx));
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
                    .children(columns),
            )
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
