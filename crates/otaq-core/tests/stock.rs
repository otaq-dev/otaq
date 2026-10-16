//! The stock of builds (T-0306; M19-a, d0085) against a running service in a temporary data folder: the limit
//! following the tickets that wait for a slot, eviction of the build farthest from the base branch's tip, the nearest
//! build claimed by one ticket only, and a disk that can clone keeping one build, forward only. The probe is forced,
//! so builds are renamed (Move) or cloned (Clone) whatever the test disk can do; builds are fake: files the test
//! writes.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use otaq_core::board::settings::{AgentLimits, GlobalSettings};
use otaq_core::domain::{Actor, Status};
use otaq_core::git::testing::{commit, git};
use otaq_core::service::testing::TestService;
use otaq_core::store::projects::Project;
use otaq_core::ticket_env::handoff::HandoffMode;
use otaq_core::ticket_env::stock::{Added, Stock};
use otaq_core::warm::clone::{CloneProbe, CloneSupport};
use otaq_core::warm::store::WarmRecord;

fn support(available: bool) -> CloneSupport {
    CloneSupport {
        available,
        file_system: if available { "apfs" } else { "exfat" }.into(),
        volume: "/Volumes/Test".into(),
    }
}

/// A service whose disk can clone (`true`) or not, with project SHOP (`shop-web`) and its stock.
async fn world(available: bool) -> (TestService, Project, Stock) {
    let w = TestService::start_with(|config| otaq_core::service::ServiceConfig {
        clone_probe: Arc::new(CloneProbe::forced(support(available))),
        ..config
    })
    .await;
    let shop = w.add_project("shop-web").await;
    let stock = Stock::new(w.warm.clone(), w.board.clone());
    let mode = if available {
        HandoffMode::Clone
    } else {
        HandoffMode::Move
    };
    assert_eq!(w.warm.mode(), mode);
    (w, shop, stock)
}

fn head(repo: &Path) -> String {
    git(repo, &["rev-parse", "HEAD"]).trim().to_owned()
}

/// Commits `count` more files on the base branch; the commits, oldest first.
fn commits(shop: &Project, count: usize) -> Vec<String> {
    let repo = Path::new(&shop.repo_path);
    (0..count)
        .map(|i| {
            commit(repo, &format!("f{i}.txt"), "x");
            head(repo)
        })
        .collect()
}

/// Adds a fake build (`out/app.bin` holding `from`) made at `built_at`, kept from `from` at `kept_at`.
async fn add(
    w: &TestService,
    stock: &Stock,
    shop: &Project,
    built_at: &str,
    from: &str,
    kept_at: i64,
) -> Added {
    let incoming = w.warm.incoming(shop).await.unwrap();
    std::fs::create_dir_all(incoming.path().join("out")).unwrap();
    std::fs::write(incoming.path().join("out").join("app.bin"), from).unwrap();
    let record = WarmRecord {
        built_at: built_at.to_owned(),
        base: built_at.to_owned(),
        from: from.to_owned(),
        kept_at,
        project: String::new(),
    };
    stock.add(shop, incoming, record).await.unwrap()
}

/// The stock's entries: number and the ticket each was kept from.
fn entries(w: &TestService, shop: &Project) -> Vec<(u64, String)> {
    let parent = w.warm.path(shop).unwrap();
    let mut found: Vec<(u64, String)> = std::fs::read_dir(&parent)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().parse().ok()?;
            let from = std::fs::read_to_string(e.path().join("out").join("app.bin")).ok()?;
            Some((n, from))
        })
        .collect();
    found.sort();
    found
}

fn trash_empty(w: &TestService) -> bool {
    std::fs::read_dir(w.data.warm_trash_path("shop-web").unwrap())
        .map(|mut d| d.next().is_none())
        .unwrap_or(true)
}

async fn until(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("{what}");
}

/// Agent limits on with one slot, taken by a Working ticket (SHOP-1), and `count` Ready tickets waiting for a slot
/// (SHOP-2…). The scheduler keeps them so: no slot is free.
async fn waiting_for_a_slot(w: &TestService, count: usize) {
    w.board
        .set_global_settings(&GlobalSettings {
            agent_limits: AgentLimits {
                enabled: true,
                max_concurrency: 1,
                ..AgentLimits::default()
            },
            ..GlobalSettings::default()
        })
        .await
        .unwrap();
    let working = w.create("SHOP", "working").await;
    w.force_status(&working, Status::Working).await;
    for i in 0..count {
        let ticket = w.create("SHOP", &format!("waiting {i}")).await;
        w.force_status(&ticket, Status::WaitingForSlot).await;
    }
}

#[tokio::test]
async fn move_mode_always_adds_then_enforces_the_limit() {
    let (w, shop, stock) = world(false).await;
    let made = commits(&shop, 4);
    // Nothing waits: the limit is 1.
    assert_eq!(stock.limit("shop-web").await.unwrap(), 1);

    assert_eq!(
        add(&w, &stock, &shop, &made[0], "SHOP-10", 10).await,
        Added::Kept { entry: 0 }
    );
    // At the tip: the one before it is farther, so it goes.
    assert_eq!(
        add(&w, &stock, &shop, &made[3], "SHOP-11", 20).await,
        Added::Kept { entry: 1 }
    );
    assert_eq!(entries(&w, &shop), [(1, "SHOP-11".into())]);
    // Farther from the tip than the one there: added, then evicted at once.
    assert_eq!(
        add(&w, &stock, &shop, &made[1], "SHOP-12", 30).await,
        Added::Evicted { entry: 2 }
    );
    assert_eq!(entries(&w, &shop), [(1, "SHOP-11".into())]);
    until("the evicted builds deleted", || trash_empty(&w)).await;
    // Within the limit nothing more goes.
    assert!(stock.enforce_limit("shop-web").await.unwrap().is_empty());

    // Turned off, nothing is added.
    let mut settings = w.board.project_settings("SHOP").await.unwrap();
    settings.warm_builds = false;
    w.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    assert_eq!(
        add(&w, &stock, &shop, &made[3], "SHOP-13", 40).await,
        Added::Dropped
    );
    assert_eq!(entries(&w, &shop), [(1, "SHOP-11".into())]);
    w.stop().await;
}

#[tokio::test]
async fn the_limit_follows_waiting_for_slot_tickets() {
    let (w, shop, stock) = world(false).await;
    waiting_for_a_slot(&w, 2).await;
    assert_eq!(stock.limit("shop-web").await.unwrap(), 2);

    // Two builds stay; Settings and Cleanup see the limit.
    let made = commits(&shop, 3);
    for (i, built_at) in made.iter().enumerate() {
        add(
            &w,
            &stock,
            &shop,
            built_at,
            &format!("SHOP-{}", 20 + i),
            i as i64,
        )
        .await;
    }
    assert_eq!(
        entries(&w, &shop),
        [(1, "SHOP-21".into()), (2, "SHOP-22".into())]
    );
    let listed = w.warm.list().await.unwrap();
    assert_eq!(listed.mode, HandoffMode::Move);
    assert_eq!(listed.builds[0].limit, Some(2));
    assert_eq!(listed.builds[0].entries.len(), 2);

    // One of them starts (it no longer waits): the limit goes down, and the next pass applies it.
    let tickets = w.board.tickets("SHOP").await.unwrap();
    let started = tickets
        .iter()
        .find(|t| t.status == Status::WaitingForSlot)
        .unwrap();
    w.force_status(started, Status::Working).await;
    assert_eq!(stock.limit("shop-web").await.unwrap(), 1);
    // A pass whose caller gives up midway doesn't stop the next one.
    let evicted = match tokio::time::timeout(Duration::ZERO, stock.enforce_limit("shop-web")).await
    {
        Ok(done) => done.unwrap(),
        Err(_cancelled) => stock.enforce_limit("shop-web").await.unwrap(),
    };
    assert_eq!(evicted, [1]);
    assert_eq!(entries(&w, &shop), [(2, "SHOP-22".into())]);
    w.stop().await;
}

#[tokio::test]
async fn a_ticket_waiting_for_a_dependency_is_not_counted() {
    let (w, _shop, stock) = world(false).await;
    waiting_for_a_slot(&w, 2).await;
    // SHOP-4 depends on SHOP-1, still Working: it can't start soon, wherever it waits.
    let child = w.create("SHOP", "child").await;
    w.board
        .add_dependency(&child.key.to_string(), "SHOP-1", &Actor::User("me".into()))
        .await
        .unwrap();
    w.force_status(&child, Status::WaitingForSlot).await;
    assert_eq!(stock.limit("shop-web").await.unwrap(), 2);
    let tickets = w.board.tickets("SHOP").await.unwrap();
    let waiting: HashSet<String> = tickets
        .iter()
        .filter(|t| t.status == Status::WaitingForSlot)
        .map(|t| t.key.to_string())
        .collect();
    assert!(
        waiting.contains("SHOP-2") && waiting.contains("SHOP-3"),
        "{waiting:?}"
    );
    w.stop().await;
}

#[tokio::test]
async fn the_farthest_entry_is_evicted_when_over_the_limit() {
    let (w, shop, stock) = world(false).await;
    waiting_for_a_slot(&w, 2).await;
    // Nine commits on the base: a build at made[k] is 8 - k files from its tip.
    let made = commits(&shop, 9);
    let (one, five, nine) = (&made[7], &made[3], {
        // Nine files from the tip: the commit before the first one.
        &git(Path::new(&shop.repo_path), &["rev-parse", "HEAD~9"])
            .trim()
            .to_owned()
    });
    // Kept in an order that isn't the distance's: the oldest is not the farthest.
    assert_eq!(
        add(&w, &stock, &shop, five, "SHOP-30", 100).await,
        Added::Kept { entry: 0 }
    );
    assert_eq!(
        add(&w, &stock, &shop, nine, "SHOP-31", 200).await,
        Added::Kept { entry: 1 }
    );
    assert_eq!(
        add(&w, &stock, &shop, one, "SHOP-32", 300).await,
        Added::Kept { entry: 2 }
    );
    assert_eq!(
        entries(&w, &shop),
        [(0, "SHOP-30".into()), (2, "SHOP-32".into())]
    );
    until("the evicted build deleted", || trash_empty(&w)).await;
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_claims_never_share_an_entry() {
    let (w, shop, stock) = world(false).await;
    waiting_for_a_slot(&w, 2).await;
    let made = commits(&shop, 3);
    add(&w, &stock, &shop, &made[0], "SHOP-40", 1).await;
    add(&w, &stock, &shop, &made[1], "SHOP-41", 2).await;
    assert_eq!(entries(&w, &shop).len(), 2);
    let tip = made[2].clone();

    // The nearest first: an ancestor of the tip with fewer changed files.
    assert_eq!(stock.ranked(&shop, &tip, None).await, [1, 0]);
    // A ticket's own build comes first when it counts.
    assert_eq!(stock.ranked(&shop, &tip, Some("SHOP-40")).await, [0, 1]);

    let tasks: Vec<_> = ["SHOP-50", "SHOP-51", "SHOP-52"]
        .into_iter()
        .map(|ticket| {
            let (stock, shop, tip) = (stock.clone(), shop.clone(), tip.clone());
            tokio::spawn(async move { stock.claim_nearest(&shop, ticket, &tip, false).await })
        })
        .collect();
    let mut claims = Vec::new();
    for task in tasks {
        claims.push(task.await.unwrap());
    }
    let got: Vec<u64> = claims.iter().flatten().map(|c| c.entry()).collect();
    assert_eq!(got.len(), 2, "two win, one finds none: {got:?}");
    assert_eq!(got.iter().collect::<HashSet<_>>().len(), 2, "{got:?}");
    // A claimed entry is never evicted, even over the limit.
    let tickets = w.board.tickets("SHOP").await.unwrap();
    for t in tickets
        .iter()
        .filter(|t| t.status == Status::WaitingForSlot)
    {
        w.force_status(t, Status::Working).await;
    }
    assert!(stock.enforce_limit("shop-web").await.unwrap().is_empty());
    // Released, the one farther from the tip goes.
    drop(claims);
    assert_eq!(stock.enforce_limit("shop-web").await.unwrap(), [0]);
    assert!(
        stock
            .claim_nearest(&shop, "SHOP-53", &tip, false)
            .await
            .is_some()
    );
    w.stop().await;
}

#[tokio::test]
async fn clone_mode_keeps_one_entry_forward_only() {
    let (w, shop, stock) = world(true).await;
    // Tickets waiting don't matter where builds are cloned: one entry.
    waiting_for_a_slot(&w, 3).await;
    assert_eq!(stock.limit("shop-web").await.unwrap(), 1);
    let made = commits(&shop, 2);
    assert_eq!(
        add(&w, &stock, &shop, &made[0], "SHOP-60", 1).await,
        Added::Kept { entry: 0 }
    );
    assert_eq!(
        add(&w, &stock, &shop, &made[1], "SHOP-61", 2).await,
        Added::Kept { entry: 1 }
    );
    // An older build never replaces a newer one.
    assert_eq!(
        add(&w, &stock, &shop, &made[0], "SHOP-62", 3).await,
        Added::Dropped
    );
    assert_eq!(entries(&w, &shop), [(1, "SHOP-61".into())]);
    assert!(stock.enforce_limit("shop-web").await.unwrap().is_empty());
    let listed = w.warm.list().await.unwrap();
    assert_eq!(listed.mode, HandoffMode::Clone);
    assert_eq!(listed.builds[0].limit, Some(1));
    until("the old build deleted", || trash_empty(&w)).await;
    w.stop().await;
}
