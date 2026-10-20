//! Migrations that change existing rows (SDD §8): a database is written by the migrations up to a version, filled
//! with rows of that time, then opened with [`Store::open`], which runs the rest. Every test uses a temporary
//! folder, never the real data folder.

use std::borrow::Cow;
use std::path::Path;
use std::str::FromStr;

use otaq_core::store::Store;
use serde_json::json;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

static ALL: Migrator = sqlx::migrate!("./migrations");

/// A database at `path` with the migrations up to `version` applied, as an older Otaq left it.
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
    let path = tmp.path().join("otaq.db");
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
    let path = tmp.path().join("otaq.db");
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
    let path = tmp.path().join("otaq.db");
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

    // Read at 0024: 0046 drops the column (T-0400).
    let pool = database_at(&path, 24).await;
    let rows: Vec<(String, String, Option<i64>)> =
        sqlx::query_as("SELECT state, board_column, ready_since FROM tickets ORDER BY number")
            .fetch_all(&pool)
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
    let path = tmp.path().join("otaq.db");
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
    let path = tmp.path().join("otaq.db");
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
        "SELECT kind, state, tool_session_id, summary_dismissed_at FROM sessions WHERE id = 's1'",
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
    let path = tmp.path().join("otaq.db");
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
        "INSERT INTO project_agents (id, project_id, tool, tool_session_id, state, created_at)
         VALUES ('a1', 'p', 'claude', 'c1', 'running', 0)",
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
        "INSERT INTO project_agents (id, project_id, tool, tool_session_id, state, created_at)
         VALUES ('a2', 'p', 'claude', 'c2', 'asleep', 0)",
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

/// T-0200 (d0068): migration 0029 lets a Project agent ask: messages and human questions keep every row, and a
/// row names exactly one asker (a ticket or a Project agent).
#[tokio::test]
async fn questions_take_a_project_agent_as_the_asker() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 28).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 41, 0)",
    )
    .await;
    for (id, n) in [("t18", 18), ("t20", 20)] {
        exec(
            &pool,
            &format!(
                "INSERT INTO tickets (id, project_id, number, kind, title, board_column, state, created_by,
                                      created_at, updated_at)
                 VALUES ('{id}', 'p', {n}, 'ticket', 'T', 'dev', 'working', 'me', 0, 0)"
            ),
        )
        .await;
    }
    exec(
        &pool,
        "INSERT INTO messages (id, from_ticket_id, to_ticket_id, question, answer, delivery, asked_at, answered_at,
                               answer_delivered_at)
         VALUES ('m1', 't20', 't18', 'q?', 'a.', 'live', 1, 2, 3)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO human_questions (id, ticket_id, question, options_json, recommended, asked_at)
         VALUES ('h1', 't18', 'Postmark or SES?', '[\"Postmark\",\"SES\"]', 1, 4)",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let m: (Option<String>, Option<String>, String, Option<i64>) = sqlx::query_as(
        "SELECT from_ticket_id, from_agent_id, answer, answer_delivered_at FROM messages WHERE id = 'm1'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(m, (Some("t20".into()), None, "a.".into(), Some(3)));
    let h: (Option<String>, String, Option<i64>) = sqlx::query_as(
        "SELECT ticket_id, options_json, recommended FROM human_questions WHERE id = 'h1'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        h,
        (Some("t18".into()), "[\"Postmark\",\"SES\"]".into(), Some(1))
    );

    exec(
        store.pool(),
        "INSERT INTO project_agents (id, project_id, tool, tool_session_id, state, created_at)
         VALUES ('a1', 'p', 'claude', 'c1', 'running', 0)",
    )
    .await;
    exec(
        store.pool(),
        "INSERT INTO messages (id, from_agent_id, to_ticket_id, question, asked_at) VALUES ('m2', 'a1', 't18', 'q', 5)",
    )
    .await;
    exec(
        store.pool(),
        "INSERT INTO human_questions (id, project_agent_id, question, asked_at) VALUES ('h2', 'a1', 'q', 6)",
    )
    .await;
    for bad in [
        "INSERT INTO messages (id, to_ticket_id, question, asked_at) VALUES ('m3', 't18', 'q', 5)",
        "INSERT INTO messages (id, from_ticket_id, from_agent_id, to_ticket_id, question, asked_at)
         VALUES ('m4', 't20', 'a1', 't18', 'q', 5)",
        "INSERT INTO human_questions (id, question, asked_at) VALUES ('h3', 'q', 6)",
    ] {
        assert!(
            sqlx::query(sqlx::AssertSqlSafe(bad))
                .execute(store.pool())
                .await
                .is_err(),
            "{bad}"
        );
    }
    // Deleting the Project agent takes its questions with it.
    exec(store.pool(), "DELETE FROM project_agents WHERE id = 'a1'").await;
    let left: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM messages) + (SELECT COUNT(*) FROM human_questions)",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(left, 2);
}

/// T-0262 (d0080): migration 0030 adds the epic's queue mode and the child's "joined the queue" mark to an M15
/// database: every ticket keeps its row and fields, epics start as `none`, nobody joined a queue yet.
#[tokio::test]
async fn epics_start_unqueued_and_keep_their_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 29).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 52, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO tickets (id, project_id, number, epic_id, child_number, kind, title, board_column, state,
                              pinned_at, started_at, created_by, created_at, updated_at)
         VALUES ('t45', 'p', 45, NULL, 0, 'epic', 'Search', 'backlog', 'backlog', 7, NULL, 'me', 0, 0),
                ('t451', 'p', 45, 't45', 1, 'ticket', 'Search index API', 'dev', 'working', NULL, 3, 'me', 1, 1),
                ('t452', 'p', 45, 't45', 2, 'ticket', 'Search results page', 'backlog', 'backlog', NULL, NULL,
                 'me', 2, 2),
                ('t51', 'p', 51, NULL, 0, 'ticket', 'Fix checkout rounding', 'ready_for_dev', 'queued', NULL, 4,
                 'me', 3, 3)",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let expected = [
        ("t45", "Search", "backlog", Some(7), None),
        ("t451", "Search index API", "working", None, Some(3)),
        ("t452", "Search results page", "backlog", None, None),
        ("t51", "Fix checkout rounding", "queued", None, Some(4)),
    ];
    for (id, title, state, pinned_at, started_at) in expected {
        let row = store.ticket(id).await.unwrap().expect("the row is kept");
        assert_eq!(
            (
                row.title.as_str(),
                row.state.as_str(),
                row.pinned_at,
                row.started_at
            ),
            (title, state, pinned_at, started_at),
            "{id}"
        );
        assert_eq!(
            (
                row.epic_queue_mode.as_str(),
                row.epic_queue_mode_at,
                row.joined_queue_by
            ),
            ("none", None, None),
            "{id}"
        );
    }
    let refused = sqlx::query("UPDATE tickets SET epic_queue_mode = 'half' WHERE id = 't45'")
        .execute(store.pool())
        .await;
    assert!(refused.is_err(), "none, whole or single only");
    exec(
        store.pool(),
        "UPDATE tickets SET epic_queue_mode = 'whole', epic_queue_mode_at = 9 WHERE id = 't45'",
    )
    .await;
    store
        .set_joined_queue_by("t452", Some("agent:SHOP-45.1"))
        .await
        .unwrap();
    let epic = store.ticket("t45").await.unwrap().unwrap();
    assert_eq!(
        (epic.epic_queue_mode.as_str(), epic.epic_queue_mode_at),
        ("whole", Some(9))
    );
    let child = store.ticket("t452").await.unwrap().unwrap();
    assert_eq!(child.joined_queue_by.as_deref(), Some("agent:SHOP-45.1"));
}

/// T-0278 (LEAD-01; d0083): migration 0031 gives Project agents an epic on an M16 database: existing agents keep
/// their rows with no epic, an epic has at most one lead, and deleting the epic deletes its lead's row.
#[tokio::test]
async fn project_agents_take_an_epic_and_an_epic_has_one_lead() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 30).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 46, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO tickets (id, project_id, number, epic_id, child_number, kind, title, board_column, state,
                              created_by, created_at, updated_at)
         VALUES ('t40', 'p', 40, NULL, 0, 'epic', 'Wishlist', 'backlog', 'backlog', 'me', 0, 0),
                ('t45', 'p', 45, NULL, 0, 'epic', 'Search', 'backlog', 'backlog', 'me', 0, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO project_agents (id, project_id, name, name_source, claude_session_id, state, created_at)
         VALUES ('a1', 'p', 'Checkout slowness', 'agent', 'c1', 'ended', 0)",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let row = sqlx::query("SELECT name, epic_id FROM project_agents WHERE id = 'a1'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>(0), "Checkout slowness");
    assert_eq!(row.get::<Option<String>, _>(1), None);
    exec(
        store.pool(),
        "INSERT INTO project_agents (id, project_id, tool, tool_session_id, state, created_at, epic_id)
         VALUES ('l40', 'p', 'claude', 'c2', 'running', 0, 't40'), ('l45', 'p', 'claude', 'c3', 'running', 0, 't45'),
                ('a2', 'p', 'claude', 'c4', 'running', 0, NULL)",
    )
    .await;
    let second = sqlx::query(
        "INSERT INTO project_agents (id, project_id, tool, tool_session_id, state, created_at, epic_id)
         VALUES ('l40b', 'p', 'claude', 'c5', 'running', 0, 't40')",
    )
    .execute(store.pool())
    .await;
    assert!(second.is_err(), "one lead per epic");
    exec(store.pool(), "DELETE FROM tickets WHERE id = 't40'").await;
    let left: Vec<String> = sqlx::query_scalar("SELECT id FROM project_agents ORDER BY id")
        .fetch_all(store.pool())
        .await
        .unwrap();
    assert_eq!(left, ["a1", "a2", "l45"]);
}

/// T-0276 (DEC-02, d0084): migration 0032 adds the empty `decisions` table to an M17 database (after 0031): every ticket keeps its
/// row, a decision starts open with no mark, and the outcome takes only its four values.
#[tokio::test]
async fn decisions_start_empty_and_tickets_keep_their_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 31).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 52, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO tickets (id, project_id, number, epic_id, child_number, kind, title, board_column, state,
                              started_at, created_by, created_at, updated_at, epic_queue_mode)
         VALUES ('t40', 'p', 40, NULL, 0, 'epic', 'Wishlist', 'backlog', 'backlog', NULL, 'me', 0, 0, 'whole'),
                ('t401', 'p', 40, 't40', 1, 'ticket', 'Wishlist API', 'dev', 'working', 3, 'me', 1, 1, 'none'),
                ('t23', 'p', 23, NULL, 0, 'ticket', 'Split user full name', 'ready_for_review',
                 'ready_for_review', 4, 'me', 2, 2, 'none')",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    for (id, title, state) in [
        ("t40", "Wishlist", "backlog"),
        ("t401", "Wishlist API", "working"),
        ("t23", "Split user full name", "ready_for_review"),
    ] {
        let row = store.ticket(id).await.unwrap().expect("the row is kept");
        assert_eq!(
            (row.title.as_str(), row.state.as_str()),
            (title, state),
            "{id}"
        );
    }
    assert_eq!(
        store
            .ticket("t40")
            .await
            .unwrap()
            .unwrap()
            .epic_queue_mode
            .as_str(),
        "whole"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM decisions")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);

    exec(
        store.pool(),
        "INSERT INTO decisions (id, ticket_id, ordinal, question, built, other_options, why, recorded_by, recorded_at)
         VALUES ('d1', 't23', 1, 'q', 'b', '[\"o\"]', 'w', 'agent:SHOP-23', 5)",
    )
    .await;
    let row = sqlx::query(
        "SELECT outcome, mark_text, fix_child_id, outcome_at FROM decisions WHERE id = 'd1'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("outcome"), "open");
    assert_eq!(row.get::<Option<String>, _>("mark_text"), None);
    assert_eq!(row.get::<Option<String>, _>("fix_child_id"), None);
    assert_eq!(row.get::<Option<i64>, _>("outcome_at"), None);
    let refused = sqlx::query("UPDATE decisions SET outcome = 'maybe' WHERE id = 'd1'")
        .execute(store.pool())
        .await;
    assert!(
        refused.is_err(),
        "open, kept, changed_sent_back or changed_fix_child only"
    );
    let duplicate = sqlx::query(
        "INSERT INTO decisions (id, ticket_id, ordinal, question, built, why, recorded_by, recorded_at)
         VALUES ('d2', 't23', 1, 'q', 'b', 'w', 'me', 6)",
    )
    .execute(store.pool())
    .await;
    assert!(duplicate.is_err(), "one number per ticket");
}

/// T-0280 (LEAD-08; d0083): migration 0034 adds the empty `lead_suggestions` table to a database after 0033: every
/// ticket keeps its row, a suggestion starts open, kind and state take only their values, one open suggestion per
/// child and kind, and deleting the child deletes its suggestions.
#[tokio::test]
async fn lead_suggestions_start_empty_one_open_per_child_and_kind() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 33).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 52, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO tickets (id, project_id, number, epic_id, child_number, kind, title, board_column, state,
                              started_at, created_by, created_at, updated_at, epic_queue_mode)
         VALUES ('t40', 'p', 40, NULL, 0, 'epic', 'Wishlist', 'backlog', 'backlog', NULL, 'me', 0, 0, 'none'),
                ('t406', 'p', 40, 't40', 6, 'ticket', 'Wishlist export (CSV)', 'backlog', 'backlog', NULL, 'me',
                 1, 1, 'none')",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let row = store
        .ticket("t406")
        .await
        .unwrap()
        .expect("the row is kept");
    assert_eq!(row.title, "Wishlist export (CSV)");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lead_suggestions")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);

    exec(
        store.pool(),
        "INSERT INTO lead_suggestions (id, epic_id, child_id, kind, reason, suggested_by, created_at)
         VALUES ('s1', 't40', 't406', 'close', 'PAY-123 doesn''t ask for an export.', 'project-agent:l1', 5)",
    )
    .await;
    let state: String = sqlx::query_scalar("SELECT state FROM lead_suggestions WHERE id = 's1'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(state, "open");
    for bad in [
        "UPDATE lead_suggestions SET state = 'maybe' WHERE id = 's1'",
        "UPDATE lead_suggestions SET kind = 'park' WHERE id = 's1'",
        "INSERT INTO lead_suggestions (id, epic_id, child_id, kind, reason, suggested_by, created_at)
         VALUES ('s2', 't40', 't406', 'close', 'again', 'project-agent:l1', 6)",
    ] {
        assert!(
            sqlx::query(sqlx::AssertSqlSafe(bad))
                .execute(store.pool())
                .await
                .is_err(),
            "{bad}"
        );
    }
    // A dismissed one doesn't count: the lead may suggest again later; deleting also is another kind.
    exec(
        store.pool(),
        "UPDATE lead_suggestions SET state = 'dismissed' WHERE id = 's1'",
    )
    .await;
    exec(
        store.pool(),
        "INSERT INTO lead_suggestions (id, epic_id, child_id, kind, reason, suggested_by, created_at)
         VALUES ('s3', 't40', 't406', 'close', 'again', 'project-agent:l1', 7),
                ('s4', 't40', 't406', 'delete', 'or this', 'project-agent:l1', 8)",
    )
    .await;
    exec(store.pool(), "DELETE FROM tickets WHERE id = 't406'").await;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lead_suggestions")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 0, "deleted with the child");
}

/// T-0284 (LEAD-06; d0083): migration 0035 adds the lead's columns to `human_questions` on a database after 0034:
/// existing questions keep their rows with no lead, a routed one takes only the four states, and deleting the lead
/// or the epic keeps the question.
#[tokio::test]
async fn questions_keep_their_rows_and_take_the_leads_columns() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 34).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 52, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO tickets (id, project_id, number, epic_id, child_number, kind, title, board_column, state,
                              started_at, created_by, created_at, updated_at, epic_queue_mode)
         VALUES ('t40', 'p', 40, NULL, 0, 'epic', 'Wishlist', 'backlog', 'backlog', NULL, 'me', 0, 0, 'none'),
                ('t402', 'p', 40, 't40', 2, 'ticket', 'Wishlist button', 'dev', 'working', 3, 'me', 1, 1,
                 'none')",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO project_agents (id, project_id, claude_session_id, state, created_at, epic_id)
         VALUES ('l40', 'p', 'c1', 'running', 2, 't40')",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO human_questions (id, ticket_id, question, options_json, asked_at)
         VALUES ('q1', 't402', 'Filled heart?', '[]', 4)",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let row = sqlx::query(
        "SELECT question, lead_id, lead_epic_id, lead_state, lead_reason, lead_at FROM human_questions
         WHERE id = 'q1'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("question"), "Filled heart?");
    for column in ["lead_id", "lead_epic_id", "lead_state", "lead_reason"] {
        assert_eq!(row.get::<Option<String>, _>(column), None, "{column}");
    }
    assert_eq!(row.get::<Option<i64>, _>("lead_at"), None);

    exec(
        store.pool(),
        "UPDATE human_questions SET lead_id = 'l40', lead_epic_id = 't40', lead_state = 'with_lead' WHERE id = 'q1'",
    )
    .await;
    for state in ["answered", "passed_on", "handed_back", "with_lead"] {
        exec(
            store.pool(),
            &format!("UPDATE human_questions SET lead_state = '{state}' WHERE id = 'q1'"),
        )
        .await;
    }
    let refused = sqlx::query("UPDATE human_questions SET lead_state = 'maybe' WHERE id = 'q1'")
        .execute(store.pool())
        .await;
    assert!(refused.is_err(), "the four states only");
    exec(store.pool(), "DELETE FROM project_agents WHERE id = 'l40'").await;
    exec(
        store.pool(),
        "UPDATE tickets SET epic_id = NULL WHERE id = 't402'",
    )
    .await;
    exec(store.pool(), "DELETE FROM tickets WHERE id = 't40'").await;
    let kept: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT lead_id, lead_epic_id, lead_state FROM human_questions WHERE id = 'q1'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(kept, (None, None, Some("with_lead".into())));
}

/// T-0281 (LEAD-10; d0083, d0048): migration 0036 adds the empty `lead_notes` table and the `lead_note` delivery kind
/// to a database after 0035: waiting deliveries are kept, a note starts unread, `read_how` takes only its values and
/// comes with `read_at`, and deleting the child deletes its notes.
#[tokio::test]
async fn lead_notes_start_empty_and_deliveries_take_the_note_kind() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 35).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 52, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO tickets (id, project_id, number, epic_id, child_number, kind, title, board_column, state,
                              started_at, created_by, created_at, updated_at, epic_queue_mode)
         VALUES ('t40', 'p', 40, NULL, 0, 'epic', 'Wishlist', 'backlog', 'backlog', NULL, 'me', 0, 0, 'none'),
                ('t404', 'p', 40, 't40', 4, 'ticket', 'Wishlist share link', 'dev', 'working', 1, 'me',
                 1, 1, 'none')",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO sessions (id, kind, project_id, ticket_id, state, started_at)
         VALUES ('s1', 'agent', 'p', 't404', 'running', 2)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO pending_deliveries (id, session_id, kind, text, created_at)
         VALUES ('d1', 's1', 'lead_wake', 'SHOP-40.1 merged', 3)",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let kept: String = sqlx::query_scalar("SELECT kind FROM pending_deliveries WHERE id = 'd1'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(kept, "lead_wake");
    exec(
        store.pool(),
        "INSERT INTO pending_deliveries (id, session_id, kind, text, created_at)
         VALUES ('d2', 's1', 'lead_note', 'Note from Wishlist lead: …', 4)",
    )
    .await;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lead_notes")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);

    exec(
        store.pool(),
        "INSERT INTO lead_notes (id, epic_id, child_id, lead_id, written_by, text, created_at)
         VALUES ('n1', 't40', 't404', 'l1', 'project-agent:l1', 'Use the same order.', 5)",
    )
    .await;
    let row = sqlx::query("SELECT read_at, read_how, view FROM lead_notes WHERE id = 'n1'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(row.get::<Option<i64>, _>("read_at"), None);
    assert_eq!(row.get::<Option<String>, _>("read_how"), None);
    assert_eq!(row.get::<Option<String>, _>("view"), None);
    for bad in [
        "UPDATE lead_notes SET read_at = 6, read_how = 'skimmed' WHERE id = 'n1'",
        "UPDATE lead_notes SET read_how = 'notes' WHERE id = 'n1'",
        "UPDATE lead_notes SET read_at = 6 WHERE id = 'n1'",
    ] {
        assert!(
            sqlx::query(sqlx::AssertSqlSafe(bad))
                .execute(store.pool())
                .await
                .is_err(),
            "{bad}"
        );
    }
    for how in ["notes", "prime", "typed", "with_stop"] {
        exec(
            store.pool(),
            &format!("UPDATE lead_notes SET read_at = 6, read_how = '{how}' WHERE id = 'n1'"),
        )
        .await;
    }
    exec(store.pool(), "DELETE FROM tickets WHERE id = 't404'").await;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lead_notes")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 0, "deleted with the child");
}

/// T-0285 (LEAD-07; d0083): migration 0037 adds the empty `lead_stuck` table to a database after 0036: one row per
/// child (its one lead attempt), `stop` and `state` take only their values, a held row has no `resolved_at`, the
/// note it points to may go, and deleting the child deletes the row.
#[tokio::test]
async fn lead_stuck_starts_empty_one_row_per_child() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 36).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 52, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO tickets (id, project_id, number, epic_id, child_number, kind, title, board_column, state,
                              started_at, created_by, created_at, updated_at, epic_queue_mode)
         VALUES ('t40', 'p', 40, NULL, 0, 'epic', 'Wishlist', 'backlog', 'backlog', NULL, 'me', 0, 0, 'none'),
                ('t403', 'p', 40, 't40', 3, 'ticket', 'Wishlist page', 'dev', 'working', 1, 'me', 1, 1, 'none')",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let count = async || -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM lead_stuck")
            .fetch_one(store.pool())
            .await
            .unwrap()
    };
    assert_eq!(count().await, 0);
    exec(
        store.pool(),
        "INSERT INTO lead_notes (id, epic_id, child_id, lead_id, written_by, text, created_at)
         VALUES ('n1', 't40', 't403', 'l1', 'project-agent:l1', 'Fix the test.', 5)",
    )
    .await;
    exec(
        store.pool(),
        "INSERT INTO lead_stuck (child_id, epic_id, lead_id, stop, detail, rounds, state, held_at)
         VALUES ('t403', 't40', 'l1', 'validation_failed', 'orders.test.ts expects `list`', 3, 'held', 4)",
    )
    .await;
    for bad in [
        "INSERT INTO lead_stuck (child_id, epic_id, lead_id, stop, state, held_at)
         VALUES ('t403', 't40', 'l1', 'idle', 'held', 6)",
        "UPDATE lead_stuck SET stop = 'crashed' WHERE child_id = 't403'",
        "UPDATE lead_stuck SET state = 'lost', resolved_at = 6 WHERE child_id = 't403'",
        "UPDATE lead_stuck SET state = 'noted' WHERE child_id = 't403'",
        "UPDATE lead_stuck SET resolved_at = 6 WHERE child_id = 't403'",
    ] {
        assert!(
            sqlx::query(sqlx::AssertSqlSafe(bad))
                .execute(store.pool())
                .await
                .is_err(),
            "{bad}"
        );
    }
    exec(
        store.pool(),
        "UPDATE lead_stuck SET state = 'noted', resolved_at = 6, note_id = 'n1' WHERE child_id = 't403'",
    )
    .await;
    exec(store.pool(), "DELETE FROM lead_notes WHERE id = 'n1'").await;
    let note: Option<String> = sqlx::query_scalar("SELECT note_id FROM lead_stuck")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(note, None, "the note may go");
    exec(store.pool(), "DELETE FROM tickets WHERE id = 't403'").await;
    assert_eq!(count().await, 0, "deleted with the child");
}

/// T-0309 (ADP-04, d0088): migration 0038 makes sessions tool-neutral on an M19 database (after 0037). An agent
/// session and a Project agent (and an epic lead) ran Claude Code and keep their ids; a Shell session runs no agent
/// tool; nothing passed a model. The index follows the rename, and a Project agent's tool id may now be empty.
#[tokio::test]
async fn sessions_and_project_agents_take_the_tool_and_keep_their_ids() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 37).await;
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
         VALUES ('t18', 'p', 18, 'ticket', 'T', 'dev', 'working', 'me', 0, 0),
                ('t40', 'p', 40, 'epic', 'Wishlist', 'backlog', 'backlog', 'me', 0, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO project_agents (id, project_id, name, name_source, claude_session_id, state, first_request,
                                     limit_stopped, created_at, ended_at, closed_at, epic_id)
         VALUES ('a1', 'p', 'Checkout slowness', 'agent', 'c-agent', 'ended', 'Why is checkout slow?', 1, 3, 4, 5,
                 NULL),
                ('l40', 'p', NULL, NULL, 'c-lead', 'running', NULL, 0, 6, NULL, NULL, 't40')",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO sessions (id, project_id, ticket_id, kind, claude_session_id, state, started_at,
                               project_agent_id)
         VALUES ('s1', 'p', 't18', 'agent', 'c-ticket', 'ended', 5, NULL),
                ('s2', 'p', 't18', 'shell', NULL, 'ended', 6, NULL),
                ('s3', 'p', NULL, 'project_agent', 'c-agent', 'ended', 7, 'a1')",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    /// `(id, tool, tool_session_id, model, project_agent_id)`.
    type SessionRow = (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    let sessions: Vec<SessionRow> = sqlx::query_as(
        "SELECT id, tool, tool_session_id, model, project_agent_id FROM sessions ORDER BY id",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    let claude = || Some("claude".to_owned());
    assert_eq!(
        sessions,
        [
            ("s1".into(), claude(), Some("c-ticket".into()), None, None),
            ("s2".into(), None, None, None, None),
            (
                "s3".into(),
                claude(),
                Some("c-agent".into()),
                None,
                Some("a1".into())
            ),
        ]
    );
    let agents: Vec<(String, String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT id, tool, tool_session_id, model FROM project_agents ORDER BY id")
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(
        agents,
        [
            ("a1".into(), "claude".into(), Some("c-agent".into()), None),
            ("l40".into(), "claude".into(), Some("c-lead".into()), None),
        ]
    );
    let row = sqlx::query(
        "SELECT name, name_source, state, first_request, limit_stopped, created_at, ended_at, closed_at, epic_id
         FROM project_agents WHERE id = 'a1'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>(0), "Checkout slowness");
    assert_eq!(row.get::<String, _>(1), "agent");
    assert_eq!(row.get::<String, _>(2), "ended");
    assert_eq!(row.get::<String, _>(3), "Why is checkout slow?");
    assert_eq!(row.get::<i64, _>(4), 1);
    assert_eq!(
        (
            row.get::<i64, _>(5),
            row.get::<Option<i64>, _>(6),
            row.get::<Option<i64>, _>(7)
        ),
        (3, Some(4), Some(5))
    );
    assert_eq!(row.get::<Option<String>, _>(8), None);
    let indexes: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name IN
         ('sessions_tool_session', 'sessions_claude_session', 'project_agents_project', 'project_agents_epic')
         ORDER BY name",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        indexes,
        [
            "project_agents_epic",
            "project_agents_project",
            "sessions_tool_session"
        ]
    );
    // A tool that reports its id after start; still one lead per epic; the sessions still point at their agent.
    exec(
        store.pool(),
        "INSERT INTO project_agents (id, project_id, tool, state, created_at) VALUES ('a2', 'p', 'claude', 'running', 8)",
    )
    .await;
    let second = sqlx::query(
        "INSERT INTO project_agents (id, project_id, tool, state, created_at, epic_id)
         VALUES ('l40b', 'p', 'claude', 'running', 9, 't40')",
    )
    .execute(store.pool())
    .await;
    assert!(second.is_err(), "one lead per epic");
    exec(store.pool(), "DELETE FROM project_agents WHERE id = 'a1'").await;
    let agent: Option<String> =
        sqlx::query_scalar("SELECT project_agent_id FROM sessions WHERE id = 's3'")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(agent, None, "the foreign key still holds");
}

/// T-0304 (M19; d0085, d0086, d0087): migration 0041 (written as 0038, renumbered after M20's 0038–0040) gives
/// tickets their environment columns on an M20 database: existing tickets have their build in the worktree and no
/// ticket resource, and unknown states are refused.
#[tokio::test]
async fn ticket_env_columns_default_and_refuse_bad_values() {
    use otaq_core::domain::{BuildState, ResourceState};

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 40).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'shop-web', '/tmp/shop-web', 'SHOP', 'main', 20, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO tickets (id, project_id, number, epic_id, child_number, kind, title, board_column, state,
                              started_at, created_by, created_at, updated_at)
         VALUES ('t18', 'p', 18, NULL, 0, 'ticket', 'Checkout API', 'ready_for_review', 'ready_for_review', 3,
                 'me', 1, 1),
                ('t19', 'p', 19, NULL, 0, 'ticket', 'Price table', 'backlog', 'backlog', NULL, 'me', 2, 2)",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    for id in ["t18", "t19"] {
        let row = store.ticket(id).await.unwrap().expect("the row is kept");
        assert_eq!(
            (row.build_state.as_str(), row.resource_state.as_str()),
            ("present", "none"),
            "{id}"
        );
        assert_eq!(
            (
                row.build_problem,
                row.resource_name,
                row.resource_problem,
                row.schema_overlap
            ),
            (None, None, None, None),
            "{id}"
        );
    }
    for bad in [
        "UPDATE tickets SET build_state = 'gone' WHERE id = 't18'",
        "UPDATE tickets SET resource_state = 'x' WHERE id = 't18'",
        "UPDATE tickets SET build_state = NULL WHERE id = 't18'",
    ] {
        let refused = sqlx::query(bad).execute(store.pool()).await;
        assert!(refused.is_err(), "{bad}");
    }

    store
        .set_build_state("t18", BuildState::Freed, Some("a file is open"))
        .await
        .unwrap();
    store
        .set_resource("t18", Some("shop_18"), ResourceState::Created, None)
        .await
        .unwrap();
    store
        .set_schema_overlap("t18", Some(r#"[{"other":"SHOP-19","path":"db/**"}]"#))
        .await
        .unwrap();
    let row = store.ticket("t18").await.unwrap().unwrap();
    assert_eq!(
        (
            row.build_state.as_str(),
            row.build_problem.as_deref(),
            row.resource_name.as_deref(),
            row.resource_state.as_str(),
            row.resource_problem.as_deref(),
            row.schema_overlap.as_deref(),
        ),
        (
            "freed",
            Some("a file is open"),
            Some("shop_18"),
            "created",
            None,
            Some(r#"[{"other":"SHOP-19","path":"db/**"}]"#)
        )
    );
}

/// T-0347 (M23; AGT-08, SCH-04; d0086, d0088): migration 0044 on an M22 database: the agent rows keep their values
/// and take the move columns, all empty; `agent_holds` starts empty and takes one hold per ticket or project and
/// role, with its reasons checked. No rows are copied (0088).
#[tokio::test]
async fn agent_rows_take_the_move_columns_and_holds_start_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 43).await;
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
         VALUES ('t31', 'p', 31, 'ticket', 'Checkout flow', 'backlog', 'backlog', 'me', 0, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO named_agents (id, name, tool, model, use_for, created_at, updated_at)
         VALUES ('strong', 'Strong', 'claude', 'opus', '', 1, 1)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO ticket_agent_rows (ticket_id, role, named_agent_id, set_by, set_at)
         VALUES ('t31', 'developer', 'strong', 'user', 2)",
    )
    .await;
    pool.close().await;

    // Up to 0044 only: from 0047 the move-over drops the rows (AGM-02, `the_move_over_*`).
    let store = database_at(&path, 44).await;
    let row = sqlx::query(
        "SELECT named_agent_id, set_by, set_at, moved_from_agent_id, moved_from_name, moved_reason, moved_tool,
                moved_at
         FROM ticket_agent_rows WHERE ticket_id = 't31'",
    )
    .fetch_one(&store)
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>(0), "strong");
    assert_eq!(row.get::<String, _>(1), "user");
    assert_eq!(row.get::<i64, _>(2), 2);
    for column in 3..7 {
        assert_eq!(
            row.get::<Option<String>, _>(column),
            None,
            "column {column}"
        );
    }
    assert_eq!(row.get::<Option<i64>, _>(7), None);
    let holds: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_holds")
        .fetch_one(&store)
        .await
        .unwrap();
    assert_eq!(holds, 0);

    // The reasons are checked.
    let bad_move =
        sqlx::query("UPDATE ticket_agent_rows SET moved_reason = 'tired' WHERE ticket_id = 't31'")
            .execute(&store)
            .await;
    assert!(bad_move.is_err());
    exec(
        &store,
        "UPDATE ticket_agent_rows SET moved_from_agent_id = 'strong', moved_from_name = 'Strong',
                moved_reason = 'tool_limit', moved_tool = 'claude', moved_at = 3 WHERE ticket_id = 't31'",
    )
    .await;
    let bad_hold = sqlx::query(
        "INSERT INTO agent_holds (id, ticket_id, role, reason, tool, since)
         VALUES ('h0', 't31', 'developer', 'bored', 'claude', 4)",
    )
    .execute(&store)
    .await;
    assert!(bad_hold.is_err());
    exec(
        &store,
        "INSERT INTO agent_holds (id, ticket_id, project_id, role, reason, tool, model, named_agent_id, since)
         VALUES ('h1', 't31', NULL, 'developer', 'tool_limit', 'claude', 'opus', 'strong', 4),
                ('h2', NULL, 'p', 'project_agent', 'tool_unavailable', 'codex', NULL, NULL, 5)",
    )
    .await;
    let missing_scope = sqlx::query(
        "INSERT INTO agent_holds (id, role, reason, tool, since)
         VALUES ('h3', 'lead', 'tool_limit', 'claude', 6)",
    )
    .execute(&store)
    .await;
    assert!(missing_scope.is_err(), "a ticket's or a project's hold");
    let fallback_held: bool =
        sqlx::query_scalar("SELECT fallback_held FROM agent_holds WHERE id = 'h1'")
            .fetch_one(&store)
            .await
            .unwrap();
    assert!(!fallback_held);
}

/// T-0374 (ATT-17, d0061): migration 0045 adds `option_notes_json`; a question asked before it reads as no notes,
/// for its ticket and for the lead.
#[tokio::test]
async fn questions_asked_before_notes_read_as_no_notes() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 44).await;
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
         VALUES ('t31', 'p', 31, 'ticket', 'Order confirmation email', 'dev', 'asks_you', 'me', 0, 0)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO human_questions (id, ticket_id, question, options_json, recommended, asked_at)
         VALUES ('h1', 't31', 'Postmark or SES?', '[\"Postmark\",\"SES\"]', 1, 4)",
    )
    .await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let stored: Option<String> =
        sqlx::query_scalar("SELECT option_notes_json FROM human_questions WHERE id = 'h1'")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(stored, None);
    let data = otaq_core::data_dir::DataDir::new(path.parent().unwrap().join("home"));
    let board = otaq_core::board::Board::new(store, None, data);
    let question = board.open_question("t31").await.unwrap().expect("open");
    assert_eq!(question.options, ["Postmark", "SES"]);
    assert!(question.notes.is_empty());
}

/// T-0400 (STG-04, STG-05; d0108): migration 0046 puts a ticket in each of today's 26 states (and an Interrupted or
/// limit-held validation) in its place and stage of the default flow, drops `board_column`, makes today's
/// validation rounds runs of `validate` and opens a run for each ticket in a stage; nothing else changes.
#[tokio::test]
async fn every_state_moves_to_its_place_and_stage() {
    use otaq_core::domain::{Place, Status};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 45).await;
    exec(
        &pool,
        "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at)
         VALUES ('p', 'pocket-notes', '/tmp/pocket-notes', 'NOTE', 'main', 41, 0)",
    )
    .await;
    // (state, interrupted_from, today's column) → (state, stage).
    type Row<'a> = (&'a str, Option<&'a str>, &'a str, &'a str, Option<&'a str>);
    let rows: &[Row] = &[
        ("backlog", None, "backlog", "backlog", None),
        ("parked", None, "backlog", "parked", None),
        ("epic_in_progress", None, "backlog", "backlog", None),
        ("queued", None, "ready_for_dev", "queued", None),
        (
            "waiting_for_slot",
            None,
            "ready_for_dev",
            "waiting_for_slot",
            None,
        ),
        ("held", None, "ready_for_dev", "held", None),
        (
            "parent_closed",
            None,
            "ready_for_dev",
            "parent_closed",
            None,
        ),
        (
            "start_interrupted",
            None,
            "ready_for_dev",
            "start_interrupted",
            None,
        ),
        ("working", None, "dev", "working", Some("dev")),
        (
            "working_in_background",
            None,
            "dev",
            "working_in_background",
            Some("dev"),
        ),
        ("asks_you", None, "dev", "asks_you", Some("dev")),
        ("needs_you", None, "dev", "needs_you", Some("dev")),
        ("idle", None, "dev", "idle", Some("dev")),
        (
            "waiting_for_limit_reset",
            None,
            "dev",
            "waiting_for_limit_reset",
            Some("dev"),
        ),
        (
            "interrupted",
            Some("idle"),
            "dev",
            "interrupted",
            Some("dev"),
        ),
        ("resume_failed", None, "dev", "resume_failed", Some("dev")),
        ("validating", None, "dev", "validating", Some("validate")),
        (
            "validator_needs_you",
            None,
            "dev",
            "validator_needs_you",
            Some("validate"),
        ),
        (
            "validation_failed",
            None,
            "dev",
            "validation_failed",
            Some("validate"),
        ),
        (
            "validation_no_result",
            None,
            "dev",
            "validation_no_result",
            Some("validate"),
        ),
        (
            "merge_conflict_fixing",
            None,
            "dev",
            "merge_conflict_fixing",
            Some("dev"),
        ),
        (
            "auto_merge_stopped",
            None,
            "dev",
            "auto_merge_stopped",
            Some("dev"),
        ),
        (
            "ready_for_review",
            None,
            "ready_for_review",
            "ready_for_review",
            Some("review"),
        ),
        ("pr_open", None, "approved", "pr_open", Some("deliver")),
        ("merged", None, "done", "merged", None),
        ("closed_not_merged", None, "done", "closed_not_merged", None),
        (
            "interrupted",
            Some("validating"),
            "dev",
            "interrupted",
            Some("validate"),
        ),
        (
            "waiting_for_limit_reset",
            Some("validating"),
            "dev",
            "waiting_for_limit_reset",
            Some("validate"),
        ),
    ];
    assert_eq!(
        rows.iter()
            .take(26)
            .map(|r| r.0)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        26,
        "each of today's 26 states once"
    );
    for (n, (state, from, column, _, _)) in rows.iter().enumerate() {
        sqlx::query(
            "INSERT INTO tickets (id, project_id, number, kind, title, board_column, state, interrupted_from,
                                  created_by, created_at, updated_at, state_since, validation_round)
             VALUES (?, 'p', ?, 'ticket', ?, ?, ?, ?, 'me', 1, 2, 3, 1)",
        )
        .bind(format!("t{}", n + 1))
        .bind(n as i64 + 1)
        .bind(format!("Ticket {state}"))
        .bind(*column)
        .bind(*state)
        .bind(*from)
        .execute(&pool)
        .await
        .unwrap();
    }
    exec(
        &pool,
        "INSERT INTO sessions (id, project_id, ticket_id, kind, tool, model, model_ran, named_agent_name, state,
                               started_at, ended_at)
         VALUES ('dev9', 'p', 't9', 'agent', 'claude', 'sonnet', 'claude-sonnet-5-5', 'Developer', 'running', 5,
                 NULL),
                ('val17', 'p', 't17', 'validator', 'claude', 'opus', NULL, 'Validator', 'running', 6, NULL)",
    )
    .await;
    exec(
        &pool,
        "INSERT INTO validations (id, ticket_id, round, trigger, session_id, started_at, ended_at, outcome, report)
         VALUES ('v1', 't17', 1, 'agent', NULL, 4, 5, 'fail', 'Missing test'),
                ('v2', 't17', 2, 'agent', 'val17', 6, NULL, NULL, NULL)",
    )
    .await;
    // Every column but the dropped one, as text, before the migration.
    let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('tickets')")
        .fetch_all(&pool)
        .await
        .unwrap();
    let kept: Vec<&String> = columns
        .iter()
        .filter(|c| !matches!(c.as_str(), "board_column" | "state"))
        .collect();
    let snapshot = |pool: SqlitePool| {
        let list = kept
            .iter()
            .map(|c| format!("COALESCE(CAST({c} AS TEXT), '∅')"))
            .collect::<Vec<_>>()
            .join(" || '|' || ");
        async move {
            sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(format!(
                "SELECT {list} FROM tickets ORDER BY number"
            )))
            .fetch_all(&pool)
            .await
            .unwrap()
        }
    };
    let before = snapshot(pool.clone()).await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let after: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT state, stage_id FROM tickets ORDER BY number")
            .fetch_all(store.pool())
            .await
            .unwrap();
    for ((state, from, _, to, stage), (got_state, got_stage)) in rows.iter().zip(&after) {
        assert_eq!(
            (got_state.as_str(), got_stage.as_deref()),
            (*to, *stage),
            "{state} (from {from:?})"
        );
    }
    let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('tickets')")
        .fetch_all(store.pool())
        .await
        .unwrap();
    assert!(!columns.iter().any(|c| c == "board_column"), "{columns:?}");
    assert_eq!(
        snapshot(store.pool().clone()).await,
        before,
        "nothing else changes"
    );

    // The board reads each place.
    let data = otaq_core::data_dir::DataDir::new(path.parent().unwrap().join("home"));
    let board = otaq_core::board::Board::new(store.clone(), None, data);
    let tickets = board.tickets("NOTE").await.unwrap();
    let place = |state: &str, n: usize| {
        let t = tickets
            .iter()
            .find(|t| t.title == format!("Ticket {state}") && t.key.number as usize == n)
            .unwrap();
        (t.place.clone(), t.status)
    };
    assert_eq!(place("parked", 2), (Place::Backlog, Status::Parked));
    assert_eq!(place("held", 6), (Place::Waiting(None), Status::Held));
    assert_eq!(
        place("working", 9),
        (Place::stage_of("dev"), Status::Working)
    );
    assert_eq!(
        place("validating", 17),
        (Place::stage_of("validate"), Status::Checking)
    );
    assert_eq!(
        place("ready_for_review", 23),
        (Place::stage_of("review"), Status::WaitingForYou)
    );
    assert_eq!(
        place("pr_open", 24),
        (Place::stage_of("deliver"), Status::PrOpen)
    );
    assert_eq!(place("merged", 25), (Place::Done, Status::Merged));
    assert_eq!(
        place("interrupted", 27),
        (Place::stage_of("validate"), Status::Interrupted)
    );

    // The rounds are runs of `validate`; tickets in a stage have an open run, with their live session.
    let runs = board.stage_runs("NOTE-17").await.unwrap();
    let runs: Vec<_> = runs
        .iter()
        .map(|r| {
            (
                r.stage.as_str(),
                r.kind.as_str(),
                r.round,
                r.result.map(|r| r.as_str()),
                r.session.as_deref(),
                r.model.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        runs,
        [
            ("validate", "check", Some(1), Some("failed"), None, None),
            (
                "validate",
                "check",
                Some(2),
                None,
                Some("val17"),
                Some("opus")
            )
        ]
    );
    let dev = board.stage_runs("NOTE-9").await.unwrap();
    assert_eq!(dev.len(), 1);
    assert_eq!(
        (
            dev[0].stage.as_str(),
            dev[0].session.as_deref(),
            dev[0].agent.as_deref(),
            dev[0].model.as_deref(),
            dev[0].result
        ),
        (
            "dev",
            Some("dev9"),
            Some("Developer"),
            Some("claude-sonnet-5-5"),
            None
        )
    );
    assert!(board.stage_runs("NOTE-1").await.unwrap().is_empty());
    assert!(board.stage_runs("NOTE-25").await.unwrap().is_empty());
    assert_eq!(
        board.stage_runs("NOTE-24").await.unwrap()[0].stage,
        "deliver"
    );
}

/// The example's M31 data (`tickets/m32-tickets.md`, "The example"), as M31's store wrote it: named agents Strong
/// (Claude Code · opus) and Light (Claude Code · haiku); the app's role defaults Developer = Light, Validator =
/// Strong, Epic lead = Strong, Project agent = Light (`agents.defaults`); the global Agent instructions, Project agent
/// instructions and imported Validate skill `team-validate` (settings rows); pocket-notes (NOTE, Validation on, 2
/// rounds, its own Agent instructions) and pocket-api (API, Validation off, its own Developer = Strong and its own
/// imported Commit skill); NOTE-1 in Review, NOTE-2 waiting for NOTE-1, NOTE-3 in Backlog with a Developer row set by
/// you (Strong), NOTE-4 in Dev on Light, API-1 in Dev, the epic NOTE-10 with NOTE-10.1 merged and NOTE-10.2 in Dev.
async fn m31_example(pool: &SqlitePool) {
    let notes = json!({
        "validation": true, "validation_rounds": 2, "agent_instructions": "use pnpm, not npm",
    });
    let api = json!({
        "validation": false,
        "agents": {"developer": "strong"},
        "workflow_skills": {"commit": {"id": "api-commit", "name": "api-commit", "original": "/team/api-commit",
                                       "imported_at": 7, "fingerprint": "f1"},
                            "push": "", "open_pr": "", "validate": ""},
    });
    for (id, name, key, settings) in [
        ("pn", "pocket-notes", "NOTE", notes),
        ("pa", "pocket-api", "API", api),
    ] {
        sqlx::query(
            "INSERT INTO projects (id, name, repo_path, key, base_branch, next_ticket_number, created_at,
                                   settings_json)
             VALUES (?, ?, ?, ?, 'main', 11, 0, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(format!("/tmp/{name}"))
        .bind(key)
        .bind(settings.to_string())
        .execute(pool)
        .await
        .unwrap();
    }
    exec(
        pool,
        "INSERT INTO named_agents (id, name, tool, model, use_for, created_at, updated_at)
         VALUES ('strong', 'Strong', 'claude', 'opus', 'Complex features, tricky bugs', 1, 1),
                ('light', 'Light', 'claude', 'haiku', 'Typos, docs, small config', 2, 2)",
    )
    .await;
    let team_validate = json!({"id": "team-validate", "name": "team-validate", "original": "/team/validate",
                               "imported_at": 5, "fingerprint": "f0", "warnings": []});
    for (key, value) in [
        (
            "agents.defaults",
            json!({"developer": "light", "validator": "strong", "epic_lead": "strong",
                   "project_agent": "light"}),
        ),
        ("agents.auto_fallback", json!(false)),
        (
            "agent_instructions",
            json!("Run `just test` before committing"),
        ),
        ("project_agent_instructions", json!("Keep tickets small")),
        (
            "workflow_skills",
            json!({"commit": "/otaq:commit", "push": "", "open_pr": "", "validate": team_validate}),
        ),
    ] {
        sqlx::query("INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, 0)")
            .bind(key)
            .bind(value.to_string())
            .execute(pool)
            .await
            .unwrap();
    }
    exec(
        pool,
        "INSERT INTO tickets (id, project_id, number, child_number, epic_id, kind, title, state, stage_id, created_by,
                              created_at, updated_at)
         VALUES ('n1', 'pn', 1, 0, NULL, 'ticket', 'Sort notes by date', 'ready_for_review', 'review', 'me', 0, 0),
                ('n2', 'pn', 2, 0, NULL, 'ticket', 'Note colors', 'queued', NULL, 'me', 0, 0),
                ('n3', 'pn', 3, 0, NULL, 'ticket', 'Fix README typo', 'backlog', NULL, 'me', 0, 0),
                ('n4', 'pn', 4, 0, NULL, 'ticket', 'Search', 'queued', NULL, 'me', 0, 0),
                ('a1', 'pa', 1, 0, NULL, 'ticket', 'Rate limits', 'working', 'dev', 'me', 0, 0),
                ('n10', 'pn', 10, 0, NULL, 'epic', 'Sharing', 'backlog', NULL, 'me', 0, 0),
                ('n10a', 'pn', 10, 1, 'n10', 'ticket', 'Share link', 'merged', NULL, 'me', 0, 0),
                ('n10b', 'pn', 10, 2, 'n10', 'ticket', 'Share page', 'working', 'dev', 'me', 0, 0)",
    )
    .await;
    exec(
        pool,
        "INSERT INTO ticket_deps (ticket_id, depends_on_id) VALUES ('n2', 'n1')",
    )
    .await;
    exec(
        pool,
        "INSERT INTO ticket_agent_rows (ticket_id, role, named_agent_id, set_by, set_at)
         VALUES ('n3', 'developer', 'strong', 'user', 3)",
    )
    .await;
    // NOTE-4's Dev session on Light (M22 stored the named agent on it; 0046's trigger copied it onto its run).
    exec(
        pool,
        "INSERT INTO sessions (id, project_id, ticket_id, kind, tool, tool_session_id, model, model_ran,
                               named_agent_id, named_agent_name, state, started_at)
         VALUES ('s4', 'pn', 'n4', 'agent', 'claude', 'c-4', 'haiku', 'claude-haiku-5-5', 'light', 'Light',
                 'running', 5)",
    )
    .await;
    // Its start: the ticket enters Dev, and its run takes the live session (0046's trigger).
    exec(
        pool,
        "UPDATE tickets SET state = 'working', stage_id = 'dev' WHERE id = 'n4'",
    )
    .await;
}

/// A project's stored settings.
async fn project_json(store: &Store, key: &str) -> serde_json::Value {
    let json: String = sqlx::query_scalar("SELECT settings_json FROM projects WHERE key = ?")
        .bind(key)
        .fetch_one(store.pool())
        .await
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

/// A table's presence.
async fn has_table(pool: &SqlitePool, name: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?)",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Every row of the tables the move-over writes, as text: what a second start must leave as it is.
async fn moved_over_state(store: &Store) -> Vec<String> {
    let mut rows = Vec::new();
    for sql in [
        "SELECT id || '|' || name || '|' || instructions || '|' || COALESCE(tool, '-') || '|' || COALESCE(model, '-')
                || '|' || COALESCE(role, '-') || '|' || came_from || '|' || updated_at FROM agents ORDER BY id",
        "SELECT agent_id || '|' || position || '|' || COALESCE(shipped, imported_id) FROM agent_skills
         ORDER BY agent_id, position",
        "SELECT key || '|' || value_json FROM settings ORDER BY key",
        "SELECT key || '|' || settings_json FROM projects ORDER BY key",
    ] {
        let found: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .fetch_all(store.pool())
            .await
            .unwrap();
        rows.extend(found);
    }
    rows
}

/// T-0405 (AGM-02, AGM-13; d0108, user decisions 2026-10-20): the first start of M32 on an M31 database moves its
/// settings over in one step, exactly as AGM-02 lists: the agents, models, instructions and skills of "After the
/// update"; pocket-api's own Developer and Commit skill and NOTE-3's row recorded as dropped; pocket-notes' Agent
/// instructions kept as its notes and the Validation settings kept; the old tables and settings gone; and a second
/// start changes nothing.
#[tokio::test]
async fn the_move_over_carries_m31s_agents_and_records_what_it_dropped() {
    use otaq_core::agent_tools::Tool;
    use otaq_core::agents::{AgentModel, CameFrom, ModelRef, Role, SkillRef};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    let pool = database_at(&path, 46).await;
    m31_example(&pool).await;
    pool.close().await;

    let store = Store::open(&path).await.unwrap();
    let agents = store.agents().await.unwrap();
    let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Planner",
            "Epic lead",
            "Developer",
            "Validator",
            "Strong",
            "Light"
        ]
    );
    let agent = |name: &str| agents.iter().find(|a| a.name == name).unwrap();
    let claude = |model: &str| AgentModel::Tool {
        tool: Tool::Claude,
        model: Some(model.into()),
    };
    let shipped = |names: &[&str]| -> Vec<SkillRef> {
        names
            .iter()
            .map(|n| SkillRef::Shipped((*n).into()))
            .collect()
    };
    // The default model ← the app's Developer default's model (Light's).
    assert_eq!(
        store.default_model().await.unwrap(),
        Some(ModelRef {
            tool: Tool::Claude,
            model: Some("haiku".into())
        })
    );
    // Each role default's model → the matching agent; the instructions to Developer and Planner.
    let developer = agent("Developer");
    assert_eq!(developer.model, claude("haiku"));
    assert_eq!(developer.instructions, "Run `just test` before committing");
    assert_eq!(developer.skills, shipped(&["work-ticket", "commit"]));
    assert_eq!(developer.came_from, CameFrom::DefaultPreset);
    let validator = agent("Validator");
    assert_eq!(validator.model, claude("opus"));
    assert_eq!(validator.instructions, "");
    // The global imported Validate skill in place of the shipped one.
    assert_eq!(
        validator.skills,
        [SkillRef::Imported("team-validate".into())]
    );
    let validate = &validator.imported[0];
    assert_eq!(
        (
            validate.name.as_str(),
            validate.original.as_str(),
            validate.stands_for.as_deref()
        ),
        ("team-validate", "/team/validate", Some("validate"))
    );
    let lead = agent("Epic lead");
    assert_eq!(
        (&lead.model, lead.role),
        (&claude("opus"), Some(Role::EpicLead))
    );
    let planner = agent("Planner");
    assert_eq!(
        (&planner.model, planner.role, planner.instructions.as_str()),
        (
            &claude("haiku"),
            Some(Role::ProjectAgent),
            "Keep tickets small"
        )
    );
    assert_eq!(planner.skills, shipped(&["project-agent"]));
    assert_eq!(planner.came_from, CameFrom::AgentsFile);
    // Every named agent: its id, name and model, Developer's instructions and skills; no fallback (per tool now).
    for (id, name, model) in [("strong", "Strong", "opus"), ("light", "Light", "haiku")] {
        let a = agent(name);
        assert_eq!(a.id, id);
        assert_eq!(a.model, claude(model));
        assert_eq!(a.instructions, developer.instructions);
        assert_eq!(a.skills, developer.skills);
        assert_eq!((a.role, &a.came_from), (None, &CameFrom::You));
    }
    assert!(store.fallbacks().await.unwrap().is_empty());
    // The default flow's names are bound to the shipped agents.
    let names = store.flow_names().await.unwrap();
    assert_eq!(names.get("Developer"), Some(&developer.id));
    assert_eq!(names.get("Validator"), Some(&validator.id));

    // Dropped, and recorded for the notice: pocket-api's own Developer (Strong) and Commit skill, NOTE-3's row.
    let dropped = store.dropped().await.unwrap().expect("ran");
    assert!(!dropped.dismissed);
    assert_eq!(dropped.projects.len(), 1, "{dropped:?}");
    let api = &dropped.projects[0];
    assert_eq!((api.key.as_str(), api.name.as_str()), ("API", "pocket-api"));
    assert_eq!(
        api.roles
            .iter()
            .map(|r| (r.role.as_str(), r.agent.as_str()))
            .collect::<Vec<_>>(),
        [("Developer", "Strong")]
    );
    assert_eq!(
        api.skills
            .iter()
            .map(|s| (s.step.as_str(), s.name.as_str()))
            .collect::<Vec<_>>(),
        [("Commit", "api-commit")]
    );
    assert!(!api.project_agent_instructions);
    assert_eq!(dropped.tickets.len(), 1);
    let row = &dropped.tickets[0];
    assert_eq!(
        (
            row.key.to_string(),
            row.role.as_str(),
            row.agent.as_deref(),
            row.set_by_user
        ),
        ("NOTE-3".to_owned(), "Developer", Some("Strong"), true)
    );

    // Kept: pocket-notes' Agent instructions as its notes, both projects' Validation settings.
    let notes = project_json(&store, "NOTE").await;
    assert_eq!(notes["project_notes"], "use pnpm, not npm");
    assert!(notes.get("agent_instructions").is_none());
    assert_eq!(
        (&notes["validation"], &notes["validation_rounds"]),
        (&json!(true), &json!(2))
    );
    let api = project_json(&store, "API").await;
    assert_eq!(api["validation"], json!(false));
    assert!(api.get("agents").is_none() && api.get("workflow_skills").is_none());

    // Gone: the old tables and settings.
    assert!(!has_table(store.pool(), "named_agents").await);
    assert!(!has_table(store.pool(), "ticket_agent_rows").await);
    for key in [
        "agents.defaults",
        "agents.auto_fallback",
        "agent_instructions",
        "project_agent_instructions",
        "workflow_skills",
    ] {
        assert_eq!(store.setting(key).await.unwrap(), None, "{key}");
    }
    // NOTE-4's session still names Light, now an agent; its run keeps the agent's id.
    let (agent_id, run_agent): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT s.agent_id, r.agent_id FROM sessions s JOIN stage_runs r ON r.session_id = s.id WHERE s.id = 's4'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        (agent_id.as_deref(), run_agent.as_deref()),
        (Some("light"), Some("light"))
    );

    // A second start changes nothing.
    let before = moved_over_state(&store).await;
    store.close().await;
    let store = Store::open(&path).await.unwrap();
    assert_eq!(moved_over_state(&store).await, before);
    assert_eq!(store.dropped().await.unwrap(), Some(dropped));
}

/// T-0405 (AGM-02): a fresh install (an empty M31 database, or none) applies the two files with nothing to carry and
/// nothing to drop; the default model waits for the first connected tool.
#[tokio::test]
async fn a_fresh_install_applies_the_two_files_with_nothing_to_drop() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("otaq.db");
    database_at(&path, 46).await.close().await;
    let store = Store::open(&path).await.unwrap();
    let agents = store.agents().await.unwrap();
    let names: Vec<(&str, bool)> = agents
        .iter()
        .map(|a| {
            (
                a.name.as_str(),
                a.model == otaq_core::agents::AgentModel::Default,
            )
        })
        .collect();
    assert_eq!(
        names,
        [
            ("Planner", true),
            ("Epic lead", true),
            ("Developer", true),
            ("Validator", true)
        ]
    );
    assert!(store.dropped().await.unwrap().unwrap().is_empty());
    assert_eq!(store.default_model().await.unwrap(), None);
    assert!(!has_table(store.pool(), "named_agents").await);
}
