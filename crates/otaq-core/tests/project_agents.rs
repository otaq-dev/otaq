//! Project agents (T-0199; PAG-01, PAG-04, PAG-06, PAG-07, SCH-03, SCH-04, REC-01; d0068): opening one starts
//! `claude` with the role and the skill in the project's folder; naming (the agent's own once, the fallback after
//! the first turn, the user's rename wins); close, discard, Ended by restart · Resume and the recent list; never
//! counted toward the agent limit; none opens during the usage limit, and limit-stopped ones continue on Resume all.
//! `claude` stands in as `sh` printing its arguments and environment, then `cat` echoing what is typed into it: no
//! test starts claude. (On Windows Git's `sh` prints the folder with `pwd -W`, as Windows writes it, and the console
//! echoes only what a program reads, hence `cat` rather than `sleep`.)

use otaq_core::board::Refusal;
use otaq_core::caller::Caller;
use otaq_core::client::{Client, ClientConfig};
use otaq_core::naming;
use otaq_core::project_agents::{AgentState, LIMIT_REFUSAL, NEW_AGENT, NameSource, ProjectAgent};
use otaq_core::protocol::{Reply, Request};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use otaq_core::session::SessionKind;
use otaq_core::session::launch::LaunchContext;
use otaq_core::signals::HookEvent;
use otaq_core::store::now_ms;
use otaq_core::usage::limit::{LimitHit, LimitKind};
use serde_json::{Value, json};

fn fake_launch() -> LaunchContext {
    let script = format!(
        "printf 'ARGS %s\\n' \"$*\"; printf 'ROLE=%s TICKET=%s PORT=%s\\n' \"${role}\" \"${{{ticket}-none}}\" \
         \"${{PORT-none}}\"; printf 'CWD=%s\\n' \"$(pwd -W 2>/dev/null || pwd -P)\"; exec cat",
        role = naming::role_env_var(),
        ticket = naming::ticket_env_var(),
    );
    LaunchContext {
        claude: vec!["sh".into(), "-c".into(), script, "sh".into()],
        bin_dir: "/nonexistent".into(),
        plugin_dir: "/nonexistent/plugin".into(),
        path: std::env::var_os("PATH").unwrap_or_default(),
        claude_config_dir: None,
    }
}

/// A folder as the fake `claude` prints it: on Windows without the `\\?\` prefix and with forward slashes, as
/// `pwd -W` writes it.
fn folder_text(folder: &std::path::Path) -> String {
    let text = folder.display().to_string();
    if cfg!(windows) {
        text.trim_start_matches(r"\\?\").replace('\\', "/")
    } else {
        text
    }
}

fn with_fake_claude(config: ServiceConfig) -> ServiceConfig {
    ServiceConfig {
        launch: Some(fake_launch()),
        ..config
    }
}

async fn world() -> TestService {
    let w = TestService::start_with(with_fake_claude).await;
    w.add_project("shop-web").await;
    w
}

fn caller(agent: &ProjectAgent) -> Caller {
    Caller::ProjectAgent {
        project: agent.project.clone(),
        agent: agent.id.clone(),
        session: agent.session.as_ref().expect("a session").id.clone(),
    }
}

/// A hook of the agent, as the service handles it: the signals map it, the Project agents apply it.
async fn hook(w: &TestService, agent: &ProjectAgent, event: HookEvent, payload: Value) {
    let signal = w
        .signals
        .hook(&caller(agent), event, &payload)
        .await
        .unwrap();
    w.project_agents
        .hook(&agent.id, event, &payload, &signal)
        .await
        .unwrap();
}

async fn prompt(w: &TestService, agent: &ProjectAgent, text: &str) {
    hook(
        w,
        agent,
        HookEvent::UserPromptSubmit,
        json!({ "prompt": text }),
    )
    .await;
}

async fn stop(w: &TestService, agent: &ProjectAgent) {
    hook(
        w,
        agent,
        HookEvent::Stop,
        json!({ "last_assistant_message": "Done." }),
    )
    .await;
}

fn refusal(e: otaq_core::board::BoardError) -> (Option<Refusal>, String) {
    (e.refusal(), e.to_string())
}

#[tokio::test(flavor = "multi_thread")]
async fn opening_starts_claude_with_the_role_and_the_skill_several_at_once() {
    let w = world().await;
    let repo = folder_text(
        &std::fs::canonicalize(w.board.project("SHOP").await.unwrap().repo_path).unwrap(),
    );
    let a = w.project_agents.open("SHOP").await.unwrap();
    let b = w.project_agents.open("SHOP").await.unwrap();
    for agent in [&a, &b] {
        assert_eq!(agent.name, NEW_AGENT);
        assert_eq!(agent.name_source, None);
        assert_eq!(agent.state, AgentState::Running);
        assert!(!agent.got_request);
        let session = agent.session.as_ref().unwrap();
        assert_eq!(session.kind, SessionKind::ProjectAgent);
        assert_eq!(session.ticket, None);
        assert_eq!(
            session.claude_session_id.as_deref(),
            Some(agent.claude_session_id.as_str())
        );
        let out = w.wait_for_output(&session.id, "CWD=").await;
        assert!(
            out.contains(&format!(
                "--session-id {} /{}:project-agent",
                agent.claude_session_id,
                naming::PLUGIN_NAMESPACE
            )),
            "{out}"
        );
        assert!(
            out.contains(&format!("--allowedTools Bash({}:*)", naming::COMMAND_NAME)),
            "{out}"
        );
        assert!(!out.contains("permission"), "{out}");
        assert!(
            out.contains(&format!(
                "ROLE={} TICKET=none PORT=none",
                naming::ROLE_PROJECT_AGENT
            )),
            "{out}"
        );
        assert!(out.contains(&format!("CWD={repo}")), "{out}");
    }
    assert_ne!(a.claude_session_id, b.claude_session_id);
    let list = w.project_agents.list("SHOP").await.unwrap();
    assert_eq!(
        list.open.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
        [a.id.as_str(), b.id.as_str()]
    );
    assert!(
        list.open
            .iter()
            .all(|a| a.session.as_ref().unwrap().is_running())
    );
    assert!(list.recent.is_empty());
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_agent_names_itself_once_and_the_users_rename_wins() {
    let w = world().await;
    let a = w.project_agents.open("SHOP").await.unwrap();
    let named = w
        .project_agents
        .name_by_agent(&a.id, "Checkout slowness", false)
        .await
        .unwrap();
    assert_eq!(named.name, "Checkout slowness");
    assert_eq!(named.name_source, Some(NameSource::Agent));

    // Once: a second call needs --rename (the user asked for it).
    let (code, text) = refusal(
        w.project_agents
            .name_by_agent(&a.id, "Something else", false)
            .await
            .unwrap_err(),
    );
    assert_eq!(code, Some(Refusal::NotAllowed));
    assert!(
        text.starts_with("This Project agent is already named \"Checkout slowness\""),
        "{text}"
    );
    let renamed = w
        .project_agents
        .name_by_agent(&a.id, "Checkout queries", true)
        .await
        .unwrap();
    assert_eq!(renamed.name, "Checkout queries");

    // The user's rename always wins.
    let mine = w
        .project_agents
        .rename(&a.id, "Slow checkout")
        .await
        .unwrap();
    assert_eq!(mine.name_source, Some(NameSource::User));
    for rename in [false, true] {
        let (code, text) = refusal(
            w.project_agents
                .name_by_agent(&a.id, "Agent's choice", rename)
                .await
                .unwrap_err(),
        );
        assert_eq!(code, Some(Refusal::NotAllowed));
        assert_eq!(
            text,
            "The user named this Project agent \"Slow checkout\"; their name stays"
        );
    }
    // Nor does the first turn's fallback overwrite it.
    prompt(&w, &mine, "why is checkout slow?").await;
    stop(&w, &mine).await;
    assert_eq!(
        w.project_agents.get(&a.id).await.unwrap().name,
        "Slow checkout"
    );
    let (code, _) = refusal(w.project_agents.rename(&a.id, "  ").await.unwrap_err());
    assert_eq!(code, Some(Refusal::EmptyField));
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_name_the_first_words_of_the_request_name_it_after_its_first_turn() {
    let w = world().await;
    let a = w.project_agents.open("SHOP").await.unwrap();
    // The skill's own turn: no request yet.
    prompt(&w, &a, &naming::skill("project-agent")).await;
    stop(&w, &a).await;
    let a = w.project_agents.get(&a.id).await.unwrap();
    assert_eq!(a.name, NEW_AGENT);
    assert!(!a.got_request);

    prompt(
        &w,
        &a,
        "Why is checkout so slow? Make tickets for what you find",
    )
    .await;
    let a = w.project_agents.get(&a.id).await.unwrap();
    assert!(a.got_request);
    assert!(a.mid_turn);
    assert_eq!(a.name, NEW_AGENT, "only at the end of the turn");
    stop(&w, &a).await;
    let a = w.project_agents.get(&a.id).await.unwrap();
    assert_eq!(a.name, "Why is checkout so");
    assert_eq!(a.name_source, Some(NameSource::Fallback));
    assert!(!a.mid_turn);

    // A later request changes nothing; the agent's own name may still replace the fallback.
    prompt(&w, &a, "Now look at the cart").await;
    stop(&w, &a).await;
    assert_eq!(
        w.project_agents.get(&a.id).await.unwrap().name,
        "Why is checkout so"
    );
    let named = w
        .project_agents
        .name_by_agent(&a.id, "Checkout slowness", false)
        .await
        .unwrap();
    assert_eq!(named.name, "Checkout slowness");
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn close_discards_an_unused_agent_and_asks_first_mid_turn() {
    let w = world().await;
    // A "New agent" that never got a request: discarded, no confirmation, not listed.
    let unused = w.project_agents.open("SHOP").await.unwrap();
    w.project_agents.close(&unused.id, false).await.unwrap();
    assert!(w.project_agents.get(&unused.id).await.is_err());
    let session = unused.session.as_ref().unwrap();
    assert!(!w.sessions.info(&session.id).unwrap().is_running());
    let list = w.project_agents.list("SHOP").await.unwrap();
    assert!(list.open.is_empty() && list.recent.is_empty());

    // Mid-turn: needs the user's "Stop and close".
    let busy = w.project_agents.open("SHOP").await.unwrap();
    prompt(&w, &busy, "Find slow queries").await;
    let (code, text) = refusal(w.project_agents.close(&busy.id, false).await.unwrap_err());
    assert_eq!(code, Some(Refusal::NeedsConfirmation));
    assert_eq!(
        text,
        "New agent is in the middle of a turn. Stop it and close it?"
    );
    w.project_agents.close(&busy.id, true).await.unwrap();
    let busy = w.project_agents.get(&busy.id).await.unwrap();
    assert_eq!(busy.state, AgentState::Ended);
    assert!(busy.closed);
    assert!(!busy.session.as_ref().unwrap().is_running());
    let list = w.project_agents.list("SHOP").await.unwrap();
    assert!(list.open.is_empty());
    assert_eq!(list.recent.len(), 1);

    // Between turns it closes without asking.
    let idle = w.project_agents.open("SHOP").await.unwrap();
    prompt(&w, &idle, "Audit the footer").await;
    stop(&w, &idle).await;
    w.project_agents.close(&idle.id, false).await.unwrap();
    assert_eq!(
        w.project_agents.get(&idle.id).await.unwrap().state,
        AgentState::Ended
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_that_exits_ends_and_resume_continues_its_conversation() {
    let w = world().await;
    let a = w.project_agents.open("SHOP").await.unwrap();
    prompt(&w, &a, "Footer audit").await;
    let first = a.session.clone().unwrap();
    w.sessions.stop(&first.id).await.unwrap();
    let ended = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let a = w.project_agents.get(&a.id).await.unwrap();
            if a.state == AgentState::Ended {
                return a;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the agent ended");
    assert!(!ended.closed, "its tab stays until closed");
    assert!(ended.ended_at.is_some());

    let resumed = w.project_agents.resume(&a.id).await.unwrap();
    assert_eq!(resumed.state, AgentState::Running);
    let session = resumed.session.clone().unwrap();
    assert_ne!(session.id, first.id);
    assert_eq!(
        session.claude_session_id.as_deref(),
        Some(a.claude_session_id.as_str())
    );
    let out = w.wait_for_output(&session.id, "CWD=").await;
    assert!(
        out.contains(&format!("--resume {}", a.claude_session_id)),
        "{out}"
    );
    assert!(
        !out.contains(&naming::skill("project-agent")),
        "no skill on resume: {out}"
    );
    // Resuming a running agent changes nothing.
    let again = w.project_agents.resume(&a.id).await.unwrap();
    assert_eq!(again.session.unwrap().id, session.id);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn after_a_restart_agents_are_ended_by_restart_and_unused_ones_are_gone() {
    let w = world().await;
    let used = w.project_agents.open("SHOP").await.unwrap();
    prompt(&w, &used, "Import the GitHub issues labelled ready").await;
    let unused = w.project_agents.open("SHOP").await.unwrap();
    let w = w.restart_with(with_fake_claude).await;

    assert!(w.project_agents.get(&unused.id).await.is_err());
    let list = w.project_agents.list("SHOP").await.unwrap();
    assert_eq!(list.open.len(), 1);
    let after = &list.open[0];
    assert_eq!(after.id, used.id);
    assert_eq!(after.state, AgentState::EndedByRestart);
    assert!(!after.mid_turn);
    assert_eq!(list.recent.len(), 1);

    let resumed = w.project_agents.resume(&used.id).await.unwrap();
    assert_eq!(resumed.state, AgentState::Running);
    let out = w
        .wait_for_output(&resumed.session.unwrap().id, "CWD=")
        .await;
    assert!(
        out.contains(&format!("--resume {}", used.claude_session_id)),
        "{out}"
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_recent_list_holds_the_ten_newest_ended_agents() {
    let w = world().await;
    let project = w.board.project("SHOP").await.unwrap();
    for n in 0..12 {
        sqlx::query(
            "INSERT INTO project_agents (id, project_id, name, name_source, claude_session_id, state,
                                         first_request, created_at, ended_at, closed_at)
             VALUES (?, ?, ?, 'agent', ?, 'ended', 'r', ?, ?, ?)",
        )
        .bind(format!("a{n:02}"))
        .bind(&project.id)
        .bind(format!("Agent {n}"))
        .bind(format!("c{n}"))
        .bind(n)
        .bind(1000 + n)
        .bind(1000 + n)
        .execute(w.board.store().pool())
        .await
        .unwrap();
    }
    // An ended one that never got a request isn't listed.
    sqlx::query(
        "INSERT INTO project_agents (id, project_id, claude_session_id, state, created_at, ended_at)
         VALUES ('unused', ?, 'cu', 'ended', 0, 5000)",
    )
    .bind(&project.id)
    .execute(w.board.store().pool())
    .await
    .unwrap();
    let list = w.project_agents.list("SHOP").await.unwrap();
    assert!(list.open.iter().all(|a| a.id == "unused"));
    let names: Vec<&str> = list.recent.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Agent 11", "Agent 10", "Agent 9", "Agent 8", "Agent 7", "Agent 6", "Agent 5",
            "Agent 4", "Agent 3", "Agent 2"
        ]
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn agents_never_count_toward_the_limit_and_none_opens_during_the_usage_limit() {
    let w = world().await;
    let a = w.project_agents.open("SHOP").await.unwrap();
    w.project_agents.open("SHOP").await.unwrap();
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    match client.request(Request::SchedulerStatus).await.unwrap() {
        Reply::SchedulerStatus(s) => assert_eq!(s.holding, 0),
        other => panic!("{other:?}"),
    }
    match client.request(Request::ServiceStatus).await.unwrap() {
        Reply::ServiceStatus(s) => assert_eq!((s.agents_working, s.agents_waiting), (0, 0)),
        other => panic!("{other:?}"),
    }

    // The limit stops the agent: it waits for Resume all, and no new one opens meanwhile.
    prompt(&w, &a, "Find slow queries").await;
    let hit = LimitHit {
        kind: LimitKind::Session,
        text: "You've hit your session limit · resets 3:45pm".into(),
    };
    hook(
        &w,
        &a,
        HookEvent::StopFailure,
        json!({ "error": "rate_limit", "last_assistant_message": hit.text }),
    )
    .await;
    w.plan_limit.record_hit(&hit, now_ms()).await.unwrap();
    assert!(w.project_agents.get(&a.id).await.unwrap().limit_stopped);
    let (code, text) = refusal(w.project_agents.open("SHOP").await.unwrap_err());
    assert_eq!(code, Some(Refusal::MoveRefused));
    assert_eq!(text, LIMIT_REFUSAL);
    let other = w.project_agents.open("SHOP").await;
    assert!(other.is_err());

    // Resume all: "continue" is typed into its session (its turn has ended), and new agents open again.
    w.plan_limit.resume_all().await.unwrap();
    let session = a.session.as_ref().unwrap().id.clone();
    let delivered: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_deliveries WHERE session_id = ? AND kind = 'continue_after_limit'",
    )
    .bind(&session)
    .fetch_one(w.board.store().pool())
    .await
    .unwrap();
    let typed = w
        .sessions
        .attach(&session)
        .map(|s| String::from_utf8_lossy(&s.scrollback).contains("continue"))
        .unwrap_or(false);
    assert!(delivered == 1 || typed, "continue queued or typed");
    prompt(&w, &a, "continue").await;
    assert!(!w.project_agents.get(&a.id).await.unwrap().limit_stopped);
    w.project_agents.open("SHOP").await.unwrap();
    w.stop().await;
}

/// T-0201 (PAG-07, USG-01): a Project agent's tokens count per agent and in today's total.
#[tokio::test(flavor = "multi_thread")]
async fn tokens_count_per_agent_and_in_todays_total() {
    let w = world().await;
    let a = w.project_agents.open("SHOP").await.unwrap();
    let b = w.project_agents.open("SHOP").await.unwrap();
    let day = w.usage.summary(None).await.unwrap().day;
    for (agent, n) in [(&a, 1000), (&b, 10)] {
        sqlx::query(
            "INSERT INTO usage (session_id, day, tokens_in, tokens_out, cache_read_tokens, cache_write_tokens)
             VALUES (?, ?, ?, 300, 5000, 100)",
        )
        .bind(&agent.session.as_ref().unwrap().id)
        .bind(&day)
        .bind(n)
        .execute(w.board.store().pool())
        .await
        .unwrap();
    }
    assert_eq!(
        w.project_agents.get(&a.id).await.unwrap().tokens.shown(),
        1400
    );
    assert_eq!(
        w.project_agents.get(&b.id).await.unwrap().tokens.shown(),
        410
    );
    assert_eq!(w.usage.summary(None).await.unwrap().today.shown(), 1810);
    w.stop().await;
}

/// T-0205 (REC-01, REC-06; d0063, d0068): what the after-restart summary reads (`service.startup_report`): the
/// Project agents the restart ended (each resumable), how many unused "New agent"s are gone, and which interrupted
/// tickets were validating (their Resume starts a fresh validation: `tests/validation.rs`).
#[tokio::test(flavor = "multi_thread")]
async fn the_startup_report_names_ended_agents_and_validating_tickets() {
    let w = world().await;
    let used = w.project_agents.open("SHOP").await.unwrap();
    prompt(&w, &used, "Audit the footer").await;
    w.project_agents.open("SHOP").await.unwrap();
    let validating = w.create("SHOP", "Cart totals").await;
    w.force_status(&validating, otaq_core::domain::Status::Validating)
        .await;
    let w = w.restart_with(with_fake_claude).await;

    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let report = match client.request(Request::StartupReport).await.unwrap() {
        Reply::StartupReport(r) => *r,
        other => panic!("{other:?}"),
    };
    assert_eq!(report.project_agents_ended, std::slice::from_ref(&used.id));
    assert_eq!(report.new_agents_discarded, 1);
    assert_eq!(report.was_validating, std::slice::from_ref(&validating.key));
    assert!(report.interrupted.contains(&validating.key));
    let after = w.project_agents.get(&used.id).await.unwrap();
    assert_eq!(after.state, AgentState::EndedByRestart);
    // Resume from the summary continues its conversation.
    let resumed = w.project_agents.resume(&used.id).await.unwrap();
    assert_eq!(resumed.state, AgentState::Running);
    w.stop().await;
}
