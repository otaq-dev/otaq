//! The board's columns (UI-02; *03 · Board — shop-web*, *03 · Board & columns*): six columns with counts;
//! Ready for Dev with the queued cards (d0066); Dev with the cards that need the user (and pins) on top, then
//! the Agent Working section; the "N more · working" fold that never hides needs-you cards (T-0027); the Done
//! column that collapses to a 34 px rail; epic cards in Backlog; and the empty-column hints. Order comes from
//! the core ([`crewboard_core::board::order`]), so the board, the service and the tests agree.

use crewboard_core::board::order::{ColumnOrder, fold_dev, sort_column};
use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Column, Kind, Status, TicketKey};

use crate::icons::IconName;
use crate::view_models::card::{CardExtras, CardViewModel};
use crate::view_models::epic_card::EpicViewModel;
use crate::view_models::queue::queue_facts;

/// Below this board width Done starts as a rail (*Width strategy · six columns*: "B · 1280 or panel open: Done
/// becomes a 34 px rail"). The board is 1216 px at the 1280 minimum, 1376 at 1440 and 1369 just past 1600, where
/// the 232 px sidebar replaces the rail; so all six columns are open from 1440 up unless the side panel takes the
/// room.
pub const DONE_OPEN_FROM_WIDTH: f32 = 1368.;

#[derive(Debug, Clone, PartialEq)]
pub enum CardEntry {
    Ticket(CardViewModel),
    Epic(EpicViewModel),
}

impl CardEntry {
    pub fn key(&self) -> &TicketKey {
        match self {
            Self::Ticket(t) => &t.key,
            Self::Epic(e) => &e.key,
        }
    }
}

/// Dev's parts, top to bottom.
#[derive(Debug, Clone, PartialEq)]
pub struct DevSections {
    /// Pinned cards and cards that need the user, above the section headers (never folded).
    pub top: Vec<CardEntry>,
    pub working: Vec<CardEntry>,
    /// Cards in the Agent Working section, the floated ones included (section counts count by status).
    pub working_count: usize,
    /// "4 more · working" while folded.
    pub fold: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnView {
    pub column: Column,
    pub title: &'static str,
    pub icon: IconName,
    /// "12" or, with a filter, "3/12" (T-0038).
    pub count: String,
    /// Every column but Dev.
    pub cards: Vec<CardEntry>,
    pub dev: Option<DevSections>,
    /// Under the title: Ready for Dev's "Starts when ready, in priority order".
    pub hint: Option<&'static str>,
    /// Shown when the column is empty.
    pub empty_hint: &'static str,
    /// Done only: drawn as a narrow rail.
    pub collapsed: bool,
    /// Cards a filter hides in this column ("6 hidden by filter").
    pub hidden: usize,
}

/// The column's icon and empty hint.
pub fn column_look(column: Column) -> (IconName, &'static str) {
    match column {
        Column::Backlog => (
            IconName::CircleDashed,
            "Write a ticket here: what to build and when it’s done.",
        ),
        Column::ReadyForDev => (
            IconName::Hourglass,
            "Drag a ticket here from Backlog to start it when it’s ready.",
        ),
        Column::Dev => (
            IconName::Activity,
            "Tickets move here when their agent starts. Each agent works in its own worktree.",
        ),
        Column::ReadyForReview => (
            IconName::Eye,
            "Agents move their ticket here when they submit it for your review.",
        ),
        Column::Approved => (
            IconName::Check,
            "Drag a ticket here from Ready for Review to approve it. It always asks how: Local merge or Open PR.",
        ),
        Column::Done => (
            IconName::CircleCheck,
            "Merged and closed tickets end up here.",
        ),
    }
}

/// The hint under Ready for Dev's title (*03 · Board — shop-web*).
pub const READY_FOR_DEV_HINT: &str = "Starts when ready, in priority order";

/// Dev's section header above its working cards.
pub const DEV_SECTION: &str = "WORKING · VALIDATING";

/// Whether Done shows as a rail: the user's choice, else by the board's width (the window less the rail or
/// sidebar and the side panel).
pub fn done_collapsed(user_choice: Option<bool>, board_width: f32) -> bool {
    user_choice.unwrap_or(board_width < DONE_OPEN_FROM_WIDTH)
}

/// The columns scrolled out of the board's view, as the chip at that edge names them (*02 · App shell & project
/// switcher → 1280 × 800 · minimum*: a narrower board "scrolls horizontally with a fade and a chip naming the
/// hidden columns").
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HiddenColumns {
    /// Off the left edge, once the board is scrolled ("Backlog · Dev").
    pub before: Option<String>,
    /// Off the right edge ("Approved · Done").
    pub after: Option<String>,
}

/// Which columns are out of view: `columns` are each column's title and its left and right edge, `view` the
/// visible span, in the same coordinates. A column is hidden as soon as more than a pixel of it is cut off.
pub fn hidden_columns(columns: &[(&str, f32, f32)], view: (f32, f32)) -> HiddenColumns {
    let names = |hidden: &dyn Fn(f32, f32) -> bool| {
        let names: Vec<&str> = columns
            .iter()
            .filter(|(_, left, right)| hidden(*left, *right))
            .map(|(title, ..)| *title)
            .collect();
        (!names.is_empty()).then(|| names.join(" · "))
    };
    HiddenColumns {
        before: names(&|left, _| left < view.0 - 1.),
        after: names(&|_, right| right > view.1 + 1.),
    }
}

/// How many Dev cards fit before the fold, from the board's height (a card is about 96 px with its gap).
pub fn dev_budget(board_height: f32) -> usize {
    (((board_height - 120.) / 96.).floor().max(4.)) as usize
}

pub struct BoardInput<'a> {
    pub tickets: &'a [Ticket],
    /// Tickets shown (a filter, T-0038); `None` shows all.
    pub visible: Option<&'a [TicketKey]>,
    pub project_base: Option<&'a str>,
    pub now_ms: i64,
    /// Dev's card budget; `None` when the user unfolded Dev.
    pub dev_budget: Option<usize>,
    pub done_collapsed: bool,
}

pub fn build(input: &BoardInput) -> Vec<ColumnView> {
    let all = input.tickets;
    let shown: Vec<Ticket> = match input.visible {
        Some(keys) => all
            .iter()
            .filter(|t| keys.contains(&t.key))
            .cloned()
            .collect(),
        None => all.to_vec(),
    };
    let entry = |key: &TicketKey| -> Option<CardEntry> {
        let t = all.iter().find(|t| &t.key == key)?;
        Some(if t.kind == Kind::Epic {
            CardEntry::Epic(EpicViewModel::from(t, all, None))
        } else {
            // What a Queued card waits for, from the core's readiness: updates as each parent moves (T-0083).
            let queue = queue_facts(t, all);
            let extras = CardExtras {
                waits_for: queue.waits_for,
                waits_for_merge: queue.waits_for_merge,
                starting: queue.starting,
                closed_parent: queue.closed_parent,
                project_base: input.project_base.map(str::to_owned),
                epic_title: t
                    .epic
                    .as_ref()
                    .and_then(|e| all.iter().find(|x| &x.key == e))
                    .map(|e| e.title.clone()),
                start_failure: (t.status == Status::StartInterrupted)
                    .then(|| {
                        t.start_problem
                            .as_ref()
                            .map(|p| p.step.failure_label().to_owned())
                    })
                    .flatten(),
                held_reason: (t.status == Status::Held)
                    .then(|| t.start_problem.as_ref().map(|p| p.message.clone()))
                    .flatten(),
                held_by_limit: crewboard_core::scheduler::held_by_plan_limit(t),
                ..CardExtras::default()
            }
            .with_signal(t, input.now_ms);
            CardEntry::Ticket(CardViewModel::from(t, &extras, input.now_ms))
        })
    };
    let entries = |keys: &[TicketKey]| keys.iter().filter_map(entry).collect::<Vec<_>>();
    Column::ALL
        .iter()
        .map(|column| {
            // Order over the whole project ("blocks more" counts every dependent), shown cards only.
            let full = sort_column(*column, all);
            let keep = |keys: &[TicketKey]| -> Vec<TicketKey> {
                keys.iter()
                    .filter(|k| shown.iter().any(|t| &t.key == *k))
                    .cloned()
                    .collect()
            };
            let order = ColumnOrder {
                column: *column,
                cards: keep(&full.cards),
            };
            let total = full.cards.len();
            let visible = order.cards.len();
            let count = if input.visible.is_some() {
                format!("{visible}/{total}")
            } else {
                total.to_string()
            };
            let (icon, empty_hint) = column_look(*column);
            let mut view = ColumnView {
                column: *column,
                title: column.label(),
                icon,
                count,
                cards: Vec::new(),
                dev: None,
                hint: (*column == Column::ReadyForDev).then_some(READY_FOR_DEV_HINT),
                empty_hint,
                collapsed: *column == Column::Done && input.done_collapsed,
                hidden: total - visible,
            };
            if *column == Column::Dev {
                view.dev = Some(dev_sections(&order, all, input.dev_budget, &entries));
            } else {
                view.cards = entries(&order.cards);
            }
            view
        })
        .collect()
}

fn dev_sections(
    order: &ColumnOrder,
    all: &[Ticket],
    budget: Option<usize>,
    entries: &dyn Fn(&[TicketKey]) -> Vec<CardEntry>,
) -> DevSections {
    let floats = |k: &TicketKey| {
        all.iter()
            .find(|t| &t.key == k)
            .is_some_and(|t| t.needs_user() || t.flags.is_pinned())
    };
    let (shown_cards, fold) = match budget {
        Some(max) => {
            let f = fold_dev(order, all, max);
            let label = f.label();
            (f.shown_cards, label)
        }
        None => (order.cards.clone(), None),
    };
    let top: Vec<TicketKey> = shown_cards.iter().filter(|k| floats(k)).cloned().collect();
    let working: Vec<TicketKey> = shown_cards.iter().filter(|k| !floats(k)).cloned().collect();
    DevSections {
        top: entries(&top),
        working: entries(&working),
        working_count: order.cards.len(),
        fold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::{NOW, ticket};
    use crewboard_core::domain::Status;

    fn board() -> Vec<Ticket> {
        let mut pinned = ticket("SHOP-19", "Price table", Status::Working);
        pinned.flags.pinned_at = Some(10);
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        let mut child = ticket("SHOP-40.3", "Wishlist page", Status::Backlog);
        child.epic = Some("SHOP-40".parse().unwrap());
        let mut t20 = ticket("SHOP-20", "Checkout page", Status::Queued);
        t20.depends_on = vec!["SHOP-18".parse().unwrap()];
        vec![
            ticket("SHOP-24", "Update footer links", Status::Backlog),
            epic,
            child,
            pinned,
            ticket("SHOP-22", "Fix flaky login redirect", Status::NeedsYou),
            ticket("SHOP-23", "Split user full name", Status::AsksYou),
            ticket("SHOP-25", "Upgrade to Next.js 15", Status::Interrupted),
            ticket("SHOP-28", "Cart badge count", Status::StartInterrupted),
            ticket("SHOP-27", "Search results paging", Status::Working),
            ticket(
                "SHOP-12",
                "Add dark mode toggle",
                Status::WorkingInBackground,
            ),
            ticket("SHOP-26", "Add sitemap", Status::Working),
            t20,
            ticket("SHOP-21", "Order confirmation email", Status::Queued),
            ticket("SHOP-29", "Coupons", Status::Held),
            ticket("SHOP-18", "Checkout API", Status::ReadyForReview),
            ticket("SHOP-17", "Add Stripe client", Status::Merged),
        ]
    }

    fn keys(entries: &[CardEntry]) -> Vec<String> {
        entries.iter().map(|e| e.key().to_string()).collect()
    }

    fn input(tickets: &[Ticket], budget: Option<usize>) -> BoardInput<'_> {
        BoardInput {
            tickets,
            visible: None,
            project_base: Some("main"),
            now_ms: NOW,
            dev_budget: budget,
            done_collapsed: false,
        }
    }

    /// T-0075: a working card that turns orange moves up to the top of Dev, below the pinned card, and back
    /// down when the signal clears.
    #[test]
    fn orange_cards_float_up_as_signals_change() {
        let mut tickets = board();
        let dev = |tickets: &[Ticket]| build(&input(tickets, None))[2].dev.clone().unwrap();
        assert!(!keys(&dev(&tickets).top).contains(&"SHOP-26".to_owned()));
        let shop26 = tickets
            .iter_mut()
            .find(|t| t.key.to_string() == "SHOP-26")
            .unwrap();
        shop26.status = Status::Idle;
        let top = keys(&dev(&tickets).top);
        assert_eq!(top[0], "SHOP-19", "pinned first");
        assert!(top.contains(&"SHOP-26".to_owned()), "{top:?}");
        let shop26 = tickets
            .iter_mut()
            .find(|t| t.key.to_string() == "SHOP-26")
            .unwrap();
        shop26.status = Status::Working;
        assert!(!keys(&dev(&tickets).top).contains(&"SHOP-26".to_owned()));
    }

    /// T-0083: Queued cards render from the readiness reason and update as a parent moves.
    #[test]
    fn queued_cards_say_what_they_wait_for_and_update_live() {
        let mut tickets = board();
        let card = |tickets: &[Ticket], key: &str| -> CardViewModel {
            build(&input(tickets, None))[1]
                .cards
                .clone()
                .into_iter()
                .find_map(|e| match e {
                    CardEntry::Ticket(vm) if vm.key.to_string() == key => Some(vm),
                    _ => None,
                })
                .unwrap()
        };
        // SHOP-18 is in Review: a callout.
        let vm = card(&tickets, "SHOP-20");
        assert_eq!(
            vm.callout.unwrap().text,
            "waits for SHOP-18 to merge (in Review)"
        );
        // SHOP-18 merged: about to start.
        let shop18 = tickets
            .iter_mut()
            .find(|t| t.key.to_string() == "SHOP-18")
            .unwrap();
        shop18.status = Status::Merged;
        let vm = card(&tickets, "SHOP-20");
        assert_eq!(vm.pill.unwrap().label, "Starting…");
        assert_eq!(vm.callout, None);
    }

    #[test]
    fn six_columns_with_counts_and_epics_in_backlog() {
        let tickets = board();
        let columns = build(&input(&tickets, None));
        let titles: Vec<_> = columns.iter().map(|c| c.title).collect();
        assert_eq!(
            titles,
            [
                "Backlog",
                "Ready for Dev",
                "Dev",
                "Ready for Review",
                "Approved",
                "Done"
            ]
        );
        let counts: Vec<_> = columns.iter().map(|c| c.count.as_str()).collect();
        assert_eq!(counts, ["3", "4", "7", "1", "0", "1"]);
        assert!(
            columns[0]
                .cards
                .iter()
                .any(|c| matches!(c, CardEntry::Epic(_)))
        );
        // Ready for Dev: Start interrupted first, then the start order (the one waiting for SHOP-18 last).
        assert_eq!(
            keys(&columns[1].cards),
            ["SHOP-28", "SHOP-21", "SHOP-29", "SHOP-20"]
        );
        assert_eq!(columns[1].hint, Some(READY_FOR_DEV_HINT));
        assert!(columns.iter().filter(|c| c.hint.is_some()).count() == 1);
        assert!(columns[4].cards.is_empty());
        assert!(
            columns[4]
                .empty_hint
                .starts_with("Drag a ticket here from Ready for Review")
        );
    }

    #[test]
    fn dev_sections_float_needs_you_and_pins() {
        let tickets = board();
        let columns = build(&input(&tickets, None));
        let dev = columns[2].dev.as_ref().unwrap();
        assert_eq!(keys(&dev.top), ["SHOP-19", "SHOP-22", "SHOP-23", "SHOP-25"]);
        assert_eq!(keys(&dev.working), ["SHOP-12", "SHOP-26", "SHOP-27"]);
        assert_eq!(dev.working_count, 7);
        assert_eq!(dev.fold, None);
    }

    #[test]
    fn fold_counts_and_never_hides_needs_you() {
        let tickets = board();
        let columns = build(&input(&tickets, Some(5)));
        let dev = columns[2].dev.as_ref().unwrap();
        assert_eq!(dev.top.len(), 4, "pinned + 3 needs-you shown");
        assert_eq!(keys(&dev.working), ["SHOP-12"]);
        assert_eq!(dev.fold.as_deref(), Some("2 more · working"));
        let tight = build(&input(&tickets, Some(1)));
        let dev = tight[2].dev.as_ref().unwrap();
        assert_eq!(dev.top.len(), 3, "the needs-you cards are never folded");
    }

    #[test]
    fn done_rail_state() {
        // Board widths: the window less the 64 px rail (232 px sidebar above 1600) and the panel with its handle.
        assert!(done_collapsed(None, 1280. - 64.), "1280 minimum");
        assert!(!done_collapsed(None, 1440. - 64.), "1440");
        assert!(
            !done_collapsed(None, 1601. - 232.),
            "just past 1600, sidebar"
        );
        assert!(!done_collapsed(None, 1920. - 232.), "1920");
        assert!(
            done_collapsed(None, 1920. - 232. - 665.),
            "1920 with the 660 px panel"
        );
        assert!(
            done_collapsed(None, 1920. - 232. - 485.),
            "1920 with the narrowest panel"
        );
        assert!(
            !done_collapsed(None, 2560. - 232. - 665.),
            "2560 with the panel"
        );
        // Six open columns never go below their minimum: padding 2 × 10, five gaps of 8.
        const { assert!((DONE_OPEN_FROM_WIDTH - 20. - 40.) / 6. >= crate::theme::density::COLUMN) };
        // With Done as a rail, the other five fit the 1280 minimum: the rail is 34 px.
        const { assert!(20. + 40. + 34. + 5. * crate::theme::density::COLUMN <= 1280. - 64.) };
        assert!(
            !done_collapsed(Some(false), 1216.),
            "the user's choice wins"
        );
        assert!(done_collapsed(Some(true), 1688.));
        let tickets = board();
        let columns = build(&BoardInput {
            done_collapsed: true,
            ..input(&tickets, None)
        });
        assert!(columns[5].collapsed && !columns[0].collapsed);
        assert_eq!(dev_budget(900.), 8);
        assert_eq!(dev_budget(300.), 4);
    }

    #[test]
    fn hidden_columns_are_named_at_their_edge() {
        // 1280 with the 660 px panel: a 551 px board, columns of 216 from x 10 with gaps of 8, Done as a rail.
        let columns = [
            ("Backlog", 10., 226.),
            ("Ready for Dev", 234., 450.),
            ("Dev", 458., 674.),
            ("Ready for Review", 682., 898.),
            ("Approved", 906., 1122.),
            ("Done", 1130., 1164.),
        ];
        let start = hidden_columns(&columns, (0., 551.));
        assert_eq!(start.before, None);
        assert_eq!(
            start.after.as_deref(),
            Some("Dev · Ready for Review · Approved · Done"),
            "a column cut off at the edge counts as hidden"
        );
        let end = hidden_columns(&columns, (623., 1174.));
        assert_eq!(end.before.as_deref(), Some("Backlog · Ready for Dev · Dev"));
        assert_eq!(end.after, None);
        let middle = hidden_columns(&columns, (458., 1009.));
        assert_eq!(middle.before.as_deref(), Some("Backlog · Ready for Dev"));
        assert_eq!(middle.after.as_deref(), Some("Approved · Done"));
        assert_eq!(
            hidden_columns(&columns, (0.5, 1164.5)),
            HiddenColumns::default(),
            "everything fits (a pixel of rounding is not hidden)"
        );
    }

    #[test]
    fn a_filter_shows_shown_over_total() {
        let tickets = board();
        let visible: Vec<TicketKey> = vec!["SHOP-24".parse().unwrap(), "SHOP-22".parse().unwrap()];
        let columns = build(&BoardInput {
            visible: Some(&visible),
            ..input(&tickets, None)
        });
        assert_eq!(columns[0].count, "1/3");
        assert_eq!(columns[0].hidden, 2);
        assert_eq!(columns[2].count, "1/7");
        assert_eq!(keys(&columns[2].dev.as_ref().unwrap().top), ["SHOP-22"]);
    }
}
