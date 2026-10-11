//! Deleting never-started tickets (TKT-08; d0060): refusals on every state that has been to Dev, links named
//! and removed in both directions, epics with their never-started children in one transaction, numbers never
//! reused, aliases gone.

mod common;

use common::{Fixture, me};
use crewboard_core::board::Refusal;
use crewboard_core::board::moves::DropTarget;
use crewboard_core::domain::Status;
use crewboard_core::store::new_id;

async fn count(f: &Fixture, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
        .fetch_one(f.board.store().pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn refused_once_a_ticket_has_been_to_dev() {
    let f = Fixture::shop().await;
    f.skip_to(19).await;
    let t = f.create("Price table").await;
    f.board
        .move_ticket("SHOP-19", DropTarget::ReadyForDev, true, &me())
        .await
        .unwrap();
    let err = f.board.delete_preview("SHOP-19").await.unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::Started));
    assert_eq!(
        err.to_string(),
        "SHOP-19 has been to Dev. Close it instead."
    );
    assert!(f.board.delete_ticket("SHOP-19").await.is_err());

    // Closed after a start, Parked, or with a branch, worktree or session row: all count.
    for (sql, title) in [
        (
            "UPDATE tickets SET state = 'parked' , started_at = 1 WHERE id = ?",
            "Parked",
        ),
        (
            "UPDATE tickets SET branch_name = 'x' WHERE id = ?",
            "Branch",
        ),
        (
            "UPDATE tickets SET worktree_path = '/tmp/x' WHERE id = ?",
            "Worktree",
        ),
    ] {
        let other = f.create(title).await;
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(&other.id)
            .execute(f.board.store().pool())
            .await
            .unwrap();
        let err = f
            .board
            .delete_preview(&other.key.to_string())
            .await
            .unwrap_err();
        assert_eq!(err.refusal(), Some(Refusal::Started), "{title}");
    }
    let with_session = f.create("Session").await;
    let project_id: String = sqlx::query_scalar("SELECT id FROM projects")
        .fetch_one(f.board.store().pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions (id, project_id, ticket_id, kind, state, started_at) VALUES (?, ?, ?, 'agent', 'ended', 1)")
        .bind(new_id())
        .bind(project_id)
        .bind(&with_session.id)
        .execute(f.board.store().pool())
        .await
        .unwrap();
    assert!(
        f.board
            .delete_preview(&with_session.key.to_string())
            .await
            .is_err()
    );
    let _ = t;
}

#[tokio::test]
async fn delete_names_and_removes_links_and_never_reuses_the_number() {
    let f = Fixture::shop().await;
    f.skip_to(20).await;
    for title in ["Checkout page", "Order confirmation email", "Fix login"] {
        f.create(title).await;
    }
    f.board
        .add_dependency("SHOP-21", "SHOP-20", &me())
        .await
        .unwrap();
    f.board
        .add_dependency("SHOP-22", "SHOP-21", &me())
        .await
        .unwrap();
    let preview = f.board.delete_preview("SHOP-21").await.unwrap();
    assert_eq!(
        preview.question(),
        "Delete SHOP-21 Order confirmation email?"
    );
    assert_eq!(
        preview.link_lines(),
        [
            "SHOP-20 is a dependency of SHOP-21; that link will be removed",
            "SHOP-21 is a dependency of SHOP-22; that link will be removed",
        ]
    );
    let review_ticket: String = sqlx::query_scalar("SELECT id FROM tickets WHERE number = 21")
        .fetch_one(f.board.store().pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO review_comments (id, ticket_id, round, file, line, side, body, created_at) VALUES (?, ?, 1, 'a', 1, 'new', 'b', 0)")
        .bind(new_id())
        .bind(&review_ticket)
        .execute(f.board.store().pool())
        .await
        .unwrap();
    f.board.delete_ticket("SHOP-21").await.unwrap();
    assert!(f.board.ticket("SHOP-21").await.is_err(), "unknown ticket");
    assert!(
        f.ticket("SHOP-22").await.depends_on.is_empty(),
        "the link is gone in both directions"
    );
    assert_eq!(count(&f, "ticket_deps").await, 0);
    assert_eq!(count(&f, "review_comments").await, 0);
    assert_eq!(
        f.create("Next").await.key.to_string(),
        "SHOP-23",
        "21 is never handed out again"
    );
}

#[tokio::test]
async fn aliases_stop_resolving() {
    let f = Fixture::shop().await;
    let epic = f.epic("Wishlist").await;
    let t = f.create("Share link").await;
    let moved = f
        .board
        .move_to_epic(&t.key.to_string(), &epic.key.to_string(), &me())
        .await
        .unwrap();
    assert!(f.board.ticket(&t.key.to_string()).await.is_ok());
    f.board.delete_ticket(&moved.key.to_string()).await.unwrap();
    assert!(
        f.board.ticket(&t.key.to_string()).await.is_err(),
        "the old key is gone too"
    );
    assert_eq!(count(&f, "ticket_key_aliases").await, 0);
}

#[tokio::test]
async fn an_epic_goes_with_its_never_started_children() {
    let f = Fixture::shop().await;
    f.skip_to(40).await;
    let epic = f.epic("Wishlist").await;
    f.child(&epic, "Wishlist API").await;
    f.child(&epic, "Wishlist button").await;
    let outside = f.create("Analytics").await;
    f.board
        .add_dependency(&outside.key.to_string(), "SHOP-40", &me())
        .await
        .unwrap();
    f.board
        .add_dependency("SHOP-40.2", "SHOP-40.1", &me())
        .await
        .unwrap();
    let preview = f.board.delete_preview("SHOP-40").await.unwrap();
    assert_eq!(
        preview.question(),
        "Delete SHOP-40 and its 2 tickets (SHOP-40.1, SHOP-40.2)? None of them started."
    );
    assert_eq!(
        preview.link_lines(),
        ["SHOP-40 is a dependency of SHOP-41; that link will be removed"],
        "links inside what goes are not listed"
    );
    f.board.delete_ticket("SHOP-40").await.unwrap();
    for key in ["SHOP-40", "SHOP-40.1", "SHOP-40.2"] {
        assert!(f.board.ticket(key).await.is_err(), "{key}");
    }
    assert_eq!(f.ticket("SHOP-41").await.status, Status::Backlog);
}

#[tokio::test]
async fn an_epic_with_a_started_child_is_refused() {
    let f = Fixture::shop().await;
    f.skip_to(40).await;
    let epic = f.epic("Wishlist").await;
    let child = f.child(&epic, "Wishlist API").await;
    f.board
        .move_ticket(&child.key.to_string(), DropTarget::ReadyForDev, true, &me())
        .await
        .unwrap();
    let err = f.board.delete_preview("SHOP-40").await.unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::Started));
    assert_eq!(
        err.to_string(),
        "SHOP-40 has started tickets. Close it instead."
    );
}
