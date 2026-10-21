//! `otaq history` (T-0426; ONB-01, ONB-02, ONB-11; d0108): the user's terminal and a Project agent read the history;
//! a ticket's agent, its check and an epic lead are refused. The history is the made-up fixture
//! (`otaq-core/tests/fixtures/claude-history.jsonl`) in a temporary Claude Code folder, never the user's own; `claude`
//! stands in as an ordinary `sh`. A last test runs the real service with `CLAUDE_CONFIG_DIR` set, as the daemon reads
//! it.

use std::path::Path;
use std::process::{Command, Output};

use otaq_core::agent_tools::LaunchContext;
use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::agent_tools::history::only_a_project_agent;
use otaq_core::board::tickets::NewTicketInput;
use otaq_core::data_dir::DataDir;
use otaq_core::domain::{Actor, Kind};
use otaq_core::git::testing::TempRepos;
use otaq_core::naming;
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::{Ran, TestService};
use otaq_core::session::SessionKind;
use serde_json::Value;

const CB: &str = env!("CARGO_BIN_EXE_otaq");

/// The made-up history (T-0426): 300 entries, one broken line, three pastes.
fn fixture() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../otaq-core/tests/fixtures/claude-history.jsonl"
    ))
}

/// What the fixture's pastes hold: never printed.
const PASTE_MARKERS: &[&str] = &[
    "PASTE-MARKER-QUOKKA-1",
    "PASTE-MARKER-NARWHAL-2",
    "PASTE-MARKER-AXOLOTL-3",
];

/// A temporary Claude Code folder holding the fixture as its `history.jsonl`.
fn claude_folder() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(fixture(), dir.path().join("history.jsonl")).unwrap();
    dir
}

/// pocket-api, with the service reading Claude Code's folder `claude`; agents are an interactive `sh`.
async fn world(claude: &Path) -> TestService {
    let home = tempfile::TempDir::new().unwrap();
    let data = home.path().join("h");
    let script = format!(
        "export {}='{}'; export PS1=''; exec sh",
        DataDir::env_var(),
        data.display()
    );
    let w = TestService::start_in(home, TempRepos::new(), |mut config| {
        config.tools.claude_config_dir = Some(claude.to_path_buf());
        ServiceConfig {
            launch: Some(LaunchContext {
                bin_dir: "/nonexistent".into(),
                plugin_dir: "/nonexistent/plugin".into(),
                path: std::env::var_os("PATH").unwrap_or_default(),
                claude: ClaudeLaunch {
                    program: vec!["sh".into(), "-c".into(), script, "sh".into()],
                    config_dir: None,
                },
            }),
            ..config
        }
    })
    .await;
    w.add_project("pocket-api").await;
    w
}

/// `otaq <args>` from the user's own terminal.
fn user(w: &TestService, args: &[&str]) -> Output {
    Command::new(CB)
        .args(args)
        .env(DataDir::env_var(), w.data.root())
        .env_remove(naming::ticket_env_var())
        .env_remove(naming::role_env_var())
        .output()
        .unwrap()
}

fn text(out: &[u8]) -> String {
    String::from_utf8_lossy(out).into_owned()
}

fn no_pastes(printed: &str) {
    for marker in PASTE_MARKERS {
        assert!(!printed.contains(marker), "{marker} printed");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_user_and_a_project_agent_read_the_history() {
    let claude = claude_folder();
    let w = world(claude.path()).await;
    let api = w.board.projects().await.unwrap()[0].key.clone();

    let all = user(&w, &["history", "--all"]);
    assert!(all.status.success(), "{all:?}");
    let printed = text(&all.stdout);
    no_pastes(&printed);
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(
        lines[0],
        "300 of your messages in 3 projects \u{b7} all time"
    );
    assert!(
        lines[1].contains(" \u{b7} pocket-api \u{b7} "),
        "{}",
        lines[1]
    );
    assert_eq!(*lines.last().unwrap(), "page 1 of 1 \u{b7} 1 line skipped");
    assert!(printed.contains("\u{b7} [pasted text left out]\n"));

    // Filters, through the global `--project`.
    let commits = user(
        &w,
        &[
            "history",
            "--all",
            "--project",
            "pocket-api",
            "--command",
            "/commit",
            "--json",
        ],
    );
    assert!(commits.status.success(), "{commits:?}");
    let json: Value = serde_json::from_slice(&commits.stdout).unwrap();
    assert_eq!(json["messages"], 9);
    assert_eq!(json["projects"], 1);
    assert_eq!(json["since"], "all");
    assert_eq!(json["entries"][0]["project"], "pocket-api");
    assert_eq!(json["entries"][0]["text"], "/commit");
    no_pastes(&text(&commits.stdout));

    // A wrong period is a usage error.
    let wrong = user(&w, &["history", "--since", "12w"]);
    assert_eq!(wrong.status.code(), Some(2), "{wrong:?}");

    let opened = w.project_agents.open(&api, "Propose a flow").await.unwrap();
    let agent = w.shell_in(&opened.session.as_ref().unwrap().id);
    let ran = agent.run(&format!("'{CB}' history --all --json")).await;
    assert_eq!(ran.status, 0, "{ran:?}");
    let json: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(json["messages"], 300);
    assert_eq!(json["skipped"], 1);
    assert_eq!(json["entries"].as_array().unwrap().len(), 300);
    no_pastes(&ran.stdout);
    w.stop().await;
}

/// A ticket's agent, its check and an epic lead are refused.
#[tokio::test(flavor = "multi_thread")]
async fn ticket_agents_checks_and_epic_leads_are_refused() {
    let claude = claude_folder();
    let w = world(claude.path()).await;
    let api = w.board.projects().await.unwrap()[0].key.clone();
    let read = format!("'{CB}' history --all --json");
    let refused = |ran: Ran| {
        assert_ne!(ran.status, 0, "{ran:?}");
        let e: Value = serde_json::from_str(&ran.stdout).unwrap();
        assert_eq!(e["error"]["message"], only_a_project_agent(), "{e}");
        no_pastes(&ran.stdout);
        assert!(!ran.stdout.contains("/commit"), "{ran:?}");
    };

    let ticket = w.create(&api, "Rate limit").await;
    refused(w.agent_shell(&ticket).await.run(&read).await);
    refused(
        w.shell(&ticket, SessionKind::Validator)
            .await
            .run(&read)
            .await,
    );
    let epic = w
        .board
        .create_ticket(
            &NewTicketInput {
                project: api.clone(),
                title: "Billing".into(),
                kind: Kind::Epic,
                ..Default::default()
            },
            &Actor::User("me".into()),
        )
        .await
        .unwrap();
    let lead = w
        .project_agents
        .leads()
        .open(&epic.key.to_string())
        .await
        .unwrap();
    refused(
        w.shell_in(&lead.agent.session.as_ref().unwrap().id)
            .run(&read)
            .await,
    );
    w.stop().await;
}

/// No history file: said so, exit 0.
#[tokio::test(flavor = "multi_thread")]
async fn no_history_on_this_computer() {
    let claude = tempfile::tempdir().unwrap();
    let w = world(claude.path()).await;
    let none = user(&w, &["history"]);
    assert!(none.status.success(), "{none:?}");
    assert_eq!(
        text(&none.stdout).trim(),
        "No Claude Code history on this computer"
    );
    w.stop().await;
}

/// The real service reads Claude Code's folder from `CLAUDE_CONFIG_DIR` (with a throwaway HOME and no tools on
/// PATH, as `daemon.rs`).
#[test]
fn the_service_reads_claude_config_dir() {
    let home = tempfile::tempdir().unwrap();
    let claude = claude_folder();
    let run = |args: &[&str]| {
        Command::new(CB)
            .args(args)
            .env(DataDir::env_var(), home.path().join("otaq"))
            .env("CLAUDE_CONFIG_DIR", claude.path())
            .env("PATH", home.path().join("no-tools"))
            .env("HOME", home.path())
            .env_remove(naming::ticket_env_var())
            .env_remove(naming::role_env_var())
            .output()
            .unwrap()
    };
    struct Stop<'a>(&'a dyn Fn(&[&str]) -> Output);
    impl Drop for Stop<'_> {
        fn drop(&mut self) {
            let _ = (self.0)(&["daemon", "stop"]);
        }
    }
    let _stop = Stop(&run);
    let started = run(&["daemon", "start"]);
    assert!(started.status.success(), "{started:?}");
    let read = run(&["history", "--all"]);
    assert!(read.status.success(), "{read:?}");
    let printed = text(&read.stdout);
    assert!(
        printed.starts_with("300 of your messages in 3 projects \u{b7} all time\n"),
        "{printed}"
    );
    no_pastes(&printed);
}

/// A test service whose test names no Claude Code folder reads an empty one in the test's own folder, never the
/// user's: nothing until the fixture is written there.
#[tokio::test(flavor = "multi_thread")]
async fn a_test_service_never_reads_the_users_history() {
    let w = TestService::start_with(|config| config).await;
    let dir = w.claude_dir();
    assert!(dir.starts_with(w.home.path()), "{}", dir.display());
    let none = user(&w, &["history", "--all"]);
    assert!(none.status.success(), "{none:?}");
    assert_eq!(
        text(&none.stdout).trim(),
        "No Claude Code history on this computer"
    );
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(fixture(), dir.join("history.jsonl")).unwrap();
    let read = user(&w, &["history", "--all", "--json"]);
    let json: Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json["messages"], 300);
    w.stop().await;
}
