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
        "INSERT INTO project_agents (id, project_id, claude_session_id, state, created_at)
         VALUES ('a1', 'p', 'c1', 'running', 0)",
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
        "INSERT INTO project_agents (id, project_id, claude_session_id, state, created_at, epic_id)
         VALUES ('l40', 'p', 'c2', 'running', 0, 't40'), ('l45', 'p', 'c3', 'running', 0, 't45'),
                ('a2', 'p', 'c4', 'running', 0, NULL)",
    )
    .await;
    let second = sqlx::query(
        "INSERT INTO project_agents (id, project_id, claude_session_id, state, created_at, epic_id)
         VALUES ('l40b', 'p', 'c5', 'running', 0, 't40')",
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
