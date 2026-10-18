//! Migrations that change existing rows (SDD §8): a database is written by the migrations up to a version, filled
//! with rows of that time, then opened with [`Store::open`], which runs the rest. Every test uses a temporary
//! folder, never the real data folder.

use std::borrow::Cow;
use std::path::Path;
use std::str::FromStr;

use otaq_core::store::Store;
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

    let store = Store::open(&path).await.unwrap();
    let row = sqlx::query(
        "SELECT named_agent_id, set_by, set_at, moved_from_agent_id, moved_from_name, moved_reason, moved_tool,
                moved_at
         FROM ticket_agent_rows WHERE ticket_id = 't31'",
    )
    .fetch_one(store.pool())
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
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(holds, 0);

    // The reasons are checked.
    let bad_move =
        sqlx::query("UPDATE ticket_agent_rows SET moved_reason = 'tired' WHERE ticket_id = 't31'")
            .execute(store.pool())
            .await;
    assert!(bad_move.is_err());
    exec(
        store.pool(),
        "UPDATE ticket_agent_rows SET moved_from_agent_id = 'strong', moved_from_name = 'Strong',
                moved_reason = 'tool_limit', moved_tool = 'claude', moved_at = 3 WHERE ticket_id = 't31'",
    )
    .await;
    let bad_hold = sqlx::query(
        "INSERT INTO agent_holds (id, ticket_id, role, reason, tool, since)
         VALUES ('h0', 't31', 'developer', 'bored', 'claude', 4)",
    )
    .execute(store.pool())
    .await;
    assert!(bad_hold.is_err());
    exec(
        store.pool(),
        "INSERT INTO agent_holds (id, ticket_id, project_id, role, reason, tool, model, named_agent_id, since)
         VALUES ('h1', 't31', NULL, 'developer', 'tool_limit', 'claude', 'opus', 'strong', 4),
                ('h2', NULL, 'p', 'project_agent', 'tool_unavailable', 'codex', NULL, NULL, 5)",
    )
    .await;
    let missing_scope = sqlx::query(
        "INSERT INTO agent_holds (id, role, reason, tool, since)
         VALUES ('h3', 'lead', 'tool_limit', 'claude', 6)",
    )
    .execute(store.pool())
    .await;
    assert!(missing_scope.is_err(), "a ticket's or a project's hold");
    let fallback_held: bool =
        sqlx::query_scalar("SELECT fallback_held FROM agent_holds WHERE id = 'h1'")
            .fetch_one(store.pool())
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
    let board = otaq_core::board::Board::new(store, None);
    let question = board.open_question("t31").await.unwrap().expect("open");
    assert_eq!(question.options, ["Postmark", "SES"]);
    assert!(question.notes.is_empty());
}
