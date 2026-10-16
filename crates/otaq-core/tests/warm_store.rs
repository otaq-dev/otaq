//! The warm build store (T-0296; WRM-02, WRM-05…WRM-07, d0082) against a running service in a temporary data folder:
//! the switch, replace (first, forward, an older one dropped), Clear waiting for a copy, the switch turned off,
//! `warm.list`, and the temporary folders a crash left. Builds are fake: files the test writes. The probe is forced,
//! so nothing here depends on the test disk being able to clone (the store only renames).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use otaq_core::client::{Client, ClientConfig};
use otaq_core::git::testing::{TempRepos, commit, git};
use otaq_core::protocol::{
    Event, ProjectParams, ProjectSettingsParams, Reply, Request, WarmClearParams,
};
use otaq_core::service::testing::TestService;
use otaq_core::store::projects::Project;
use otaq_core::warm::clone::{CloneProbe, CloneSupport};
use otaq_core::warm::store::{
    ClearOutcome, Replaced, WarmBuildInfo, WarmList, WarmRecord, record_file_name,
};
use tokio::sync::broadcast;

fn support(available: bool) -> CloneSupport {
    CloneSupport {
        available,
        file_system: if available { "apfs" } else { "exfat" }.into(),
        volume: "/Volumes/Test".into(),
    }
}

async fn world(available: bool) -> TestService {
    TestService::start_with(|config| otaq_core::service::ServiceConfig {
        clone_probe: Arc::new(CloneProbe::forced(support(available))),
        ..config
    })
    .await
}

fn head(repo: &Path) -> String {
    git(repo, &["rev-parse", "HEAD"]).trim().to_owned()
}

fn record(base: &str, from: &str) -> WarmRecord {
    WarmRecord {
        built_at: base.to_owned(),
        base: base.to_owned(),
        from: from.to_owned(),
        kept_at: 1_760_000_000_000,
        project: String::new(),
    }
}

/// Keeps a fake build (`out/app.bin` holding `content`) for `project` as `from` with this base.
async fn keep(
    w: &TestService,
    project: &Project,
    base: &str,
    from: &str,
    content: &str,
) -> Replaced {
    let incoming = w.warm.incoming(project).await.unwrap();
    std::fs::create_dir_all(incoming.path().join("out")).unwrap();
    std::fs::write(incoming.path().join("out").join("app.bin"), content).unwrap();
    w.warm
        .replace(project, incoming, record(base, from))
        .await
        .unwrap()
}

fn built(w: &TestService, project: &Project) -> Option<String> {
    std::fs::read_to_string(w.warm.path(project).unwrap().join("out").join("app.bin")).ok()
}

async fn client(w: &TestService) -> Client {
    Client::connect(ClientConfig::new(&w.data)).await.unwrap()
}

async fn list(client: &Client) -> WarmList {
    match client.request(Request::WarmList).await.unwrap() {
        Reply::WarmList(list) => *list,
        other => panic!("{other:?}"),
    }
}

async fn warm_event(events: &mut broadcast::Receiver<Event>) -> Event {
    loop {
        let event = tokio::time::timeout(Duration::from_secs(10), events.recv())
            .await
            .expect("an event in time")
            .unwrap();
        if matches!(event, Event::WarmBuildChanged { .. }) {
            return event;
        }
    }
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

fn changed(project: &str) -> Event {
    Event::WarmBuildChanged {
        repo: "shop-web".into(),
        project: Some(project.into()),
    }
}

#[tokio::test]
async fn warm_builds_are_on_by_default_where_the_disk_can_clone() {
    let w = world(true).await;
    let shop = w.add_project("shop-web").await;
    assert!(w.warm.enabled("SHOP").await.unwrap());
    let mut settings = w.board.project_settings("SHOP").await.unwrap();
    assert!(settings.warm_builds);
    settings.warm_builds = false;
    w.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    assert!(!w.warm.enabled("SHOP").await.unwrap());
    assert_eq!(
        w.warm.path(&shop).unwrap(),
        w.data.root().join("warm").join("shop-web")
    );
    w.stop().await;

    // A disk that can't clone: off whatever the switch says, and the reason is in `warm.list`.
    let w = world(false).await;
    w.add_project("shop-web").await;
    assert!(w.board.project_settings("SHOP").await.unwrap().warm_builds);
    assert!(!w.warm.enabled("SHOP").await.unwrap());
    let c = client(&w).await;
    let listed = list(&c).await;
    assert_eq!(listed.support, support(false));
    assert!(listed.builds.is_empty());
    w.stop().await;
}

#[tokio::test]
async fn replace_keeps_the_first_and_newer_builds_and_drops_older_ones() {
    let w = world(true).await;
    let shop = w.add_project("shop-web").await;
    let repo = Path::new(&shop.repo_path);
    let first = head(repo);
    commit(repo, "a.txt", "a");
    let second = head(repo);
    commit(repo, "b.txt", "b");
    let third = head(repo);
    let mut events = w.events.subscribe();

    // The first build is kept, with its record.
    assert_eq!(
        keep(&w, &shop, &second, "SHOP-21", "second").await,
        Replaced::Kept
    );
    assert_eq!(warm_event(&mut events).await, changed("SHOP"));
    assert_eq!(built(&w, &shop).as_deref(), Some("second"));
    let kept = w.warm.record(&shop).await.unwrap();
    assert_eq!(
        (kept.base.as_str(), kept.from.as_str()),
        (second.as_str(), "SHOP-21")
    );
    assert_eq!(kept.project, shop.id);

    // An older one never replaces it; the dropped files are deleted.
    assert_eq!(
        keep(&w, &shop, &first, "SHOP-19", "first").await,
        Replaced::Dropped
    );
    assert_eq!(built(&w, &shop).as_deref(), Some("second"));
    let incoming = w.data.warm_dir().join("shop-web.incoming");
    until("the dropped build deleted", || !incoming.exists()).await;

    // The same base, then a newer one, replace it; the old folder goes in the background.
    assert_eq!(
        keep(&w, &shop, &second, "SHOP-22", "again").await,
        Replaced::Kept
    );
    assert_eq!(
        keep(&w, &shop, &third, "SHOP-23", "third").await,
        Replaced::Kept
    );
    assert_eq!(built(&w, &shop).as_deref(), Some("third"));
    assert_eq!(w.warm.record(&shop).await.unwrap().from, "SHOP-23");
    let old = w.data.warm_dir().join("shop-web.old");
    until("the old build deleted", || !old.exists()).await;

    // Nothing is kept once warm builds are off.
    let mut settings = w.board.project_settings("SHOP").await.unwrap();
    settings.warm_builds = false;
    w.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    commit(repo, "c.txt", "c");
    let fourth = head(repo);
    assert_eq!(
        keep(&w, &shop, &fourth, "SHOP-24", "fourth").await,
        Replaced::Dropped
    );
    assert_eq!(built(&w, &shop).as_deref(), Some("third"));
    w.stop().await;
}

#[tokio::test]
async fn clear_waits_for_a_running_copy_then_clears() {
    let w = world(true).await;
    let shop = w.add_project("shop-web").await;
    let base = head(Path::new(&shop.repo_path));
    keep(&w, &shop, &base, "SHOP-20", "built").await;
    let c = client(&w).await;
    let mut events = w.events.subscribe();

    // SHOP-21's new worktree is being seeded from it.
    let seed = w
        .warm
        .read(&shop, "SHOP-21")
        .await
        .expect("a warm build to seed from");
    assert_eq!(seed.record().unwrap().from, "SHOP-20");
    assert_eq!(seed.path(), w.warm.path(&shop).unwrap());

    let clear = Request::WarmClear(WarmClearParams {
        project: Some("SHOP".into()),
        repo: None,
    });
    let Reply::WarmClear(outcome) = c.request(clear.clone()).await.unwrap() else {
        panic!()
    };
    assert_eq!(
        outcome,
        ClearOutcome::Waiting {
            copying: vec!["SHOP-21".into()]
        }
    );
    assert_eq!(warm_event(&mut events).await, changed("SHOP"));
    // "Clearing… (waiting for SHOP-21's copy)": the copy goes on, nothing is cleared yet.
    let listed = list(&c).await;
    let [build]: [WarmBuildInfo; 1] = listed.builds.try_into().unwrap();
    assert_eq!(build.project.as_deref(), Some("SHOP"));
    assert_eq!(build.status.copying, ["SHOP-21"]);
    assert!(build.status.clearing);
    assert!(built(&w, &shop).is_some());
    // A second Clear joins the waiting one.
    let Reply::WarmClear(again) = c.request(clear).await.unwrap() else {
        panic!()
    };
    assert!(matches!(again, ClearOutcome::Waiting { .. }));

    // A new seed waits behind the Clear, then finds nothing to seed from.
    let next = {
        let (warm, shop) = (w.warm.clone(), shop.clone());
        tokio::spawn(async move { warm.read(&shop, "SHOP-22").await.is_some() })
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!next.is_finished());

    // The copy ends: the Clear goes on.
    drop(seed);
    assert_eq!(warm_event(&mut events).await, changed("SHOP"));
    assert!(built(&w, &shop).is_none());
    assert!(!next.await.unwrap());
    let old = w.data.warm_dir().join("shop-web.old");
    until("the cleared build deleted", || !old.exists()).await;
    assert!(list(&c).await.builds.is_empty());
    // Clearing never turns the switch off.
    assert!(w.board.project_settings("SHOP").await.unwrap().warm_builds);
    // Nothing left to clear.
    let Reply::WarmClear(outcome) = c
        .request(Request::WarmClear(WarmClearParams {
            project: Some("SHOP".into()),
            repo: None,
        }))
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(outcome, ClearOutcome::Nothing);
    w.stop().await;
}

#[tokio::test]
async fn turning_the_switch_off_clears_the_warm_build() {
    let w = world(true).await;
    let shop = w.add_project("shop-web").await;
    let base = head(Path::new(&shop.repo_path));
    keep(&w, &shop, &base, "SHOP-20", "built").await;
    let c = client(&w).await;
    let mut events = w.events.subscribe();
    let Reply::ProjectSettings(reply) = c
        .request(Request::ProjectSettingsGet(ProjectParams {
            project: "SHOP".into(),
        }))
        .await
        .unwrap()
    else {
        panic!()
    };
    let mut settings = reply.settings;
    settings.warm_builds = false;
    c.request(Request::ProjectSettingsSet(ProjectSettingsParams {
        project: "SHOP".into(),
        settings,
    }))
    .await
    .unwrap();
    assert_eq!(warm_event(&mut events).await, changed("SHOP"));
    assert!(built(&w, &shop).is_none());
    assert!(!w.board.project_settings("SHOP").await.unwrap().warm_builds);
    w.stop().await;
}

#[tokio::test]
async fn warm_list_shows_each_build_with_or_without_a_record() {
    let w = world(true).await;
    let shop = w.add_project("shop-web").await;
    let base = head(Path::new(&shop.repo_path));
    keep(&w, &shop, &base, "SHOP-21", "12345").await;
    // A folder no project owns, without a record, and the store's own folders, which never list.
    let warm = w.data.warm_dir();
    std::fs::create_dir_all(warm.join("old-app").join("deps")).unwrap();
    std::fs::write(warm.join("old-app").join("deps").join("lib"), "abc").unwrap();
    std::fs::create_dir_all(warm.join("shop-web.old")).unwrap();
    std::fs::create_dir_all(warm.join(".probe")).unwrap();

    let c = client(&w).await;
    let listed = list(&c).await;
    assert_eq!(listed.support, support(true));
    let [old_app, shop_web]: [WarmBuildInfo; 2] = listed.builds.try_into().unwrap();
    assert_eq!(old_app.repo, "old-app");
    assert_eq!(old_app.project, None);
    assert_eq!(old_app.size_bytes, 3);
    assert_eq!(
        (old_app.built_at, old_app.from, old_app.kept_at),
        (None, None, None)
    );

    assert_eq!(shop_web.repo, "shop-web");
    assert_eq!(shop_web.project.as_deref(), Some("SHOP"));
    assert_eq!(shop_web.built_at.as_deref(), Some(&base[..7]));
    assert_eq!(shop_web.from.as_deref(), Some("SHOP-21"));
    assert_eq!(shop_web.kept_at, Some(1_760_000_000_000));
    let record_size = std::fs::metadata(w.warm.path(&shop).unwrap().join(record_file_name()))
        .unwrap()
        .len();
    assert_eq!(shop_web.size_bytes, 5 + record_size);
    assert!(shop_web.status.copying.is_empty() && !shop_web.status.clearing);

    // A folder no project owns is cleared by its name.
    let Reply::WarmClear(outcome) = c
        .request(Request::WarmClear(WarmClearParams {
            project: None,
            repo: Some("old-app".into()),
        }))
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(outcome, ClearOutcome::Cleared);
    assert!(!warm.join("old-app").exists());
    w.stop().await;
}

#[tokio::test]
async fn folders_a_crash_left_are_removed_at_start() {
    let home = tempfile::TempDir::new().unwrap();
    let warm = home.path().join("h").join("warm");
    for folder in ["shop-web", "shop-web.incoming", "shop-web.old"] {
        std::fs::create_dir_all(warm.join(folder).join("out")).unwrap();
        std::fs::write(warm.join(folder).join("out").join("app.bin"), "x").unwrap();
    }
    let w = TestService::start_in(home, TempRepos::new(), |config| {
        otaq_core::service::ServiceConfig {
            clone_probe: Arc::new(CloneProbe::forced(support(true))),
            ..config
        }
    })
    .await;
    assert!(warm.join("shop-web").join("out").join("app.bin").is_file());
    assert!(!warm.join("shop-web.incoming").exists());
    assert!(!warm.join("shop-web.old").exists());
    w.stop().await;
}
