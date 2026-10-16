//! An epic lead's commands from inside its session (T-0280; LEAD-08, DEC-04, CLI-07; d0083): `claude` stands in as
//! an ordinary `sh` started the way a lead is, so `otaq suggest close|delete <KEY> --stdin` and `otaq decision mark
//! <ID> --source … --stdin` typed into it reach the service as the lead. A ticket's agent is refused. No test
//! starts claude.

use otaq_core::board::tickets::NewTicketInput;
use otaq_core::data_dir::DataDir;
use otaq_core::decisions::NewDecision;
use otaq_core::domain::{Actor, Kind};
use otaq_core::epic_lead::suggest::{self, SuggestionKind};
use otaq_core::git::testing::TempRepos;
use otaq_core::policy::{ONLY_A_LEAD_MARKS, ONLY_A_LEAD_SUGGESTS};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use otaq_core::session::launch::LaunchContext;
use serde_json::Value;

const CB: &str = env!("CARGO_BIN_EXE_otaq");

/// shop-web with the epic SHOP-1 Wishlist, its children SHOP-1.1 Wishlist API and SHOP-1.2 Wishlist export (CSV);
/// `claude` is an interactive `sh` in the data folder.
async fn world() -> TestService {
    let home = tempfile::TempDir::new().unwrap();
    let data = home.path().join("h");
    let script = format!(
        "export {}='{}'; export PS1=''; exec sh",
        DataDir::env_var(),
        data.display()
    );
    let w = TestService::start_in(home, TempRepos::new(), |config| ServiceConfig {
        launch: Some(LaunchContext {
            claude: vec!["sh".into(), "-c".into(), script, "sh".into()],
            bin_dir: "/nonexistent".into(),
            plugin_dir: "/nonexistent/plugin".into(),
            path: std::env::var_os("PATH").unwrap_or_default(),
            claude_config_dir: None,
        }),
        ..config
    })
    .await;
    w.add_project("shop-web").await;
    let me = Actor::User("me".into());
    let epic = w
        .board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Wishlist".into(),
                kind: Kind::Epic,
                ..Default::default()
            },
            &me,
        )
        .await
        .unwrap();
    for title in ["Wishlist API", "Wishlist export (CSV)"] {
        w.board
            .create_ticket(
                &NewTicketInput {
                    project: "SHOP".into(),
                    title: title.into(),
                    epic: Some(epic.key.to_string()),
                    ..Default::default()
                },
                &me,
            )
            .await
            .unwrap();
    }
    w
}

#[tokio::test(flavor = "multi_thread")]
async fn the_lead_suggests_and_marks_from_its_session_and_an_agent_cant() {
    let w = world().await;
    let lead = w.project_agents.leads().open("SHOP-1").await.unwrap();
    let shell = w.shell_in(&lead.agent.session.as_ref().unwrap().id);

    let ran = shell
        .run_with_stdin(
            &format!("'{CB}' suggest close SHOP-1.2 --stdin"),
            "PAY-123 asks for no export.\n",
        )
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    assert_eq!(
        ran.stdout.trim(),
        "Suggested closing SHOP-1.2. The user decides in the inbox; nothing waits for it, so carry on."
    );
    let listed = suggest::list_for_epic(&w.board, "SHOP-1").await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].kind, SuggestionKind::Close);
    assert_eq!(listed[0].reason, "PAY-123 asks for no export.");
    // --silent prints the id; --json the suggestion.
    let ran = shell
        .run_with_stdin(
            &format!("'{CB}' suggest delete SHOP-1.2 --stdin --json"),
            "or delete it",
        )
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    let json: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(json["kind"], "delete");
    assert_eq!(json["state"], "open");
    // Without --stdin, a usage error.
    let ran = shell.run(&format!("'{CB}' suggest close SHOP-1.1")).await;
    assert_ne!(ran.status, 0);
    assert!(ran.stderr.contains("--stdin"), "{ran:?}");

    // The mark.
    let api = w.board.ticket("SHOP-1.1").await.unwrap();
    otaq_core::decisions::record(
        &w.board,
        "SHOP-1.1",
        &NewDecision {
            question: "Order of saved items?".into(),
            built: "newest first".into(),
            other_options: vec!["oldest first".into()],
            why: "w".into(),
        },
        &Actor::Agent(api.key.clone()),
    )
    .await
    .unwrap();
    let ran = shell
        .run_with_stdin(
            &format!("'{CB}' decision mark SHOP-1.1#1 --source PAY-123 --stdin"),
            "items are listed in the order they were added\n",
        )
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    assert_eq!(
        ran.stdout.trim(),
        "Marked SHOP-1.1#1. The user sees it in the review; keeping or changing it is theirs."
    );
    let ran = shell.run(&format!("'{CB}' show SHOP-1.1")).await;
    assert!(
        ran.stdout
            .contains("contradicts PAY-123: items are listed in the order they were added"),
        "{ran:?}"
    );

    // A ticket's agent is refused both.
    let agent = w.agent_shell(&api).await;
    let ran = agent
        .run_with_stdin(&format!("'{CB}' suggest close SHOP-1.2 --stdin"), "r")
        .await;
    assert_ne!(ran.status, 0);
    assert!(ran.stderr.contains(ONLY_A_LEAD_SUGGESTS), "{ran:?}");
    let ran = agent
        .run_with_stdin(
            &format!("'{CB}' decision mark SHOP-1.1#1 --source PAY-123 --stdin"),
            "t",
        )
        .await;
    assert_ne!(ran.status, 0);
    assert!(ran.stderr.contains(ONLY_A_LEAD_MARKS), "{ran:?}");
    w.stop().await;
}
