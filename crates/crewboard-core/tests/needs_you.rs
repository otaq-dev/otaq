//! Needs-you items (ATT-06, SCH-03) with the shared sample data across shop-web and billing-api.

mod common;

use common::{Fixture, me};
use crewboard_core::board::moves::DropTarget;
use crewboard_core::board::needs_you::{
    NeedsYouGroup, ProjectTickets, group_of, needs_you_count, needs_you_items,
};
use crewboard_core::board::questions::HumanQuestion;
use crewboard_core::board::tickets::{
    AgentSignal, Base, DoneWhenItem, NewTicketInput, StartProblem, Ticket,
};
use crewboard_core::domain::{Actor, Flags, Kind, StartStep, Status, TicketKey};
use crewboard_core::store::now_ms;

const NOW: i64 = 1_791_600_000_000;
const MIN: i64 = 60_000;

fn t(key: &str, title: &str, status: Status, minutes_ago: i64) -> Ticket {
    Ticket {
        id: key.into(),
        key: key.parse::<TicketKey>().unwrap(),
        kind: Kind::Ticket,
        title: title.into(),
        body: String::new(),
        done_when: vec![DoneWhenItem {
            text: "works".into(),
            added_by: me(),
        }],
        epic: None,
        column: status.column(),
        status,
        base: Base::Branch("main".into()),
        base_locked: false,
        external_id: None,
        depends_on: Vec::new(),
        aliases: Vec::new(),
        next_child_number: None,
        flags: Flags::default(),
        created_by: Actor::User("me".into()),
        updated_by: None,
        started_at: None,
        created_at: 0,
        updated_at: 0,
        status_since: Some(NOW - minutes_ago * MIN),
        queued_at: None,
        work: None,
        start_problem: None,
        interrupted_from: None,
        signal: None,
        question: None,
    }
}

fn deps(mut ticket: Ticket, parents: &[&str]) -> Ticket {
    ticket.depends_on = parents.iter().map(|p| p.parse().unwrap()).collect();
    ticket
}

/// An orange ticket whose signal began `minutes_ago`.
fn orange(key: &str, title: &str, status: Status, text: &str, minutes_ago: i64) -> Ticket {
    let mut ticket = t(key, title, status, 999);
    ticket.signal = Some(AgentSignal {
        text: Some(text.into()),
        since: Some(NOW - minutes_ago * MIN),
        wakeup_at: None,
    });
    ticket
}

fn shop_web() -> Vec<Ticket> {
    let mut asks = orange(
        "SHOP-23",
        "Split user full name",
        Status::AsksYou,
        "signal text",
        6,
    );
    asks.question = Some(HumanQuestion {
        id: "q".into(),
        text: "Should fullName be split everywhere, or only in the profile form?".into(),
        options: Vec::new(),
        recommended: None,
        asked_at: NOW - 6 * MIN,
    });
    let mut start = t("SHOP-28", "Cart badge count", Status::StartInterrupted, 20);
    start.start_problem = Some(StartProblem {
        step: StartStep::Setup,
        message: "setup command failed (exit 1)".into(),
    });
    vec![
        orange(
            "SHOP-12",
            "Add dark mode toggle",
            Status::AsksYou,
            "Should dark mode follow the system setting?",
            8,
        ),
        t("SHOP-17", "Add Stripe client", Status::Merged, 600),
        deps(
            t("SHOP-18", "Checkout API", Status::ReadyForReview, 25),
            &["SHOP-17"],
        ),
        t("SHOP-19", "Price table", Status::Working, 30),
        deps(
            t("SHOP-20", "Checkout page", Status::Queued, 40),
            &["SHOP-18", "SHOP-19"],
        ),
        deps(
            t("SHOP-21", "Order confirmation email", Status::Queued, 40),
            &["SHOP-20"],
        ),
        orange(
            "SHOP-22",
            "Fix flaky login redirect",
            Status::NeedsYou,
            "Allow rm -rf .next?",
            2,
        ),
        asks,
        t("SHOP-24", "Update footer links", Status::Backlog, 100),
        t("SHOP-25", "Upgrade to Next.js 15", Status::Interrupted, 180),
        t("SHOP-26", "Add sitemap", Status::ClosedNotMerged, 50),
        t("SHOP-27", "Search results paging", Status::Working, 5),
        start,
        deps(
            t("SHOP-30", "Sitemap for blog posts", Status::ParentClosed, 4),
            &["SHOP-26"],
        ),
    ]
}

fn billing_api() -> Vec<Ticket> {
    vec![
        t("BILL-4", "Invoice PDF export", Status::Working, 10),
        t("BILL-5", "VAT rounding fix", Status::ReadyForReview, 60),
        t("BILL-6", "Late fee rule", Status::Held, 15),
    ]
}

fn items(shop: &[Ticket], bill: &[Ticket]) -> Vec<(String, NeedsYouGroup, String, u32)> {
    let projects = [
        ProjectTickets {
            key: "SHOP",
            name: "shop-web",
            tickets: shop,
        },
        ProjectTickets {
            key: "BILL",
            name: "billing-api",
            tickets: bill,
        },
    ];
    needs_you_items(&projects, NOW)
        .into_iter()
        .map(|i| (i.key.to_string(), i.group, i.reason, i.blocked))
        .collect()
}

#[test]
fn groups_in_inbox_order_across_projects() {
    let list = items(&shop_web(), &billing_api());
    let keys: Vec<&str> = list.iter().map(|(k, ..)| k.as_str()).collect();
    assert_eq!(
        keys,
        [
            // 1 · orange, oldest first
            "SHOP-12", "SHOP-23", "SHOP-22", //
            // 2 · interrupted, start interrupted, parent closed
            "SHOP-25", "SHOP-28", "SHOP-30", //
            // 3 · review: SHOP-18 blocks two tickets, so it comes before the older BILL-5
            "SHOP-18", "BILL-5",
        ]
    );
    let reasons: Vec<&str> = list.iter().map(|(_, _, r, _)| r.as_str()).collect();
    assert_eq!(
        reasons,
        [
            "asks: Should dark mode follow the system setting?",
            "asks: Should fullName be split everywhere, or only in the profile form?",
            "permission: Allow rm -rf .next?",
            "session interrupted",
            "start interrupted · setup command failed",
            "parent SHOP-26 was closed without merging",
            "ready for review",
            "ready for review",
        ]
    );
    assert_eq!(list[6].3, 2, "SHOP-18 blocks SHOP-20 and SHOP-21");
    assert!(
        list[6..]
            .iter()
            .all(|(_, g, ..)| *g == NeedsYouGroup::Review)
    );
}

#[test]
fn blocks_more_first_inside_a_group() {
    let mut shop = shop_web();
    // SHOP-22's fix blocks SHOP-24 now: it moves ahead of the older orange items.
    let i = shop
        .iter()
        .position(|t| t.key.to_string() == "SHOP-24")
        .unwrap();
    shop[i].depends_on = vec!["SHOP-22".parse().unwrap()];
    let list = items(&shop, &billing_api());
    let orange: Vec<&str> = list
        .iter()
        .filter(|(_, g, ..)| *g == NeedsYouGroup::Orange)
        .map(|(k, ..)| k.as_str())
        .collect();
    assert_eq!(orange, ["SHOP-22", "SHOP-12", "SHOP-23"]);
}

#[test]
fn every_field_of_an_item() {
    let shop = shop_web();
    let projects = [ProjectTickets {
        key: "SHOP",
        name: "shop-web",
        tickets: &shop,
    }];
    let list = needs_you_items(&projects, NOW);
    let first = &list[0];
    assert_eq!(first.project, "SHOP");
    assert_eq!(first.project_name, "shop-web");
    assert_eq!(first.key.to_string(), "SHOP-12");
    assert_eq!(first.title, "Add dark mode toggle");
    assert_eq!(first.status, Status::AsksYou);
    assert_eq!(
        first.age_ms,
        8 * MIN,
        "from the signal, not the status time"
    );
    assert_eq!(first.since, NOW - 8 * MIN);
    assert_eq!(
        list.iter()
            .find(|i| i.key.to_string() == "SHOP-25")
            .unwrap()
            .age_ms,
        180 * MIN
    );
}

#[test]
fn back_pressure_counts_every_group() {
    let shop = shop_web();
    let bill = billing_api();
    let projects = [
        ProjectTickets {
            key: "SHOP",
            name: "shop-web",
            tickets: &shop,
        },
        ProjectTickets {
            key: "BILL",
            name: "billing-api",
            tickets: &bill,
        },
    ];
    let list = needs_you_items(&projects, NOW);
    assert_eq!(needs_you_count(&list), 8);
}

#[test]
fn states_that_arrive_later_slot_in_by_status() {
    for status in [Status::Interrupted, Status::ResumeFailed] {
        assert_eq!(group_of(status), Some(NeedsYouGroup::Stopped));
    }
    for status in [
        Status::Working,
        Status::WorkingInBackground,
        Status::WaitingForLimitReset,
        Status::Queued,
        Status::WaitingForSlot,
        Status::Held,
        Status::PrOpen,
        Status::Merged,
    ] {
        assert_eq!(group_of(status), None, "{status}");
    }
    let mut shop = shop_web();
    shop.push(t("SHOP-29", "Resume me", Status::ResumeFailed, 300));
    let list = items(&shop, &[]);
    assert_eq!(list[3].0, "SHOP-29", "the oldest of group 2");
    assert_eq!(list[3].2, "resume failed");
}

#[tokio::test]
async fn the_board_derives_items_across_projects_with_status_times() {
    let f = Fixture::shop().await;
    f.add_project("billing-api").await;
    let ask = f.create("Split user full name").await;
    let review = f
        .create_with(NewTicketInput {
            project: "BILL".into(),
            title: "VAT rounding fix".into(),
            done_when: vec!["rounds".into()],
            ..Default::default()
        })
        .await;
    let before = now_ms();
    let queued = f
        .board
        .move_ticket(&review.key.to_string(), DropTarget::DevQueued, false, &me())
        .await
        .unwrap();
    assert!(queued.status_since.unwrap() >= before, "set by the move");

    let pool = f.board.store().pool();
    for (ticket, status) in [(&ask, Status::AsksYou), (&review, Status::ReadyForReview)] {
        sqlx::query("UPDATE tickets SET board_column = ?, state = ?, state_since = ? WHERE id = ?")
            .bind(status.column().as_str())
            .bind(status.as_str())
            .bind(NOW - 5 * MIN)
            .bind(&ticket.id)
            .execute(pool)
            .await
            .unwrap();
    }
    let list = f.board.needs_you_items(NOW).await.unwrap();
    let keys: Vec<String> = list.iter().map(|i| i.key.to_string()).collect();
    assert_eq!(keys, [ask.key.to_string(), review.key.to_string()]);
    assert_eq!(list[1].project_name, "billing-api");
    assert_eq!(list[1].age_ms, 5 * MIN);
    assert_eq!(list[0].reason, "asks you");
}
