//! Migrations that change existing rows (SDD §8): a database is written by the migrations up to a version, filled
//! with rows of that time, then opened with [`Store::open`], which runs the rest. Every test uses a temporary
//! folder, never the real data folder.

use std::borrow::Cow;
use std::path::Path;
use std::str::FromStr;

use crewboard_core::store::Store;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

static ALL: Migrator = sqlx::migrate!("./migrations");

/// A database at `path` with the migrations up to `version` applied, as an older Crewboard left it.
async fn database_at(path: &Path, version: i64) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .unwrap();
    let older = Migrator {
        migrations: Cow::Owned(
            ALL.iter()
                .filter(|m| m.version <= version)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    older.run(&pool).await.unwrap();
    pool
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(pool)
        .await
        .unwrap();
}

/// T-0175 (TKT-04, d0067): migration 0022 appends each ticket's "Done when" items to its description under a
/// "## Done when" heading, in position order, and drops the table; a ticket without items keeps its body byte
/// for byte.
#[tokio::test]
async fn done_when_items_move_into_the_description() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("crewboard.db");
    let pool = database_at(&path, 21).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 41, 0)",
    )
    .await;
    for (id, number, kind, body) in [
        ("t18", 18, "ticket", "Show the prices.\n\n"),
        ("t19", 19, "ticket", "  Keep me exactly.\n\n"),
        ("t20", 20, "ticket", ""),
        ("t40", 40, "epic", "Wishlist epic."),
    ] {
        sqlx::query(
            "INSERT INTO tickets (id, project_id, number, kind, title, body, board_column, state, created_by,
                                  created_at, updated_at)
             VALUES (?, 'p', ?, ?, 'T', ?, 'backlog', 'backlog', 'me', 0, 0)",
        )
        .bind(id)
        .bind(number)
        .bind(kind)
        .bind(body)
        .execute(&pool)
        .await
        .unwrap();
    }
    // Stored out of order: the position decides.
    for (ticket, position, text) in [
        ("t18", 1, "works on mobile widths"),
        ("t18", 0, "sorted by price"),
        ("t20", 0, "only item"),
        ("t40", 0, "every child merged"),
    ] {
        sqlx::query(
            "INSERT INTO ticket_done_when (ticket_id, position, text, added_by) VALUES (?, ?, ?, 'me')",
        )
        .bind(ticket)
        .bind(position)
        .bind(text)
        .execute(&pool)
        .await
        .unwrap();
    }
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let body = |id: &'static str| {
        let pool = store.pool().clone();
        async move {
            sqlx::query("SELECT body FROM tickets WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap()
                .get::<String, _>(0)
        }
    };
    assert_eq!(
        body("t18").await,
        "Show the prices.\n\n## Done when\n\n- sorted by price\n- works on mobile widths"
    );
    assert_eq!(
        body("t19").await,
        "  Keep me exactly.\n\n",
        "no items: untouched"
    );
    assert_eq!(body("t20").await, "## Done when\n\n- only item");
    assert_eq!(
        body("t40").await,
        "Wishlist epic.\n\n## Done when\n\n- every child merged"
    );
    let tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ticket_done_when'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(tables, 0, "the table is dropped");
}

/// T-0177 (TKT-09, d0065): migration 0023 gives tickets and epics priority 3; epic children keep none and follow
/// their epic.
#[tokio::test]
async fn existing_tickets_get_normal_and_children_follow_their_epic() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("crewboard.db");
    let pool = database_at(&path, 22).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 41, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO tickets (id, project_id, number, epic_id, child_number, kind, title, board_column, state,
                              created_by, created_at, updated_at)
         VALUES ('t18', 'p', 18, NULL, 0, 'ticket', 'T', 'backlog', 'backlog', 'me', 0, 0),
                ('t40', 'p', 40, NULL, 0, 'epic', 'E', 'backlog', 'backlog', 'me', 0, 0),
                ('t401', 'p', 40, 't40', 1, 'ticket', 'C', 'backlog', 'backlog', 'me', 0, 0)",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let rows: Vec<(String, Option<i64>)> =
        sqlx::query_as("SELECT id, priority FROM tickets ORDER BY id")
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(
        rows,
        [
            ("t18".to_owned(), Some(3)),
            ("t40".to_owned(), Some(3)),
            ("t401".to_owned(), None)
        ]
    );
    let refused = sqlx::query("UPDATE tickets SET priority = 5 WHERE id = 't18'")
        .execute(store.pool())
        .await;
    assert!(refused.is_err(), "1 to 4 only");
}

/// T-0178 (d0066): migration 0024 moves every queued status to the Ready for Dev column and adds ready_since.
#[tokio::test]
async fn queued_tickets_move_to_ready_for_dev() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("crewboard.db");
    let pool = database_at(&path, 23).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 41, 0)",
    )
    .await;
    let states = [
        "queued",
        "waiting_for_slot",
        "held",
        "parent_closed",
        "start_interrupted",
        "working",
        "backlog",
    ];
    for (n, state) in states.iter().enumerate() {
        let column = match *state {
            "backlog" => "backlog",
            _ => "dev",
        };
        sqlx::query(
            "INSERT INTO tickets (id, project_id, number, kind, title, board_column, state, created_by,
                                  created_at, updated_at)
             VALUES (?, 'p', ?, 'ticket', 'T', ?, ?, 'me', 0, 0)",
        )
        .bind(*state)
        .bind(n as i64 + 1)
        .bind(column)
        .bind(*state)
        .execute(&pool)
        .await
        .unwrap();
    }
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let rows: Vec<(String, String, Option<i64>)> =
        sqlx::query_as("SELECT state, board_column, ready_since FROM tickets ORDER BY number")
            .fetch_all(store.pool())
            .await
            .unwrap();
    for (state, column, ready_since) in rows {
        let expected = match state.as_str() {
            "working" => "dev",
            "backlog" => "backlog",
            _ => "ready_for_dev",
        };
        assert_eq!(column, expected, "{state}");
        assert_eq!(ready_since, None, "the scheduler fills it in");
    }
}

/// T-0182 (d0063, d0064): migration 0025 adds the validations table and the ticket columns; a ticket from before
/// M12 has no Review entry.
#[tokio::test]
async fn validation_columns_start_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("crewboard.db");
    let pool = database_at(&path, 24).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 41, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO tickets (id, project_id, number, kind, title, board_column, state, created_by, created_at,
                              updated_at)
         VALUES ('t18', 'p', 18, 'ticket', 'T', 'ready_for_review', 'ready_for_review', 'me', 0, 0)",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let row: (Option<String>, i64, i64, Option<String>, bool) = sqlx::query_as(
        "SELECT review_entry, validation_round, auto_merge_conflicts, auto_merge_stop, merged_automatically
         FROM tickets WHERE id = 't18'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(row, (None, 0, 0, None, false));
    exec(
        store.pool(),
        "INSERT INTO validations (id, ticket_id, round, trigger, started_at) VALUES ('v1', 't18', 1, 'agent', 0)",
    )
    .await;
}

/// T-0183 (d0063): migration 0026 rebuilds `sessions` so a validator (and a Project agent) can be stored; every
/// session and the rows that point at it (usage, deliveries) survive.
#[tokio::test]
async fn sessions_take_the_new_kinds_and_keep_their_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("crewboard.db");
    let pool = database_at(&path, 25).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 41, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO tickets (id, project_id, number, kind, title, board_column, state, created_by, created_at,
                              updated_at)
         VALUES ('t18', 'p', 18, 'ticket', 'T', 'dev', 'working', 'me', 0, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO sessions (id, project_id, ticket_id, kind, claude_session_id, state, started_at, summary_dismissed_at)
         VALUES ('s1', 'p', 't18', 'agent', 'c1', 'running', 5, 7)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO pending_deliveries (id, session_id, kind, text, created_at) VALUES ('d1', 's1', 'agent_message', 'hi', 0)",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let row: (String, String, Option<String>, Option<i64>) = sqlx::query_as(
        "SELECT kind, state, claude_session_id, summary_dismissed_at FROM sessions WHERE id = 's1'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        row,
        ("agent".into(), "running".into(), Some("c1".into()), Some(7))
    );
    let deliveries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pending_deliveries")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(deliveries, 1, "nothing cascaded");
    for kind in ["validator", "project_agent"] {
        sqlx::query(
            "INSERT INTO sessions (id, project_id, ticket_id, kind, state, started_at) VALUES (?, 'p', 't18', ?, 'running', 0)",
        )
        .bind(format!("s-{kind}"))
        .bind(kind)
        .execute(store.pool())
        .await
        .unwrap();
    }
    assert!(
        sqlx::query(
            "INSERT INTO sessions (id, project_id, kind, state, started_at) VALUES ('x', 'p', 'nope', 'running', 0)",
        )
        .execute(store.pool())
        .await
        .is_err()
    );
    let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(fk, 1, "foreign keys are on again");
}

/// T-0199 (d0068): migration 0028 adds `project_agents` and `sessions.project_agent_id`; existing sessions keep
/// their rows with no Project agent, and deleting a Project agent keeps its sessions.
#[tokio::test]
async fn project_agents_start_empty_and_their_sessions_outlive_them() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("crewboard.db");
    let pool = database_at(&path, 27).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 41, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO sessions (id, project_id, kind, state, started_at) VALUES ('s1', 'p', 'project', 'ended', 5)",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let agent: Option<String> =
        sqlx::query_scalar("SELECT project_agent_id FROM sessions WHERE id = 's1'")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(agent, None);
    exec(
        store.pool(),
        "INSERT INTO project_agents (id, project_id, claude_session_id, state, created_at)
         VALUES ('a1', 'p', 'c1', 'running', 0)",
    )
    .await;
    exec(
        store.pool(),
        "INSERT INTO sessions (id, project_id, kind, state, started_at, project_agent_id)
         VALUES ('s2', 'p', 'project_agent', 'running', 6, 'a1')",
    )
    .await;
    let row = sqlx::query(
        "SELECT name, name_source, first_request, limit_stopped, closed_at FROM project_agents WHERE id = 'a1'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<Option<String>, _>(0), None);
    assert_eq!(row.get::<Option<String>, _>(1), None);
    assert_eq!(row.get::<Option<String>, _>(2), None);
    assert_eq!(row.get::<i64, _>(3), 0);
    assert_eq!(row.get::<Option<i64>, _>(4), None);
    let bad = sqlx::query(
        "INSERT INTO project_agents (id, project_id, claude_session_id, state, created_at)
         VALUES ('a2', 'p', 'c2', 'asleep', 0)",
    )
    .execute(store.pool())
    .await;
    assert!(bad.is_err(), "states are checked");
    exec(store.pool(), "DELETE FROM project_agents WHERE id = 'a1'").await;
    let agent: Option<String> =
        sqlx::query_scalar("SELECT project_agent_id FROM sessions WHERE id = 's2'")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(agent, None, "the session stays");
}
