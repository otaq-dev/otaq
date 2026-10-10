//! The board's columns (UI-02; *03 · Board — shop-web*, *03 · Board & columns*): five columns with counts;
//! Dev with the cards that need the user (and pins) on top, then the Agent Working section, then Queued with
//! its hint; the "N more · W working · Q queued" fold that never hides needs-you cards (T-0027); the Done
//! column that collapses to a 34 px rail; epic cards in Backlog; and the empty-column hints. Order comes from
//! the core ([`crewboard_core::board::order`]), so the board, the service and the tests agree.

use crewboard_core::board::order::{ColumnOrder, fold_dev, sort_column};
use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Column, Kind, Status, TicketKey};

use crate::icons::IconName;
use crate::view_models::card::{CardExtras, CardViewModel};
use crate::view_models::epic_card::EpicViewModel;
use crate::view_models::queue::queue_facts;

/// Below this window width Done starts as a rail (1280 px, or with the panel open; *Width strategy*).
pub const DONE_OPEN_FROM_WIDTH: f32 = 1440.;

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
    pub queued: Vec<CardEntry>,
    pub queued_count: usize,
    /// "4 more · 2 working · 2 queued" while folded.
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
        Column::Dev => (
            IconName::Activity,
            "Drag a ticket here from Backlog to start it. Its agent works in its own worktree.",
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

/// The hint under Dev's Queued header.
pub const QUEUED_HINT: &str = "Starts when every dependency is merged";

/// Whether Done shows as a rail: the user's choice, else by window width.
pub fn done_collapsed(user_choice: Option<bool>, window_width: f32) -> bool {
    user_choice.unwrap_or(window_width < DONE_OPEN_FROM_WIDTH)
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
                queued: keep(&full.queued),
            };
            let total = full.cards.len() + full.queued.len();
            let visible = order.cards.len() + order.queued.len();
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
    let (shown_cards, shown_queued, fold) = match budget {
        Some(max) => {
            let f = fold_dev(order, all, max);
            let label = f.label();
            (f.shown_cards, f.shown_queued, label)
        }
        None => (order.cards.clone(), order.queued.clone(), None),
    };
    let top: Vec<TicketKey> = shown_cards.iter().filter(|k| floats(k)).cloned().collect();
    let working: Vec<TicketKey> = shown_cards.iter().filter(|k| !floats(k)).cloned().collect();
    let in_queued_section = |k: &TicketKey| {
        all.iter()
            .find(|t| &t.key == k)
            .is_some_and(|t| t.status.is_queued())
    };
    let queued_count =
        order.queued.len() + order.cards.iter().filter(|k| in_queued_section(k)).count();
    DevSections {
        top: entries(&top),
        working: entries(&working),
        working_count: order.cards.len() + order.queued.len() - queued_count,
        queued: entries(&shown_queued),
        queued_count,
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
        let dev = |tickets: &[Ticket]| build(&input(tickets, None))[1].dev.clone().unwrap();
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
            let dev = build(&input(tickets, None))[1].dev.clone().unwrap();
            dev.queued
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
    fn five_columns_with_counts_and_epics_in_backlog() {
        let tickets = board();
        let columns = build(&input(&tickets, None));
        let titles: Vec<_> = columns.iter().map(|c| c.title).collect();
        assert_eq!(
            titles,
            ["Backlog", "Dev", "Ready for Review", "Approved", "Done"]
        );
        let counts: Vec<_> = columns.iter().map(|c| c.count.as_str()).collect();
        assert_eq!(counts, ["3", "11", "1", "0", "1"]);
        assert!(
            columns[0]
                .cards
                .iter()
                .any(|c| matches!(c, CardEntry::Epic(_)))
        );
        assert!(columns[3].cards.is_empty());
        assert!(
            columns[3]
                .empty_hint
                .starts_with("Drag a ticket here from Ready for Review")
        );
    }

    #[test]
    fn dev_sections_float_needs_you_and_pins() {
        let tickets = board();
        let columns = build(&input(&tickets, None));
        let dev = columns[1].dev.as_ref().unwrap();
        assert_eq!(
            keys(&dev.top),
            ["SHOP-19", "SHOP-22", "SHOP-23", "SHOP-25", "SHOP-28"]
        );
        assert_eq!(keys(&dev.working), ["SHOP-12", "SHOP-26", "SHOP-27"]);
        assert_eq!(keys(&dev.queued), ["SHOP-20", "SHOP-21", "SHOP-29"]);
        assert_eq!(
            (dev.working_count, dev.queued_count),
            (7, 4),
            "Start interrupted counts as queued"
        );
        assert_eq!(dev.fold, None);
    }

    #[test]
    fn fold_counts_and_never_hides_needs_you() {
        let tickets = board();
        let columns = build(&input(&tickets, Some(6)));
        let dev = columns[1].dev.as_ref().unwrap();
        assert_eq!(dev.top.len(), 5, "pinned + 4 needs-you shown");
        assert_eq!(keys(&dev.working), ["SHOP-12"]);
        assert!(dev.queued.is_empty());
        assert_eq!(dev.fold.as_deref(), Some("5 more · 2 working · 3 queued"));
    }

    #[test]
    fn done_rail_state() {
        assert!(done_collapsed(None, 1280.));
        assert!(!done_collapsed(None, 1440.));
        assert!(
            !done_collapsed(Some(false), 1280.),
            "the user's choice wins"
        );
        assert!(done_collapsed(Some(true), 1920.));
        let tickets = board();
        let columns = build(&BoardInput {
            done_collapsed: true,
            ..input(&tickets, None)
        });
        assert!(columns[4].collapsed && !columns[0].collapsed);
        assert_eq!(dev_budget(900.), 8);
        assert_eq!(dev_budget(300.), 4);
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
        assert_eq!(columns[1].count, "1/11");
        assert_eq!(keys(&columns[1].dev.as_ref().unwrap().top), ["SHOP-22"]);
    }
}
