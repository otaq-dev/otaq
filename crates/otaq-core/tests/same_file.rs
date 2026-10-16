//! Children changing the same file (T-0282, LEAD-09; d0083) against temporary repositories: running children of
//! one epic that changed a file in common flag each other ("same file as SHOP-40.4"), quietly. The repository's
//! own folder stands for the user's checkout.

use std::path::{Path, PathBuf};

use otaq_core::board::tickets::{NewTicketInput, Ticket};
use otaq_core::domain::{Actor, Kind, SameFile, Status};
use otaq_core::epic_lead::overlap::{Overlap, overlaps, refresh};
use otaq_core::git::testing::{commit, git};
use otaq_core::naming;
use otaq_core::service::testing::TestService;

fn me() -> Actor {
    Actor::User("me".into())
}

async fn epic(w: &TestService) -> Ticket {
    w.board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Wishlist".into(),
                kind: Kind::Epic,
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap()
}

/// A working child of `epic` with its own worktree.
async fn running(w: &TestService, epic: &Ticket, title: &str) -> (Ticket, PathBuf) {
    let t = w
        .board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: title.into(),
                epic: Some(epic.key.to_string()),
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    let wt = w.start_worktree(&t).await;
    w.force_status(&t, Status::Working).await;
    (t, wt)
}

fn write(wt: &Path, file: &str, content: &str) {
    std::fs::create_dir_all(wt.join(file).parent().unwrap()).unwrap();
    std::fs::write(wt.join(file), content).unwrap();
}

async fn same_file(w: &TestService, t: &Ticket) -> Vec<SameFile> {
    w.board
        .ticket(&t.key.to_string())
        .await
        .unwrap()
        .flags
        .same_file
}

async fn count(w: &TestService, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .fetch_one(w.board.store().pool())
        .await
        .unwrap()
}

/// 40.2 committed `lib/wishlist.ts`, 40.4 changed it without committing: both are flagged, with the file. 40.3
/// changed another file: it isn't. Nothing is queued for anyone and nothing goes to History.
#[tokio::test(flavor = "multi_thread")]
async fn two_running_children_editing_one_file_flag_each_other() {
    let w = TestService::start().await;
    let project = w.add_project("shop-web").await;
    let repo = PathBuf::from(&project.repo_path);
    write(&repo, "lib/wishlist.ts", "export {}\n");
    commit(&repo, "lib/wishlist.ts", "export {}\n");
    let epic = epic(&w).await;
    let (api, api_wt) = running(&w, &epic, "Wishlist API").await;
    let (page, page_wt) = running(&w, &epic, "Wishlist page").await;
    let (button, button_wt) = running(&w, &epic, "Wishlist button").await;

    commit(&api_wt, "lib/wishlist.ts", "export const api = 1;\n");
    write(&button_wt, "lib/wishlist.ts", "export const button = 1;\n");
    write(&page_wt, "src/page.tsx", "page\n");
    let deliveries = count(&w, "SELECT COUNT(*) FROM pending_deliveries").await;
    let events = count(&w, "SELECT COUNT(*) FROM events").await;

    let changed = refresh(&w.board).await.unwrap();
    let mut changed_keys: Vec<String> = changed.iter().map(|(_, k)| k.to_string()).collect();
    changed_keys.sort();
    assert_eq!(
        changed_keys,
        vec![api.key.to_string(), button.key.to_string()]
    );

    let file = vec!["lib/wishlist.ts".to_owned()];
    assert_eq!(
        same_file(&w, &api).await,
        vec![SameFile {
            other: button.key.clone(),
            files: file.clone(),
        }]
    );
    assert_eq!(
        same_file(&w, &button).await,
        vec![SameFile {
            other: api.key.clone(),
            files: file.clone(),
        }]
    );
    assert!(same_file(&w, &page).await.is_empty());
    assert_eq!(
        overlaps(&w.board, &epic.key.to_string()).await.unwrap(),
        vec![Overlap {
            a: api.key.clone(),
            b: button.key.clone(),
            files: file,
        }]
    );

    // It wakes no one and pauses no one: no delivery, no History row, both still working.
    assert_eq!(
        count(&w, "SELECT COUNT(*) FROM pending_deliveries").await,
        deliveries
    );
    assert_eq!(count(&w, "SELECT COUNT(*) FROM events").await, events);
    assert_eq!(w.status(&api.key.to_string()).await, Status::Working);
    assert_eq!(w.status(&button.key.to_string()).await, Status::Working);
    // A second refresh changes nothing.
    assert!(refresh(&w.board).await.unwrap().is_empty());
}

/// An untracked new file counts; an ignored one doesn't; children of another epic never overlap.
#[tokio::test(flavor = "multi_thread")]
async fn untracked_files_count_and_ignored_ones_and_other_epics_dont() {
    let w = TestService::start().await;
    let project = w.add_project("shop-web").await;
    let repo = PathBuf::from(&project.repo_path);
    commit(&repo, ".gitignore", "dist/\n");
    let wishlist = epic(&w).await;
    let search = epic(&w).await;
    let (a, a_wt) = running(&w, &wishlist, "Wishlist share link").await;
    let (b, b_wt) = running(&w, &wishlist, "Wishlist export").await;
    let (c, c_wt) = running(&w, &search, "Search index API").await;
    write(&a_wt, "lib/share.ts", "a\n");
    write(&b_wt, "lib/share.ts", "b\n");
    write(&c_wt, "lib/share.ts", "c\n");
    write(&a_wt, "dist/bundle.js", "a\n");
    write(&b_wt, "dist/bundle.js", "b\n");

    refresh(&w.board).await.unwrap();
    let flag = same_file(&w, &a).await;
    assert_eq!(flag.len(), 1);
    assert_eq!(flag[0].other, b.key);
    assert_eq!(flag[0].files, vec!["lib/share.ts".to_owned()]);
    assert!(same_file(&w, &c).await.is_empty());
}

/// A child that merged or was parked drops out, and so does its partner's flag.
#[tokio::test(flavor = "multi_thread")]
async fn a_child_that_merged_or_parked_drops_out() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let epic = epic(&w).await;
    let (a, a_wt) = running(&w, &epic, "Wishlist API").await;
    let (b, b_wt) = running(&w, &epic, "Wishlist button").await;
    let (c, c_wt) = running(&w, &epic, "Wishlist page").await;
    for wt in [&a_wt, &b_wt, &c_wt] {
        write(wt, "lib/wishlist.ts", "edit\n");
    }
    refresh(&w.board).await.unwrap();
    assert_eq!(same_file(&w, &a).await.len(), 2);

    w.force_status(&b, Status::Merged).await;
    w.review
        .park(&c.key.to_string(), true, &me())
        .await
        .unwrap();
    let mut changed: Vec<String> = refresh(&w.board)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, k)| k.to_string())
        .collect();
    changed.sort();
    assert_eq!(
        changed,
        vec![a.key.to_string(), b.key.to_string(), c.key.to_string()]
    );
    assert!(same_file(&w, &a).await.is_empty());
    assert!(same_file(&w, &b).await.is_empty());
    assert!(same_file(&w, &c).await.is_empty());
    assert!(
        overlaps(&w.board, &epic.key.to_string())
            .await
            .unwrap()
            .is_empty()
    );
}

/// The files are read once and reused while HEAD, the epic branch and the index stay; a commit, a stage, a move
/// of the epic branch or the child's own command reads them again.
#[tokio::test(flavor = "multi_thread")]
async fn the_cache_skips_recomputing_when_nothing_changed() {
    let w = TestService::start().await;
    let project = w.add_project("shop-web").await;
    let repo = PathBuf::from(&project.repo_path);
    let epic = epic(&w).await;
    let (a, a_wt) = running(&w, &epic, "Wishlist API").await;
    let (b, b_wt) = running(&w, &epic, "Wishlist button").await;
    let cache = w.board.overlaps();

    refresh(&w.board).await.unwrap();
    assert_eq!(cache.recomputes(), 2);
    refresh(&w.board).await.unwrap();
    refresh(&w.board).await.unwrap();
    assert_eq!(cache.recomputes(), 2, "nothing changed");

    // An unstaged edit moves nothing git keeps: it waits for the child's next command or turn end.
    write(&a_wt, "lib/wishlist.ts", "a\n");
    write(&b_wt, "lib/wishlist.ts", "b\n");
    refresh(&w.board).await.unwrap();
    assert_eq!(cache.recomputes(), 2);
    assert!(same_file(&w, &a).await.is_empty());
    cache.touch(&a.id);
    cache.touch(&b.id);
    refresh(&w.board).await.unwrap();
    assert_eq!(cache.recomputes(), 4);
    assert_eq!(same_file(&w, &a).await.len(), 1);

    commit(&a_wt, "lib/wishlist.ts", "a2\n");
    refresh(&w.board).await.unwrap();
    assert_eq!(cache.recomputes(), 5, "a's HEAD moved");

    git(&b_wt, &["add", "lib/wishlist.ts"]);
    refresh(&w.board).await.unwrap();
    assert_eq!(cache.recomputes(), 6, "b's index changed");

    // A merge into the epic branch reads both again.
    let branch = naming::branch(&epic.key.to_string());
    git(&repo, &["checkout", "--quiet", &branch]);
    commit(&repo, "merged.txt", "merged\n");
    git(&repo, &["checkout", "--quiet", "main"]);
    refresh(&w.board).await.unwrap();
    assert_eq!(cache.recomputes(), 8);
    refresh(&w.board).await.unwrap();
    assert_eq!(cache.recomputes(), 8);
}
