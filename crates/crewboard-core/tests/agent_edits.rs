//! Agent edits (TKT-05): agent changes raise "Updated by agent" with a before/after list; user edits don't;
//! marking them seen clears the marker and keeps the history.

mod common;

use common::{Fixture, agent, me, shop_input};
use crewboard_core::board::tickets::{DoneWhenEdit, Field, NewTicketInput, TicketUpdate};

#[tokio::test]
async fn user_edits_raise_no_marker() {
    let f = Fixture::shop().await;
    let t = f.create("Checkout").await;
    let key = t.key.to_string();
    f.board
        .update_ticket(
            &key,
            &TicketUpdate {
                body: Some("why".into()),
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    let edits = f.board.agent_edits(&key).await.unwrap();
    assert!(!edits.updated_by_agent);
    assert!(edits.changes.is_empty());
    assert!(!f.ticket(&key).await.flags.updated_by_agent);
}

#[tokio::test]
async fn agent_edits_show_until_seen() {
    let f = Fixture::shop().await;
    let t = f
        .create_with(NewTicketInput {
            title: "Checkout".into(),
            body: "old body".into(),
            done_when: vec!["Pays".into()],
            ..shop_input()
        })
        .await;
    let key = t.key.to_string();
    let bot = agent("SHOP-19");
    f.board
        .update_ticket(
            &key,
            &TicketUpdate {
                body: Some("new body".into()),
                done_when: vec![
                    DoneWhenEdit::Add {
                        text: "Receipt email".into(),
                    },
                    DoneWhenEdit::Add {
                        text: "Refund path".into(),
                    },
                ],
                ..Default::default()
            },
            &bot,
        )
        .await
        .unwrap();

    let edits = f.board.agent_edits(&key).await.unwrap();
    assert!(edits.updated_by_agent);
    assert_eq!(edits.last_agent, Some(bot.clone()));
    assert!(edits.last_at.is_some());
    assert_eq!(edits.changes.len(), 3);
    let body = edits
        .changes
        .iter()
        .find(|c| c.change.field == Field::Body)
        .unwrap();
    assert_eq!(body.change.before.as_deref(), Some("old body"));
    assert_eq!(body.change.after.as_deref(), Some("new body"));
    assert_eq!(
        edits.changes[0].change.after.as_deref(),
        Some("Refund path"),
        "newest first"
    );

    let ticket = f.ticket(&key).await;
    assert!(ticket.flags.updated_by_agent);
    let by_agent: Vec<_> = ticket
        .done_when
        .iter()
        .filter(|i| i.added_by == bot)
        .map(|i| i.text.as_str())
        .collect();
    assert_eq!(
        by_agent,
        ["Receipt email", "Refund path"],
        "agent-added items are identifiable"
    );

    f.board.mark_agent_edits_seen(&key, &me()).await.unwrap();
    let edits = f.board.agent_edits(&key).await.unwrap();
    assert!(!edits.updated_by_agent);
    assert!(edits.changes.is_empty());
    assert!(!f.ticket(&key).await.flags.updated_by_agent);
    let history: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE ticket_id = ? AND kind = 'updated'")
            .bind(&t.id)
            .fetch_one(f.board.store().pool())
            .await
            .unwrap();
    assert_eq!(history, 1, "the history stays");

    // A later agent edit raises the marker again, with only the new change.
    f.board
        .update_ticket(
            &key,
            &TicketUpdate {
                title: Some("Checkout v2".into()),
                ..Default::default()
            },
            &bot,
        )
        .await
        .unwrap();
    let edits = f.board.agent_edits(&key).await.unwrap();
    assert_eq!(edits.changes.len(), 1);
    assert_eq!(edits.changes[0].change.field, Field::Title);
}
