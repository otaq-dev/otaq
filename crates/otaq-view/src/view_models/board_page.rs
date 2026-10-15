//! The board as the Tauri page draws it (T-0220; UI-02, TKT-06, CLS-03, CLS-06, REV-14): [`board::build`]'s columns
//! turned into plain, serialized shapes with every text already worded (counts, hints, the Approved note, the Dev
//! section and fold, the Done rail, the empty board), and the drag the UIT-04 way: [`drag_answer`] gives every
//! column's verdict for one ticket at once, from the core's `check_move` ([`drag::feedback`], [`drag::drop_hint`]),
//! so the page draws the whole drag without asking again; a drop then asks once more what happens
//! ([`drop_step`]): a move, the Park confirmation ([`ParkDialog`]) or the approve choice ([`ApproveMenu`]).

use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Column, Status, TicketKey};
use otaq_core::pr::provider::PrProvider;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::keymap::Platform;
use crate::view_models::approve::{self, ApproveChoice};
use crate::view_models::board::{self, BoardInput, CardEntry, ColumnView, DevSections};
use crate::view_models::card_page::{EpicCardView, TicketCardView};
use crate::view_models::chain;
use crate::view_models::close;
use crate::view_models::drag::{self, DropAction, DropFeedback};
use crate::view_models::empty_board;
use crate::view_models::epic_strip::Strip;
use crate::view_models::search_page::FilterState;

/// A column as the page names it: `"backlog"`, `"ready_for_dev"`, `"dev"`, `"ready_for_review"`, `"approved"`,
/// `"done"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "BoardColumnId")]
pub enum ColumnId {
    Backlog,
    ReadyForDev,
    Dev,
    ReadyForReview,
    Approved,
    Done,
}

impl ColumnId {
    pub fn of(column: Column) -> Self {
        match column {
            Column::Backlog => Self::Backlog,
            Column::ReadyForDev => Self::ReadyForDev,
            Column::Dev => Self::Dev,
            Column::ReadyForReview => Self::ReadyForReview,
            Column::Approved => Self::Approved,
            Column::Done => Self::Done,
        }
    }

    pub fn column(self) -> Column {
        match self {
            Self::Backlog => Column::Backlog,
            Self::ReadyForDev => Column::ReadyForDev,
            Self::Dev => Column::Dev,
            Self::ReadyForReview => Column::ReadyForReview,
            Self::Approved => Column::Approved,
            Self::Done => Column::Done,
        }
    }
}

/// A card on the board. `needs_you` feeds the "7 above · 6 need you" chip of a scrolled column.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "BoardCard")]
pub enum BoardCard {
    Ticket {
        card: Box<TicketCardView>,
        needs_you: bool,
    },
    Epic {
        card: Box<EpicCardView>,
    },
}

impl BoardCard {
    pub fn key(&self) -> &str {
        match self {
            Self::Ticket { card, .. } => &card.key,
            Self::Epic { card } => &card.key,
        }
    }
}

/// A quiet line under a column's title with its icon (Approved: "Otaq doesn’t watch PRs. Mark it when it
/// lands.").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardNote")]
pub struct Note {
    pub icon: IconName,
    pub text: String,
}

/// Dev's parts: the floated cards (pins and needs-you) above the section header, then the working cards and the fold.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "BoardDevSections")]
pub struct DevPage {
    pub top: Vec<BoardCard>,
    /// "WORKING · VALIDATING".
    pub section: String,
    pub working_count: String,
    pub working: Vec<BoardCard>,
    /// "2 more · working" while folded; a click unfolds.
    pub fold: Option<String>,
    /// The keys behind the fold: the fold chip turns accent when one of them is in the highlighted chain (T-0229).
    pub folded: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "BoardColumn")]
pub struct ColumnPage {
    pub id: ColumnId,
    pub title: String,
    pub icon: IconName,
    pub count: String,
    /// Under the title: Ready for Dev's "Starts when ready, in priority order".
    pub hint: Option<String>,
    pub note: Option<Note>,
    /// Shown when the column has no card.
    pub empty_hint: String,
    /// Done only: drawn as the 34 px rail.
    pub collapsed: bool,
    /// Done only: the header button's tooltip ("Collapse Done" / "Expand Done").
    pub toggle: Option<String>,
    /// "6 hidden by filter".
    pub hidden: Option<String>,
    /// Every column but Dev.
    pub cards: Vec<BoardCard>,
    pub dev: Option<DevPage>,
    /// Backlog's "Add ticket" row (and its header's plus).
    pub add: Option<String>,
}

/// The chip of a column scrolled down: `"{n} above"`, then `" · {n} need you"` when any of them need the user. The
/// page counts the cards above its view and fills in `{n}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardScrollChip")]
pub struct ScrollChip {
    pub above: String,
    pub need_you: String,
}

pub const SCROLL_CHIP_ABOVE: &str = "{n} above";
pub const SCROLL_CHIP_NEED_YOU: &str = "{n} need you";

/// The page's board.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "BoardView")]
pub struct BoardPage {
    pub columns: Vec<ColumnPage>,
    /// The centred card of a project with no tickets (the columns stay behind it).
    pub empty: Option<empty_board::View>,
    pub scroll_chip: ScrollChip,
    /// The chain of every linked card, for the highlight on hover or selection (DEP-05, T-0229).
    pub chains: Vec<chain::View>,
    /// The "Epics in flight" strip above the columns, while an epic is a track (EPC-10, T-0269); its stations
    /// follow `columns`.
    pub strip: Option<Strip>,
}

/// What the page sends with `board.view`: the project, the board's size (for the Done rail and Dev's fold) and the
/// user's own choices, kept per project by the page.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(rename = "BoardParams")]
pub struct Params {
    pub project: String,
    pub width: f32,
    pub height: f32,
    /// The user folded (`true`) or opened (`false`) Done; `None` follows the width.
    pub done_collapsed: Option<bool>,
    pub dev_unfolded: bool,
    /// The header's search and filter (T-0221): cards that don't match hide, and each column says how many.
    #[serde(default)]
    pub filter: FilterState,
}

/// The project the board shows, for the empty board's wording.
pub struct ProjectFacts<'a> {
    pub name: &'a str,
    pub repo_path: &'a str,
    pub base_branch: &'a str,
    /// Its Project agents by id and name (cards name the one that created them).
    pub agent_names: &'a [(String, String)],
    /// Interrupted tickets that were validating when the service restarted (T-0244).
    pub was_validating: &'a [TicketKey],
}

pub const APPROVED_NOTE: &str = "doesn\u{2019}t watch PRs. Mark it when it lands.";

fn card(entry: CardEntry, tickets: &[Ticket]) -> Option<BoardCard> {
    Some(match entry {
        CardEntry::Ticket(vm) => {
            let ticket = tickets.iter().find(|t| t.key == vm.key)?;
            BoardCard::Ticket {
                needs_you: ticket.needs_user(),
                card: Box::new(TicketCardView::from_vm(vm, ticket)),
            }
        }
        CardEntry::Epic(vm) => BoardCard::Epic {
            card: Box::new(EpicCardView::from_vm(vm)),
        },
    })
}

fn cards(entries: Vec<CardEntry>, tickets: &[Ticket]) -> Vec<BoardCard> {
    entries
        .into_iter()
        .filter_map(|e| card(e, tickets))
        .collect()
}

fn column_page(view: ColumnView, tickets: &[Ticket]) -> ColumnPage {
    let id = ColumnId::of(view.column);
    let pr_open = view.cards.iter().any(|c| match c {
        CardEntry::Ticket(vm) => tickets
            .iter()
            .any(|t| t.key == vm.key && t.status == Status::PrOpen),
        CardEntry::Epic(_) => false,
    });
    let dev = view.dev.map(|d: DevSections| DevPage {
        top: cards(d.top, tickets),
        section: board::DEV_SECTION.to_owned(),
        working_count: d.working_count.to_string(),
        working: cards(d.working, tickets),
        fold: d.fold,
        folded: d.folded.iter().map(ToString::to_string).collect(),
    });
    ColumnPage {
        id,
        title: view.title.to_owned(),
        icon: view.icon,
        count: view.count,
        hint: view.hint.map(str::to_owned),
        note: (id == ColumnId::Approved && pr_open).then(|| Note {
            icon: IconName::Info,
            text: format!("{} {APPROVED_NOTE}", otaq_core::naming::PRODUCT_NAME),
        }),
        empty_hint: view.empty_hint.to_owned(),
        collapsed: view.collapsed,
        toggle: (id == ColumnId::Done).then(|| {
            if view.collapsed {
                "Expand Done"
            } else {
                "Collapse Done"
            }
            .to_owned()
        }),
        hidden: (view.hidden > 0).then(|| format!("{} hidden by filter", view.hidden)),
        cards: cards(view.cards, tickets),
        dev,
        add: (id == ColumnId::Backlog).then(|| "Add ticket".to_owned()),
    }
}

/// The board for `tickets` of one project, at `params`' size.
pub fn page(
    tickets: &[Ticket],
    project: &ProjectFacts,
    params: &Params,
    now_ms: i64,
    platform: Platform,
) -> BoardPage {
    let visible = params.filter.filter().visible(tickets);
    let input = BoardInput {
        tickets,
        visible: visible.as_deref(),
        project_base: Some(project.base_branch),
        now_ms,
        dev_budget: (!params.dev_unfolded).then(|| board::dev_budget(params.height)),
        done_collapsed: board::done_collapsed(params.done_collapsed, params.width),
        agent_names: project.agent_names,
        was_validating: project.was_validating,
    };
    let built = board::build(&input);
    let shown: Vec<Column> = built.iter().map(|c| c.column).collect();
    BoardPage {
        strip: Strip::from(tickets, &shown),
        columns: built.into_iter().map(|c| column_page(c, tickets)).collect(),
        empty: empty_board::shows(tickets.len(), false)
            .then(|| empty_board::view(project.name, project.repo_path, platform)),
        scroll_chip: ScrollChip {
            above: SCROLL_CHIP_ABOVE.to_owned(),
            need_you: SCROLL_CHIP_NEED_YOU.to_owned(),
        },
        chains: chain::views(tickets),
    }
}

/// How a column looks while the card is over it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "DragVerdict")]
pub enum Verdict {
    /// The card's own column: no styling, dropping does nothing.
    Home,
    /// Accent outline and the hint under the pointer.
    Allowed { icon: IconName, hint: String },
    /// Red outline, a ban cursor and the reason; dropping does nothing.
    Forbidden {
        title: String,
        detail: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ColumnVerdict")]
pub struct ColumnVerdict {
    pub column: ColumnId,
    pub verdict: Verdict,
}

/// The answer to `board.drag`, asked once when a drag starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DragAnswer")]
pub struct DragAnswer {
    pub key: String,
    pub columns: Vec<ColumnVerdict>,
}

pub fn drag_answer(ticket: &Ticket) -> DragAnswer {
    DragAnswer {
        key: ticket.key.to_string(),
        columns: Column::ALL
            .iter()
            .map(|&column| ColumnVerdict {
                column: ColumnId::of(column),
                verdict: match drag::feedback(ticket, column) {
                    DropFeedback::Home => Verdict::Home,
                    DropFeedback::Forbidden { title, detail } => Verdict::Forbidden {
                        title,
                        detail: detail.map(str::to_owned),
                    },
                    DropFeedback::Allowed => {
                        let (icon, hint) = drag::drop_hint(ticket, column)
                            .unwrap_or((IconName::Check, String::new()));
                        Verdict::Allowed { icon, hint }
                    }
                },
            })
            .collect(),
    }
}

/// One thing a confirmation keeps: "otaq/SHOP-25", "worktree · kept with its files".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ConfirmRow")]
pub struct Row {
    pub icon: IconName,
    pub text: String,
}

/// *03 → Park · Dev → Backlog (allowed)*: "Park SHOP-25?", what stops and what is kept (CLS-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ParkDialog")]
pub struct ParkDialog {
    pub key: String,
    pub title: String,
    pub message: String,
    pub kept: Vec<Row>,
    pub confirm: String,
    pub confirm_icon: IconName,
}

pub fn park_dialog(ticket: &Ticket) -> ParkDialog {
    let text = close::park_text(ticket);
    ParkDialog {
        key: ticket.key.to_string(),
        title: text.title,
        message: text.message,
        kept: text
            .kept
            .into_iter()
            .map(|(icon, text)| Row { icon, text })
            .collect(),
        confirm: "Park".to_owned(),
        confirm_icon: IconName::SquareParking,
    }
}

/// One choice of the approve popover, with its number key ("1 Local merge").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ApproveItem")]
pub struct ApproveItem {
    pub choice: ApproveChoice,
    pub number: u32,
    pub icon: IconName,
    pub title: String,
    pub detail: String,
    pub enabled: bool,
}

/// *03 → Approve choice on drop*: a drop on Approved never approves by itself; it asks how (UI-02, T-0095).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ApproveMenu")]
pub struct ApproveMenu {
    pub key: String,
    pub title: String,
    pub items: Vec<ApproveItem>,
    pub note: String,
}

fn choice_icon(choice: ApproveChoice) -> IconName {
    match choice {
        ApproveChoice::LocalMerge => IconName::GitMerge,
        ApproveChoice::OpenPr => IconName::GitPullRequest,
        ApproveChoice::PushToPr => IconName::Upload,
    }
}

pub fn approve_menu(
    ticket: &Ticket,
    target: &str,
    tickets: &[Ticket],
    pr: Option<&PrProvider>,
) -> ApproveMenu {
    ApproveMenu {
        key: ticket.key.to_string(),
        title: approve::menu_title(ticket),
        items: approve::options(ticket, target, tickets, pr)
            .into_iter()
            .zip(1..)
            .map(|(o, number)| ApproveItem {
                choice: o.choice,
                number,
                icon: choice_icon(o.choice),
                title: o.title,
                detail: o.detail,
                enabled: o.enabled,
            })
            .collect(),
        note: approve::menu_note(ticket),
    }
}

/// What a drop on `column` asks of the app, before anything runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropStep {
    /// Its own column, or refused: the card flies back.
    Nothing,
    /// Send `ticket.move`.
    Move,
    /// Dev → Backlog: the Park confirmation first.
    Park,
    /// Ready for Review → Approved: the approve choice.
    Approve,
    NotYet(String),
}

pub fn drop_step(ticket: &Ticket, column: Column) -> DropStep {
    match drag::on_drop(ticket, column) {
        DropAction::Nothing => DropStep::Nothing,
        DropAction::Move => DropStep::Move,
        DropAction::AskPark => DropStep::Park,
        DropAction::ChooseApprove => DropStep::Approve,
        DropAction::NotYet(message) => DropStep::NotYet(message),
    }
}

/// The answer to `board.drop`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "DropOutcome")]
pub enum DropOutcome {
    /// Nothing happened: the card flies back.
    Nothing,
    /// The service carried out the move; the board refetches on its event.
    Moved,
    /// The page shows the confirmation, then sends `board.park`.
    Park { dialog: ParkDialog },
    /// The page shows the popover; a choice opens its dialog.
    Approve { menu: ApproveMenu },
    /// Allowed by the rules, but not built yet: shown as a toast.
    NotYet { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::{NOW, ticket};
    use otaq_core::domain::Kind;

    fn facts() -> ProjectFacts<'static> {
        ProjectFacts {
            name: "shop-web",
            repo_path: "/tmp/shop-web",
            base_branch: "main",
            agent_names: &[],
            was_validating: &[],
        }
    }

    fn params(width: f32) -> Params {
        Params {
            project: "SHOP".into(),
            width,
            height: 856.,
            done_collapsed: None,
            dev_unfolded: false,
            filter: FilterState::default(),
        }
    }

    fn sample() -> Vec<Ticket> {
        let mut pinned = ticket("SHOP-19", "Price table", Status::Working);
        pinned.flags.pinned_at = Some(10);
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
        pr.flags.pr_number = Some(481);
        vec![
            ticket("SHOP-24", "Update footer links", Status::Backlog),
            epic,
            pinned,
            ticket("SHOP-22", "Fix flaky login redirect", Status::NeedsYou),
            ticket("SHOP-27", "Search results paging", Status::Working),
            ticket("SHOP-21", "Order confirmation email", Status::Queued),
            ticket("SHOP-18", "Checkout API", Status::ReadyForReview),
            pr,
            ticket("SHOP-17", "Add Stripe client", Status::Merged),
        ]
    }

    /// EPC-11 on the page: the epic card carries its rows (draggable before anything starts); once SHOP-45.1 is
    /// queued the epic is no Backlog card and SHOP-45.2 is no card at all, and Backlog counts what it shows.
    #[test]
    fn epics_and_children_show_once() {
        let mut epic = ticket("SHOP-45", "Search", Status::Backlog);
        epic.kind = Kind::Epic;
        let child = |key: &str, status| {
            let mut t = ticket(key, "Child", status);
            t.epic = Some("SHOP-45".parse().unwrap());
            t
        };
        let mut tickets = vec![
            epic,
            child("SHOP-45.1", Status::Backlog),
            child("SHOP-45.2", Status::Backlog),
        ];
        let board = page(&tickets, &facts(), &params(1376.), NOW, Platform::Mac);
        let backlog = &board.columns[0];
        assert_eq!(backlog.count, "1");
        let [BoardCard::Epic { card }] = backlog.cards.as_slice() else {
            panic!("one epic card: {:?}", backlog.cards)
        };
        let rows: Vec<(&str, bool)> = card
            .children
            .iter()
            .map(|c| (c.key.as_str(), c.draggable))
            .collect();
        assert_eq!(rows, [("SHOP-45.1", true), ("SHOP-45.2", true)]);

        tickets[1].status = Status::Queued;
        tickets[1].column = Column::ReadyForDev;
        let board = page(&tickets, &facts(), &params(1376.), NOW, Platform::Mac);
        assert!(board.columns[0].cards.is_empty());
        assert_eq!(board.columns[0].count, "0");
        let ready: Vec<&str> = board.columns[1].cards.iter().map(BoardCard::key).collect();
        assert_eq!(ready, ["SHOP-45.1"]);
    }

    /// EPC-10 on the page: no strip while the epic is a card; once SHOP-45.1 is queued the page carries the
    /// strip, one station per shown column but Backlog, with the queued child under Ready for Dev and the other
    /// as a "not queued" chip.
    #[test]
    fn the_page_carries_the_strip() {
        let mut epic = ticket("SHOP-45", "Search", Status::Backlog);
        epic.kind = Kind::Epic;
        let child = |key: &str, status| {
            let mut t = ticket(key, "Child", status);
            t.epic = Some("SHOP-45".parse().unwrap());
            t
        };
        let mut tickets = vec![
            epic,
            child("SHOP-45.1", Status::Backlog),
            child("SHOP-45.2", Status::Backlog),
            ticket("SHOP-51", "Fix checkout rounding", Status::Working),
        ];
        let board = page(&tickets, &facts(), &params(1376.), NOW, Platform::Mac);
        assert_eq!(board.strip, None);

        tickets[1].status = Status::Queued;
        tickets[1].column = Column::ReadyForDev;
        tickets[1].queued_at = Some(1_000);
        let board = page(&tickets, &facts(), &params(1376.), NOW, Platform::Mac);
        let strip = board.strip.clone().expect("a track");
        assert_eq!(strip.title, "Epics in flight · 1");
        let [track] = strip.tracks.as_slice() else {
            panic!("one track: {:?}", strip.tracks)
        };
        assert_eq!(track.key, "SHOP-45");
        let stations: Vec<(ColumnId, Vec<&str>)> = track
            .stations
            .iter()
            .map(|s| (s.column, s.stops.iter().map(|s| s.key.as_str()).collect()))
            .collect();
        let expected: Vec<(ColumnId, Vec<&str>)> = board.columns[1..]
            .iter()
            .map(|c| {
                let keys = if c.id == ColumnId::ReadyForDev {
                    vec!["SHOP-45.1"]
                } else {
                    vec![]
                };
                (c.id, keys)
            })
            .collect();
        assert_eq!(stations, expected);
        assert_eq!(track.not_queued[0].key, "SHOP-45.2");
        let json = serde_json::to_value(&board).unwrap();
        assert_eq!(
            json["strip"]["tracks"][0]["stations"][0]["column"],
            "ready_for_dev"
        );
    }

    #[test]
    fn six_columns_reach_the_page_worded() {
        let tickets = sample();
        let page = page(&tickets, &facts(), &params(1376.), NOW, Platform::Mac);
        let ids: Vec<ColumnId> = page.columns.iter().map(|c| c.id).collect();
        assert_eq!(
            ids,
            [
                ColumnId::Backlog,
                ColumnId::ReadyForDev,
                ColumnId::Dev,
                ColumnId::ReadyForReview,
                ColumnId::Approved,
                ColumnId::Done
            ]
        );
        assert_eq!(page.empty, None);
        let backlog = &page.columns[0];
        assert_eq!(backlog.add.as_deref(), Some("Add ticket"));
        assert!(
            backlog
                .cards
                .iter()
                .any(|c| matches!(c, BoardCard::Epic { .. }))
        );
        assert_eq!(
            page.columns[1].hint.as_deref(),
            Some(board::READY_FOR_DEV_HINT)
        );
        let dev = page.columns[2].dev.as_ref().unwrap();
        let top: Vec<&str> = dev.top.iter().map(BoardCard::key).collect();
        assert_eq!(top, ["SHOP-19", "SHOP-22"], "pinned, then needs you");
        assert!(matches!(
            dev.top[1],
            BoardCard::Ticket {
                needs_you: true,
                ..
            }
        ));
        assert_eq!(dev.section, "WORKING · VALIDATING");
        let approved = &page.columns[4];
        assert_eq!(
            approved.note.as_ref().unwrap().text,
            format!(
                "{} doesn’t watch PRs. Mark it when it lands.",
                otaq_core::naming::PRODUCT_NAME
            )
        );
        assert_eq!(page.columns[3].note, None);
        let done = &page.columns[5];
        assert!(!done.collapsed, "1440: all six open");
        assert_eq!(done.toggle.as_deref(), Some("Collapse Done"));
        let json = serde_json::to_value(&page).unwrap();
        assert_eq!(json["columns"][1]["id"], "ready_for_dev");
        assert_eq!(json["columns"][0]["cards"][0]["kind"], "ticket");
        assert_eq!(json["scroll_chip"]["above"], "{n} above");
    }

    #[test]
    fn done_folds_at_1280_and_the_user_choice_wins() {
        let tickets = sample();
        let narrow = page(&tickets, &facts(), &params(1216.), NOW, Platform::Mac);
        assert!(narrow.columns[5].collapsed);
        assert_eq!(narrow.columns[5].toggle.as_deref(), Some("Expand Done"));
        let opened = page(
            &tickets,
            &facts(),
            &Params {
                done_collapsed: Some(false),
                ..params(1216.)
            },
            NOW,
            Platform::Mac,
        );
        assert!(!opened.columns[5].collapsed);
    }

    #[test]
    fn dev_folds_by_height_until_unfolded() {
        let mut tickets = sample();
        for n in 50..60 {
            tickets.push(ticket(&format!("SHOP-{n}"), "Work", Status::Working));
        }
        let short = Params {
            height: 400.,
            ..params(1376.)
        };
        let folded = page(&tickets, &facts(), &short, NOW, Platform::Mac);
        let dev = folded.columns[2].dev.as_ref().unwrap();
        assert!(dev.fold.as_deref().unwrap().ends_with("more · working"));
        let open = page(
            &tickets,
            &facts(),
            &Params {
                dev_unfolded: true,
                ..short
            },
            NOW,
            Platform::Mac,
        );
        assert_eq!(open.columns[2].dev.as_ref().unwrap().fold, None);
    }

    /// The header's filter (T-0221): non-matching cards hide, counts read "shown/total", and each column says how
    /// many it hides.
    #[test]
    fn the_filter_hides_cards_and_columns_say_how_many() {
        let tickets = sample();
        let filtered = page(
            &tickets,
            &facts(),
            &Params {
                filter: FilterState {
                    tokens: vec!["is:needs-me".into()],
                    text: String::new(),
                },
                ..params(1376.)
            },
            NOW,
            Platform::Mac,
        );
        let dev = &filtered.columns[2];
        let shown: Vec<&str> = dev
            .dev
            .as_ref()
            .unwrap()
            .top
            .iter()
            .map(BoardCard::key)
            .collect();
        assert_eq!(shown, ["SHOP-22"]);
        assert_eq!(dev.count, "1/3");
        assert_eq!(dev.hidden.as_deref(), Some("2 hidden by filter"));
        assert!(filtered.columns[0].cards.is_empty());
        let review: Vec<&str> = filtered.columns[3]
            .cards
            .iter()
            .map(BoardCard::key)
            .collect();
        assert_eq!(review, ["SHOP-18"], "review needs you too");
        let unfiltered = page(&tickets, &facts(), &params(1376.), NOW, Platform::Mac);
        assert_eq!(unfiltered.columns[2].hidden, None);
    }

    #[test]
    fn an_empty_project_shows_the_empty_board_over_its_columns() {
        let page = page(&[], &facts(), &params(1376.), NOW, Platform::Mac);
        let empty = page.empty.unwrap();
        assert_eq!(empty.title, "shop-web is ready");
        assert_eq!(page.columns.len(), 6);
        assert!(
            page.columns[4]
                .empty_hint
                .starts_with("Drag a ticket here from Ready for Review")
        );
    }

    #[test]
    fn a_drag_answers_every_column_at_once() {
        let backlog = ticket("SHOP-24", "Update footer links", Status::Backlog);
        let answer = drag_answer(&backlog);
        assert_eq!(answer.columns.len(), 6);
        let verdict = |id: ColumnId| {
            answer
                .columns
                .iter()
                .find(|c| c.column == id)
                .unwrap()
                .verdict
                .clone()
        };
        assert_eq!(verdict(ColumnId::Backlog), Verdict::Home);
        assert_eq!(
            verdict(ColumnId::ReadyForDev),
            Verdict::Allowed {
                icon: IconName::Play,
                hint: "Drop to queue · starts in priority order".into()
            }
        );
        assert_eq!(
            verdict(ColumnId::ReadyForReview),
            Verdict::Forbidden {
                title: "Only agents move tickets to Ready for Review".into(),
                detail: Some("Drag to Ready for Dev to queue it.".into())
            },
            "*Drag · over a forbidden column*"
        );
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        let answer = drag_answer(&epic);
        for id in [ColumnId::ReadyForDev, ColumnId::Dev] {
            let v = &answer
                .columns
                .iter()
                .find(|c| c.column == id)
                .unwrap()
                .verdict;
            assert!(
                matches!(v, Verdict::Allowed { .. }),
                "an epic drop queues its children (d0080)"
            );
        }
        let json = serde_json::to_value(drag_answer(&backlog)).unwrap();
        assert_eq!(json["columns"][0]["verdict"]["kind"], "home");
    }

    #[test]
    fn drops_say_what_comes_next() {
        let working = ticket("SHOP-25", "Upgrade to Next.js 15", Status::Working);
        assert_eq!(drop_step(&working, Column::Backlog), DropStep::Park);
        let park = park_dialog(&working);
        assert_eq!(park.title, "Park SHOP-25?");
        assert!(park.message.contains("will be stopped"), "{}", park.message);
        assert_eq!(park.kept.len(), 3);
        let review = ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
        assert_eq!(drop_step(&review, Column::Approved), DropStep::Approve);
        let menu = approve_menu(&review, "main", std::slice::from_ref(&review), None);
        assert_eq!(menu.title, "Approve SHOP-18 Checkout API");
        assert_eq!(menu.items[0].choice, ApproveChoice::LocalMerge);
        assert_eq!((menu.items[0].number, menu.items[1].number), (1, 2));
        assert_eq!(menu.items[1].icon, IconName::GitPullRequest);
        assert_eq!(
            menu.note,
            "Both open a confirmation step before anything runs."
        );
        let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
        pr.flags.pr_number = Some(481);
        assert_eq!(
            drop_step(&pr, Column::ReadyForReview),
            DropStep::Move,
            "PR rework"
        );
        let closed = ticket("SHOP-30", "Order history", Status::ClosedNotMerged);
        assert_eq!(
            drop_step(&closed, Column::Backlog),
            DropStep::Move,
            "Reopen"
        );
        assert_eq!(drop_step(&closed, Column::Done), DropStep::Nothing);
        let json = serde_json::to_value(DropOutcome::Park { dialog: park }).unwrap();
        assert_eq!(json["kind"], "park");
        assert_eq!(json["dialog"]["confirm"], "Park");
    }
    /// Frame 12's chain at one storyboard step: SHOP-17 merged, SHOP-18 and SHOP-19 as given, SHOP-20 needs both,
    /// SHOP-21 needs SHOP-20.
    fn storyboard(s18: Status, s19: Status, s20: Status) -> Vec<Ticket> {
        let with = |key: &str, title: &str, status: Status, deps: &[&str]| {
            let mut t = ticket(key, title, status);
            t.depends_on = deps.iter().map(|d| d.parse().unwrap()).collect();
            t
        };
        let mut shop_20 = with("SHOP-20", "Checkout page", s20, &["SHOP-18", "SHOP-19"]);
        if s20 == Status::Working {
            shop_20.work = Some(otaq_core::board::tickets::Work {
                base_sha: Some("3b7d0e1c9a".into()),
                ..Default::default()
            });
        }
        vec![
            with("SHOP-17", "Add Stripe client", Status::Merged, &[]),
            with("SHOP-18", "Checkout API", s18, &["SHOP-17"]),
            with("SHOP-19", "Price table", s19, &[]),
            shop_20,
            with(
                "SHOP-21",
                "Order confirmation email",
                Status::Queued,
                &["SHOP-20"],
            ),
        ]
    }

    /// One card of the page as JSON text, to look for the words the storyboard draws on it.
    fn card_text(page: &BoardPage, key: &str) -> String {
        page.columns
            .iter()
            .flat_map(|c| {
                let dev = c.dev.iter().flat_map(|d| d.top.iter().chain(&d.working));
                c.cards.iter().chain(dev)
            })
            .find(|c| c.key() == key)
            .map(|c| serde_json::to_string(c).unwrap())
            .unwrap_or_else(|| panic!("{key} is on the board"))
    }

    /// *12 · Stack & dependencies → 2 · The chain runs … 6 · Closed parent*, as the page receives them (the hand
    /// check walks them on the real board).
    #[test]
    fn the_storyboards_reach_the_page() {
        let at = |s18, s19, s20| {
            let tickets = storyboard(s18, s19, s20);
            page(&tickets, &facts(), &params(1376.), NOW, Platform::Mac)
        };
        // 1 · Planning: the dependency badges.
        let planning = at(Status::Backlog, Status::Backlog, Status::Backlog);
        assert!(card_text(&planning, "SHOP-20").contains("SHOP-18 SHOP-19"));
        // 2 · The chain runs, then SHOP-18 in Review.
        let runs = at(Status::Working, Status::Working, Status::Queued);
        assert!(card_text(&runs, "SHOP-20").contains("waits for SHOP-18, SHOP-19 to merge"));
        assert!(card_text(&runs, "SHOP-21").contains("waits for SHOP-20 to merge"));
        let review = at(Status::ReadyForReview, Status::Working, Status::Queued);
        assert!(card_text(&review, "SHOP-20").contains("SHOP-18 in Review"));
        // 3 · One parent merged.
        let one = at(Status::Merged, Status::ReadyForReview, Status::Queued);
        assert!(card_text(&one, "SHOP-20").contains("waits for SHOP-19 to merge"));
        // 4 · All parents merged: SHOP-20 works from the new main; SHOP-21 still waits.
        let all = at(Status::Merged, Status::Merged, Status::Working);
        assert!(
            card_text(&all, "SHOP-20").contains("from main @ 3b7d0e1 · all dependencies merged")
        );
        assert!(card_text(&all, "SHOP-21").contains("waits for SHOP-20 to merge"));
        // 5 · PR variant.
        let mut tickets = storyboard(Status::PrOpen, Status::Merged, Status::Queued);
        tickets[1].flags.pr_number = Some(482);
        let pr = page(&tickets, &facts(), &params(1376.), NOW, Platform::Mac);
        assert!(card_text(&pr, "SHOP-20").contains("PR #482 open"));
        assert!(card_text(&pr, "SHOP-18").contains("Mark merged"));
        // 6 · Closed parent never counts as merged.
        let closed = at(
            Status::Merged,
            Status::ClosedNotMerged,
            Status::ParentClosed,
        );
        let shop_20 = card_text(&closed, "SHOP-20");
        assert!(shop_20.contains("Parent closed"), "{shop_20}");
        assert!(shop_20.contains("SHOP-19 was closed without merging"));
        // The highlight of SHOP-20 lights the whole chain.
        let chain = all.chains.iter().find(|c| c.key == "SHOP-20").unwrap();
        assert_eq!(
            chain.members,
            ["SHOP-17", "SHOP-18", "SHOP-19", "SHOP-20", "SHOP-21"]
        );
        assert!(planning.chains.iter().all(|c| c.key != "SHOP-22"));
    }
}
