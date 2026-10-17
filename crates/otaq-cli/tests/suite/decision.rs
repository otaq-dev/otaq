//! `otaq decision record --stdin` (T-0276; DEC-02, d0084) from inside an agent session (an ordinary `sh`, never
//! `claude`) and from the user's terminal, and the decisions in `otaq show` and `otaq prime`.

use std::io::Write;
use std::process::{Command, Output, Stdio};

use otaq_core::data_dir::DataDir;
use otaq_core::naming;
use otaq_core::service::testing::TestService;
use serde_json::Value;

const CB: &str = env!("CARGO_BIN_EXE_otaq");

const SPLIT_NAME: &str = "Question: Split fullName everywhere,\n  or only in the profile form?\nBuilt: only in the \
                          profile form\nOther option: everywhere\nOther option: in the API only\nWhy: the description \
                          only mentions the profile form\n";

/// The user's terminal: no ticket variable.
fn cb(w: &TestService, args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(CB)
        .args(args)
        .env(DataDir::env_var(), w.data.root())
        .env_remove(naming::ticket_env_var())
        .current_dir(w.home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.unwrap_or_default().as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
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

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_records_on_its_own_ticket_and_show_and_prime_list_it() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    w.create("SHOP", "Add wishlist").await;
    let agent = w.agent_shell(&shop1).await;

    let ran = agent
        .run_with_stdin(&format!("'{CB}' decision record --stdin"), SPLIT_NAME)
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    assert_eq!(
        ran.stdout.trim(),
        "Recorded SHOP-1#1. Carry on; the user sees it in the review."
    );
    let ran = agent
        .run_with_stdin(
            &format!("'{CB}' decision record --stdin --silent"),
            SPLIT_NAME,
        )
        .await;
    assert_eq!(ran.stdout.trim(), "SHOP-1#2", "{ran:?}");
    let ran = agent
        .run_with_stdin(
            &format!("'{CB}' decision record --stdin --json"),
            SPLIT_NAME,
        )
        .await;
    let recorded: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(recorded["id"], "SHOP-1#3");
    assert_eq!(
        recorded["question"],
        "Split fullName everywhere, or only in the profile form?"
    );
    assert_eq!(recorded["other_options"][1], "in the API only");
    assert_eq!(recorded["recorded_by"], "agent:SHOP-1");
    assert_eq!(recorded["outcome"], "open");

    // A missing field: refused with what's missing; nothing is recorded.
    let ran = agent
        .run_with_stdin(
            &format!("'{CB}' decision record --stdin --json"),
            "Question: q\nBuilt: b\nOther option: o\n",
        )
        .await;
    assert_ne!(ran.status, 0);
    let error: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(
        error["error"]["message"],
        "Recorded decisions need Question, Built, at least one Other option, and Why. Missing: Why."
    );
    // Another ticket: refused (CLI-02).
    let ran = agent
        .run_with_stdin(
            &format!("'{CB}' decision record SHOP-2 --stdin --json"),
            SPLIT_NAME,
        )
        .await;
    assert_ne!(ran.status, 0);
    let error: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(
        error["error"]["message"],
        "SHOP-1's agent can't change SHOP-2"
    );

    // `show` lists them; `--json` includes them.
    let text = stdout(&cb(&w, &["show", "SHOP-1"], None));
    assert!(
        text.contains(
            "\n\nDecisions\n  SHOP-1#1 Split fullName everywhere, or only in the profile form?\n    \
             Built: only in the profile form\n    Other option: everywhere\n    Other option: in the API only\n    \
             Why: the description only mentions the profile form\n  SHOP-1#2 "
        ),
        "{text}"
    );
    let shown: Value =
        serde_json::from_str(&stdout(&cb(&w, &["show", "SHOP-1", "--json"], None))).unwrap();
    assert_eq!(shown["decisions"].as_array().unwrap().len(), 3);
    assert_eq!(shown["decisions"][0]["id"], "SHOP-1#1");
    assert_eq!(shown["decisions"][0]["built"], "only in the profile form");
    let other: Value =
        serde_json::from_str(&stdout(&cb(&w, &["show", "SHOP-2", "--json"], None))).unwrap();
    assert_eq!(other["decisions"], serde_json::json!([]));
    assert!(!stdout(&cb(&w, &["show", "SHOP-2"], None)).contains("Decisions"));

    // `prime` lists the ticket's own, for a resumed or sent-back agent.
    let ran = agent.run(&format!("'{CB}' prime")).await;
    assert!(ran.stdout.contains("## Decisions you recorded"), "{ran:?}");
    assert!(ran.stdout.contains("- SHOP-1#3 Split fullName"), "{ran:?}");
    let ran = agent.run(&format!("'{CB}' prime --json")).await;
    let context: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(context["decisions"][2]["id"], "SHOP-1#3");
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_users_terminal_names_the_ticket() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    w.create("SHOP", "Split user full name").await;

    let out = cb(
        &w,
        &["decision", "record", "SHOP-1", "--stdin"],
        Some(SPLIT_NAME),
    );
    assert_eq!(
        stdout(&out).trim(),
        "Recorded SHOP-1#1. Carry on; the user sees it in the review."
    );
    // Without --stdin, or with text before the first label: a usage error.
    let out = cb(&w, &["decision", "record", "SHOP-1"], Some(SPLIT_NAME));
    assert!(!out.status.success());
    let out = cb(
        &w,
        &["decision", "record", "SHOP-1", "--stdin"],
        Some("I built newest first\n"),
    );
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("Start each field with its label"),
        "{out:?}"
    );
    w.stop().await;
}
