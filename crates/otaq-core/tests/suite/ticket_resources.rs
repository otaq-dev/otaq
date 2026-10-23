//! Ticket resources and the project's template (M19-d, d0087): a Template refresh holds the project's template lock
//! for writing, so a start's Create waits until it is done, and two refreshes of one project run one at a time.

use crate::common::resources as resources_common;

use std::time::Duration;

use otaq_core::agent_tools::LaunchContext;
use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::board::settings::ProjectSettings;
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Actor, Status};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use otaq_core::start::StartOutcome;
use resources_common::{appears, append, by_os, lines, resource_row, sleep, then};

fn me() -> Actor {
    Actor::User("me".into())
}

/// An ordinary command standing in for `claude`: prints a line, then waits like an interactive program.
fn stand_in() -> LaunchContext {
    LaunchContext {
        bin_dir: "/opt/cb/bin".into(),
        plugin_dir: "/opt/cb/plugin".into(),
        path: std::env::var_os("PATH").unwrap(),
        claude: ClaudeLaunch {
            program: vec![
                "sh".into(),
                "-c".into(),
                "echo agent started; exec cat".into(),
                "sh".into(),
            ],
            config_dir: None,
        },
    }
}

/// A ticket put straight into Merged, as Done's cleanup sees it.
async fn merged(w: &TestService, title: &str) -> Ticket {
    let t = w.create("SHOP", title).await;
    w.force_status(&t, Status::Merged).await;
    w.board.ticket(&t.key.to_string()).await.unwrap()
}

/// **The M19-d acceptance**: a start's Create waits while the project's template is being refreshed. The refresh
/// marks that it began, waits a second and only then writes the file Create reads, so a Create that ran during the
/// refresh would fail.
#[tokio::test]
async fn create_waits_while_the_template_refreshes() {
    let w = TestService::start_with(|config| ServiceConfig {
        launch: Some(stand_in()),
        ..config
    })
    .await;
    w.add_project("shop-web").await;
    let began = w.home.path().join("refresh-began.txt");
    let done = w.home.path().join("template.txt");
    w.board
        .set_project_settings(
            "SHOP",
            &ProjectSettings {
                resource_create: by_os(
                    &format!("cat '{}'", done.display()),
                    &format!("type \"{}\"", done.display()),
                ),
                resource_template_refresh: then(&[
                    append(&began, "began"),
                    sleep(1),
                    append(&done, "template-ready"),
                ]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let finished = merged(&w, "Migration").await;
    let starting = w.create("SHOP", "Orders").await;
    w.force_status(&starting, Status::Queued).await;

    let review = w.review.clone();
    let refresh = tokio::spawn(async move { review.finish(&finished).await });
    appears(&began).await;
    assert!(!done.exists(), "the refresh is still under way");
    let outcome = w
        .starter
        .start("SHOP-2", &me(), otaq_core::session::Start::Auto)
        .await
        .unwrap();
    assert!(matches!(outcome, StartOutcome::Started(_)), "{outcome:?}");
    refresh.await.unwrap().unwrap();

    assert_eq!(resource_row(&w, &starting).await.1, "created");
    let log = std::fs::read_to_string(w.data.setup_log_path("SHOP-2").unwrap()).unwrap();
    assert!(
        log.contains("template-ready"),
        "Create read the template: {log}"
    );
    w.stop().await;
}

/// Two Dones of one project refresh its template one after the other, never at once.
#[tokio::test]
async fn refreshes_of_one_project_run_one_at_a_time() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let log = w.home.path().join("refreshes.txt");
    w.board
        .set_project_settings(
            "SHOP",
            &ProjectSettings {
                resource_template_refresh: then(&[
                    append(&log, "in"),
                    sleep(1),
                    append(&log, "out"),
                ]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let first = merged(&w, "One").await;
    let second = merged(&w, "Two").await;
    let (a, b) = (w.review.clone(), w.review.clone());
    let both = tokio::time::timeout(Duration::from_secs(60), async {
        tokio::join!(async move { a.finish(&first).await }, async move {
            b.finish(&second).await
        })
    })
    .await
    .expect("both refreshes ran");
    both.0.unwrap();
    both.1.unwrap();
    assert_eq!(lines(&log), ["in", "out", "in", "out"]);
    w.stop().await;
}
