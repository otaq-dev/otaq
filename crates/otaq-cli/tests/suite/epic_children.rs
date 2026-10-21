//! New children of an epic in flight (EPC-13, EPC-11; d0080): a child created while the whole epic is queued joins
//! the queue, after a single-child start it waits in Backlog, and adding or reopening a child after Review makes
//! the epic a track again. Against a service with the sample board (SHOP-40 Wishlist: 40.1 Merged, 40.2 Working,
//! 40.3 Backlog).

use std::process::{Command, Output};

use otaq_core::board::epic_flight::{EpicPlace, EpicQueueMode, epic_place};
use otaq_core::board::tickets::{EventKind, NewTicketInput, Ticket};
use otaq_core::data_dir::DataDir;
use otaq_core::domain::{Actor, Place, Status};
use otaq_core::naming;
use otaq_core::service::testing::TestService;
use serde_json::Value;

const CB: &str = env!("CARGO_BIN_EXE_otaq");

fn cb(w: &TestService, args: &[&str]) -> Output {
    Command::new(CB)
        .args(args)
        .env(DataDir::env_var(), w.data.root())
        .env_remove(naming::ticket_env_var())
        .current_dir(w.home.path())
        .output()
        .unwrap()
}

fn ok(out: &Output) -> String {
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn me() -> Actor {
    Actor::User("me".into())
}

async fn add_child(w: &TestService, title: &str) -> Ticket {
    w.board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: title.into(),
                epic: Some("SHOP-40".into()),
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap()
}

async fn joined_events(w: &TestService, key: &str) -> Vec<Value> {
    w.board
        .history(key)
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EventKind::JoinedQueue)
        .map(|e| e.payload)
        .collect()
}

async fn place(w: &TestService) -> EpicPlace {
    let epic = w.board.ticket("SHOP-40").await.unwrap();
    epic_place(&epic, &w.board.tickets("SHOP").await.unwrap())
}

/// SHOP-40.2's agent runs `otaq create --parent SHOP-40`: with the whole epic queued the child goes straight to
/// Ready for Dev, marked as added by SHOP-40.2; after a single-child start the next one stays in Backlog.
#[tokio::test(flavor = "multi_thread")]
async fn an_agents_child_joins_a_whole_queued_epic() {
    let w = TestService::start().await;
    w.shop_sample().await;
    w.board
        .set_queue_mode("SHOP-40", EpicQueueMode::Whole, &me())
        .await
        .unwrap();
    let shop40_2 = w.board.ticket("SHOP-40.2").await.unwrap();
    let agent = w.agent_shell(&shop40_2).await;

    let ran = agent
        .run(&format!(
            "'{CB}' create 'Wishlist sharing' --parent SHOP-40 --json"
        ))
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    let child: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(child["key"], "SHOP-40.4");
    assert_eq!(child["place"], "waiting");
    assert_eq!(child["status"], Status::Queued.as_str());
    assert_eq!(child["created_by"], "agent:SHOP-40.2");
    assert_eq!(child["joined_queue_by"], "agent:SHOP-40.2");
    let joined = joined_events(&w, "SHOP-40.4").await;
    assert_eq!(joined.len(), 1);
    assert_eq!(joined[0]["epic"], "SHOP-40");

    // A single-child start: the next new child waits as "not queued".
    w.board
        .set_queue_mode("SHOP-40", EpicQueueMode::Single, &me())
        .await
        .unwrap();
    let out = ok(&cb(
        &w,
        &["create", "Wishlist email", "--parent", "SHOP-40", "--json"],
    ));
    let child: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(child["key"], "SHOP-40.5");
    assert_eq!(child["place"], "backlog");
    assert!(child.get("joined_queue_by").is_none(), "{child}");
    assert!(joined_events(&w, "SHOP-40.5").await.is_empty());
    let child = w.board.ticket("SHOP-40.5").await.unwrap();
    assert_eq!(child.started_at, None, "never started");
    w.stop().await;
}

/// "Add ticket to epic" by the user: the child joins the queue "by you"; with no queue mode it lands in Backlog.
#[tokio::test(flavor = "multi_thread")]
async fn the_users_child_joins_by_you() {
    let w = TestService::start().await;
    w.shop_sample().await;
    let waits = add_child(&w, "Wishlist count").await;
    assert_eq!(waits.place, Place::Backlog);
    assert_eq!(waits.joined_queue_by, None);

    w.board
        .set_queue_mode("SHOP-40", EpicQueueMode::Whole, &me())
        .await
        .unwrap();
    let joins = add_child(&w, "Wishlist sharing").await;
    assert_eq!(joins.key.to_string(), "SHOP-40.5");
    assert_eq!(joins.status, Status::Queued);
    assert_eq!(joins.joined_queue_by, Some(me()));
    assert!(joins.queued_at.is_some() && joins.started_at.is_some());
    assert!(joins.base_locked, "queued like a drag: the base is locked");
    assert_eq!(joined_events(&w, "SHOP-40.5").await.len(), 1);
    w.stop().await;
}

/// The epic is in Ready for Review with its queue mode kept: a new child brings it back as a track and joins the
/// queue.
#[tokio::test(flavor = "multi_thread")]
async fn adding_after_review_makes_a_track_and_queues_the_child() {
    let w = TestService::start().await;
    w.shop_sample().await;
    w.board
        .set_queue_mode("SHOP-40", EpicQueueMode::Whole, &me())
        .await
        .unwrap();
    for key in ["SHOP-40.2", "SHOP-40.3"] {
        let child = w.board.ticket(key).await.unwrap();
        w.force_status(&child, Status::Merged).await;
    }
    let epic = w.board.ticket("SHOP-40").await.unwrap();
    w.force_status(&epic, Status::WaitingForYou).await;
    assert_eq!(place(&w).await, EpicPlace::Delivery);

    let child = add_child(&w, "Wishlist sharing").await;
    assert_eq!(child.place, Place::Waiting(None));
    assert_eq!(child.joined_queue_by, Some(me()));
    let epic = w.board.ticket("SHOP-40").await.unwrap();
    assert_eq!(epic.place, Place::Backlog);
    assert_eq!(epic.epic_queue_mode, EpicQueueMode::Whole, "kept");
    assert_eq!(place(&w).await, EpicPlace::Track);
    w.stop().await;
}

/// Reopening a Closed child of an epic in Review: the epic is a track again and the child waits in Backlog ("not
/// queued"), even with the whole epic queued.
#[tokio::test(flavor = "multi_thread")]
async fn reopening_after_review_makes_a_track_with_the_child_in_backlog() {
    let w = TestService::start().await;
    w.shop_sample().await;
    w.board
        .set_queue_mode("SHOP-40", EpicQueueMode::Whole, &me())
        .await
        .unwrap();
    let closed = w.board.ticket("SHOP-40.2").await.unwrap();
    w.force_status(&closed, Status::ClosedNotMerged).await;
    let merged = w.board.ticket("SHOP-40.3").await.unwrap();
    w.force_status(&merged, Status::Merged).await;
    let epic = w.board.ticket("SHOP-40").await.unwrap();
    w.force_status(&epic, Status::WaitingForYou).await;

    let reopened = w.board.reopen("SHOP-40.2", &me()).await.unwrap();
    assert_eq!(reopened.place, Place::Backlog);
    assert_eq!(reopened.joined_queue_by, None);
    assert!(joined_events(&w, "SHOP-40.2").await.is_empty());
    let epic = w.board.ticket("SHOP-40").await.unwrap();
    assert_eq!(epic.place, Place::Backlog);
    assert_eq!(place(&w).await, EpicPlace::Track);
    w.stop().await;
}
