//! Taking an epic back (T-0264, EPC-14, EPC-15 Undo; d0080, d0052, d0066): its track dropped on Backlog unqueues
//! the waiting children and leaves the running ones alone; when nothing waits, Park all parks every running child
//! after one confirmation; the queue toast's Undo takes back only what still waits. The example is shop-web's
//! Search epic SHOP-45 (Search index API, Search results page). `claude` stands in as an `sh` that waits.

mod common;

use std::time::Duration;

use common::{Fixture, me};
use otaq_core::agent_tools::LaunchContext;
use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::board::Refusal;
use otaq_core::board::epic_flight::{
    EpicPlace, EpicQueueMode, ParkAllConfirmation, TakeBackPlan, epic_place, take_back_plan,
};
use otaq_core::board::tickets::{EventKind, NewTicketInput, Ticket};
use otaq_core::client::{Client, ClientConfig};
use otaq_core::domain::{Actor, Kind, Status, TicketKey, ValidationTrigger};
use otaq_core::protocol::{CloseParams, EpicUndoQueueParams, Reply, Request};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::{AgentShell, TestService};
use otaq_core::session::SessionKind;
use serde_json::json;

fn key(t: &Ticket) -> String {
    t.key.to_string()
}

fn keys(list: &[TicketKey]) -> Vec<String> {
    list.iter().map(ToString::to_string).collect()
}

/// A service whose `claude` (agent or validator) is an `sh` that waits; the scheduler only runs when woken.
async fn world() -> TestService {
    let w = TestService::start_with(|config| ServiceConfig {
        launch: Some(LaunchContext {
            bin_dir: "/nonexistent".into(),
            plugin_dir: "/nonexistent/plugin".into(),
            path: std::env::var_os("PATH").unwrap_or_default(),
            claude: ClaudeLaunch {
                program: vec![
                    "sh".into(),
                    "-c".into(),
                    "printf 'ARGS %s\\n' \"$*\"; exec sleep 30".into(),
                    "sh".into(),
                ],
                config_dir: None,
            },
        }),
        scheduler_tick: Duration::from_secs(3600),
        ..config
    })
    .await;
    w.add_project("shop-web").await;
    w
}

/// SHOP-45 Search with SHOP-45.1 Search index API and SHOP-45.2 Search results page, nothing started. The tests
/// change the board directly (no change event wakes the scheduler) until a request leaves nothing queued, so the
/// scheduler never starts a queued child behind their back.
async fn search(w: &TestService) -> (Ticket, Ticket, Ticket) {
    while w.create("SHOP", "filler").await.key.number < 44 {}
    let epic = w
        .board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Search".into(),
                kind: Kind::Epic,
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    assert_eq!(key(&epic), "SHOP-45");
    let api = child(w, &epic, "Search index API").await;
    let page = child(w, &epic, "Search results page").await;
    (epic, api, page)
}

async fn child(w: &TestService, epic: &Ticket, title: &str) -> Ticket {
    w.board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: title.into(),
                epic: Some(key(epic)),
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap()
}

/// `child` running in Dev: its branch and worktree, Working, an agent session (kept while the shell lives).
async fn running(w: &TestService, child: &Ticket) -> AgentShell {
    w.start_worktree(child).await;
    w.force_status(child, Status::Working).await;
    w.agent_shell(child).await
}

async fn plan(w: &TestService, epic: &Ticket) -> TakeBackPlan {
    let tickets = w.board.tickets("SHOP").await.unwrap();
    take_back_plan(&w.board.ticket(&key(epic)).await.unwrap(), &tickets)
}

async fn place(w: &TestService, epic: &Ticket) -> EpicPlace {
    let tickets = w.board.tickets("SHOP").await.unwrap();
    epic_place(&w.board.ticket(&key(epic)).await.unwrap(), &tickets)
}

async fn mode(w: &TestService, epic: &Ticket) -> EpicQueueMode {
    w.board.ticket(&key(epic)).await.unwrap().epic_queue_mode
}

/// Flow 3: SHOP-45.1 runs, 45.2 waits. The drop unqueues 45.2 (and a child that joined the queue on its own);
/// 45.1 keeps running, so the track stays.
#[tokio::test(flavor = "multi_thread")]
async fn unqueue_leaves_the_running_child_alone() {
    let w = world().await;
    let (epic, api, page) = search(&w).await;
    w.board.queue_epic(&key(&epic), &me()).await.unwrap();
    let _agent = running(&w, &api).await;

    let plan_now = plan(&w, &epic).await;
    assert_eq!(
        plan_now,
        TakeBackPlan::Unqueue {
            epic: epic.key.clone(),
            unqueue: vec![page.key.clone()],
            never_started: vec![page.key.clone()],
            keep_running: vec![api.key.clone()],
        }
    );
    assert_eq!(plan_now.hint(), "Unqueue 1 never-started child (45.2)");
    assert_eq!(plan_now.note().as_deref(), Some("SHOP-45.1 keeps running"));
    assert_eq!(plan_now.confirmation(), None);

    // A child added while the whole epic is queued joins the queue; taking back takes it too, and its mark goes.
    let late = child(&w, &epic, "Search suggestions").await;
    assert_eq!(late.status, Status::Queued);
    assert!(late.joined_queue_by.is_some());
    assert_eq!(
        plan(&w, &epic).await.hint(),
        "Unqueue 2 never-started children (45.2, 45.3)"
    );

    let done = w.review.take_back_epic(&key(&epic), false, &me()).await;
    assert!(
        matches!(done, Ok(TakeBackPlan::Unqueue { .. })),
        "no confirmation: {done:?}"
    );
    for t in [&page, &late] {
        let t = w.board.ticket(&key(t)).await.unwrap();
        assert_eq!(t.status, Status::Backlog);
        assert!(t.joined_queue_by.is_none());
        assert!(t.queued_at.is_none());
        let history = w.board.history(&key(&t)).await.unwrap();
        assert_eq!(history[0].kind, EventKind::Moved);
        assert_eq!(history[0].payload["to"], "backlog");
        assert_eq!(history[0].payload["with_epic"], key(&epic));
    }
    assert_eq!(w.status(&key(&api)).await, Status::Working);
    assert!(w.sessions.running_of(&api.id, SessionKind::Agent).is_some());
    assert_eq!(mode(&w, &epic).await, EpicQueueMode::None);
    assert_eq!(place(&w, &epic).await, EpicPlace::Track, "45.1 has started");
    let history = w.board.history(&key(&epic)).await.unwrap();
    assert_eq!(history[0].kind, EventKind::EpicQueueModeSet);
    assert_eq!(history[0].payload, json!({"from": "whole", "to": "none"}));
    w.stop().await;
}

/// Nothing had started: the track turns back into the card. Taking back again is refused.
#[tokio::test(flavor = "multi_thread")]
async fn unqueue_with_nothing_started_is_the_card_again() {
    let w = world().await;
    let (epic, api, page) = search(&w).await;
    w.board.queue_epic(&key(&epic), &me()).await.unwrap();
    assert_eq!(place(&w, &epic).await, EpicPlace::Track);

    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let take_back = || {
        client.request(Request::EpicTakeBack(CloseParams {
            key: key(&epic),
            confirm: false,
        }))
    };
    let reply = take_back().await.unwrap();
    let Reply::EpicTakenBack(TakeBackPlan::Unqueue { unqueue, .. }) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(keys(&unqueue), [key(&api), key(&page)]);
    for t in [&api, &page] {
        let t = w.board.ticket(&key(t)).await.unwrap();
        assert_eq!(t.status, Status::Backlog);
        assert!(!t.has_started(), "never started again (d0066)");
    }
    assert_eq!(mode(&w, &epic).await, EpicQueueMode::None);
    assert_eq!(place(&w, &epic).await, EpicPlace::Card);

    assert_eq!(
        plan(&w, &epic).await,
        TakeBackPlan::Refused {
            epic: epic.key.clone(),
            reason: "Nothing of SHOP-45 is queued or running".into(),
        }
    );
    let err = take_back().await.unwrap_err();
    assert!(
        err.to_string()
            .contains("Nothing of SHOP-45 is queued or running"),
        "{err}"
    );
    let err = w
        .review
        .take_back_epic(&key(&api), true, &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NotAnEpic));
    w.stop().await;
}

/// State 11: every child has started and both run. One confirmation, then Park for each (d0052): agents and the
/// validator stop, the work and the conversations stay, and the epic is a card with "Parked · has work" rows.
#[tokio::test(flavor = "multi_thread")]
async fn park_all_parks_every_running_child_after_one_confirmation() {
    let w = world().await;
    let (epic, api, page) = search(&w).await;
    w.board.queue_epic(&key(&epic), &me()).await.unwrap();
    let _agent = running(&w, &api).await;
    let _page_agent = running(&w, &page).await;
    // 45.2's agent submitted: its validator runs (d0063).
    w.validation
        .submit(
            &key(&page),
            "Results show.",
            &Actor::Agent(page.key.clone()),
            ValidationTrigger::Agent,
        )
        .await
        .unwrap();
    assert_eq!(w.status(&key(&page)).await, Status::Validating);
    assert!(
        w.sessions
            .running_of(&page.id, SessionKind::Validator)
            .is_some()
    );

    let plan_now = plan(&w, &epic).await;
    assert_eq!(
        plan_now,
        TakeBackPlan::ParkAll {
            epic: epic.key.clone(),
            park: vec![api.key.clone(), page.key.clone()],
        }
    );
    assert_eq!(plan_now.hint(), "Park 2 running children");
    assert_eq!(plan_now.note(), None);
    assert_eq!(
        plan_now.confirmation(),
        Some(ParkAllConfirmation {
            title: "Park 2 running tickets?".into(),
            body: "Their work is kept. SHOP-45.1 and SHOP-45.2 stop and go back to Backlog with their \
                   conversations."
                .into(),
            cancel: "Cancel".into(),
            confirm: "Park 2 tickets".into(),
        })
    );

    // Without the yes: nothing happens.
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let take_back = |confirm| {
        client.request(Request::EpicTakeBack(CloseParams {
            key: key(&epic),
            confirm,
        }))
    };
    let err = take_back(false).await.unwrap_err();
    assert!(err.to_string().contains("Their work is kept."), "{err}");
    let err = w
        .review
        .take_back_epic(&key(&epic), false, &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NeedsConfirmation));
    assert_eq!(w.status(&key(&api)).await, Status::Working);
    assert!(w.sessions.running_of(&api.id, SessionKind::Agent).is_some());
    assert_eq!(mode(&w, &epic).await, EpicQueueMode::Whole);

    let reply = take_back(true).await.unwrap();
    assert!(
        matches!(reply, Reply::EpicTakenBack(TakeBackPlan::ParkAll { .. })),
        "{reply:?}"
    );
    for t in [&api, &page] {
        let after = w.board.ticket(&key(t)).await.unwrap();
        assert_eq!(after.status, Status::Parked);
        assert_eq!(after.status.label(), "Parked · has work");
        assert!(w.sessions.running_of(&t.id, SessionKind::Agent).is_none());
        assert!(
            w.sessions
                .running_of(&t.id, SessionKind::Validator)
                .is_none()
        );
        let row = w.board.store().ticket(&t.id).await.unwrap().unwrap();
        assert!(row.parked);
        assert!(row.branch_name.is_some(), "the work is kept");
        assert!(row.agent_port.is_none(), "the ports are freed");
        let worktree = row.worktree_path.unwrap();
        assert!(std::path::Path::new(&worktree).exists());
    }
    assert_eq!(mode(&w, &epic).await, EpicQueueMode::None);
    assert_eq!(place(&w, &epic).await, EpicPlace::Card);
    w.stop().await;
}

/// The queue toast's Undo right away takes back everything: the card again, mode None.
#[tokio::test(flavor = "multi_thread")]
async fn undo_right_after_queueing_is_the_card_again() {
    let w = world().await;
    let (epic, api, page) = search(&w).await;
    let outcome = w.board.queue_epic(&key(&epic), &me()).await.unwrap();
    let undo: Vec<String> = outcome.keys().map(ToString::to_string).collect();

    let undone = w
        .board
        .undo_epic_queue(&key(&epic), &undo, &me())
        .await
        .unwrap();
    assert_eq!(keys(&undone.taken_back), [key(&api), key(&page)]);
    assert!(undone.already_started.is_empty());
    assert_eq!(
        serde_json::to_value(&undone).unwrap(),
        json!({"epic": "SHOP-45", "taken_back": ["SHOP-45.1", "SHOP-45.2"], "already_started": []}),
        "what the toast reads"
    );
    for t in [&api, &page] {
        assert_eq!(w.status(&key(t)).await, Status::Backlog);
    }
    assert_eq!(mode(&w, &epic).await, EpicQueueMode::None);
    assert_eq!(place(&w, &epic).await, EpicPlace::Card);
    w.stop().await;
}

/// Undo after 45.1 started: 45.2 goes back, the reply names 45.1, nothing is parked and the mode stays.
#[tokio::test(flavor = "multi_thread")]
async fn undo_after_a_child_started_names_it() {
    let w = world().await;
    let (epic, api, page) = search(&w).await;
    let outcome = w.board.queue_epic(&key(&epic), &me()).await.unwrap();
    let _agent = running(&w, &api).await;

    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let reply = client
        .request(Request::EpicUndoQueue(EpicUndoQueueParams {
            key: key(&epic),
            keys: outcome.keys().map(ToString::to_string).collect(),
        }))
        .await
        .unwrap();
    let Reply::EpicQueueUndone(undone) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(keys(&undone.taken_back), [key(&page)]);
    assert_eq!(keys(&undone.already_started), [key(&api)]);
    assert_eq!(w.status(&key(&page)).await, Status::Backlog);
    assert_eq!(w.status(&key(&api)).await, Status::Working, "never parks");
    assert!(w.sessions.running_of(&api.id, SessionKind::Agent).is_some());
    assert_eq!(mode(&w, &epic).await, EpicQueueMode::Whole);
    assert_eq!(place(&w, &epic).await, EpicPlace::Track);

    // A key of another epic is refused.
    let other = w.create("SHOP", "Fix checkout rounding").await;
    let err = w
        .board
        .undo_epic_queue(&key(&epic), &[key(&other)], &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::MoveRefused));
    w.stop().await;
}

/// A parked child resumed by the queue waits in Ready for Dev with its work: taking back (and Undo) returns it to
/// Parked, and the hint doesn't call it never-started (Q57).
#[tokio::test]
async fn a_resumed_child_goes_back_to_parked() {
    let f = Fixture::shop().await;
    f.skip_to(45).await;
    let epic = f.epic("Search").await;
    let api = f.child(&epic, "Search index API").await;
    let page = f.child(&epic, "Search results page").await;
    sqlx::query(
        "UPDATE tickets SET board_column = 'backlog', state = 'parked', parked = TRUE, branch_name = 'otaq/x'
         WHERE id = ?",
    )
    .bind(&api.id)
    .execute(f.board.store().pool())
    .await
    .unwrap();
    let outcome = f.board.queue_epic(&key(&epic), &me()).await.unwrap();
    assert_eq!(keys(&outcome.resumed), [key(&api)]);

    let tickets = f.board.tickets("SHOP").await.unwrap();
    let plan_now = take_back_plan(&f.ticket(&key(&epic)).await, &tickets);
    assert_eq!(plan_now.hint(), "Unqueue 2 queued children (45.1, 45.2)");
    let TakeBackPlan::Unqueue { unqueue, .. } = &plan_now else {
        panic!("{plan_now:?}")
    };
    f.board
        .unqueue_epic_children(&key(&epic), unqueue, &me())
        .await
        .unwrap();
    assert_eq!(f.ticket(&key(&api)).await.status, Status::Parked);
    assert_eq!(f.ticket(&key(&page)).await.status, Status::Backlog);

    // Undo of a resume: back to Parked too.
    f.board.queue_epic_child(&key(&api), &me()).await.unwrap();
    let undone = f
        .board
        .undo_epic_queue(&key(&epic), &[key(&api)], &me())
        .await
        .unwrap();
    assert_eq!(keys(&undone.taken_back), [key(&api)]);
    assert_eq!(f.ticket(&key(&api)).await.status, Status::Parked);
    assert_eq!(
        f.ticket(&key(&epic)).await.epic_queue_mode,
        EpicQueueMode::None
    );
}

/// The words for one running child, and for three.
#[tokio::test]
async fn park_all_words_for_one_and_for_three() {
    let f = Fixture::shop().await;
    f.skip_to(45).await;
    let epic = f.epic("Search").await;
    let a = f.child(&epic, "Search index API").await;
    let b = f.child(&epic, "Search results page").await;
    let c = f.child(&epic, "Search suggestions").await;
    f.force_status(&a, Status::Working).await;
    f.force_status(&b, Status::Merged).await;
    f.force_status(&c, Status::Merged).await;
    let tickets = f.board.tickets("SHOP").await.unwrap();
    let one = take_back_plan(&f.ticket(&key(&epic)).await, &tickets);
    assert_eq!(one.hint(), "Park 1 running child");
    assert_eq!(
        one.confirmation(),
        Some(ParkAllConfirmation {
            title: "Park 1 running ticket?".into(),
            body:
                "Its work is kept. SHOP-45.1 stops and goes back to Backlog with its conversation."
                    .into(),
            cancel: "Cancel".into(),
            confirm: "Park 1 ticket".into(),
        })
    );

    for t in [&b, &c] {
        f.force_status(t, Status::Working).await;
    }
    let tickets = f.board.tickets("SHOP").await.unwrap();
    let three = take_back_plan(&f.ticket(&key(&epic)).await, &tickets);
    assert_eq!(three.hint(), "Park 3 running children");
    assert_eq!(
        three.confirmation().unwrap().body,
        "Their work is kept. SHOP-45.1, SHOP-45.2 and SHOP-45.3 stop and go back to Backlog with their \
         conversations."
    );
}
