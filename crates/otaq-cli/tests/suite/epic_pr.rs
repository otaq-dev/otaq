//! The epic's delivery session (T-0146, EPC-08, d0059): Open PR on an epic starts a short-lived session in a
//! worktree on the epic branch (here an ordinary `sh` that prints its arguments and then echoes what it is sent,
//! never `claude`), proposes one commit per merged child, and ends with `pr opened` or Cancel, removing the
//! worktree unless it has uncommitted changes.

use std::path::{Path, PathBuf};

use otaq_core::agent_tools::LaunchContext;
use otaq_core::agent_tools::claude::{ClaudeLaunch, find_plugin_dir};
use otaq_core::board::projects::AddProject;
use otaq_core::board::settings::WorkflowStep;
use otaq_core::board::tickets::{NewTicketInput, Ticket};
use otaq_core::domain::{Actor, Kind, Status};
use otaq_core::git::testing::{commit, git};
use otaq_core::pr::proposal::Proposal;
use otaq_core::pr::service::PrOutcome;
use otaq_core::service::testing::TestService;
use otaq_core::session::SessionKind;
use otaq_core::workflow_skills::testing::skill_folder;
use serde_json::json;

const CB: &str = env!("CARGO_BIN_EXE_otaq");

fn me() -> Actor {
    Actor::User("me".into())
}

/// An ordinary command standing in for `claude`: prints its arguments, then echoes its input.
fn stand_in() -> LaunchContext {
    let bin_dir = Path::new(CB).parent().unwrap().to_path_buf();
    LaunchContext {
        plugin_dir: find_plugin_dir(&bin_dir).unwrap(),
        bin_dir,
        path: std::env::var_os("PATH").unwrap_or_default(),
        claude: ClaudeLaunch {
            program: vec![
                "sh".into(),
                "-c".into(),
                "echo \"delivery: $* in $(pwd)\"; exec cat".into(),
                "sh".into(),
            ],
            config_dir: None,
        },
    }
}

struct Setup {
    w: TestService,
    epic: Ticket,
    repo: PathBuf,
}

/// SHOP-1 Wishlist (External ID PAY-123) with three children, each merged locally into the epic branch.
async fn setup() -> Setup {
    let w = TestService::start_with(|mut c| {
        c.launch = Some(stand_in());
        c
    })
    .await;
    let repo = w.repos.clone_with_origin("shop-web", "main", 1);
    w.board
        .add_project(&AddProject {
            path: repo.display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    // origin is a local folder: the project uses its own Open PR skill (REV-11).
    let folder = skill_folder(w.data.root(), "my-pr");
    w.board
        .import_workflow_skill(Some("SHOP"), WorkflowStep::OpenPr, &folder)
        .await
        .unwrap();
    let epic = w
        .board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Wishlist".into(),
                kind: Kind::Epic,
                external_id: Some("PAY-123".into()),
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    for (n, title) in ["Wishlist API", "Wishlist button", "Wishlist page"]
        .into_iter()
        .enumerate()
    {
        let child = w
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
        let worktree = w.start_worktree(&child).await;
        commit(&worktree, &format!("part{n}.ts"), "x\n");
        w.force_status(&child, Status::Working).await;
        w.board
            .submit_for_review(&child.key.to_string(), "Done.", &me())
            .await
            .unwrap();
        let merged = w
            .review
            .local_merge(
                &child.key.to_string(),
                &format!("{}: {title}", child.key),
                false,
                &me(),
            )
            .await
            .unwrap();
        assert!(
            matches!(merged, otaq_core::review::LocalMergeOutcome::Merged { .. }),
            "{merged:?}"
        );
    }
    let epic = w.board.ticket(&epic.key.to_string()).await.unwrap();
    assert_eq!(epic.status, Status::WaitingForYou, "every child merged");
    Setup { w, epic, repo }
}

fn delivery(w: &TestService, epic: &Ticket) -> Option<String> {
    w.sessions
        .running_of(&epic.id, SessionKind::Delivery)
        .map(|s| s.id)
}

#[tokio::test(flavor = "multi_thread")]
async fn open_pr_on_an_epic_runs_a_delivery_session_and_ends_it() {
    let s = setup().await;
    let w = &s.w;
    let outcome = w.prs.open("SHOP-1", false, false, &me()).await.unwrap();
    assert!(matches!(outcome, PrOutcome::Started { .. }), "{outcome:?}");
    let session = delivery(w, &s.epic).expect("a delivery session");
    let printed = w.wait_for_output(&session, "delivery:").await;
    // The imported skill, written into the delivery worktree before the launch (WSK-05, WSK-06).
    let named = "`.agents/skills/otaq-custom-open-pr/`";
    let printed = if printed.contains(named) {
        printed
    } else {
        w.wait_for_output(&session, named).await
    };
    assert!(printed.contains(named), "{printed}");
    let epic = w.board.ticket("SHOP-1").await.unwrap();
    let worktree = PathBuf::from(epic.work.as_ref().unwrap().worktree.clone().unwrap());
    for root in [".agents", ".claude"] {
        assert!(
            worktree
                .join(root)
                .join("skills/otaq-custom-open-pr/SKILL.md")
                .is_file()
        );
    }
    assert_eq!(
        git(&worktree, &["branch", "--show-current"]).trim(),
        "otaq/SHOP-1",
        "the delivery worktree is on the epic branch"
    );
    assert!(printed.contains(&worktree.file_name().unwrap().to_string_lossy().to_string()));
    let stored = w.sessions.stored(&session).await.unwrap();
    assert!(
        stored.tool_session_id.is_some(),
        "its Claude session ID is kept"
    );
    assert_eq!(
        (stored.tool, stored.model.as_deref()),
        (Some(otaq_core::agent_tools::Tool::Claude), None)
    );

    // One commit per merged child, each child's squash commit as it is.
    let view = w.board.pr_flow_view("SHOP-1").await.unwrap();
    assert_eq!(
        view.commits
            .iter()
            .map(|c| c.message.as_str())
            .collect::<Vec<_>>(),
        [
            "SHOP-1.1: Wishlist API",
            "SHOP-1.2: Wishlist button",
            "SHOP-1.3: Wishlist page"
        ]
    );
    assert_eq!(view.external_id.as_deref(), Some("PAY-123"));
    assert_eq!(view.target, "main");

    // The skill proposes (as the epic's agent), the user confirms, the skill pushes and reports.
    let proposal = json!({
        "branch": "feature/PAY-123-wishlist",
        "commits": view.commits.iter().map(|c| json!({"sha": c.sha, "message": c.message})).collect::<Vec<_>>(),
        "title": "PAY-123 Wishlist",
        "description": "The wishlist."
    });
    let agent = Actor::Agent("SHOP-1".parse().unwrap());
    w.board
        .pr_propose("SHOP-1", &proposal.to_string(), &agent)
        .await
        .unwrap();
    let edits: Proposal = serde_json::from_value(proposal).unwrap();
    w.prs.confirm("SHOP-1", &edits, false, &me()).await.unwrap();
    w.wait_for_output(&session, "The user confirmed the pull request")
        .await;
    git(
        &s.repo,
        &["push", "--quiet", "origin", "feature/PAY-123-wishlist"],
    );
    let epic = w
        .prs
        .opened(
            "SHOP-1",
            "https://github.com/acme/shop-web/pull/490",
            &agent,
        )
        .await
        .unwrap();
    assert_eq!(epic.status, Status::PrOpen);
    assert_eq!(epic.flags.pr_number, Some(490));
    assert!(delivery(w, &epic).is_none(), "the session ended");
    assert!(!worktree.exists(), "the clean delivery worktree is gone");
    assert!(epic.work.as_ref().unwrap().worktree.is_none());
    s.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_ends_the_delivery_session_and_keeps_a_dirty_worktree() {
    let s = setup().await;
    let w = &s.w;
    w.prs.open("SHOP-1", false, false, &me()).await.unwrap();
    let session = delivery(w, &s.epic).expect("a delivery session");
    w.wait_for_output(&session, "delivery:").await;
    let epic = w.board.ticket("SHOP-1").await.unwrap();
    let worktree = PathBuf::from(epic.work.as_ref().unwrap().worktree.clone().unwrap());
    std::fs::write(worktree.join("notes.txt"), "left behind").unwrap();

    let epic = w.prs.cancel("SHOP-1", &me()).await.unwrap();
    assert_eq!(epic.status, Status::WaitingForYou);
    assert!(delivery(w, &epic).is_none(), "the session ended");
    assert!(worktree.join("notes.txt").exists(), "kept: it has changes");
    let scan = w.cleanup.scan().await.unwrap();
    assert!(
        format!("{scan:?}").contains("SHOP-1"),
        "Cleanup lists the kept worktree: {scan:?}"
    );
    s.w.stop().await;
}
