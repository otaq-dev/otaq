//! The board's columns (UI-02, SHL-06; *03 · Board & columns*; d0087): five columns with counts, Backlog · Ready
//! for Dev · Dev · Review · Done. The states and their stored columns are the core's six (§7); only the page groups
//! them: the Review column holds Ready for Review and Approved as two sections ([`SectionView`]), each with its own
//! count and its own cards in the old columns' order, and the column counts both. Ready for Dev shows the queued
//! cards (d0066); Dev the cards that need the user (and pins) on top, then the Agent Working section; the "N more ·
//! working" fold that never hides needs-you cards (T-0027); the Done column that collapses to a 34 px rail; epic
//! cards in Backlog; and the empty-column hints. Order comes from the core ([`otaq_core::board::order`]), so the
//! board, the service and the tests agree.
//!
//! Each child is on the board exactly once (EPC-11, d0080): an epic is a card in Backlog only in its Card place
//! and a delivery card in Review / Done; in its Track place the strip draws it instead. Epic children never show
//! as cards in Backlog: they are rows of the epic card, or "not queued" stops on its track. In Done an epic's
//! merged children fold into one line (EPC-16, [`DoneFold`]); its closed children stay cards.

use otaq_core::board::epic_flight::{EpicPlace, children_of, epic_place};
use otaq_core::board::order::{ColumnOrder, fold_dev, sort_column};
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Column, Kind, Status, TicketKey};

use crate::icons::IconName;
use crate::view_models::board_page::ColumnId;
use crate::view_models::card::{AgentFacts, CardExtras, CardViewModel};
use crate::view_models::epic_card::EpicViewModel;
use crate::view_models::queue::queue_facts;
use otaq_core::named_agents::rows::BoardAgents;

/// From this board width Done opens as a column; below it Done is its rail (SHL-01; CHANGES-v §1: "At ≥ 1360 px of
/// board width Done opens as a column"). The board width is the main area's: the pane and the project rail are not
/// part of it, and the panel over the board doesn't change it.
pub const DONE_OPEN_FROM_WIDTH: f32 = 1360.;

/// The columns fill the board's width; below this width per column the board scrolls sideways (SHL-01).
pub const MIN_COLUMN_WIDTH: f32 = 200.;

/// Done as a rail.
pub const DONE_RAIL_WIDTH: f32 = 34.;

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
    /// The cards behind the fold, in column order (the fold chip turns accent when one is in the highlighted chain).
    pub folded: Vec<TicketKey>,
}

/// One section of the Review column (*Row · Review column (0086 · V)*): Ready for Review or Approved, with its own
/// count and cards in that state's order (§7.5). Folding it is the page's state; the count stays.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionView {
    /// The state it holds: [`Column::ReadyForReview`] or [`Column::Approved`].
    pub column: Column,
    /// "READY FOR REVIEW", "APPROVED".
    pub title: &'static str,
    /// "2", or "1/2" with a filter.
    pub count: String,
    pub cards: Vec<CardEntry>,
    /// Cards a filter hides in this section.
    pub hidden: usize,
    /// The quiet line of an empty section ("Nothing approved").
    pub empty: &'static str,
    /// Approved only: what the section says while a Ready for Review card is dragged and it is empty.
    pub drag_hint: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnView {
    pub id: ColumnId,
    pub title: &'static str,
    pub icon: IconName,
    /// "12" or, with a filter, "3/12" (T-0038); with an epic focused, its cards over all (Q55). Review: both
    /// sections.
    pub count: String,
    /// Every column but Dev and Review.
    pub cards: Vec<CardEntry>,
    pub dev: Option<DevSections>,
    /// Review only: Ready for Review, then Approved.
    pub sections: Vec<SectionView>,
    /// Under the title: Ready for Dev's "Starts when ready, in priority order".
    pub hint: Option<&'static str>,
    /// Shown when the column is empty.
    pub empty_hint: &'static str,
    /// Done only: drawn as a narrow rail.
    pub collapsed: bool,
    /// Cards a filter hides in this column ("6 hidden by filter").
    pub hidden: usize,
    /// Done only: one line per epic over its merged children, which follow each other in `cards` from where the
    /// first of them sorts.
    pub folds: Vec<DoneFold>,
}

/// An epic's merged children folded into one line in Done (EPC-16; *01 · Components → Done Fold*): "Wishlist · 4
/// of 6 merged", merged out of all its children. A click shows the cards; the column counts the line as one.
#[derive(Debug, Clone, PartialEq)]
pub struct DoneFold {
    pub epic: TicketKey,
    pub label: String,
    /// The folded cards, in column order.
    pub keys: Vec<TicketKey>,
}

/// Folds the merged epic children among `keys` (a column's cards in order): each epic's merged children move up
/// to where the first of them sorts, and get a [`DoneFold`]. Closed children and other cards stay where they are.
pub fn done_folds(keys: &[TicketKey], all: &[Ticket]) -> (Vec<TicketKey>, Vec<DoneFold>) {
    let folded_under = |key: &TicketKey| {
        all.iter()
            .find(|t| &t.key == key)
            .filter(|t| t.status == Status::Merged)
            .and_then(|t| t.epic.as_ref())
    };
    let mut order = Vec::with_capacity(keys.len());
    let mut folds: Vec<DoneFold> = Vec::new();
    for key in keys {
        let Some(epic) = folded_under(key) else {
            order.push(key.clone());
            continue;
        };
        if folds.iter().any(|f| &f.epic == epic) {
            continue;
        }
        let members: Vec<TicketKey> = keys
            .iter()
            .filter(|k| folded_under(k) == Some(epic))
            .cloned()
            .collect();
        let title = all
            .iter()
            .find(|t| &t.key == epic)
            .map_or_else(|| epic.to_string(), |t| t.title.clone());
        let (merged, children) = all
            .iter()
            .find(|t| &t.key == epic)
            .map(|e| {
                let children = children_of(e, all);
                let merged = children
                    .iter()
                    .filter(|c| c.status == Status::Merged)
                    .count();
                (merged, children.len())
            })
            .unwrap_or((members.len(), members.len()));
        order.extend(members.iter().cloned());
        folds.push(DoneFold {
            epic: epic.clone(),
            label: format!("{title} · {merged} of {children} merged"),
            keys: members,
        });
    }
    (order, folds)
}

/// How many items `keys` make with their folds: a fold is one.
fn items(keys: &[TicketKey], folds: &[DoneFold]) -> usize {
    keys.len() - folds.iter().map(|f| f.keys.len() - 1).sum::<usize>()
}

/// The column's title, icon and empty hint.
pub fn column_look(id: ColumnId) -> (&'static str, IconName, &'static str) {
    match id {
        ColumnId::Backlog => (
            "Backlog",
            IconName::CircleDashed,
            "Write a ticket here: what to build and when it’s done.",
        ),
        ColumnId::ReadyForDev => (
            "Ready for Dev",
            IconName::Hourglass,
            "Drag a ticket here from Backlog to start it when it’s ready.",
        ),
        ColumnId::Dev => (
            "Dev",
            IconName::Activity,
            "Tickets move here when their agent starts. Each agent works in its own worktree.",
        ),
        ColumnId::Review => (
            "Review",
            IconName::Eye,
            "Agents move their ticket here when they submit it for your review.",
        ),
        ColumnId::Done => (
            "Done",
            IconName::CircleCheck,
            "Merged and closed tickets end up here.",
        ),
    }
}

/// The hint under Ready for Dev's title (*03 · Board — shop-web*).
pub const READY_FOR_DEV_HINT: &str = "Starts when ready, in priority order";

/// Dev's section header above its working cards.
pub const DEV_SECTION: &str = "WORKING · VALIDATING";

/// The Review column's section headers (*Row · Review column (0086 · V)*).
pub const READY_FOR_REVIEW_SECTION: &str = "READY FOR REVIEW";
pub const APPROVED_SECTION: &str = "APPROVED";

/// An empty section's quiet line: the frame's "Nothing approved", and the same for Ready for Review.
pub const NOTHING_TO_REVIEW: &str = "Nothing to review";
pub const NOTHING_APPROVED: &str = "Nothing approved";

/// The empty Approved section while a Ready for Review card is dragged (the old Approved column's hint).
pub const APPROVED_DRAG_HINT: &str = "Drag a ticket here from Ready for Review to approve it. It always asks how: Local merge or Open PR.";

/// Whether Done shows as a rail: the user's choice, else by the board's width (the main area's).
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
    /// Off the right edge ("Review · Done").
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
    /// Tickets at full strength while an epic is focused (an `epic:` filter, Q55): the others are shown dimmed and
    /// the counts count only these. `None`: nothing dims.
    pub strong: Option<&'a [TicketKey]>,
    pub project_base: Option<&'a str>,
    pub now_ms: i64,
    /// Dev's card budget; `None` when the user unfolded Dev.
    pub dev_budget: Option<usize>,
    pub done_collapsed: bool,
    /// The project's Project agents by id and current name: cards they created name them (d0068).
    pub agent_names: &'a [(String, String)],
    /// Interrupted tickets that were validating (the startup report, T-0244).
    pub was_validating: &'a [TicketKey],
    /// The cards' named agents (`board.agents`, T-0345); `None` names none.
    pub agents: Option<&'a BoardAgents>,
}

/// Whether the ticket is a card in its column (EPC-11): an epic only in its Card or Delivery place, an epic child
/// anywhere but Backlog. `all` is the ticket's project.
pub fn is_card(ticket: &Ticket, all: &[Ticket]) -> bool {
    if ticket.kind == Kind::Epic {
        epic_place(ticket, all) != EpicPlace::Track
    } else {
        ticket.epic.is_none() || ticket.column != Column::Backlog
    }
}

/// One state's cards, in its order, with what its count counts.
struct StateCards {
    order: ColumnOrder,
    folds: Vec<DoneFold>,
    /// Items over all (a Done fold is one).
    total: usize,
    /// Items the filter shows.
    visible: usize,
    /// Items at full strength while an epic is focused.
    strong: Option<usize>,
}

impl StateCards {
    fn of(column: Column, input: &BoardInput, cards: &[&TicketKey], shown: &[&TicketKey]) -> Self {
        let all = input.tickets;
        // Order over the whole project ("blocks more" counts every dependent), cards only; the counts count the
        // cards the column shows.
        let full = sort_column(column, all);
        let keep = |keys: &[TicketKey], of: &[&TicketKey]| -> Vec<TicketKey> {
            keys.iter().filter(|k| of.contains(k)).cloned().collect()
        };
        let (shown_cards, folds) = done_folds(&keep(&full.cards, shown), all);
        let every = keep(&full.cards, cards);
        let total = items(&every, &done_folds(&every, all).1);
        let visible = items(&shown_cards, &folds);
        let strong = input.strong.map(|strong| {
            let full = keep(&shown_cards, &strong.iter().collect::<Vec<_>>());
            items(&full, &done_folds(&full, all).1)
        });
        Self {
            order: ColumnOrder {
                column,
                cards: shown_cards,
            },
            folds,
            total,
            visible,
            strong,
        }
    }
}

/// "12", "3/12" under a filter, or the focused epic's items over all.
fn count_text(visible: usize, total: usize, strong: Option<usize>, filtered: bool) -> String {
    match strong {
        Some(n) => format!("{n}/{total}"),
        None if filtered => format!("{visible}/{total}"),
        None => total.to_string(),
    }
}

pub fn build(input: &BoardInput) -> Vec<ColumnView> {
    let all = input.tickets;
    let cards: Vec<&TicketKey> = all
        .iter()
        .filter(|t| is_card(t, all))
        .map(|t| &t.key)
        .collect();
    let shown: Vec<&TicketKey> = match input.visible {
        Some(keys) => cards.iter().copied().filter(|k| keys.contains(k)).collect(),
        None => cards.clone(),
    };
    let filtered = input.visible.is_some();
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
                sleeve: crate::view_models::card::sleeve(t, all),
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
                held_by_limit: otaq_core::scheduler::held_by_tool_limit(t),
                was_validating: input.was_validating.contains(&t.key),
                agents: input
                    .agents
                    .and_then(|a| {
                        let card = a.cards.iter().find(|c| c.key == t.key)?;
                        Some(AgentFacts::of(card, a.agent_count))
                    })
                    .unwrap_or_default(),
                agent_name: match &t.created_by {
                    otaq_core::domain::Actor::ProjectAgent(id) => input
                        .agent_names
                        .iter()
                        .find(|(agent, _)| agent == id)
                        .map(|(_, name)| name.clone()),
                    _ => None,
                },
                ..CardExtras::default()
            }
            .with_signal(t, input.now_ms);
            CardEntry::Ticket(CardViewModel::from(t, &extras, input.now_ms))
        })
    };
    let entries = |keys: &[TicketKey]| keys.iter().filter_map(entry).collect::<Vec<_>>();
    ColumnId::ALL
        .iter()
        .map(|&id| {
            let states: Vec<StateCards> = id
                .states()
                .iter()
                .map(|&column| StateCards::of(column, input, &cards, &shown))
                .collect();
            let total: usize = states.iter().map(|s| s.total).sum();
            let visible: usize = states.iter().map(|s| s.visible).sum();
            let strong = input
                .strong
                .map(|_| states.iter().filter_map(|s| s.strong).sum());
            let (title, icon, empty_hint) = column_look(id);
            let mut view = ColumnView {
                id,
                title,
                icon,
                count: count_text(visible, total, strong, filtered),
                cards: Vec::new(),
                dev: None,
                sections: Vec::new(),
                hint: (id == ColumnId::ReadyForDev).then_some(READY_FOR_DEV_HINT),
                empty_hint,
                collapsed: id == ColumnId::Done && input.done_collapsed,
                hidden: total - visible,
                folds: Vec::new(),
            };
            match id {
                ColumnId::Dev => {
                    view.dev = Some(dev_sections(
                        &states[0].order,
                        all,
                        input.dev_budget,
                        &entries,
                    ));
                }
                ColumnId::Review => {
                    view.sections = states
                        .into_iter()
                        .map(|s| section(s, filtered, &entries))
                        .collect();
                }
                _ => {
                    let [state] = <[StateCards; 1]>::try_from(states)
                        .unwrap_or_else(|_| unreachable!("one state per column but Review"));
                    view.cards = entries(&state.order.cards);
                    view.folds = state.folds;
                }
            }
            view
        })
        .collect()
}

fn section(
    state: StateCards,
    filtered: bool,
    entries: &dyn Fn(&[TicketKey]) -> Vec<CardEntry>,
) -> SectionView {
    let approved = state.order.column == Column::Approved;
    SectionView {
        column: state.order.column,
        title: if approved {
            APPROVED_SECTION
        } else {
            READY_FOR_REVIEW_SECTION
        },
        count: count_text(state.visible, state.total, state.strong, filtered),
        cards: entries(&state.order.cards),
        hidden: state.total - state.visible,
        empty: if approved {
            NOTHING_APPROVED
        } else {
            NOTHING_TO_REVIEW
        },
        drag_hint: approved.then_some(APPROVED_DRAG_HINT),
    }
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
    let folded = order
        .cards
        .iter()
        .filter(|k| !shown_cards.contains(k))
        .cloned()
        .collect();
    DevSections {
        top: entries(&top),
        working: entries(&working),
        working_count: order.cards.len(),
        fold,
        folded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::{NOW, ticket};
    use otaq_core::domain::Status;

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
            strong: None,
            project_base: Some("main"),
            now_ms: NOW,
            dev_budget: budget,
            done_collapsed: false,
            agent_names: &[],
            was_validating: &[],
            agents: None,
        }
    }

    /// T-0075: a working card that turns turquoise moves up to the top of Dev, below the pinned card, and back
    /// down when the signal clears.
    #[test]
    fn needs_you_cards_float_up_as_signals_change() {
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

    /// T-0243: a Backlog card a Project agent created names it by its current name and glows at first.
    #[test]
    fn project_agent_cards_name_their_agent() {
        let mut t = ticket("SHOP-36", "Cache product prices", Status::Backlog);
        t.created_by = otaq_core::domain::Actor::ProjectAgent("a1".into());
        t.created_at = NOW - 1_000;
        let tickets = vec![t];
        let names = [("a1".to_owned(), "Checkout slowness".to_owned())];
        let columns = build(&BoardInput {
            agent_names: &names,
            ..input(&tickets, None)
        });
        let Some(CardEntry::Ticket(vm)) = columns[0].cards.first() else {
            panic!("a Backlog card")
        };
        assert!(vm.fresh);
        assert_eq!(vm.origin.as_deref(), Some("Created by Checkout slowness"));
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
        // SHOP-18 is in Review: the situation line says so.
        let vm = card(&tickets, "SHOP-20");
        assert_eq!(
            vm.situation.unwrap().text,
            "waits for SHOP-18 to merge (in Review)"
        );
        // SHOP-18 merged: about to start.
        let shop18 = tickets
            .iter_mut()
            .find(|t| t.key.to_string() == "SHOP-18")
            .unwrap();
        shop18.status = Status::Merged;
        let vm = card(&tickets, "SHOP-20");
        assert_eq!(vm.flag, None);
        assert!(vm.situation.unwrap().text.starts_with("Starting…"));
    }

    /// SHOP-45 Search with its two children, and the standalone SHOP-51.
    fn search(child_status: [Status; 2]) -> Vec<Ticket> {
        let mut epic = ticket("SHOP-45", "Search", Status::Backlog);
        epic.kind = Kind::Epic;
        epic.external_id = Some("PAY-130".into());
        let child = |key: &str, title: &str, status| {
            let mut t = ticket(key, title, status);
            t.epic = Some("SHOP-45".parse().unwrap());
            t
        };
        vec![
            epic,
            child("SHOP-45.1", "Search index API", child_status[0]),
            child("SHOP-45.2", "Search results page", child_status[1]),
            ticket("SHOP-51", "Fix checkout rounding", Status::Backlog),
        ]
    }

    fn column_keys(column: &ColumnView) -> Vec<String> {
        match &column.dev {
            Some(dev) => keys(&dev.top)
                .into_iter()
                .chain(keys(&dev.working))
                .collect(),
            None => keys(&column.cards)
                .into_iter()
                .chain(column.sections.iter().flat_map(|s| keys(&s.cards)))
                .collect(),
        }
    }

    /// Each child is on the board exactly once: never a card in Backlog.
    fn no_child_card_in_backlog(columns: &[ColumnView], tickets: &[Ticket]) {
        for key in column_keys(&columns[0]) {
            let t = tickets.iter().find(|t| t.key.to_string() == key).unwrap();
            assert!(t.epic.is_none(), "{key} is an epic child card in Backlog");
        }
    }

    /// EPC-11: SHOP-45 with nothing started is one card with its children as rows, and no child cards.
    #[test]
    fn an_epic_before_it_starts_is_one_card() {
        let tickets = search([Status::Backlog, Status::Backlog]);
        let columns = build(&input(&tickets, None));
        assert_eq!(column_keys(&columns[0]), ["SHOP-45", "SHOP-51"]);
        assert_eq!(columns[0].count, "2");
        let Some(CardEntry::Epic(epic)) = columns[0].cards.first() else {
            panic!("the epic card")
        };
        assert_eq!(epic.children.len(), 2);
        assert!(epic.children.iter().all(|c| c.draggable));
        assert!(columns[1..].iter().all(|c| column_keys(c).is_empty()));
        no_child_card_in_backlog(&columns, &tickets);
    }

    /// EPC-11, EPC-12: SHOP-45.1 queued makes the epic a track: no epic card, 45.1 in Ready for Dev, 45.2 nowhere
    /// in Backlog (a "not queued" stop on the track). Backlog counts what it shows.
    #[test]
    fn a_queued_child_makes_the_epic_a_track() {
        let tickets = search([Status::Queued, Status::Backlog]);
        let columns = build(&input(&tickets, None));
        assert_eq!(column_keys(&columns[0]), ["SHOP-51"]);
        assert_eq!(columns[0].count, "1");
        assert_eq!(column_keys(&columns[1]), ["SHOP-45.1"]);
        assert_eq!(columns[1].count, "1");
        assert!(columns.iter().all(|c| {
            !column_keys(c)
                .iter()
                .any(|k| k == "SHOP-45" || k == "SHOP-45.2")
        }));
        no_child_card_in_backlog(&columns, &tickets);

        // Running children are cards as usual; a merged one is in Done.
        let tickets = search([Status::Merged, Status::Working]);
        let columns = build(&input(&tickets, None));
        assert_eq!(column_keys(&columns[0]), ["SHOP-51"]);
        assert_eq!(column_keys(&columns[2]), ["SHOP-45.2"]);
        assert_eq!(column_keys(&columns[4]), ["SHOP-45.1"]);

        // SHOP-45.1 merged and SHOP-45.2 taken back: the track stays, 45.2 is not a Backlog card.
        let tickets = search([Status::Merged, Status::Backlog]);
        let columns = build(&input(&tickets, None));
        assert_eq!(column_keys(&columns[0]), ["SHOP-51"]);
        no_child_card_in_backlog(&columns, &tickets);
    }

    /// EPC-14: after Park all the epic is a card again, its rows "Parked · has work" and draggable.
    #[test]
    fn after_park_all_the_epic_is_a_card_with_parked_rows() {
        let tickets = search([Status::Parked, Status::Parked]);
        let columns = build(&input(&tickets, None));
        assert_eq!(column_keys(&columns[0]), ["SHOP-45", "SHOP-51"]);
        let Some(CardEntry::Epic(epic)) = columns[0].cards.first() else {
            panic!("the epic card")
        };
        let rows: Vec<(&str, bool)> = epic
            .children
            .iter()
            .map(|c| (c.status_label, c.draggable))
            .collect();
        assert_eq!(
            rows,
            [("Parked · has work", true), ("Parked · has work", true)]
        );
        no_child_card_in_backlog(&columns, &tickets);
    }

    /// EPC-04: SHOP-40 in Review is its delivery card there, as before; its merged children stay in Done.
    #[test]
    fn an_epic_in_review_is_its_delivery_card() {
        let mut epic = ticket("SHOP-40", "Wishlist", Status::ReadyForReview);
        epic.kind = Kind::Epic;
        let mut tickets = vec![epic];
        for key in ["SHOP-40.1", "SHOP-40.2"] {
            let mut t = ticket(key, "Child", Status::Merged);
            t.epic = Some("SHOP-40".parse().unwrap());
            tickets.push(t);
        }
        let columns = build(&input(&tickets, None));
        assert!(column_keys(&columns[0]).is_empty());
        assert_eq!(columns[0].count, "0");
        assert!(matches!(
            columns[3].sections[0].cards.as_slice(),
            [CardEntry::Epic(e)] if e.key.to_string() == "SHOP-40"
        ));
        assert_eq!(column_keys(&columns[4]), ["SHOP-40.1", "SHOP-40.2"]);
    }

    /// SHOP-40 Wishlist in flight: 40.1 and 40.2 merged, 40.3 working, 40.5 closed; SHOP-17 and SHOP-30 merged
    /// on their own. `created_at` sets Done's order: 40.5, 17, 40.1, 30, 40.2.
    fn wishlist_done() -> Vec<Ticket> {
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        let child = |key: &str, status, created_at| {
            let mut t = ticket(key, "Child", status);
            t.epic = Some("SHOP-40".parse().unwrap());
            t.created_at = created_at;
            t
        };
        let mut t17 = ticket("SHOP-17", "Add Stripe client", Status::Merged);
        t17.created_at = 5;
        let mut t30 = ticket("SHOP-30", "Coupons", Status::Merged);
        t30.created_at = 15;
        vec![
            epic,
            child("SHOP-40.1", Status::Merged, 10),
            child("SHOP-40.2", Status::Merged, 20),
            child("SHOP-40.3", Status::Working, 30),
            child("SHOP-40.5", Status::ClosedNotMerged, 3),
            t17,
            t30,
        ]
    }

    /// EPC-16: two merged children and a closed one give one fold line ("Wishlist · 2 of 4 merged", merged out of
    /// all four children) and one card; the fold sits where 40.1 sorts, and Done counts it as one.
    #[test]
    fn merged_children_fold_into_one_line_in_done() {
        let tickets = wishlist_done();
        let columns = build(&input(&tickets, None));
        let done = &columns[4];
        assert_eq!(
            keys(&done.cards),
            ["SHOP-40.5", "SHOP-17", "SHOP-40.1", "SHOP-40.2", "SHOP-30"],
            "the merged children follow each other from where the first sorts; the closed one stays a card"
        );
        assert_eq!(
            done.folds,
            [DoneFold {
                epic: "SHOP-40".parse().unwrap(),
                label: "Wishlist · 2 of 4 merged".into(),
                keys: vec!["SHOP-40.1".parse().unwrap(), "SHOP-40.2".parse().unwrap()],
            }]
        );
        assert_eq!(done.count, "4", "40.5, 17, the fold and 30");
        assert!(columns[..4].iter().all(|c| c.folds.is_empty()));

        // The first merged child folds at once ("1 of 4"), and a closed child never does.
        let mut tickets = wishlist_done();
        tickets[2].status = Status::Working;
        tickets[2].column = Column::Dev;
        let done = &build(&input(&tickets, None))[4];
        assert_eq!(done.folds.len(), 1);
        assert_eq!(done.folds[0].label, "Wishlist · 1 of 4 merged");
        assert_eq!(done.count, "4");

        // Under a filter the fold counts once, shown over total.
        let tickets = wishlist_done();
        let visible: Vec<TicketKey> = ["SHOP-40.2", "SHOP-17"]
            .iter()
            .map(|k| k.parse().unwrap())
            .collect();
        let done = &build(&BoardInput {
            visible: Some(&visible),
            ..input(&tickets, None)
        })[4];
        assert_eq!(keys(&done.cards), ["SHOP-17", "SHOP-40.2"]);
        assert_eq!(done.folds[0].keys.len(), 1);
        assert_eq!(done.folds[0].label, "Wishlist · 2 of 4 merged");
        assert_eq!((done.count.as_str(), done.hidden), ("2/4", 2));
    }

    /// SHL-06: five columns; Review counts both sections.
    #[test]
    fn five_columns_with_counts_and_epics_in_backlog() {
        let tickets = board();
        let columns = build(&input(&tickets, None));
        let titles: Vec<_> = columns.iter().map(|c| c.title).collect();
        assert_eq!(
            titles,
            ["Backlog", "Ready for Dev", "Dev", "Review", "Done"]
        );
        let ids: Vec<_> = columns.iter().map(|c| c.id).collect();
        assert_eq!(ids, ColumnId::ALL);
        let counts: Vec<_> = columns.iter().map(|c| c.count.as_str()).collect();
        assert_eq!(counts, ["2", "4", "7", "1", "1"]);
        // The epic is a card with SHOP-40.3 as its row, not a second Backlog card.
        assert_eq!(keys(&columns[0].cards), ["SHOP-24", "SHOP-40"]);
        assert!(
            columns[0]
                .cards
                .iter()
                .any(|c| matches!(c, CardEntry::Epic(e) if e.children.len() == 1))
        );
        // Ready for Dev: Start interrupted first, then the start order (the one waiting for SHOP-18 last).
        assert_eq!(
            keys(&columns[1].cards),
            ["SHOP-28", "SHOP-21", "SHOP-29", "SHOP-20"]
        );
        assert_eq!(columns[1].hint, Some(READY_FOR_DEV_HINT));
        assert!(columns.iter().filter(|c| c.hint.is_some()).count() == 1);
        // Only Review has sections; its empty Approved section says so.
        assert!(columns.iter().filter(|c| !c.sections.is_empty()).count() == 1);
        let review = &columns[3];
        assert!(review.cards.is_empty());
        let sections: Vec<_> = review
            .sections
            .iter()
            .map(|s| (s.column, s.title, s.count.as_str(), keys(&s.cards)))
            .collect();
        assert_eq!(
            sections,
            [
                (
                    Column::ReadyForReview,
                    READY_FOR_REVIEW_SECTION,
                    "1",
                    vec!["SHOP-18".to_owned()]
                ),
                (Column::Approved, APPROVED_SECTION, "0", vec![]),
            ]
        );
        assert_eq!(review.sections[1].empty, "Nothing approved");
        assert!(
            review.sections[1]
                .drag_hint
                .unwrap()
                .starts_with("Drag a ticket here from Ready for Review")
        );
        assert_eq!(review.sections[0].drag_hint, None);
    }

    /// §7.5 in each section: pinned first (most recently pinned first), then the old columns' order; the column
    /// counts both sections, under a filter too.
    #[test]
    fn review_sections_keep_their_own_order_and_counts() {
        let mut pinned = ticket("SHOP-31", "Hotfix", Status::ReadyForReview);
        pinned.flags.pinned_at = Some(20);
        let mut older_pin = ticket("SHOP-32", "Footer", Status::ReadyForReview);
        older_pin.flags.pinned_at = Some(10);
        let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
        pr.flags.pr_number = Some(481);
        let tickets = vec![
            ticket("SHOP-18", "Checkout API", Status::ReadyForReview),
            older_pin,
            pinned,
            pr,
            ticket("SHOP-27", "Search paging", Status::PrOpen),
        ];
        let columns = build(&input(&tickets, None));
        let review = &columns[3];
        assert_eq!(review.count, "5");
        assert_eq!(
            keys(&review.sections[0].cards),
            ["SHOP-31", "SHOP-32", "SHOP-18"]
        );
        assert_eq!(review.sections[0].count, "3");
        assert_eq!(review.sections[1].count, "2");
        assert_eq!(keys(&review.sections[1].cards).len(), 2);

        let visible: Vec<TicketKey> = vec!["SHOP-18".parse().unwrap(), "SHOP-26".parse().unwrap()];
        let filtered = build(&BoardInput {
            visible: Some(&visible),
            ..input(&tickets, None)
        });
        let review = &filtered[3];
        assert_eq!(
            (review.count.as_str(), review.hidden),
            ("2/5", 3),
            "the column sums its sections"
        );
        assert_eq!(
            (review.sections[0].count.as_str(), review.sections[0].hidden),
            ("1/3", 2)
        );
        assert_eq!(
            (review.sections[1].count.as_str(), review.sections[1].hidden),
            ("1/2", 1)
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
        assert_eq!(dev.folded.len(), 2, "the folded cards are named");
        let tight = build(&input(&tickets, Some(1)));
        let dev = tight[2].dev.as_ref().unwrap();
        assert_eq!(dev.top.len(), 3, "the needs-you cards are never folded");
    }

    /// SHL-01: Done opens as a column from 1360 px of board width (the main area's), folds below; the user's
    /// choice wins either way.
    #[test]
    fn done_rail_state() {
        assert!(!done_collapsed(None, 1360.), "open at 1360");
        assert!(done_collapsed(None, 1359.), "folded at 1359");
        // The main area at the three window classes, less the default pane and the 64 px project rail.
        assert!(done_collapsed(None, 1280. - 64. - 320.), "1280");
        assert!(done_collapsed(None, 1440. - 64. - 400.), "1440");
        assert!(!done_collapsed(None, 1920. - 64. - 460.), "1920");
        assert!(!done_collapsed(Some(false), 896.), "the user's choice wins");
        assert!(done_collapsed(Some(true), 1920.));
        // Five open columns at 1360 are at least the minimum, with padding 2 × 10 and four gaps of 8.
        const { assert!((DONE_OPEN_FROM_WIDTH - 20. - 32.) / 5. >= MIN_COLUMN_WIDTH) };
        let tickets = board();
        let columns = build(&BoardInput {
            done_collapsed: true,
            agent_names: &[],
            was_validating: &[],
            ..input(&tickets, None)
        });
        assert!(columns[4].collapsed && !columns[0].collapsed);
        assert_eq!(dev_budget(900.), 8);
        assert_eq!(dev_budget(300.), 4);
    }

    #[test]
    fn hidden_columns_are_named_at_their_edge() {
        // A 551 px board: columns of the 200 px minimum from x 10 with gaps of 8, Done as a rail.
        let columns = [
            ("Backlog", 10., 210.),
            ("Ready for Dev", 218., 418.),
            ("Dev", 426., 626.),
            ("Review", 634., 834.),
            ("Done", 842., 876.),
        ];
        let start = hidden_columns(&columns, (0., 551.));
        assert_eq!(start.before, None);
        assert_eq!(
            start.after.as_deref(),
            Some("Dev · Review · Done"),
            "a column cut off at the edge counts as hidden"
        );
        let end = hidden_columns(&columns, (335., 886.));
        assert_eq!(end.before.as_deref(), Some("Backlog · Ready for Dev"));
        assert_eq!(end.after, None);
        let middle = hidden_columns(&columns, (218., 769.));
        assert_eq!(middle.before.as_deref(), Some("Backlog"));
        assert_eq!(middle.after.as_deref(), Some("Review · Done"));
        assert_eq!(
            hidden_columns(&columns, (0.5, 876.5)),
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
        assert_eq!(columns[0].count, "1/2", "the epic's row is not a card");
        assert_eq!(columns[0].hidden, 1);
        assert_eq!(columns[2].count, "1/7");
        assert_eq!(keys(&columns[2].dev.as_ref().unwrap().top), ["SHOP-22"]);
    }

    /// Q55: a focused epic dims the other cards; they stay in their columns, nothing says "hidden", and the count
    /// counts the cards at full strength.
    #[test]
    fn a_focused_epic_dims_and_counts_its_cards() {
        let tickets = board();
        let strong: Vec<TicketKey> = vec!["SHOP-40".parse().unwrap(), "SHOP-40.3".parse().unwrap()];
        let columns = build(&BoardInput {
            strong: Some(&strong),
            ..input(&tickets, None)
        });
        assert_eq!(
            keys(&columns[0].cards),
            ["SHOP-24", "SHOP-40"],
            "dimmed, not hidden"
        );
        assert_eq!(columns[0].count, "1/2");
        assert_eq!(columns[0].hidden, 0);
        assert_eq!(columns[2].count, "0/7");
    }
}
