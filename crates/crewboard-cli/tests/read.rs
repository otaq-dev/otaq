//! The read commands (T-0068): `show`, `list`, `ready`, `blocked`, `project list | show` against a service in a
//! temporary data folder with the sample board (SHOP-12…SHOP-40.3).

use std::process::{Command, Output};

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

fn stdout(out: &Output) -> String {
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn json(out: &Output) -> Value {
    serde_json::from_str(&stdout(out)).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn show_reads_a_ticket_by_key_number_alias_and_variable() {
    let w = TestService::start().await;
    w.shop_sample().await;

    let text = stdout(&cb(&w, &["show", "SHOP-20"]));
    assert!(text.starts_with("SHOP-20 · Checkout page\n"), "{text}");
    assert!(text.contains("Status:      Queued (Dev)"), "{text}");
    assert!(
        text.contains("Depends on:  SHOP-18 (Ready for Review), SHOP-19 (Working)"),
        "{text}"
    );
    assert_eq!(stdout(&cb(&w, &["view", "SHOP-20"])), text, "view is show");

    let shown = json(&cb(&w, &["show", "SHOP-20", "--json"]));
    assert_eq!(shown["ticket"]["key"], "SHOP-20");
    assert_eq!(shown["ticket"]["depends_on"][1], "SHOP-19");
    assert!(shown.get("history").is_none() && shown.get("children").is_none());

    // A bare number in the current project; the ticket variable when the key is omitted.
    let by_number = json(&cb(&w, &["-P", "SHOP", "show", "18", "--json"]));
    assert_eq!(by_number["ticket"]["title"], "Checkout API");
    let out = Command::new(CB)
        .args(["show", "--json"])
        .env(DataDir::env_var(), w.data.root())
        .env(naming::ticket_env_var(), "SHOP-23")
        .output()
        .unwrap();
    assert_eq!(json(&out)["ticket"]["title"], "Split user full name");

    // An old key resolves through its alias (SHOP-41 moved into SHOP-40).
    w.create("SHOP", "Wishlist sharing").await;
    w.board
        .move_to_epic("SHOP-41", "SHOP-40", &Actor::User("me".into()))
        .await
        .unwrap();
    let moved = json(&cb(&w, &["show", "SHOP-41", "--json"]));
    assert_eq!(moved["ticket"]["key"], "SHOP-40.4");

    // --long adds the history.
    let long = json(&cb(&w, &["show", "SHOP-20", "--long", "--json"]));
    assert!(
        long["history"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "dependency_added")
    );
    assert!(stdout(&cb(&w, &["show", "SHOP-20", "--long"])).contains("History (newest first)"));
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn epics_list_their_children() {
    let w = TestService::start().await;
    w.shop_sample().await;

    let text = stdout(&cb(&w, &["show", "SHOP-40", "--children"]));
    assert!(text.contains("Status:      Epic · Backlog"), "{text}");
    for line in [
        "SHOP-40.1  P3 Merged",
        "SHOP-40.2  P3 Working",
        "SHOP-40.3  P3 Backlog",
    ] {
        assert!(text.contains(line), "{line} in\n{text}");
    }
    let children = json(&cb(&w, &["show", "SHOP-40", "--children", "--json"]));
    assert_eq!(children["children"].as_array().unwrap().len(), 3);

    let listed = json(&cb(&w, &["list", "--parent", "SHOP-40", "--json"]));
    let keys: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["SHOP-40.1", "SHOP-40.2", "SHOP-40.3"]);

    let tree = stdout(&cb(&w, &["-P", "SHOP", "list", "--tree"]));
    let epic_line = tree
        .lines()
        .position(|l| l.starts_with("SHOP-40 "))
        .unwrap();
    assert!(
        tree.lines()
            .nth(epic_line + 1)
            .unwrap()
            .starts_with("  SHOP-40.1"),
        "{tree}"
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn list_filters_and_readiness() {
    let w = TestService::start().await;
    w.shop_sample().await;

    let working = json(&cb(
        &w,
        &["-P", "SHOP", "list", "--status", "working", "--json"],
    ));
    let keys: Vec<&str> = working
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["SHOP-19", "SHOP-27", "SHOP-40.2"]);
    let dev = json(&cb(
        &w,
        &["--project", "SHOP", "list", "--status", "dev", "--json"],
    ));
    assert!(dev.as_array().unwrap().iter().all(|t| t["column"] == "dev"));

    // Inside the repository, the project is found from the folder.
    let repo = w.board.project("SHOP").await.unwrap().repo_path;
    let out = Command::new(CB)
        .args(["list", "--silent"])
        .env(DataDir::env_var(), w.data.root())
        .env_remove(naming::ticket_env_var())
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(stdout(&out).lines().any(|l| l == "SHOP-20"));

    let blocked = stdout(&cb(&w, &["-P", "SHOP", "blocked"]));
    assert!(
        blocked.contains("SHOP-20")
            && blocked.contains("waits for SHOP-18, SHOP-19 to merge (SHOP-18 in Review)"),
        "{blocked}"
    );
    assert!(blocked.contains("SHOP-21") && blocked.contains("waits for SHOP-20 to merge"));
    let blocked_json = json(&cb(&w, &["-P", "SHOP", "blocked", "--json"]));
    let shop20 = blocked_json
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == "SHOP-20")
        .unwrap();
    assert_eq!(
        shop20["reason"],
        "waits for SHOP-18, SHOP-19 to merge (SHOP-18 in Review)"
    );
    assert_eq!(shop20["waiting_for"][0]["status"], "ready_for_review");
    let ready = json(&cb(&w, &["-P", "SHOP", "ready", "--json"]));
    let ready: Vec<&str> = ready
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap())
        .collect();
    assert!(
        ready.contains(&"SHOP-40.2"),
        "its parent SHOP-40.1 is merged"
    );
    assert!(
        !ready.contains(&"SHOP-20") && !ready.contains(&"SHOP-17"),
        "{ready:?}"
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn projects_and_errors() {
    let w = TestService::start().await;
    w.shop_sample().await;

    let projects = json(&cb(&w, &["project", "list", "--json"]));
    assert_eq!(projects[0]["key"], "SHOP");
    assert!(stdout(&cb(&w, &["project", "list"])).starts_with("SHOP   shop-web"));
    let shop = json(&cb(&w, &["project", "show", "SHOP", "--json"]));
    assert_eq!(shop["base_branch"], "main");

    // Unknown keys: {"error": …} and a non-zero exit.
    let out = cb(&w, &["show", "SHOP-99", "--json"]);
    assert!(!out.status.success());
    let error: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(error["error"]["kind"], "not_found");
    let out = cb(&w, &["project", "show", "NOPE", "--json"]);
    assert!(!out.status.success());
    // A bare number without a current project.
    let out = cb(&w, &["show", "18", "--json"]);
    assert!(!out.status.success());
    let error: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(error["error"]["kind"], "usage");
    w.stop().await;
}

/// Agents may read anything (CLI-02): the same commands from inside SHOP-19's agent session.
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_reads_the_whole_project() {
    let w = TestService::start().await;
    w.shop_sample().await;
    let shop19 = w.board.ticket("SHOP-19").await.unwrap();
    let agent = w.agent_shell(&shop19).await;
    let own = agent.run(&format!("'{CB}' show --json")).await;
    assert_eq!(own.status, 0, "{own:?}");
    let own: Value = serde_json::from_str(&own.stdout).unwrap();
    assert_eq!(own["ticket"]["key"], "SHOP-19");
    let other = agent.run(&format!("'{CB}' show 20")).await;
    assert!(
        other.stdout.starts_with("SHOP-20 · Checkout page"),
        "{other:?}"
    );
    let blocked = agent.run(&format!("'{CB}' blocked --silent")).await;
    assert!(
        blocked.stdout.lines().any(|l| l == "SHOP-21"),
        "{blocked:?}"
    );
    w.stop().await;
}
