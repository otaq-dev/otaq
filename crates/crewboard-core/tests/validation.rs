//! Validation before Review (T-0183; VAL-01, VAL-13, SCH-04; d0063): the agent's submit opens a round and
//! starts a validator in the ticket's worktree; validation off keeps the old path; uncommitted files refuse it
//! first; a project's Validate skill is the prompt; under the usage limit it waits for Resume all.
//! `claude` stands in as `sh` printing its arguments and environment: no test starts claude.

use crewboard_core::board::Refusal;
use crewboard_core::board::settings::{ProjectSettings, WorkflowSkillOverrides};
use crewboard_core::board::tickets::Ticket;
use crewboard_core::client::{Client, ClientConfig};
use crewboard_core::domain::{Actor, Status, ValidationTrigger};
use crewboard_core::naming;
use crewboard_core::protocol::{Reply, Request};
use crewboard_core::service::ServiceConfig;
use crewboard_core::service::testing::TestService;
use crewboard_core::session::SessionKind;
use crewboard_core::session::launch::LaunchContext;
use crewboard_core::store::now_ms;
use crewboard_core::usage::limit::{LimitHit, LimitKind};

/// A service whose `claude` prints its arguments, its role, ticket and PORT, then waits.
async fn world() -> TestService {
    let script = format!(
        "printf 'ARGS %s\\n' \"$*\"; printf 'ROLE=%s TICKET=%s PORT=%s\\n' \"${role}\" \"${ticket}\" \
         \"${{PORT-none}}\"; printf 'CWD=%s\\n' \"$(pwd -P)\"; exec sleep 30",
        role = naming::role_env_var(),
        ticket = naming::ticket_env_var(),
    );
    TestService::start_with(|config| ServiceConfig {
        launch: Some(LaunchContext {
            claude: vec!["sh".into(), "-c".into(), script, "sh".into()],
            bin_dir: "/nonexistent".into(),
            plugin_dir: "/nonexistent/plugin".into(),
            path: std::env::var_os("PATH").unwrap_or_default(),
            claude_config_dir: None,
        }),
        ..config
    })
    .await
}

/// A working ticket with a committed worktree.
async fn working(w: &TestService, title: &str) -> Ticket {
    let t = w.create("SHOP", title).await;
    w.give_worktree(&t).await;
    w.force_status(&t, Status::Working).await;
    w.board.ticket(&t.key.to_string()).await.unwrap()
}

fn agent(t: &Ticket) -> Actor {
    Actor::Agent(t.key.clone())
}

async fn submit(w: &TestService, t: &Ticket) -> Result<Ticket, crewboard_core::board::BoardError> {
    w.validation
        .submit(
            &t.key.to_string(),
            "Cart totals add up.",
            &agent(t),
            ValidationTrigger::Agent,
        )
        .await
}

async fn rounds(w: &TestService, t: &Ticket) -> Vec<(i64, String, Option<String>, Option<String>)> {
    sqlx::query_as(
        "SELECT round, trigger, session_id, head_sha FROM validations WHERE ticket_id = ? ORDER BY started_at",
    )
    .bind(&t.id)
    .fetch_all(w.board.store().pool())
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn submit_starts_a_validator_in_the_worktree() {
    let w = world().await;
    w.add_project("shop-web").await;
    let t = working(&w, "Cart totals").await;

    let after = submit(&w, &t).await.unwrap();
    assert_eq!((after.column, after.status), (t.column, Status::Validating));
    let validator = w
        .sessions
        .running_of(&t.id, SessionKind::Validator)
        .expect("a validator runs");
    let out = w
        .wait_for_output(&validator.id, &naming::skill("validate"))
        .await;
    assert!(
        out.contains(&format!(
            "ROLE={} TICKET={} PORT=none",
            naming::ROLE_VALIDATOR,
            t.key
        )),
        "{out}"
    );
    assert!(!out.contains("work-ticket"), "{out}");
    let worktree = t.work.as_ref().unwrap().worktree.clone().unwrap();
    let worktree = std::fs::canonicalize(worktree).unwrap();
    assert!(
        out.contains(&format!("CWD={}", worktree.display())),
        "runs in the worktree: {out}"
    );
    assert!(
        validator.claude_session_id.is_some(),
        "stored like every Claude session"
    );

    // The round: 1, started by the agent, with the validator's session and the head.
    let rows = rounds(&w, &t).await;
    assert_eq!(rows.len(), 1);
    let (round, trigger, session, head) = &rows[0];
    assert_eq!((*round, trigger.as_str()), (1, "agent"));
    assert_eq!(session.as_deref(), Some(validator.id.as_str()));
    assert_eq!(head.as_ref().map(String::len), Some(40));
    // The summary is the ticket's latest submit, for prime and the dependents.
    let history = w.board.history(&t.key.to_string()).await.unwrap();
    assert!(
        history
            .iter()
            .any(|e| e.payload["summary"] == "Cart totals add up.")
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn validation_off_goes_straight_to_review() {
    let w = world().await;
    w.add_project("shop-web").await;
    w.board
        .set_project_settings(
            "SHOP",
            &ProjectSettings {
                validation: Some(false),
                ..ProjectSettings::default()
            },
        )
        .await
        .unwrap();
    let t = working(&w, "Cart totals").await;
    let after = submit(&w, &t).await.unwrap();
    assert_eq!(after.status, Status::ReadyForReview);
    assert!(
        w.sessions
            .running_of(&t.id, SessionKind::Validator)
            .is_none()
    );
    assert!(rounds(&w, &t).await.is_empty());
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn uncommitted_files_refuse_before_any_validator() {
    let w = world().await;
    w.add_project("shop-web").await;
    let t = working(&w, "Cart totals").await;
    let worktree = t.work.as_ref().unwrap().worktree.clone().unwrap();
    std::fs::write(std::path::Path::new(&worktree).join("scratch.txt"), "x").unwrap();
    let err = submit(&w, &t).await.unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::Uncommitted));
    assert_eq!(w.status(&t.key.to_string()).await, Status::Working);
    assert!(
        w.sessions
            .running_of(&t.id, SessionKind::Validator)
            .is_none()
    );
    assert!(rounds(&w, &t).await.is_empty());
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_projects_validate_skill_is_the_prompt() {
    let w = world().await;
    w.add_project("shop-web").await;
    w.board
        .set_project_settings(
            "SHOP",
            &ProjectSettings {
                workflow_skills: WorkflowSkillOverrides {
                    validate: "/my-validate".into(),
                    ..WorkflowSkillOverrides::default()
                },
                ..ProjectSettings::default()
            },
        )
        .await
        .unwrap();
    let t = working(&w, "Cart totals").await;
    submit(&w, &t).await.unwrap();
    let validator = w
        .sessions
        .running_of(&t.id, SessionKind::Validator)
        .unwrap();
    let out = w.wait_for_output(&validator.id, "ARGS").await;
    let out = if out.contains("/my-validate") {
        out
    } else {
        w.wait_for_output(&validator.id, "/my-validate").await
    };
    assert!(!out.contains(&naming::skill("validate")), "{out}");
    w.stop().await;
}

/// SCH-04: while the usage limit holds, the round opens but no validator starts; Resume all starts it.
#[tokio::test(flavor = "multi_thread")]
async fn under_the_usage_limit_the_validator_waits_for_resume_all() {
    let w = world().await;
    w.add_project("shop-web").await;
    let other = working(&w, "Search").await;
    let hit = LimitHit {
        kind: LimitKind::Session,
        text: "You've hit your session limit · resets 3:45pm".into(),
    };
    w.plan_limit.hit(&hit, &other.id, now_ms()).await.unwrap();

    let t = working(&w, "Cart totals").await;
    let after = submit(&w, &t).await.unwrap();
    assert_eq!(after.status, Status::WaitingForLimitReset);
    assert!(
        w.sessions
            .running_of(&t.id, SessionKind::Validator)
            .is_none()
    );
    assert_eq!(rounds(&w, &t).await.len(), 1, "the round is open");

    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let reply = client.request(Request::UsageResumeAll).await.unwrap();
    let Reply::Resumed(keys) = reply else {
        panic!("{reply:?}")
    };
    assert!(keys.contains(&t.key), "{keys:?}");
    assert_eq!(w.status(&t.key.to_string()).await, Status::Validating);
    let validator = w
        .sessions
        .running_of(&t.id, SessionKind::Validator)
        .expect("started on Resume all");
    w.wait_for_output(&validator.id, &naming::skill("validate"))
        .await;
    assert_eq!(
        rounds(&w, &t).await[0].2.as_deref(),
        Some(validator.id.as_str())
    );
    w.stop().await;
}
