//! Auto-merge (T-0189; AMG-01 … AMG-05; d0064) against temporary repositories: a ticket passes validation (its
//! validator is an ordinary `sh`; the verdict is reported from that session) and the project's Auto-merge decides
//! what happens next. The repository's own folder stands for the user's checkout. A conflict handed back to the
//! agent brings a build freed at Review back first (T-0309, M19-c).

use crate::common::freed;

use std::path::PathBuf;

use otaq_core::agent_tools::LaunchContext;
use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::board::settings::{AutoMerge, ProjectSettings};
use otaq_core::board::tickets::{NewTicketInput, Ticket};
use otaq_core::domain::{
    Actor, AutoMergeStop, BuildState, Kind, Status, ValidationAction, ValidationTrigger,
};
use otaq_core::git::testing::{commit, git};
use otaq_core::naming;
use otaq_core::review::auto_merge::AutoMerged;
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use otaq_core::session::SessionKind;

fn me() -> Actor {
    Actor::User("me".into())
}

async fn world() -> TestService {
    TestService::start_with(|config| ServiceConfig {
        launch: Some(LaunchContext {
            bin_dir: "/nonexistent".into(),
            plugin_dir: "/nonexistent/plugin".into(),
            path: std::env::var_os("PATH").unwrap_or_default(),
            claude: ClaudeLaunch {
                program: vec![
                    "sh".into(),
                    "-c".into(),
                    "exec sleep 30".into(),
                    "sh".into(),
                ],
                config_dir: None,
            },
        }),
        ..config
    })
    .await
}

async fn auto_merge(w: &TestService, value: AutoMerge) {
    w.board
        .set_project_settings(
            "SHOP",
            &ProjectSettings {
                auto_merge: Some(value),
                ..ProjectSettings::default()
            },
        )
        .await
        .unwrap();
}

/// A working ticket (or an epic's child) with one commit on `file` in its worktree.
async fn working(w: &TestService, title: &str, epic: Option<&Ticket>, file: &str) -> Ticket {
    let t = w
        .board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: title.into(),
                epic: epic.map(|e| e.key.to_string()),
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    let wt = w.start_worktree(&t).await;
    commit(&wt, file, &format!("{title}\n"));
    w.force_status(&t, Status::Working).await;
    w.board.ticket(&t.key.to_string()).await.unwrap()
}

/// Submits and passes validation; returns the ticket after it.
async fn pass(w: &TestService, t: &Ticket) -> Ticket {
    let key = t.key.to_string();
    w.validation
        .submit(
            &key,
            "done",
            &Actor::Agent(t.key.clone()),
            ValidationTrigger::Agent,
        )
        .await
        .unwrap();
    let validator = w
        .sessions
        .running_of(&t.id, SessionKind::Validator)
        .unwrap()
        .id;
    w.validation
        .verdict(&key, true, "ok", &validator)
        .await
        .unwrap();
    w.board.ticket(&key).await.unwrap()
}

async fn epic(w: &TestService) -> Ticket {
    w.board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Wishlist".into(),
                kind: Kind::Epic,
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap()
}

/// Off: Ready for Review. Epic children only: children merge into the epic branch, standalone tickets don't. All
/// tickets: both. The merge is local and never pushed.
#[tokio::test(flavor = "multi_thread")]
async fn which_tickets_merge_by_themselves() {
    let w = world().await;
    let project = w.add_project("shop-web").await;
    let repo = PathBuf::from(&project.repo_path);
    let epic = epic(&w).await;

    let off = working(&w, "Off", None, "off.ts").await;
    assert_eq!(pass(&w, &off).await.status, Status::ReadyForReview);

    auto_merge(&w, AutoMerge::EpicChildren).await;
    let child = working(&w, "Wishlist API", Some(&epic), "wishlist.ts").await;
    let standalone = working(&w, "Footer", None, "footer.ts").await;
    let main = git(&repo, &["rev-parse", "main"]);
    let merged = pass(&w, &child).await;
    assert_eq!(merged.status, Status::Merged);
    assert!(merged.flags.merged_automatically);
    assert_eq!(
        git(
            &repo,
            &[
                "log",
                "-1",
                "--format=%s",
                &naming::branch(&epic.key.to_string())
            ]
        ),
        format!("{}: Wishlist API", child.key)
    );
    assert_eq!(git(&repo, &["rev-parse", "main"]), main, "main untouched");
    assert_eq!(pass(&w, &standalone).await.status, Status::ReadyForReview);

    auto_merge(&w, AutoMerge::All).await;
    let all = working(&w, "Header", None, "header.ts").await;
    let merged = pass(&w, &all).await;
    assert_eq!(merged.status, Status::Merged);
    assert_eq!(
        git(&repo, &["log", "-1", "--format=%s", "main"]),
        format!("{}: Header", all.key)
    );
    let history = w.board.history(&all.key.to_string()).await.unwrap();
    assert!(
        history
            .iter()
            .any(|e| e.kind == otaq_core::board::tickets::EventKind::MergedAutomatically)
    );
    w.stop().await;
}

/// The user's routes to Review and validation off never auto-merge.
#[tokio::test(flavor = "multi_thread")]
async fn the_users_routes_never_auto_merge() {
    let w = world().await;
    w.add_project("shop-web").await;
    auto_merge(&w, AutoMerge::All).await;

    let submitted = working(&w, "Submitted", None, "a.ts").await;
    w.force_status(&submitted, Status::Idle).await;
    let after = w
        .board
        .submit_by_user(&submitted.key.to_string(), "", &me())
        .await
        .unwrap();
    assert_eq!(after.status, Status::ReadyForReview);

    let skipped = working(&w, "Skipped", None, "b.ts").await;
    w.validation
        .submit(
            &skipped.key.to_string(),
            "done",
            &Actor::Agent(skipped.key.clone()),
            ValidationTrigger::Agent,
        )
        .await
        .unwrap();
    let after = w
        .validation
        .user_action(&skipped.key.to_string(), ValidationAction::Skip, "", &me())
        .await
        .unwrap();
    assert_eq!(after.status, Status::ReadyForReview);

    w.board
        .set_project_settings(
            "SHOP",
            &ProjectSettings {
                auto_merge: Some(AutoMerge::All),
                validation: Some(false),
                ..ProjectSettings::default()
            },
        )
        .await
        .unwrap();
    let off = working(&w, "Validation off", None, "c.ts").await;
    let after = w
        .validation
        .submit(
            &off.key.to_string(),
            "done",
            &Actor::Agent(off.key.clone()),
            ValidationTrigger::Agent,
        )
        .await
        .unwrap();
    assert_eq!(after.status, Status::ReadyForReview);
    w.stop().await;
}

/// Success: Done, the cleanup (worktree removed, the agent's session ended) and the waiting tickets start.
#[tokio::test(flavor = "multi_thread")]
async fn a_merge_cleans_up_and_starts_the_waiting_tickets() {
    let w = world().await;
    w.add_project("shop-web").await;
    auto_merge(&w, AutoMerge::All).await;
    let parent = working(&w, "Checkout API", None, "api.ts").await;
    let agent = w.agent_shell(&parent).await;
    let waiting = w.create("SHOP", "Checkout page").await;
    w.board
        .add_dependency(&waiting.key.to_string(), &parent.key.to_string(), &me())
        .await
        .unwrap();
    w.force_status(&waiting, Status::Queued).await;

    let mut listener = w.events.listen();
    let merged = pass(&w, &parent).await;
    assert_eq!(merged.status, Status::Merged);
    // The toast (T-0196, UI-07); no notification.
    let toast = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Ok(otaq_core::protocol::Event::MergedAutomatically { title, message, .. }) =
                listener.receiver.recv().await
            {
                return (title, message);
            }
        }
    })
    .await
    .expect("the auto-merge toast");
    assert_eq!(
        toast.0,
        format!("{} merged automatically into main", parent.key)
    );
    assert!(
        toast
            .1
            .starts_with("Validated · round 1 · local, not pushed"),
        "{}",
        toast.1
    );
    let worktree = parent.work.as_ref().unwrap().worktree.clone().unwrap();
    assert!(
        !std::path::Path::new(&worktree).exists(),
        "worktree removed"
    );
    assert!(!w.sessions.info(&agent.session).unwrap().is_running());
    // Its start runs in the background, as after any merge.
    let start = std::time::Instant::now();
    while w.status(&waiting.key.to_string()).await == Status::Queued {
        assert!(
            start.elapsed() < std::time::Duration::from_secs(10),
            "the waiting ticket never started"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    w.stop().await;
}

/// A conflict goes back to the agent with restack, no click; the third in a row stops; a merge resets it.
#[tokio::test(flavor = "multi_thread")]
async fn conflicts_go_back_to_the_agent_until_the_third() {
    let w = world().await;
    let project = w.add_project("shop-web").await;
    let repo = PathBuf::from(&project.repo_path);
    auto_merge(&w, AutoMerge::All).await;
    let t = working(&w, "Pricing", None, "lib.ts").await;
    let agent = w.agent_shell(&t).await;
    commit(&repo, "lib.ts", "main\n");

    let after = pass(&w, &t).await;
    assert_eq!(after.status, Status::MergeConflictFixing);
    assert_eq!(after.auto_merge_conflicts, 1);
    let pending = w.deliveries.pending(&agent.session).await.unwrap();
    assert_eq!(pending.last().unwrap().text, naming::skill("restack"));
    assert_eq!(
        after.validation_round, 0,
        "the next submit validates from 0"
    );

    for n in 2..=3 {
        let t = w.board.ticket(&t.key.to_string()).await.unwrap();
        let after = pass(&w, &t).await;
        if n < 3 {
            assert_eq!(after.status, Status::MergeConflictFixing);
        } else {
            assert_eq!(after.status, Status::AutoMergeStopped);
            assert_eq!(after.auto_merge_stop, Some(AutoMergeStop::KeepsConflicting));
            assert_eq!(
                after.signal.unwrap().text.as_deref(),
                Some("Keeps conflicting with `main`")
            );
        }
    }

    // The user resolves it in main: Retry merge lands it and resets the count.
    commit(&repo, "lib.ts", "Pricing\n");
    let after = w
        .validation
        .user_action(&t.key.to_string(), ValidationAction::RetryMerge, "", &me())
        .await
        .unwrap();
    assert_eq!(after.status, Status::Merged);
    assert_eq!(after.auto_merge_conflicts, 0);
    w.stop().await;
}

/// A dirty checkout of the target stops with the Local merge's message; Move to Review keeps "Validated".
#[tokio::test(flavor = "multi_thread")]
async fn a_dirty_checkout_stops_it() {
    let w = world().await;
    let project = w.add_project("shop-web").await;
    let repo = PathBuf::from(&project.repo_path);
    auto_merge(&w, AutoMerge::All).await;
    let t = working(&w, "Checkout API", None, "api.ts").await;
    std::fs::write(repo.join("file1.txt"), "my edit\n").unwrap();
    let after = pass(&w, &t).await;
    assert_eq!(after.status, Status::AutoMergeStopped);
    assert_eq!(after.auto_merge_stop, Some(AutoMergeStop::CheckoutDirty));
    assert_eq!(
        after.signal.unwrap().text.as_deref(),
        Some("shop-web has uncommitted changes on main. Clean up and retry.")
    );
    assert_eq!(
        std::fs::read_to_string(repo.join("file1.txt")).unwrap(),
        "my edit\n"
    );
    let after = w
        .validation
        .user_action(
            &t.key.to_string(),
            ValidationAction::MoveToReview,
            "",
            &me(),
        )
        .await
        .unwrap();
    assert_eq!(after.status, Status::ReadyForReview);
    assert_eq!(
        after.flags.review_entry.unwrap().to_string(),
        "Validated · round 1"
    );
    w.stop().await;
}

/// A base that diverged from its remote stops with GIT-01's message.
#[tokio::test(flavor = "multi_thread")]
async fn a_diverged_base_stops_it() {
    let w = world().await;
    let clone = w.repos.clone_with_origin("shop-web", "main", 1);
    w.board
        .add_project(&otaq_core::board::projects::AddProject {
            path: clone.display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let seed = w.repos.root().join("shop-web-seed");
    auto_merge(&w, AutoMerge::All).await;
    let t = working(&w, "Checkout API", None, "api.ts").await;
    commit(&seed, "team.ts", "from the team\n");
    git(&seed, &["push", "--quiet", "origin", "main"]);
    commit(&clone, "mine.ts", "local only\n");
    let after = pass(&w, &t).await;
    assert_eq!(after.status, Status::AutoMergeStopped);
    assert_eq!(after.auto_merge_stop, Some(AutoMergeStop::BaseDiverged));
    assert_eq!(
        after.signal.unwrap().text.as_deref(),
        Some("`main` and `origin/main` have diverged. Push or pull, then retry")
    );
    w.stop().await;
}

/// Auto-merge's conflict goes back to the agent without a click: a build freed at Review comes back before the
/// restack skill is queued.
#[tokio::test(flavor = "multi_thread")]
async fn a_conflict_handed_to_the_agent_gets_the_build_back() {
    let w = freed::world().await;
    let project = freed::shop_web(&w, |s| s.auto_merge = Some(AutoMerge::All)).await;
    let repo = PathBuf::from(&project.repo_path);
    let (t, wt) = freed::freed(&w, "Pricing", "lib.ts").await;
    let key = t.key.to_string();
    let agent = w.agent_shell(&t).await;
    commit(&repo, "lib.ts", "main\n");

    assert_eq!(
        w.review.auto_merge(&key).await.unwrap(),
        AutoMerged::Conflict
    );

    assert_eq!(w.status(&key).await, Status::MergeConflictFixing);
    assert_eq!(freed::app_bin(&wt).as_deref(), Some("Pricing"));
    assert_eq!(freed::state(&w, &key).await, BuildState::Present);
    assert_eq!(freed::entries(&w, &project), Vec::<u64>::new());
    let pending = w.deliveries.pending(&agent.session).await.unwrap();
    assert_eq!(pending.last().unwrap().text, naming::skill("restack"));
    w.stop().await;
}
