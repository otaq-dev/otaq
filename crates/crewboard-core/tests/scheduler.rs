//! The scheduler (SCH-01, SCH-02; T-0079): pure pass decisions, and the loop with an injected start function:
//! the storyboard chain, oldest first, event and tick triggers, overlapping passes, and what it never touches.

mod common;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{Fixture, me};
use crewboard_core::board::moves::DropTarget;
use crewboard_core::board::tickets::{Base, NewTicketInput, Ticket};
use crewboard_core::domain::{Actor, Flags, Kind, Status, TicketKey};
use crewboard_core::protocol::Event;
use crewboard_core::scheduler::{Plan, Scheduler, StartTickets, StatusChange, ToStart, plan};
use crewboard_core::service::EventBus;
use crewboard_core::start::Starter;
use tokio::sync::watch;

fn t(key: &str, status: Status, queued_at: i64) -> Ticket {
    Ticket {
        id: key.into(),
        key: key.parse::<TicketKey>().unwrap(),
        kind: Kind::Ticket,
        title: key.into(),
        body: String::new(),
        done_when: Vec::new(),
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
        status_since: None,
        queued_at: Some(queued_at),
        work: None,
        start_problem: None,
        signal: None,
        question: None,
    }
}

fn deps(mut ticket: Ticket, parents: &[&str]) -> Ticket {
    ticket.depends_on = parents.iter().map(|p| p.parse().unwrap()).collect();
    ticket
}

fn keys(list: &[ToStart]) -> Vec<String> {
    list.iter().map(|t| t.key.to_string()).collect()
}

#[test]
fn ready_tickets_start_oldest_first_by_queue_time() {
    let board = [
        t("SHOP-1", Status::Queued, 30),
        t("SHOP-2", Status::Queued, 10),
        t("BILL-1", Status::Queued, 20),
    ];
    let pass = plan(&board, &HashSet::new());
    assert_eq!(keys(&pass.start), ["SHOP-2", "BILL-1", "SHOP-1"]);
    assert!(pass.changes.is_empty());
}

#[test]
fn the_scheduler_never_queues_retries_or_touches_other_states() {
    let mut board: Vec<Ticket> = [
        Status::Backlog,
        Status::Parked,
        Status::Held,
        Status::StartInterrupted,
        Status::Working,
        Status::Idle,
        Status::ReadyForReview,
        Status::PrOpen,
        Status::Merged,
        Status::ClosedNotMerged,
    ]
    .iter()
    .enumerate()
    .map(|(i, s)| t(&format!("SHOP-{}", i + 1), *s, 0))
    .collect();
    assert_eq!(plan(&board, &HashSet::new()), Plan::default());
    // Even with every dependency merged.
    for ticket in &mut board {
        ticket.depends_on.clear();
    }
    assert_eq!(plan(&board, &HashSet::new()), Plan::default());
}

#[test]
fn starting_tickets_are_skipped() {
    let board = [
        t("SHOP-1", Status::Queued, 1),
        t("SHOP-2", Status::Queued, 2),
    ];
    let pass = plan(&board, &HashSet::from(["SHOP-1".to_owned()]));
    assert_eq!(keys(&pass.start), ["SHOP-2"]);
}

#[test]
fn a_closed_parent_moves_the_child_to_parent_closed_and_back() {
    let board = [
        t("SHOP-19", Status::ClosedNotMerged, 0),
        deps(t("SHOP-20", Status::Queued, 1), &["SHOP-19"]),
    ];
    let pass = plan(&board, &HashSet::new());
    assert!(pass.start.is_empty());
    assert_eq!(
        pass.changes,
        [StatusChange {
            id: "SHOP-20".into(),
            key: "SHOP-20".parse().unwrap(),
            from: Status::Queued,
            to: Status::ParentClosed,
        }]
    );
    // The parent is reopened: SHOP-20 waits again.
    let board = [
        t("SHOP-19", Status::Backlog, 0),
        deps(t("SHOP-20", Status::ParentClosed, 1), &["SHOP-19"]),
    ];
    let pass = plan(&board, &HashSet::new());
    assert_eq!(pass.changes[0].to, Status::Queued);
    assert!(pass.start.is_empty());
    // The dependency is removed instead: back to Queued and started.
    let board = [
        t("SHOP-19", Status::ClosedNotMerged, 0),
        t("SHOP-20", Status::ParentClosed, 1),
    ];
    let pass = plan(&board, &HashSet::new());
    assert_eq!(pass.changes[0].to, Status::Queued);
    assert_eq!(keys(&pass.start), ["SHOP-20"]);
}

/// A start function that records what it was asked to start and keeps those tickets "starting" for good.
#[derive(Default)]
struct Recorder {
    started: Mutex<Vec<String>>,
    starting: Mutex<HashSet<String>>,
}

impl Recorder {
    fn started(&self) -> Vec<String> {
        self.started.lock().unwrap().clone()
    }
}

impl StartTickets for Recorder {
    fn spawn_start(&self, ticket: &ToStart) {
        self.started.lock().unwrap().push(ticket.key.to_string());
        self.starting.lock().unwrap().insert(ticket.id.clone());
    }

    fn starting(&self) -> HashSet<String> {
        self.starting.lock().unwrap().clone()
    }
}

struct World {
    f: Fixture,
    recorder: Arc<Recorder>,
    events: EventBus,
    scheduler: Scheduler,
}

async fn world(tick: Duration) -> World {
    let f = Fixture::shop().await;
    let recorder = Arc::new(Recorder::default());
    let events = EventBus::new(64);
    let scheduler = Scheduler::new(f.board.clone(), recorder.clone(), events.clone(), tick);
    World {
        f,
        recorder,
        events,
        scheduler,
    }
}

async fn create(f: &Fixture, title: &str, parents: &[&str]) -> Ticket {
    let t = f
        .create_with(NewTicketInput {
            title: title.into(),
            done_when: vec!["works".into()],
            ..common::shop_input()
        })
        .await;
    for parent in parents {
        f.board
            .add_dependency(&t.key.to_string(), parent, &me())
            .await
            .unwrap();
    }
    t
}

async fn queue(f: &Fixture, key: &str) {
    f.board
        .move_ticket(key, DropTarget::DevQueued, false, &me())
        .await
        .unwrap();
}

async fn set(f: &Fixture, key: &str, status: Status) {
    let t = f.ticket(key).await;
    sqlx::query("UPDATE tickets SET board_column = ?, state = ? WHERE id = ?")
        .bind(status.column().as_str())
        .bind(status.as_str())
        .bind(&t.id)
        .execute(f.board.store().pool())
        .await
        .unwrap();
}

#[tokio::test]
async fn storyboard_chain_shop_20_starts_only_after_both_parents_merged() {
    let w = world(Duration::from_secs(3600)).await;
    let f = &w.f;
    // SHOP-1 stands in for SHOP-17 (merged), SHOP-2 for SHOP-18, SHOP-3 for SHOP-19, SHOP-4 for SHOP-20.
    create(f, "Add Stripe client", &[]).await;
    set(f, "SHOP-1", Status::Merged).await;
    create(f, "Checkout API", &["SHOP-1"]).await;
    create(f, "Price table", &[]).await;
    create(f, "Checkout page", &["SHOP-2", "SHOP-3"]).await;
    for key in ["SHOP-2", "SHOP-3", "SHOP-4"] {
        queue(f, key).await;
    }

    // 2 · the chain runs: SHOP-2 and SHOP-3 start in parallel, SHOP-4 waits.
    w.scheduler.pass().await.unwrap();
    assert_eq!(w.recorder.started(), ["SHOP-2", "SHOP-3"]);
    set(f, "SHOP-2", Status::ReadyForReview).await;
    set(f, "SHOP-3", Status::Working).await;
    w.scheduler.pass().await.unwrap();
    assert_eq!(w.recorder.started().len(), 2);

    // 3 · one parent merged: still waiting.
    set(f, "SHOP-2", Status::Merged).await;
    w.scheduler.pass().await.unwrap();
    assert_eq!(w.recorder.started().len(), 2);
    assert_eq!(f.ticket("SHOP-4").await.status, Status::Queued);

    // 4 · all parents merged: SHOP-4 starts.
    set(f, "SHOP-3", Status::Merged).await;
    let pass = w.scheduler.pass().await.unwrap();
    assert_eq!(keys(&pass.start), ["SHOP-4"]);
    assert_eq!(w.recorder.started(), ["SHOP-2", "SHOP-3", "SHOP-4"]);
}

#[tokio::test]
async fn oldest_first_by_the_time_they_entered_queued() {
    let w = world(Duration::from_secs(3600)).await;
    let f = &w.f;
    for title in ["a", "b", "c"] {
        create(f, title, &[]).await;
    }
    // Queued in the order 3, 1, 2 (created 1, 2, 3).
    for key in ["SHOP-3", "SHOP-1", "SHOP-2"] {
        queue(f, key).await;
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    w.scheduler.pass().await.unwrap();
    assert_eq!(w.recorder.started(), ["SHOP-3", "SHOP-1", "SHOP-2"]);
}

#[tokio::test]
async fn two_passes_at_once_start_a_ticket_once() {
    let w = world(Duration::from_secs(3600)).await;
    create(&w.f, "a", &[]).await;
    queue(&w.f, "SHOP-1").await;
    let (a, b) = tokio::join!(w.scheduler.pass(), w.scheduler.pass());
    a.unwrap();
    b.unwrap();
    assert_eq!(w.recorder.started(), ["SHOP-1"]);
}

#[tokio::test]
async fn a_closed_parent_is_recorded_and_the_card_says_so() {
    let w = world(Duration::from_secs(3600)).await;
    let f = &w.f;
    create(f, "parent", &[]).await;
    create(f, "child", &["SHOP-1"]).await;
    queue(f, "SHOP-2").await;
    set(f, "SHOP-1", Status::ClosedNotMerged).await;
    w.scheduler.pass().await.unwrap();
    assert_eq!(f.ticket("SHOP-2").await.status, Status::ParentClosed);
    assert!(w.recorder.started().is_empty());
    let history = f.board.history("SHOP-2").await.unwrap();
    assert!(
        history.iter().any(|e| e.payload["to"] == "parent_closed"),
        "{history:?}"
    );
}

async fn wait_until(what: &str, check: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !check() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what}"));
}

#[tokio::test]
async fn a_state_change_event_triggers_a_pass() {
    let w = world(Duration::from_secs(3600)).await;
    let (stop, stopped) = watch::channel(false);
    let task = tokio::spawn(w.scheduler.clone().run(stopped));
    let f = &w.f;
    create(f, "parent", &[]).await;
    create(f, "child", &["SHOP-1"]).await;
    queue(f, "SHOP-2").await;
    // The first tick passes at once; SHOP-2 waits for SHOP-1.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(w.recorder.started().is_empty());

    // SHOP-1 merged: one event, one pass.
    set(f, "SHOP-1", Status::Merged).await;
    let parent = f.ticket("SHOP-1").await;
    w.events.publish(Event::CardStatusChanged {
        project: "SHOP".into(),
        id: parent.id.clone(),
        key: parent.key.clone(),
        status: Status::Merged,
    });
    let recorder = w.recorder.clone();
    wait_until("SHOP-2 never started", move || {
        recorder.started() == ["SHOP-2"]
    })
    .await;

    stop.send_replace(true);
    task.await.unwrap();
}

#[tokio::test]
async fn the_slow_tick_catches_a_change_without_an_event() {
    let w = world(Duration::from_millis(100)).await;
    let (stop, stopped) = watch::channel(false);
    let task = tokio::spawn(w.scheduler.clone().run(stopped));
    let f = &w.f;
    create(f, "parent", &[]).await;
    create(f, "child", &["SHOP-1"]).await;
    queue(f, "SHOP-2").await;
    set(f, "SHOP-1", Status::Merged).await; // no event published
    let recorder = w.recorder.clone();
    wait_until("the tick never started SHOP-2", move || {
        recorder.started() == ["SHOP-2"]
    })
    .await;
    stop.send_replace(true);
    task.await.unwrap();
}

#[tokio::test]
async fn the_start_sequence_runs_a_ticket_once_when_called_twice_at_once() {
    let f = Fixture::shop().await;
    let data = crewboard_core::data_dir::DataDir::new(f.home.path().join("home"));
    let events = EventBus::new(64);
    let sessions = crewboard_core::session::Sessions::new(f.board.store().clone(), events.clone());
    let starter = Starter::new(
        f.board.clone(),
        data,
        crewboard_core::ports::PortPool::new(f.board.store().clone()),
        sessions,
        None,
        events,
    );
    create(&f, "a", &[]).await;
    queue(&f, "SHOP-1").await;
    let actor = me();
    let (a, b) = tokio::join!(
        starter.start("SHOP-1", &actor),
        starter.start("SHOP-1", &actor)
    );
    let outcomes = [a.unwrap(), b.unwrap()];
    let skipped = outcomes
        .iter()
        .filter(|o| matches!(o, crewboard_core::start::StartOutcome::Skipped))
        .count();
    assert_eq!(skipped, 1, "{outcomes:?}");
    // Without a launch context the one start stops at its launch step.
    assert_eq!(f.ticket("SHOP-1").await.status, Status::StartInterrupted);
    let started: Vec<(String,)> =
        sqlx::query_as("SELECT kind FROM events WHERE kind = 'start_interrupted'")
            .fetch_all(f.board.store().pool())
            .await
            .unwrap();
    assert_eq!(started.len(), 1);
}

#[tokio::test]
async fn a_start_cut_short_by_the_service_is_shown_not_rerun() {
    let f = Fixture::shop().await;
    create(&f, "a", &[]).await;
    queue(&f, "SHOP-1").await;
    // The service stopped while the start was at its setup step.
    let t = f.ticket("SHOP-1").await;
    sqlx::query("UPDATE tickets SET start_problem_step = 'setup' WHERE id = ?")
        .bind(&t.id)
        .execute(f.board.store().pool())
        .await
        .unwrap();
    assert_eq!(
        f.ticket("SHOP-1").await.start_problem,
        None,
        "a running step isn't a problem"
    );
    assert_eq!(Starter::interrupt_cut_short(&f.board).await.unwrap(), 1);
    let t = f.ticket("SHOP-1").await;
    assert_eq!(t.status, Status::StartInterrupted);
    let problem = t.start_problem.unwrap();
    assert_eq!(problem.step.as_str(), "setup");
    assert!(problem.message.contains("stopped"));
    // The scheduler leaves it alone.
    assert_eq!(
        plan(&f.board.tickets("SHOP").await.unwrap(), &HashSet::new()),
        Plan::default()
    );
}
