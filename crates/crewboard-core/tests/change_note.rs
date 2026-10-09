//! The ticket-change note (T-0074, SES-08): a user's save on a working ticket queues a note for its agent, which
//! is typed once the agent is idle; agent edits and tickets that aren't working queue nothing. An ordinary `sh`
//! stands in for `claude`.

use std::time::Duration;

use crewboard_core::board::tickets::{DoneWhenEdit, TicketUpdate};
use crewboard_core::caller::Caller;
use crewboard_core::client::{Client, ClientConfig};
use crewboard_core::domain::Status;
use crewboard_core::protocol::{DepParams, Request, TicketUpdateParams};
use crewboard_core::service::testing::TestService;
use crewboard_core::signals::HookEvent;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn queued_for_user_edits_on_working_tickets_and_delivered_when_idle() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Price table").await;
    let shop2 = w.create("SHOP", "Checkout page").await;
    w.create("SHOP", "Checkout API").await;
    w.force_status(&shop1, Status::Working).await;
    w.force_status(&shop2, Status::Queued).await;
    let agent = w.agent_shell(&shop1).await;
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let add_item = |key: &str, text: &str| {
        Request::TicketUpdate(TicketUpdateParams {
            key: key.into(),
            update: TicketUpdate {
                done_when: vec![DoneWhenEdit::Add { text: text.into() }],
                ..Default::default()
            },
        })
    };

    // The user's save: one note with only what changed.
    client
        .request(add_item("SHOP-1", "works on mobile widths"))
        .await
        .unwrap();
    let pending = w.deliveries.pending(&agent.session).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].kind, "ticket_change");
    assert!(
        pending[0].text.starts_with(
            "The ticket was updated by the user. Added to 'Done when': 'works on mobile widths'."
        ),
        "{}",
        pending[0].text
    );
    // A second save before delivery: queued in order.
    client
        .request(Request::DepAdd(DepParams {
            child: "SHOP-1".into(),
            parent: "SHOP-3".into(),
        }))
        .await
        .unwrap();
    let pending = w.deliveries.pending(&agent.session).await.unwrap();
    assert_eq!(pending.len(), 2);
    assert!(pending[1].text.contains("It now depends on SHOP-3."));
    assert_eq!(
        w.board
            .ticket("SHOP-1")
            .await
            .unwrap()
            .flags
            .messages_waiting,
        2
    );

    // A ticket that isn't working gets nothing.
    client
        .request(add_item("SHOP-2", "pay button"))
        .await
        .unwrap();
    assert_eq!(w.deliveries.pending(&agent.session).await.unwrap().len(), 2);

    // The agent's own edit sends nothing.
    let ran = agent
        .run(&format!(
            "printf '%s\\n' '{}' | nc -U -w 1 '{}'",
            json!({"protocol_version": 1, "id": 1, "request": {"method": "ticket.update",
                   "params": {"key": "SHOP-1", "title": "Price table v2"}}}),
            w.data.socket_path().display()
        ))
        .await;
    assert!(ran.stdout.contains("\"reply\""), "{ran:?}");
    assert_eq!(w.deliveries.pending(&agent.session).await.unwrap().len(), 2);

    // Idle: both notes are typed.
    let me = Caller::Agent {
        ticket: shop1.key.clone(),
        ticket_id: shop1.id.clone(),
        session: agent.session.clone(),
    };
    w.signals
        .hook(
            &me,
            HookEvent::Stop,
            &json!({"background_tasks": [], "session_crons": []}),
        )
        .await
        .unwrap();
    w.deliveries.deliver_if_ready(&agent.session).await.unwrap();
    assert!(
        w.deliveries
            .pending(&agent.session)
            .await
            .unwrap()
            .is_empty()
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let out =
                String::from_utf8_lossy(&w.sessions.attach(&agent.session).unwrap().scrollback)
                    .into_owned();
            if out.contains("It now depends on SHOP-3.") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the notes were typed");
    drop(client);
    w.stop().await;
}
