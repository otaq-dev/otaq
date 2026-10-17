//! Send back (T-0086): the comments go to the agent's live session, a resumed one, or the ticket waits in Queued
//! while the agent limit is full. Sessions are an ordinary `sh`; no test starts claude.

use otaq_core::agent_tools::LaunchContext;
use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::board::Refusal;
use otaq_core::board::comments::{NewComment, Side};
use otaq_core::board::settings::{AgentLimits, GlobalSettings};
use otaq_core::board::tickets::{EventKind, Ticket};
use otaq_core::domain::{Actor, SentBack, Status};
use otaq_core::naming;
use otaq_core::review::ReturnOutcome;
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;

fn me() -> Actor {
    Actor::User("me".into())
}

fn skill() -> String {
    naming::skill("address-review")
}

async fn in_review_with_comments(world: &TestService, comments: usize) -> Ticket {
    let t = world.create("SHOP", "Search results paging").await;
    world.force_status(&t, Status::ReadyForReview).await;
    for n in 0..comments {
        world
            .board
            .add_comment(&NewComment {
                key: t.key.to_string(),
                file: "lib/search.ts".into(),
                line: u32::try_from(n + 1).unwrap(),
                side: Side::New,
                body: format!("comment {n}"),
            })
            .await
            .unwrap();
    }
    t
}

fn done(outcome: ReturnOutcome) -> Ticket {
    match outcome {
        ReturnOutcome::Done { ticket } => *ticket,
        other => panic!("expected done, got {other:?}"),
    }
}

#[tokio::test]
async fn a_live_session_gets_the_skill_and_the_ticket_works_again() {
    let world = TestService::start().await;
    world.add_project("shop-web").await;
    let t = in_review_with_comments(&world, 3).await;
    let agent = world.agent_shell(&t).await;

    let after = done(
        world
            .review
            .send_back("SHOP-1", "Also check page 0", false, &me())
            .await
            .unwrap(),
    );
    assert_eq!(after.status, Status::Working);
    assert_eq!(
        after.flags.sent_back,
        Some(SentBack {
            comments: 3,
            round: 1,
            decisions: 0
        })
    );
    world.wait_for_output(&agent.session, &skill()).await;

    let comments = world.board.comments("SHOP-1", None).await.unwrap();
    assert_eq!(comments.pending, 0);
    assert!(comments.comments.iter().all(|c| c.sent_at.is_some()));
    assert_eq!(comments.round, 2);
    assert_eq!(
        world.board.latest_send_back_note(&t.id).await.unwrap(),
        Some((1, Some("Also check page 0".into())))
    );
    let history = world.board.history("SHOP-1").await.unwrap();
    assert!(history.iter().any(|e| e.kind == EventKind::SentBack));

    // The flag stays until the ticket is back in Review.
    world.give_worktree(&t).await;
    world
        .board
        .submit_for_review("SHOP-1", "fixed", &Actor::Agent(t.key.clone()))
        .await
        .unwrap();
    assert_eq!(
        world.board.ticket("SHOP-1").await.unwrap().flags.sent_back,
        None
    );
    world.stop().await;
}

#[tokio::test]
async fn unsent_text_asks_first_and_clear_it_and_send_goes_on() {
    let world = TestService::start().await;
    world.add_project("shop-web").await;
    let t = in_review_with_comments(&world, 1).await;
    let agent = world.agent_shell(&t).await;
    world.sessions.input(&agent.session, b"my draft").unwrap();

    let first = world
        .review
        .send_back("SHOP-1", "", false, &me())
        .await
        .unwrap();
    assert_eq!(
        first,
        ReturnOutcome::NeedsConfirmation {
            ticket: t.key.clone()
        }
    );
    assert_eq!(world.status("SHOP-1").await, Status::ReadyForReview);
    assert_eq!(
        world.board.comments("SHOP-1", None).await.unwrap().pending,
        1
    );

    let after = done(
        world
            .review
            .send_back("SHOP-1", "", true, &me())
            .await
            .unwrap(),
    );
    assert_eq!(after.status, Status::Working);
    world.wait_for_output(&agent.session, &skill()).await;
    world.stop().await;
}

#[tokio::test]
async fn a_full_agent_limit_queues_the_ticket_with_the_skill_to_run() {
    let world = TestService::start().await;
    world.add_project("shop-web").await;
    world
        .board
        .set_global_settings(&GlobalSettings {
            agent_limits: AgentLimits {
                enabled: true,
                max_concurrency: 1,
                back_pressure: 3,
            },
            ..Default::default()
        })
        .await
        .unwrap();
    let busy = world.create("SHOP", "busy").await;
    world.force_status(&busy, Status::Working).await;
    let t = in_review_with_comments(&world, 2).await;
    let agent = world.agent_shell(&t).await;
    // Unsent text doesn't matter now: nothing is typed until a slot frees.
    world.sessions.input(&agent.session, b"draft").unwrap();

    let after = done(
        world
            .review
            .send_back("SHOP-2", "", false, &me())
            .await
            .unwrap(),
    );
    assert!(after.status.is_queued(), "{}", after.status);
    assert!(after.queued_at.is_some());
    let row = world.board.store().ticket(&t.id).await.unwrap().unwrap();
    assert_eq!(row.resume_skill.as_deref(), Some("address-review"));
    assert_eq!(
        after.flags.sent_back,
        Some(SentBack {
            comments: 2,
            round: 1,
            decisions: 0
        })
    );
    world.stop().await;
}

#[tokio::test]
async fn a_gone_session_is_resumed_with_the_skill_in_the_same_click() {
    // `claude` stands in as `sh` printing its arguments, so the test reads the command line it got.
    let world = TestService::start_with(|config| ServiceConfig {
        launch: Some(LaunchContext {
            bin_dir: "/nonexistent".into(),
            plugin_dir: "/nonexistent/plugin".into(),
            path: std::env::var_os("PATH").unwrap_or_default(),
            claude: ClaudeLaunch {
                program: vec![
                    "sh".into(),
                    "-c".into(),
                    "printf 'ARGS %s\\n' \"$*\"; exec sleep 30".into(),
                    "sh".into(),
                ],
                config_dir: None,
            },
        }),
        ..config
    })
    .await;
    world.add_project("shop-web").await;
    let t = in_review_with_comments(&world, 1).await;
    let worktree = world.start_worktree(&t).await;
    world.record_ended_agent(&t, "0199-old-conversation").await;

    let after = done(
        world
            .review
            .send_back("SHOP-1", "", false, &me())
            .await
            .unwrap(),
    );
    assert_eq!(after.status, Status::Working, "{:?}", after.start_problem);
    let session = world
        .sessions
        .running_of(&t.id, otaq_core::session::SessionKind::Agent)
        .expect("resumed");
    let out = world.wait_for_output(&session.id, "ARGS").await;
    assert!(
        out.contains(&format!("--resume 0199-old-conversation {}", skill())),
        "{out}"
    );
    assert_eq!(
        session.tool_session_id.as_deref(),
        Some("0199-old-conversation")
    );
    assert_eq!(
        (session.tool, session.model.as_deref()),
        (Some(otaq_core::agent_tools::Tool::Claude), None)
    );
    assert!(worktree.is_dir());
    // Kept until the agent shows the resumed conversation got it (a failed resume must send it again).
    let row = world.board.store().ticket(&t.id).await.unwrap().unwrap();
    assert_eq!(row.resume_skill.as_deref(), Some("address-review"));
    let agent = otaq_core::caller::Caller::Agent {
        ticket: t.key.clone(),
        ticket_id: t.id.clone(),
        session: session.id.clone(),
    };
    world
        .signals
        .hook(
            &agent,
            otaq_core::agent_tools::Tool::Claude,
            "UserPromptSubmit",
            &serde_json::json!({"hook_event_name": "UserPromptSubmit", "prompt": skill()}),
        )
        .await
        .unwrap();
    let row = world.board.store().ticket(&t.id).await.unwrap().unwrap();
    assert_eq!(row.resume_skill, None, "the skill ran");
    world.sessions.stop(&session.id).await.unwrap();
    world.stop().await;
}

#[tokio::test]
async fn send_back_is_refused_on_epics_and_outside_review() {
    let world = TestService::start().await;
    world.add_project("shop-web").await;
    let t = world.create("SHOP", "Working").await;
    world.force_status(&t, Status::Working).await;
    let err = world
        .review
        .send_back("SHOP-1", "", false, &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::MoveRefused));

    let epic = world
        .board
        .create_ticket(
            &otaq_core::board::tickets::NewTicketInput {
                project: "SHOP".into(),
                title: "Wishlist".into(),
                kind: otaq_core::domain::Kind::Epic,
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    world.force_status(&epic, Status::ReadyForReview).await;
    let err = world
        .review
        .send_back(&epic.key.to_string(), "", false, &me())
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "Add or reopen a child ticket to change it");
    world.stop().await;
}

/// T-0090: a Local merge conflict flags the ticket; Send to agent to fix types the restack skill and prime names
/// the target.
#[tokio::test]
async fn a_merge_conflict_goes_to_the_agent_with_the_restack_skill() {
    use otaq_core::git::testing::commit;
    use otaq_core::review::LocalMergeOutcome;

    let world = TestService::start().await;
    let project = world.add_project("shop-web").await;
    let repo = std::path::PathBuf::from(&project.repo_path);
    let t = world.create("SHOP", "Pricing").await;
    let wt = world.start_worktree(&t).await;
    commit(&wt, "lib.ts", "ticket\n");
    commit(&repo, "lib.ts", "main\n");
    world.force_status(&t, Status::ReadyForReview).await;
    let agent = world.agent_shell(&t).await;

    // No conflict yet: nothing to fix.
    let err = world
        .review
        .send_to_fix("SHOP-1", false, &me())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::MoveRefused));

    let merged = world
        .review
        .local_merge("SHOP-1", "m", false, &me())
        .await
        .unwrap();
    assert!(
        matches!(merged, LocalMergeOutcome::Conflict { .. }),
        "{merged:?}"
    );
    let flagged = world.board.ticket("SHOP-1").await.unwrap();
    assert_eq!(flagged.flags.conflicts_with.as_deref(), Some("main"));
    assert_eq!(flagged.status, Status::ReadyForReview);

    let after = done(
        world
            .review
            .send_to_fix("SHOP-1", false, &me())
            .await
            .unwrap(),
    );
    assert_eq!(after.status, Status::Working);
    assert_eq!(
        after.flags.conflicts_with.as_deref(),
        Some("main"),
        "until it is back in Review"
    );
    world
        .wait_for_output(&agent.session, &naming::skill("restack"))
        .await;
    let prime = otaq_core::prime::render(
        &world
            .board
            .prime_context("SHOP-1", world.adapters.for_new_session())
            .await
            .unwrap(),
    );
    assert!(
        prime.contains("This branch conflicts with the latest `main`. Rebase it onto `main`"),
        "{prime}"
    );
    assert!(prime.contains("- lib.ts"), "{prime}");
    assert!(
        world
            .board
            .history("SHOP-1")
            .await
            .unwrap()
            .iter()
            .any(|e| e.kind == EventKind::SentToFix)
    );

    world
        .board
        .submit_for_review("SHOP-1", "rebased", &Actor::Agent(t.key.clone()))
        .await
        .unwrap();
    let back = world.board.ticket("SHOP-1").await.unwrap();
    assert_eq!(back.flags.conflicts_with, None);
    world.stop().await;
}
