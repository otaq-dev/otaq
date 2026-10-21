//! The schema-path warning (T-0313; M19-e, d0087) against a running service and temporary repositories: two tickets
//! in Review that both add files under one of the project's schema paths are warned about each other; the warning
//! clears when one leaves Review; with no schema paths no `git` runs for it. Files are never touched.
//!
//! The service scans on the events a request publishes; tests that change a status directly announce it
//! themselves, as a request would ([`announce`]).

use std::path::{Path, PathBuf};
use std::time::Duration;

use otaq_core::board::tickets::{EventKind, Ticket};
use otaq_core::domain::{SchemaOverlap, Status};
use otaq_core::git::testing::{commit, git, snapshot};
use otaq_core::protocol::Event;
use otaq_core::service::testing::TestService;

const MIGRATIONS: &str = "db/migrations/**";

/// shop-web, with `schema_paths` as its schema paths.
async fn shop_web(w: &TestService, schema_paths: &[&str]) {
    w.add_project("shop-web").await;
    let mut settings = w.board.project_settings("SHOP").await.unwrap();
    settings.schema_paths = schema_paths.iter().map(|p| (*p).to_owned()).collect();
    w.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
}

/// A started ticket that committed `file` on its branch, then put in Ready for Review.
async fn in_review(w: &TestService, title: &str, file: &str) -> (Ticket, PathBuf) {
    let t = w.create("SHOP", title).await;
    let wt = w.start_worktree(&t).await;
    std::fs::create_dir_all(wt.join(file).parent().unwrap()).unwrap();
    commit(&wt, file, &format!("-- {title}\n"));
    w.force_status(&t, Status::WaitingForYou).await;
    (w.board.ticket(&t.key.to_string()).await.unwrap(), wt)
}

/// Publishes the ticket's status as a request that changed it would.
async fn announce(w: &TestService, key: &str) {
    let t = w.board.ticket(key).await.unwrap();
    w.events.publish(Event::CardStatusChanged {
        project: t.key.project.to_string(),
        id: t.id.clone(),
        key: t.key.clone(),
        status: t.status,
    });
}

async fn overlap(w: &TestService, key: &str) -> Vec<SchemaOverlap> {
    w.board
        .ticket(key)
        .await
        .unwrap()
        .work
        .unwrap_or_default()
        .schema_overlap
}

/// Waits until `key`'s warning names exactly `others`.
async fn until_overlap(w: &TestService, key: &str, others: &[&str]) {
    for _ in 0..400 {
        let now: Vec<String> = overlap(w, key)
            .await
            .iter()
            .map(|o| o.other.to_string())
            .collect();
        if now == others {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!(
        "{key}'s warning never named {others:?}: {:?}",
        overlap(w, key).await
    );
}

/// Every file under `wt` (but `.git`) with its content.
fn files(wt: &Path) -> Vec<(String, String)> {
    snapshot(wt)
        .into_iter()
        .filter(|p| p != ".git" && !p.starts_with(".git/"))
        .filter(|p| wt.join(p).is_file())
        .map(|p| {
            let content = std::fs::read_to_string(wt.join(&p)).unwrap();
            (p, content)
        })
        .collect()
}

/// **The M19-e acceptance**: two Review tickets adding migrations get a warning naming each other, with the path
/// and the other's files, recorded in their history once; a Review ticket adding elsewhere gets none. No file
/// changes and `git status` stays clean.
#[tokio::test]
async fn two_review_tickets_adding_migrations_get_a_warning() {
    let w = TestService::start().await;
    shop_web(&w, &[MIGRATIONS, "prisma/schema.prisma"]).await;
    let (a, wt_a) = in_review(&w, "Orders table", "db/migrations/0042_orders.sql").await;
    let (b, wt_b) = in_review(&w, "Coupons table", "db/migrations/0042_coupons.sql").await;
    let (c, wt_c) = in_review(&w, "Footer links", "src/footer.ts").await;
    let before: Vec<_> = [&wt_a, &wt_b, &wt_c].map(|wt| files(wt)).into();

    for t in [&a, &b, &c] {
        announce(&w, &t.key.to_string()).await;
    }
    until_overlap(&w, &a.key.to_string(), &[&b.key.to_string()]).await;
    until_overlap(&w, &b.key.to_string(), &[&a.key.to_string()]).await;

    assert_eq!(
        overlap(&w, &a.key.to_string()).await,
        vec![SchemaOverlap {
            other: b.key.clone(),
            path: MIGRATIONS.into(),
            files: vec!["db/migrations/0042_coupons.sql".into()],
        }]
    );
    assert!(overlap(&w, &c.key.to_string()).await.is_empty());

    // Scanned again (the sweep): nothing new is recorded.
    w.ticket_env.sweep().await;
    for (t, other) in [(&a, &b), (&b, &a)] {
        let history = w.board.history(&t.key.to_string()).await.unwrap();
        let recorded: Vec<_> = history
            .iter()
            .filter(|e| e.kind == EventKind::SchemaOverlap)
            .collect();
        assert_eq!(recorded.len(), 1, "{}", t.key);
        assert_eq!(recorded[0].payload["other"], other.key.to_string());
        assert_eq!(recorded[0].payload["path"], MIGRATIONS);
    }

    // Files are never touched.
    let after: Vec<_> = [&wt_a, &wt_b, &wt_c].map(|wt| files(wt)).into();
    assert_eq!(before, after);
    for wt in [&wt_a, &wt_b, &wt_c] {
        assert_eq!(git(wt, &["status", "--porcelain"]), "", "{}", wt.display());
    }
    w.stop().await;
}

#[tokio::test]
async fn the_warning_clears_when_one_leaves_review() {
    let w = TestService::start().await;
    shop_web(&w, &[MIGRATIONS]).await;
    let (a, _) = in_review(&w, "Orders table", "db/migrations/0042_orders.sql").await;
    let (b, _) = in_review(&w, "Coupons table", "db/migrations/0042_coupons.sql").await;
    announce(&w, &a.key.to_string()).await;
    announce(&w, &b.key.to_string()).await;
    until_overlap(&w, &a.key.to_string(), &[&b.key.to_string()]).await;
    until_overlap(&w, &b.key.to_string(), &[&a.key.to_string()]).await;

    // Sent back to the agent's column: both warnings clear.
    w.force_status(&b, Status::NeedsYou).await;
    announce(&w, &b.key.to_string()).await;
    until_overlap(&w, &a.key.to_string(), &[]).await;
    until_overlap(&w, &b.key.to_string(), &[]).await;

    // Back in Review: the warning comes back, and is recorded as new again.
    w.force_status(&b, Status::WaitingForYou).await;
    announce(&w, &b.key.to_string()).await;
    until_overlap(&w, &a.key.to_string(), &[&b.key.to_string()]).await;
    let history = w.board.history(&a.key.to_string()).await.unwrap();
    assert_eq!(
        history
            .iter()
            .filter(|e| e.kind == EventKind::SchemaOverlap)
            .count(),
        2
    );
    w.stop().await;
}

#[tokio::test]
async fn no_schema_paths_no_git_calls() {
    let w = TestService::start().await;
    shop_web(&w, &[]).await;
    let (a, _) = in_review(&w, "Orders table", "db/migrations/0042_orders.sql").await;
    let (b, _) = in_review(&w, "Coupons table", "db/migrations/0042_coupons.sql").await;
    // Scanned directly (no event, so no scan of the service's own runs alongside to count).
    w.ticket_env.rescan_schema("SHOP").await;
    w.ticket_env.sweep().await;
    assert_eq!(w.ticket_env.schema_scans(), 0);
    assert!(overlap(&w, &a.key.to_string()).await.is_empty());
    assert!(overlap(&w, &b.key.to_string()).await.is_empty());

    // With a schema path, each Review ticket is read once per scan.
    let mut settings = w.board.project_settings("SHOP").await.unwrap();
    settings.schema_paths = vec![MIGRATIONS.into()];
    w.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    let before = w.ticket_env.schema_scans();
    w.ticket_env.rescan_schema("SHOP").await;
    assert_eq!(w.ticket_env.schema_scans() - before, 2);
    assert_eq!(overlap(&w, &a.key.to_string()).await.len(), 1);

    // The schema paths emptied again: the warning clears, still without git.
    settings.schema_paths.clear();
    w.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    let before = w.ticket_env.schema_scans();
    w.ticket_env.rescan_schema("SHOP").await;
    assert_eq!(w.ticket_env.schema_scans(), before);
    assert!(overlap(&w, &a.key.to_string()).await.is_empty());
    w.stop().await;
}

/// A schema path naming a folder (`db/migrations`, no `**`) covers the files inside it, as its `git diff` pathspec
/// does.
#[tokio::test]
async fn a_folder_schema_path_warns_too() {
    let w = TestService::start().await;
    shop_web(&w, &["db/migrations"]).await;
    let (a, _) = in_review(&w, "Orders table", "db/migrations/0042_orders.sql").await;
    let (b, _) = in_review(&w, "Coupons table", "db/migrations/0042_coupons.sql").await;
    w.ticket_env.rescan_schema("SHOP").await;
    assert_eq!(
        overlap(&w, &a.key.to_string()).await,
        vec![SchemaOverlap {
            other: b.key.clone(),
            path: "db/migrations".into(),
            files: vec!["db/migrations/0042_coupons.sql".into()],
        }]
    );
    w.stop().await;
}
