//! `crewboard dep add | remove | tree | cycles` (T-0070, DEP-01, DEP-02, CLI-02) on the sample board.

use std::process::{Command, Output};

use crewboard_core::board::tickets::NewTicketInput;
use crewboard_core::data_dir::DataDir;
use crewboard_core::domain::Actor;
use crewboard_core::naming;
use crewboard_core::service::testing::TestService;
use serde_json::Value;

const CB: &str = env!("CARGO_BIN_EXE_crewboard");

fn cb(w: &TestService, args: &[&str]) -> Output {
    Command::new(CB)
        .args(args)
        .env(DataDir::env_var(), w.data.root())
        .env_remove(naming::ticket_env_var())
        .current_dir(w.home.path())
        .output()
        .unwrap()
}

fn ok(out: &Output) -> String {
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout.clone()).unwrap()
}

/// The human error line on stderr, after a non-zero exit.
fn refused(out: &Output) -> String {
    assert!(!out.status.success());
    String::from_utf8(out.stderr.clone()).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn add_remove_and_refusals() {
    let w = TestService::start().await;
    w.shop_sample().await;

    assert_eq!(
        ok(&cb(&w, &["dep", "add", "SHOP-24", "SHOP-17"])),
        "SHOP-24 now depends on SHOP-17\n"
    );
    let json: Value = serde_json::from_str(&ok(&cb(
        &w,
        &["dep", "remove", "SHOP-24", "SHOP-17", "--json"],
    )))
    .unwrap();
    assert_eq!(json["depends_on"], serde_json::json!([]));
    assert_eq!(
        ok(&cb(&w, &["-P", "SHOP", "dep", "add", "24", "17"])),
        "SHOP-24 now depends on SHOP-17\n",
        "bare numbers resolve in the current project"
    );

    // A loop names its path (DEP-01).
    let error = refused(&cb(&w, &["dep", "add", "SHOP-20", "SHOP-21"]));
    assert!(error.contains("SHOP-21 → SHOP-20 → SHOP-21"), "{error}");

    // Linked tickets must share a base (DEP-02).
    w.board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Release fix".into(),
                base_branch: Some("release/2.3".into()),
                ..Default::default()
            },
            &Actor::User("me".into()),
        )
        .await
        .unwrap();
    let error = refused(&cb(&w, &["dep", "add", "SHOP-41", "SHOP-20"]));
    assert_eq!(
        error.trim(),
        "error: SHOP-41 merges into `release/2.3`, SHOP-20 into `main`. Linked tickets must use the same base branch."
    );
    let out = cb(&w, &["dep", "add", "SHOP-41", "SHOP-20", "--json"]);
    assert!(!out.status.success());
    let e: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(e["error"]["data"]["refusal"], "cross_base");
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn tree_and_cycles() {
    let w = TestService::start().await;
    w.shop_sample().await;

    let tree = ok(&cb(&w, &["dep", "tree", "SHOP-20"]));
    let keys: Vec<String> = tree
        .lines()
        .map(|l| {
            let depth = (l.len() - l.trim_start().len()) / 2;
            format!("{depth}:{}", l.split_whitespace().next().unwrap())
        })
        .collect();
    assert_eq!(
        keys,
        [
            "0:SHOP-18",
            "1:SHOP-20",
            "2:SHOP-21",
            "0:SHOP-19",
            "1:SHOP-20",
            "2:SHOP-21"
        ],
        "{tree}"
    );
    assert!(
        tree.lines()
            .nth(1)
            .unwrap()
            .ends_with("Checkout page  ← this ticket"),
        "{tree}"
    );
    assert!(tree.lines().next().unwrap().contains("Ready for Review"));
    let rows: Value =
        serde_json::from_str(&ok(&cb(&w, &["dep", "tree", "SHOP-20", "--json"]))).unwrap();
    assert_eq!(rows[1]["current"], true);
    assert_eq!(
        ok(&cb(&w, &["dep", "tree", "SHOP-12"])),
        "No dependencies · nothing waits on SHOP-12\n"
    );

    assert_eq!(
        ok(&cb(&w, &["-P", "SHOP", "dep", "cycles"])),
        "No dependency loops in SHOP\n"
    );
    assert_eq!(
        ok(&cb(&w, &["-P", "SHOP", "dep", "cycles", "--json"])).trim(),
        "[]"
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_links_only_from_its_own_ticket() {
    let w = TestService::start().await;
    w.shop_sample().await;
    let shop19 = w.board.ticket("SHOP-19").await.unwrap();
    let agent = w.agent_shell(&shop19).await;

    let own = agent.run(&format!("'{CB}' dep add SHOP-19 SHOP-17")).await;
    assert_eq!(
        (own.status, own.stdout.as_str()),
        (0, "SHOP-19 now depends on SHOP-17\n"),
        "{own:?}"
    );
    let other = agent.run(&format!("'{CB}' dep add SHOP-24 SHOP-17")).await;
    assert_ne!(other.status, 0);
    assert!(
        other
            .stderr
            .contains("SHOP-19's agent can't change SHOP-24"),
        "{other:?}"
    );
    assert!(
        w.board
            .ticket("SHOP-24")
            .await
            .unwrap()
            .depends_on
            .is_empty()
    );
    w.stop().await;
}
