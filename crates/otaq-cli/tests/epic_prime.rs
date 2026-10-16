//! `otaq prime --epic` (T-0283, LEAD-03): the epic named with the flag or by `$OTAQ_EPIC` (the lead's session sets
//! it), as Markdown or `--json`. Against a service with the sample board (SHOP-40 Wishlist: 40.1 Merged, 40.2
//! Working, 40.3 Backlog).

use std::process::{Command, Output};

use otaq_core::data_dir::DataDir;
use otaq_core::naming;
use otaq_core::service::testing::TestService;
use serde_json::Value;

const CB: &str = env!("CARGO_BIN_EXE_otaq");

fn cb(w: &TestService, args: &[&str], epic_env: Option<&str>) -> Output {
    let mut command = Command::new(CB);
    command
        .args(args)
        .env(DataDir::env_var(), w.data.root())
        .env_remove(naming::ticket_env_var())
        .env_remove(naming::epic_env_var())
        .current_dir(w.home.path());
    if let Some(epic) = epic_env {
        command.env(naming::epic_env_var(), epic);
    }
    command.output().unwrap()
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

#[tokio::test(flavor = "multi_thread")]
async fn prime_epic_by_flag_by_env_and_as_json() {
    let w = TestService::start().await;
    w.shop_sample().await;

    let text = ok(&cb(&w, &["prime", "--epic", "SHOP-40"], None));
    assert!(
        text.starts_with("# Epic lead · SHOP-40 · Wishlist\n"),
        "{text}"
    );
    assert!(
        text.contains(
            "- SHOP-40.3 · Wishlist page (Backlog · P3 · Normal) · never started · added by you\n"
        ),
        "{text}"
    );
    assert!(text.contains(
        "You may change only never-started children of SHOP-40; name a written source for every answer, note and mark."
    ));
    // A bare `--epic` in the lead's session reads $OTAQ_EPIC.
    assert_eq!(ok(&cb(&w, &["prime", "--epic"], Some("SHOP-40"))), text);

    let json: Value = serde_json::from_str(&ok(&cb(
        &w,
        &["prime", "--epic", "SHOP-40", "--json"],
        None,
    )))
    .unwrap();
    assert_eq!(json["epic"]["key"], "SHOP-40");
    assert_eq!(json["epic"]["external_id"], "PAY-123");
    assert_eq!(json["lead"], "Wishlist lead");
    let keys: Vec<&str> = json["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["SHOP-40.1", "SHOP-40.2", "SHOP-40.3"]);
    assert_eq!(json["children"][2]["started"], false);

    // Without a key or $OTAQ_EPIC it says what to do; a ticket that isn't an epic is refused.
    let none = cb(&w, &["prime", "--epic"], None);
    assert!(!none.status.success());
    assert!(
        String::from_utf8_lossy(&none.stderr).contains("name an epic (--epic SHOP-40)"),
        "{}",
        String::from_utf8_lossy(&none.stderr)
    );
    let plain = cb(&w, &["prime", "--epic", "SHOP-18"], None);
    assert!(!plain.status.success());
    assert!(String::from_utf8_lossy(&plain.stderr).contains("SHOP-18 isn't an epic"));
    w.stop().await;
}
