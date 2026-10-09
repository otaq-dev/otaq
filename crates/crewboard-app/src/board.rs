//! The board (UI-02; *03 · Board — shop-web · 1440 · Light / Dark → Body → Board*, *03 · Board & columns →
//! Empty column & empty board, Very full column · 14 cards, Width strategy*): five columns (Dev with its
//! needs-you cards on top, Agent Working, then Queued), each scrolling on its own with its header pinned and a
//! "N more" chip when there is more below; Dev's fold; Done as a 34 px rail; epic cards in Backlog. It draws
//! [`crate::view_models::board`] from the workspace, which updates cards in place from the service's events.

use std::collections::HashSet;

use crewboard_core::domain::{Column, TicketKey};
use gpui::{
    AnyElement, Context, Entity, EventEmitter, FontWeight, IntoElement, ScrollHandle, SharedString,
    Window, div, prelude::*, px,
};

use crate::components::card::TicketCard;
use crate::components::epic_card::EpicCard;
use crate::icons::{IconName, icon};
use crate::shell::ShellRequest;
use crate::theme::{ActiveTheme, Colors, FONT_MONO, density, radius};
use crate::view_models::board::{
    BoardInput, CardEntry, ColumnView, DevSections, QUEUED_HINT, build, dev_budget, done_collapsed,
};
use crate::workspace::Workspace;

pub struct BoardView {
    workspace: Entity<Workspace>,
    pub selected: Option<TicketKey>,
    dev_unfolded: bool,
    /// The user opened or closed the Done rail; else it follows the window width.
    done_choice: Option<bool>,
    collapsed_epics: HashSet<TicketKey>,
    scrolls: Vec<ScrollHandle>,
}

impl EventEmitter<ShellRequest> for BoardView {}

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
        }
    }

    fn select(&mut self, key: TicketKey, cx: &mut Context<Self>) {
        self.selected = Some(key);
        cx.notify();
    }

    fn card(&self, entry: &CardEntry, cx: &mut Context<Self>) -> AnyElement {
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
        div()
            .id(("column", i))
            .flex_1()
            .min_w(px(density::COLUMN))
            .h_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(6.))
            .rounded(px(radius::LG))
            .bg(c.bg_column)
            .child(self.header(view, c, cx))
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
        div()
            .id("board")
            .flex_1()
            .size_full()
            .flex()
            .gap(px(8.))
            .p(px(10.))
            .overflow_x_scroll()
            .children(columns)
    }
}
