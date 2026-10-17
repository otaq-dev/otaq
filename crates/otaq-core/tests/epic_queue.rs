//! Queueing an epic (T-0263, EPC-12, EPC-14 resume, EPC-15 outcome; d0080): the epic card or track dropped on Ready
//! for Dev or Dev (or ⇧⌘D) queues every child in Backlog and resumes the parked ones; one child row queues only
//! that child. The scheduler's rules then hold: dependencies (d0049), the optional limit (d0016). The example is
//! shop-web's Search epic (Search index API, Search results page) and Wishlist (API, button, page in a chain).

mod common;

use std::collections::HashSet;
use std::time::Duration;

use common::{Fixture, me};
use otaq_core::board::Refusal;
use otaq_core::board::epic_flight::{EpicPlace, EpicQueueMode, EpicQueueOutcome, epic_place};
use otaq_core::board::moves::DropTarget;
use otaq_core::board::settings::{AgentLimits, AutoMerge, ProjectSettings};
use otaq_core::board::tickets::{EventKind, Ticket};
use otaq_core::client::{Client, ClientConfig};
use otaq_core::domain::{Status, TicketKey};
use otaq_core::protocol::{EpicQueueParams, Reply, Request};
use otaq_core::scheduler::{Gate, plan};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use otaq_core::session::SessionKind;
use otaq_core::session::launch::LaunchContext;
use serde_json::json;

fn keys(list: &[TicketKey]) -> Vec<String> {
    list.iter().map(ToString::to_string).collect()
}

fn key(t: &Ticket) -> String {
    t.key.to_string()
}

/// The Search epic with its two children, nothing started.
async fn search(f: &Fixture) -> (Ticket, Ticket, Ticket) {
    let epic = f.epic("Search").await;
    let api = f.child(&epic, "Search index API").await;
    let page = f.child(&epic, "Search results page").await;
    (epic, api, page)
}

async fn place(f: &Fixture, epic: &Ticket) -> EpicPlace {
    let tickets = f.board.tickets("SHOP").await.unwrap();
    epic_place(&f.ticket(&key(epic)).await, &tickets)
}

#[tokio::test]
async fn queueing_the_epic_queues_every_never_started_child() {
    let f = Fixture::shop().await;
    let (epic, api, page) = search(&f).await;
    assert_eq!(place(&f, &epic).await, EpicPlace::Card);

    let outcome = f.board.queue_epic(&key(&epic), &me()).await.unwrap();
    assert_eq!(keys(&outcome.queued), [key(&api), key(&page)]);
    assert!(outcome.resumed.is_empty());
    assert!(outcome.auto_merge_off, "Auto-merge is Off by default");
    assert_eq!(outcome.project, "shop-web");
    assert_eq!(
        serde_json::to_value(&outcome).unwrap(),
        json!({
            "epic": key(&epic), "queued": [key(&api), key(&page)], "resumed": [],
            "auto_merge_off": true, "project": "shop-web", "lead_started": false
        }),
        "what the toast reads"
    );

    for child in [&api, &page] {
        let child = f.ticket(&key(child)).await;
        assert_eq!(child.status, Status::Queued);
        assert!(child.queued_at.is_some());
        let history = f.board.history(&key(&child)).await.unwrap();
        let moved = history.iter().find(|e| e.kind == EventKind::Moved).unwrap();
        assert_eq!(moved.payload["to"], "queued");
        assert_eq!(
            moved.payload["with_epic"],
            key(&epic),
            "Queued with the epic"
        );
        assert_eq!(moved.actor, me());
    }
    let after = f.ticket(&key(&epic)).await;
    assert_eq!(after.epic_queue_mode, EpicQueueMode::Whole);
    assert!(after.base_locked, "the epic's base locks with its children");
    assert_eq!(place(&f, &epic).await, EpicPlace::Track);
    let history = f.board.history(&key(&epic)).await.unwrap();
    assert_eq!(history[0].kind, EventKind::EpicQueueModeSet);
    assert_eq!(history[0].payload, json!({"from": "none", "to": "whole"}));

    // Nothing left to queue.
    let err = f.board.queue_epic(&key(&epic), &me()).await.unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::MoveRefused));
    assert_eq!(
        err.to_string(),
        format!("Every child of {} is already queued or started", epic.key)
    );
}

/// A drop of the epic on Ready for Dev or Dev (or ⇧⌘D, a drop on Ready for Dev) is the same queue.
#[tokio::test]
async fn dropping_the_epic_queues_it() {
    let f = Fixture::shop().await;
    let (epic, api, page) = search(&f).await;
    let moved = f
        .board
        .move_ticket(&key(&epic), DropTarget::Dev, false, &me())
        .await
        .unwrap();
    assert_eq!(moved.id, epic.id);
    assert_eq!(moved.epic_queue_mode, EpicQueueMode::Whole);
    assert_eq!(moved.status, epic.status, "the epic itself stays put");
    for child in [&api, &page] {
        assert_eq!(f.ticket(&key(child)).await.status, Status::Queued);
    }
}

#[tokio::test]
async fn one_child_row_queues_only_that_child() {
    let f = Fixture::shop().await;
    let (epic, api, page) = search(&f).await;

    let outcome = f.board.queue_epic_child(&key(&api), &me()).await.unwrap();
    assert_eq!(keys(&outcome.queued), [key(&api)]);
    assert!(outcome.resumed.is_empty());
    assert_eq!(outcome.epic, epic.key);
    assert_eq!(f.ticket(&key(&api)).await.status, Status::Queued);
    assert_eq!(
        f.ticket(&key(&page)).await.status,
        Status::Backlog,
        "not queued"
    );
    assert_eq!(
        f.ticket(&key(&epic)).await.epic_queue_mode,
        EpicQueueMode::Single
    );
    assert_eq!(place(&f, &epic).await, EpicPlace::Track);
    let history = f.board.history(&key(&api)).await.unwrap();
    assert_eq!(history[0].payload["with_epic"], key(&epic));

    // Already queued: refused.
    let err = f
        .board
        .queue_epic_child(&key(&api), &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::MoveRefused));

    // Dragging the track to Ready for Dev later queues the rest, and the mode becomes Whole.
    let outcome = f.board.queue_epic(&key(&epic), &me()).await.unwrap();
    assert_eq!(keys(&outcome.queued), [key(&page)]);
    assert_eq!(
        f.ticket(&key(&epic)).await.epic_queue_mode,
        EpicQueueMode::Whole
    );

    // Whole stays Whole when one more row is queued on its own.
    let late = f.child(&epic, "Search suggestions").await;
    f.board
        .move_ticket(&key(&late), DropTarget::ReadyForDev, false, &me())
        .await
        .unwrap();
    assert_eq!(f.ticket(&key(&late)).await.status, Status::Queued);
    assert_eq!(
        f.ticket(&key(&epic)).await.epic_queue_mode,
        EpicQueueMode::Whole
    );
}

/// A child's own drag is a single-child queue too: None becomes Single.
#[tokio::test]
async fn dragging_a_child_makes_the_mode_single() {
    let f = Fixture::shop().await;
    let (epic, api, _page) = search(&f).await;
    f.board
        .move_ticket(&key(&api), DropTarget::Dev, false, &me())
        .await
        .unwrap();
    assert_eq!(
        f.ticket(&key(&epic)).await.epic_queue_mode,
        EpicQueueMode::Single
    );
}

#[tokio::test]
async fn dependencies_still_hold() {
    let f = Fixture::shop().await;
    let epic = f.epic("Wishlist").await;
    let api = f.child(&epic, "Wishlist API").await;
    let button = f.child(&epic, "Wishlist button").await;
    let page = f.child(&epic, "Wishlist page").await;
    f.board
        .add_dependency(&key(&button), &key(&api), &me())
        .await
        .unwrap();
    f.board
        .add_dependency(&key(&page), &key(&button), &me())
        .await
        .unwrap();
    let outcome = f.board.queue_epic(&key(&epic), &me()).await.unwrap();
    assert_eq!(outcome.queued.len(), 3);

    let tickets = f.board.tickets("SHOP").await.unwrap();
    let pass = plan(&tickets, &HashSet::new(), &Gate::default());
    let starts: Vec<String> = pass.start.iter().map(|s| s.key.to_string()).collect();
    assert_eq!(starts, [key(&api)], "the button waits for the API");
}

#[tokio::test]
async fn the_limit_holds() {
    let f = Fixture::shop().await;
    let (epic, api, page) = search(&f).await;
    f.board.queue_epic(&key(&epic), &me()).await.unwrap();
    let tickets = f.board.tickets("SHOP").await.unwrap();
    let gate = Gate {
        limits: AgentLimits {
            enabled: true,
            max_concurrency: 1,
            back_pressure: 10,
        },
        ..Gate::default()
    };
    let pass = plan(&tickets, &HashSet::new(), &gate);
    let starts: Vec<String> = pass.start.iter().map(|s| s.key.to_string()).collect();
    assert_eq!(starts, [key(&api)]);
    assert!(
        pass.changes
            .iter()
            .any(|c| c.key == page.key && c.to == Status::WaitingForSlot),
        "{:?}",
        pass.changes
    );
}

#[tokio::test]
async fn the_outcome_says_whether_auto_merge_is_off() {
    let f = Fixture::shop().await;
    let (epic, _api, _page) = search(&f).await;
    f.board
        .set_project_settings(
            "SHOP",
            &ProjectSettings {
                auto_merge: Some(AutoMerge::EpicChildren),
                ..ProjectSettings::default()
            },
        )
        .await
        .unwrap();
    let outcome = f.board.queue_epic(&key(&epic), &me()).await.unwrap();
    assert!(!outcome.auto_merge_off);
}

#[tokio::test]
async fn what_cannot_be_queued_is_refused() {
    let f = Fixture::shop().await;
    let (epic, api, page) = search(&f).await;
    let standalone = f.create("Fix checkout rounding").await;
    let err = f
        .board
        .queue_epic(&key(&standalone), &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NotAnEpic));
    let err = f
        .board
        .queue_epic_child(&key(&standalone), &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::MoveRefused));

    // Every child active or done.
    f.force_status(&api, Status::Working).await;
    f.force_status(&page, Status::ClosedNotMerged).await;
    let err = f.board.queue_epic(&key(&epic), &me()).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        format!("Every child of {} is already queued or started", epic.key)
    );

    // Its delivery card keeps today's rules.
    f.force_status(&epic, Status::ReadyForReview).await;
    let err = f.board.queue_epic(&key(&epic), &me()).await.unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::MoveRefused));
}

async fn until_status(w: &TestService, key: &str, want: Status) {
    let reached = tokio::time::timeout(Duration::from_secs(20), async {
        while w.status(key).await != want {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    if reached.is_err() {
        let t = w.board.ticket(key).await.unwrap();
        panic!(
            "{key} never became {want}: {} {:?}",
            t.status, t.start_problem
        );
    }
}

/// After Park all the epic is a card with a parked row; queueing it again resumes that child's conversation and
/// queues the never-started one. `claude` stands in as an `sh` printing its arguments. Through `epic.queue`.
#[tokio::test]
async fn queueing_after_park_resumes_the_parked_child() {
    let w = TestService::start_with(|config| ServiceConfig {
        launch: Some(LaunchContext {
            claude: vec![
                "sh".into(),
                "-c".into(),
                "printf 'ARGS %s\\n' \"$*\"; exec sleep 30".into(),
                "sh".into(),
            ],
            bin_dir: "/nonexistent".into(),
            plugin_dir: "/nonexistent/plugin".into(),
            path: std::env::var_os("PATH").unwrap_or_default(),
            claude_config_dir: None,
        }),
        scheduler_tick: Duration::from_secs(3600),
        ..config
    })
    .await;
    w.add_project("shop-web").await;
    let epic = w
        .board
        .create_ticket(
            &otaq_core::board::tickets::NewTicketInput {
                project: "SHOP".into(),
                title: "Search".into(),
                kind: otaq_core::domain::Kind::Epic,
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    let child = |title: &str| otaq_core::board::tickets::NewTicketInput {
        project: "SHOP".into(),
        title: title.into(),
        epic: Some(key(&epic)),
        ..Default::default()
    };
    let api = w
        .board
        .create_ticket(&child("Search index API"), &me())
        .await
        .unwrap();
    let page = w
        .board
        .create_ticket(&child("Search results page"), &me())
        .await
        .unwrap();

    w.board.queue_epic_child(&key(&api), &me()).await.unwrap();
    w.starter.start(&key(&api), &me()).await.unwrap();
    until_status(&w, &key(&api), Status::Working).await;
    let first = w.sessions.running_of(&api.id, SessionKind::Agent).unwrap();
    let conversation = first.tool_session_id.clone().unwrap();
    w.review.park(&key(&api), true, &me()).await.unwrap();
    assert_eq!(w.status(&key(&api)).await, Status::Parked);
    let tickets = w.board.tickets("SHOP").await.unwrap();
    assert_eq!(
        epic_place(&w.board.ticket(&key(&epic)).await.unwrap(), &tickets),
        EpicPlace::Card,
        "a parked row"
    );

    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let reply = client
        .request(Request::EpicQueue(EpicQueueParams {
            key: key(&epic),
            child: None,
        }))
        .await
        .unwrap();
    let Reply::EpicQueued(outcome) = reply else {
        panic!("{reply:?}")
    };
    let outcome: EpicQueueOutcome = outcome;
    assert_eq!(keys(&outcome.resumed), [key(&api)]);
    assert_eq!(keys(&outcome.queued), [key(&page)]);
    assert_eq!(
        w.board.ticket(&key(&epic)).await.unwrap().epic_queue_mode,
        EpicQueueMode::Whole
    );

    // The scheduler (woken by the change) starts both: the parked one in its same conversation.
    until_status(&w, &key(&api), Status::Working).await;
    let resumed = w.sessions.running_of(&api.id, SessionKind::Agent).unwrap();
    assert_ne!(resumed.id, first.id);
    let out = w.wait_for_output(&resumed.id, "ARGS").await;
    assert!(out.contains(&format!("--resume {conversation}")), "{out}");
    let row = w.board.store().ticket(&api.id).await.unwrap().unwrap();
    assert!(!row.parked);
    assert!(row.agent_port.is_some(), "new ports");
    until_status(&w, &key(&page), Status::Working).await;
    let fresh = w.sessions.running_of(&page.id, SessionKind::Agent).unwrap();
    let out = w.wait_for_output(&fresh.id, "ARGS").await;
    assert!(!out.contains("--resume"), "{out}");
    w.sessions.stop(&resumed.id).await.unwrap();
    w.sessions.stop(&fresh.id).await.unwrap();

    // One row of another epic's child through the request: refused when it isn't that epic's.
    let err = client
        .request(Request::EpicQueue(EpicQueueParams {
            key: "SHOP-99".into(),
            child: Some(key(&page)),
        }))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("isn't a child of SHOP-99"),
        "{err}"
    );
    w.stop().await;
}
