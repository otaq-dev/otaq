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

/// The running validator's session.
fn validator_of(w: &TestService, t: &Ticket) -> String {
    w.sessions
        .running_of(&t.id, SessionKind::Validator)
        .expect("a validator runs")
        .id
}

async fn verdict(w: &TestService, t: &Ticket, pass: bool, report: &str) -> Ticket {
    let session = validator_of(w, t);
    w.validation
        .verdict(&t.key.to_string(), pass, report, &session)
        .await
        .unwrap()
}

async fn set_rounds(w: &TestService, rounds: u32) {
    w.board
        .set_project_settings(
            "SHOP",
            &ProjectSettings {
                validation_rounds: Some(rounds),
                ..ProjectSettings::default()
            },
        )
        .await
        .unwrap();
}

async fn outcomes(
    w: &TestService,
    t: &Ticket,
) -> Vec<(i64, Option<String>, Option<String>, Option<String>)> {
    sqlx::query_as(
        "SELECT round, outcome, report, notes_json FROM validations WHERE ticket_id = ? ORDER BY started_at",
    )
    .bind(&t.id)
    .fetch_all(w.board.store().pool())
    .await
    .unwrap()
}

const FINDINGS: &str =
    "src/cart.ts:12 the total ignores the discount\n- Note: covered by SHOP-40.2";

/// Pass: Ready for Review "Validated · round 1", the round's head is the reviewed head, the report is stored.
#[tokio::test(flavor = "multi_thread")]
async fn a_pass_enters_review_validated() {
    let w = world().await;
    w.add_project("shop-web").await;
    let t = working(&w, "Cart totals").await;
    submit(&w, &t).await.unwrap();
    let head = rounds(&w, &t).await[0].3.clone();
    let after = verdict(
        &w,
        &t,
        true,
        "Checked the totals.\nNote: covered by SHOP-40.2",
    )
    .await;
    assert_eq!(after.status, Status::ReadyForReview);
    let entry = after.flags.review_entry.unwrap();
    assert_eq!(entry.to_string(), "Validated · round 1");
    let row = w.board.store().ticket(&t.id).await.unwrap().unwrap();
    assert_eq!(row.review_head_sha, head);
    assert!(row.review_entered_at.is_some());
    assert_eq!(
        outcomes(&w, &t).await,
        [(
            1,
            Some("pass".into()),
            Some("Checked the totals.\nNote: covered by SHOP-40.2".into()),
            Some(r#"["covered by SHOP-40.2"]"#.into())
        )]
    );
    w.stop().await;
}

/// Fail with rounds left: the findings go to the agent with "round N of M" and it works again; the last round
/// turns orange and delivers nothing.
#[tokio::test(flavor = "multi_thread")]
async fn a_fail_goes_back_to_the_agent_until_the_last_round() {
    let w = world().await;
    w.add_project("shop-web").await;
    let t = working(&w, "Cart totals").await;
    let agent = w.agent_shell(&t).await;

    submit(&w, &t).await.unwrap();
    let after = verdict(&w, &t, false, FINDINGS).await;
    assert_eq!(after.status, Status::Working);
    assert_eq!(
        after.flags.validation_round.unwrap().to_string(),
        "Validation failed · round 1 of 3"
    );
    let pending = w.deliveries.pending(&agent.session).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].kind, "validation");
    assert_eq!(
        pending[0].text,
        format!(
            "Validation failed (round 1 of 3). Fix these, commit, and submit again:\n\n{FINDINGS}"
        )
    );

    for round in 2..=3 {
        let t = w.board.ticket(&t.key.to_string()).await.unwrap();
        submit(&w, &t).await.unwrap();
        assert_eq!(rounds(&w, &t).await.len(), round, "round {round} opens");
        verdict(&w, &t, false, FINDINGS).await;
    }
    let last = w.board.ticket(&t.key.to_string()).await.unwrap();
    assert_eq!(last.status, Status::ValidationFailed);
    assert_eq!(last.validation_round, 3);
    assert_eq!(
        w.deliveries.pending(&agent.session).await.unwrap().len(),
        2,
        "rounds 1 and 2 were delivered, the last one isn't"
    );
    let all = outcomes(&w, &t).await;
    assert_eq!(all.len(), 3);
    assert!(
        all.iter()
            .all(|(_, o, r, _)| o.as_deref() == Some("fail") && r.is_some())
    );
    w.stop().await;
}

/// One round: the first failure is the last. Five rounds: four go back to the agent.
#[tokio::test(flavor = "multi_thread")]
async fn rounds_one_and_five() {
    let w = world().await;
    w.add_project("shop-web").await;
    set_rounds(&w, 1).await;
    let one = working(&w, "Cart totals").await;
    submit(&w, &one).await.unwrap();
    assert_eq!(
        verdict(&w, &one, false, FINDINGS).await.status,
        Status::ValidationFailed
    );

    set_rounds(&w, 5).await;
    let five = working(&w, "Checkout").await;
    for round in 1..=5 {
        let t = w.board.ticket(&five.key.to_string()).await.unwrap();
        submit(&w, &t).await.unwrap();
        let after = verdict(&w, &t, false, FINDINGS).await;
        let expected = if round < 5 {
            Status::Working
        } else {
            Status::ValidationFailed
        };
        assert_eq!(after.status, expected, "round {round}");
    }
    w.stop().await;
}

/// Only the round's own validator session reports; the round's verdict ends it once its turn ends.
#[tokio::test(flavor = "multi_thread")]
async fn only_the_running_validator_reports_and_it_ends_after_its_turn() {
    let w = world().await;
    w.add_project("shop-web").await;
    let t = working(&w, "Cart totals").await;
    submit(&w, &t).await.unwrap();
    let err = w
        .validation
        .verdict(&t.key.to_string(), true, "", "another-session")
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NotAllowed));
    let session = validator_of(&w, &t);
    verdict(&w, &t, true, "ok").await;
    assert!(
        w.sessions.info(&session).unwrap().is_running(),
        "its turn isn't over yet"
    );
    w.validation.after_validator_turn(&session).await.unwrap();
    assert!(
        w.sessions
            .running_of(&t.id, SessionKind::Validator)
            .is_none()
    );
    w.stop().await;
}

/// After Send back the counter restarts at 0, and `prime --validate` lists the review comments to check.
#[tokio::test(flavor = "multi_thread")]
async fn send_back_restarts_the_counter_and_prime_shows_the_comments() {
    use crewboard_core::board::comments::{NewComment, Side};
    let w = world().await;
    w.add_project("shop-web").await;
    let t = working(&w, "Cart totals").await;
    let _agent = w.agent_shell(&t).await;
    submit(&w, &t).await.unwrap();
    verdict(&w, &t, false, FINDINGS).await;
    let t = w.board.ticket(&t.key.to_string()).await.unwrap();
    submit(&w, &t).await.unwrap();
    verdict(&w, &t, true, "ok").await;
    let t = w.board.ticket(&t.key.to_string()).await.unwrap();
    assert_eq!(
        t.flags.review_entry.unwrap().to_string(),
        "Validated · round 2"
    );

    w.board
        .add_comment(&NewComment {
            key: t.key.to_string(),
            file: "src/cart.ts".into(),
            line: 12,
            side: Side::New,
            body: "Round the total".into(),
        })
        .await
        .unwrap();
    w.review
        .send_back(
            &t.key.to_string(),
            "Also the tax",
            false,
            &Actor::User("me".into()),
        )
        .await
        .unwrap();
    let back = w.board.ticket(&t.key.to_string()).await.unwrap();
    assert_eq!(back.validation_round, 0);
    assert!(back.flags.review_entry.is_none());

    submit(&w, &back).await.unwrap();
    let prime = w.board.validate_prime(&t.key.to_string()).await.unwrap();
    assert_eq!((prime.round, prime.rounds), (1, 3));
    let text = crewboard_core::prime::render_validate(&prime);
    assert!(
        text.contains("## The user's review (Send back, round 1)"),
        "{text}"
    );
    assert!(text.contains("Note: Also the tax"), "{text}");
    assert!(text.contains("- src/cart.ts:12: Round the total"), "{text}");
    assert!(
        !text.contains("Earlier rounds"),
        "the counter restarted: {text}"
    );
    w.stop().await;
}
