//! The board (UI-02; *03 · Board — shop-web · 1440 · Light / Dark → Body → Board*, *03 · Board & columns →
//! Empty column & empty board, Very full column · 14 cards, Width strategy*): five columns (Dev with its
//! needs-you cards on top, Agent Working, then Queued), each scrolling on its own with its header pinned and a
//! "N more" chip when there is more below; Dev's fold; Done as a 34 px rail; epic cards in Backlog. It draws
//! [`crate::view_models::board`] from the workspace, which updates cards in place from the service's events.

use std::collections::HashSet;

use crewboard_core::board::moves::DropTarget;
use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Column, TicketKey};
use crewboard_core::protocol::{MoveParams, Reply, Request};
use gpui::{
    AnyElement, App, Context, DragMoveEvent, Entity, EventEmitter, FocusHandle, FontWeight,
    IntoElement, Pixels, ScrollHandle, SharedString, Window, div, prelude::*, px,
};

use crate::components::card::TicketCard;
use crate::components::dialog::ConfirmDialog;
use crate::components::epic_card::EpicCard;
use crate::icons::{IconName, icon};
use crate::shell::ShellRequest;
use crate::theme::{ActiveTheme, Colors, FONT_MONO, density, radius};
use crate::view_models::board::{
    BoardInput, CardEntry, ColumnView, DevSections, QUEUED_HINT, build, dev_budget, done_collapsed,
};
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
        self.selected = Some(key);
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
        let mut wrapper = div()
            .id(SharedString::from(format!("drag-source-{key}")))
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
        let id = SharedString::from(format!("card-{key}"));
        let click = {
            let key = key.clone();
            cx.listener(move |board, _, _, cx| board.select(key.clone(), cx))
        };
        match entry {
            CardEntry::Ticket(vm) => TicketCard::new(id, vm.clone())
                .selected(selected)
                .on_click(click)
                .into_any_element(),
            CardEntry::Epic(vm) => {
                let toggle_key = key.clone();
                EpicCard::new(id, vm.clone())
                    .selected(selected)
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
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap(px(if view.column == Column::Dev { 5. } else { 6. }))
                    .overflow_y_scroll()
                    .track_scroll(&handle)
                    .children(body)
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
        let viewport = window.viewport_size();
        let ws = self.workspace.read(cx);
        let base = ws.current().map(|p| p.base_branch.clone());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or_default();
        let budget = (!self.dev_unfolded).then(|| dev_budget(f32::from(viewport.height)));
        let columns = build(&BoardInput {
            tickets: &ws.tickets,
            visible: None,
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
            .children(dialog)
    }
}
