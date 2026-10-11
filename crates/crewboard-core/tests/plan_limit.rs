//! The plan-limit hold and Resume all (T-0129, SCH-04, USG-06, ATT-09; d0020, d0046, d0048): the scheduler starts
//! nothing while the limit is active, limit-stopped agents raise no inbox items, and Resume all queues one
//! "continue" per waiting agent as an automatic delivery (an ordinary `sh` stands in for `claude`).

use std::collections::HashSet;
use std::time::Duration;

use crewboard_core::board::tickets::{Base, StartProblem, Ticket};
use crewboard_core::caller::Caller;
use crewboard_core::domain::{Actor, Flags, Kind, StartStep, Status, TicketKey};
use crewboard_core::protocol::{Reply, Request};
use crewboard_core::scheduler::{Gate, PLAN_LIMIT_HELD, held_by_plan_limit, plan};
use crewboard_core::service::testing::TestService;
use crewboard_core::signals::{HookEvent, Signal};
use crewboard_core::store::now_ms;
use crewboard_core::usage::limit::{CONTINUE, LimitHit, LimitKind};
use serde_json::json;

fn t(key: &str, status: Status) -> Ticket {
    Ticket {
        id: key.into(),
        key: key.parse::<TicketKey>().unwrap(),
        kind: Kind::Ticket,
        title: key.into(),
        body: String::new(),
        priority: Default::default(),
        priority_inherited: false,
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
        queued_at: Some(0),
        ready_since: None,
        work: None,
        start_problem: None,
        interrupted_from: None,
        signal: None,
        question: None,
        pushed_differs: None,
    }
}

#[test]
fn with_the_limit_active_nothing_starts_and_ready_tickets_are_held() {
    let mut waiting = t("SHOP-3", Status::Queued);
    waiting.depends_on = vec!["SHOP-9".parse().unwrap()];
    let tickets = [
        t("SHOP-1", Status::Queued),
        t("SHOP-2", Status::WaitingForSlot),
        waiting,
        t("SHOP-9", Status::Working),
    ];
    let gate = Gate {
        plan_limit: true,
        ..Gate::default()
    };
    let pass = plan(&tickets, &HashSet::new(), &gate);
    assert!(pass.start.is_empty(), "nothing starts");
    let held: Vec<(String, Status)> = pass
        .changes
        .iter()
        .map(|c| (c.key.to_string(), c.to))
        .collect();
    // The ticket still waiting for its parent keeps saying so.
    assert_eq!(
        held,
        [
            ("SHOP-1".into(), Status::Held),
            ("SHOP-2".into(), Status::Held)
        ]
    );

    // Once the limit is no longer active, the scheduler lifts its own hold: the ticket starts from Queued.
    let mut held = t("SHOP-1", Status::Held);
    held.start_problem = Some(StartProblem {
        step: StartStep::PlanLimit,
        message: PLAN_LIMIT_HELD.into(),
    });
    assert!(held_by_plan_limit(&held));
    let pass = plan(&[held.clone()], &HashSet::new(), &Gate::default());
    assert_eq!(pass.start.len(), 1);
    assert_eq!(pass.changes[0].to, Status::Queued);
    // A base that diverged stays held until the user's Retry start.
    held.start_problem = Some(StartProblem {
        step: StartStep::SyncBase,
        message: "diverged".into(),
    });
    assert!(!held_by_plan_limit(&held));
    assert!(
        plan(&[held], &HashSet::new(), &Gate::default())
            .start
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn limit_stopped_agents_raise_no_items_and_resume_all_continues_them() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    let shop2 = w.create("SHOP", "Coupon codes").await;
    let shop3 = w.create("SHOP", "Order email").await;
    for t in [&shop1, &shop2, &shop3] {
        w.force_status(t, Status::Working).await;
    }
    let a1 = w.agent_shell(&shop1).await;
    let a2 = w.agent_shell(&shop2).await;
    let _a3 = w.agent_shell(&shop3).await;

    // SHOP-1 and SHOP-2 stop at the limit (their StopFailure hooks); SHOP-3 is still working.
    let failure = json!({"hook_event_name": "StopFailure", "error": "rate_limit",
        "last_assistant_message": "You've hit your session limit · resets 3:45pm"});
    let hit = LimitHit {
        kind: LimitKind::Session,
        text: "You've hit your session limit · resets 3:45pm".into(),
    };
    for (agent, ticket) in [(&a1, &shop1), (&a2, &shop2)] {
        let caller = Caller::Agent {
            ticket: ticket.key.clone(),
            ticket_id: ticket.id.clone(),
            session: agent.session.clone(),
        };
        let signal = w
            .signals
            .hook(&caller, HookEvent::StopFailure, &failure)
            .await
            .unwrap();
        assert!(matches!(signal, Signal::LimitReached { .. }));
        w.plan_limit.hit(&hit, &ticket.id, now_ms()).await.unwrap();
    }
    assert_eq!(w.status("SHOP-1").await, Status::WaitingForLimitReset);
    // Neither raises an item, and they count as working for the optional agent limit.
    assert!(w.inbox.list().await.unwrap().items.is_empty());
    // An agent the limit stopped stays out of the inbox even if it ends up Idle while the banner is up.
    w.force_status(&shop2, Status::Idle).await;
    assert!(w.inbox.list().await.unwrap().items.is_empty());
    w.force_status(&shop2, Status::WaitingForLimitReset).await;

    // The user has half-typed something into SHOP-1's terminal: its "continue" waits for the empty line.
    w.sessions.input(&a1.session, b"echo draft").unwrap();

    // Nothing is queued until the click.
    assert!(w.deliveries.pending(&a1.session).await.unwrap().is_empty());
    let client =
        crewboard_core::client::Client::connect(crewboard_core::client::ClientConfig::new(&w.data))
            .await
            .unwrap();
    let reply = client.request(Request::UsageResumeAll).await.unwrap();
    let Reply::Resumed(keys) = reply else {
        panic!("{reply:?}")
    };
    let keys: Vec<String> = keys.iter().map(ToString::to_string).collect();
    assert_eq!(keys, ["SHOP-1", "SHOP-2"]);

    // SHOP-2's input line was empty: typed at once. SHOP-1's waits for the user's line.
    wait_for_output(&w, &a2.session, CONTINUE).await;
    assert_eq!(w.deliveries.pending(&a1.session).await.unwrap().len(), 1);
    w.sessions.input(&a1.session, b"\r").unwrap();
    w.deliveries.deliver_if_ready(&a1.session).await.unwrap();
    wait_for_output(&w, &a1.session, CONTINUE).await;

    // The limit and its hiding are gone.
    assert!(!w.plan_limit.state().await.unwrap().unwrap().limit_active());
    w.force_status(&shop2, Status::Idle).await;
    assert_eq!(w.inbox.list().await.unwrap().items.len(), 1);
    w.stop().await;
}

async fn wait_for_output(w: &TestService, session: &str, needle: &str) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let out = String::from_utf8_lossy(&w.sessions.attach(session).unwrap().scrollback)
                .into_owned();
            if out.contains(needle) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{needle:?} never typed"));
}

#[tokio::test(flavor = "multi_thread")]
async fn this_is_the_usage_limit_folds_every_idle_agent_into_the_banner() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    w.add_project("billing-api").await;
    let shop1 = w.create("SHOP", "Price table").await;
    let shop2 = w.create("SHOP", "Split user full name").await;
    let bill1 = w.create("BILL", "Invoice PDF export").await;
    let asks = w.create("SHOP", "Order email").await;
    for t in [&shop1, &shop2, &bill1] {
        w.force_status(t, Status::Idle).await;
    }
    w.force_status(&asks, Status::AsksYou).await;
    assert_eq!(w.inbox.list().await.unwrap().items.len(), 4);

    let client =
        crewboard_core::client::Client::connect(crewboard_core::client::ClientConfig::new(&w.data))
            .await
            .unwrap();
    let reply = client.request(Request::UsageMarkLimit).await.unwrap();
    let Reply::LimitMarked(keys) = reply else {
        panic!("{reply:?}")
    };
    let keys: Vec<String> = keys.iter().map(ToString::to_string).collect();
    assert_eq!(keys, ["BILL-1", "SHOP-1", "SHOP-2"]);
    for key in ["SHOP-1", "SHOP-2", "BILL-1"] {
        assert_eq!(w.status(key).await, Status::WaitingForLimitReset, "{key}");
        let history = w.board.history(key).await.unwrap();
        assert!(
            history.iter().any(|e| e.kind
                == crewboard_core::board::tickets::EventKind::LimitMarked
                && !e.actor.is_agent()),
            "{key}: logged as the user's"
        );
    }
    // Asks you keeps its item; the Idle ones are gone.
    let items = w.inbox.list().await.unwrap().items;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key.to_string(), "SHOP-3");
    assert_eq!(w.status("SHOP-3").await, Status::AsksYou);
    // One limit, reset time unknown (no status-line data).
    let limit = w.plan_limit.state().await.unwrap().unwrap().limit.unwrap();
    assert_eq!(limit.kind, LimitKind::Other);
    assert_eq!(limit.resets_at, None);
    w.stop().await;
}
