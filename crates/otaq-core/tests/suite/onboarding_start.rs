//! T-0428 (ONB-05, d0108, d0068): **Look and propose a flow** opens the project's first Project agent on the
//! Project agent role's model with the `flow` skill and a first message naming what there is; refused while the
//! tool is at its limit; a second start while it runs returns it. `claude` stands in as `sh` printing its
//! arguments, then `cat`; the history file is made up, in a throwaway config folder; `gh` and `glab` aren't on PATH.

use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::agent_tools::{LaunchContext, Tool};
use otaq_core::agents::{AgentModel, Role};
use otaq_core::client::{ClientError, request_once};
use otaq_core::naming;
use otaq_core::onboarding::HISTORY_FILE;
use otaq_core::project_agents::first_flow::NOTHING_TO_LOOK_AT;
use otaq_core::project_agents::{AgentState, LIMIT_REFUSAL, ProjectAgent};
use otaq_core::protocol::{ErrorBody, ProjectParams, Reply, Request};
use otaq_core::service::testing::TestService;
use otaq_core::store::now_ms;
use otaq_core::tools::CheckEnv;
use otaq_core::usage::limit::{LimitHit, LimitKind};

fn fake_launch() -> LaunchContext {
    LaunchContext {
        bin_dir: "/nonexistent".into(),
        plugin_dir: "/nonexistent/plugin".into(),
        path: std::env::var_os("PATH").unwrap_or_default(),
        claude: ClaudeLaunch {
            program: vec![
                "sh".into(),
                "-c".into(),
                "printf 'ARGS %s\\n' \"$*\"; exec cat".into(),
                "sh".into(),
            ],
            config_dir: None,
        },
    }
}

struct World {
    w: TestService,
    _tmp: tempfile::TempDir,
}

/// A service with a made-up history (one entry) and no `gh` or `glab`, and the project shop-web (one commit).
async fn world(history: bool) -> World {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    let claude = tmp.path().join("claude");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&claude).unwrap();
    if history {
        // Made up, shaped like the real file.
        std::fs::write(
            claude.join(HISTORY_FILE),
            r#"{"display":"/commit","pastedContents":{},"project":"/Users/sam/shop-web","sessionId":"s1","timestamp":1760000000000}"#,
        )
        .unwrap();
    }
    let env = CheckEnv {
        path: bin.as_os_str().to_owned(),
        claude_config_dir: Some(claude.clone()),
        managed_settings_dir: claude.join("managed"),
    };
    let w = TestService::start_with(move |mut c| {
        c.tools = env;
        c.launch = Some(fake_launch());
        c
    })
    .await;
    w.add_project("shop-web").await;
    World { w, _tmp: tmp }
}

async fn start_flow(w: &TestService, project: &str) -> Result<ProjectAgent, ErrorBody> {
    let reply = request_once(
        &w.data.socket_path(),
        Request::OnboardingStartFlow(ProjectParams {
            project: project.into(),
        }),
    )
    .await;
    match reply {
        Ok(Reply::ProjectAgent(agent)) => Ok(*agent),
        Ok(other) => panic!("{other:?}"),
        Err(ClientError::Service(body)) => Err(body),
        Err(e) => panic!("{e}"),
    }
}

async fn first_request(w: &TestService, id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT first_request FROM project_agents WHERE id = ?")
        .bind(id)
        .fetch_one(w.board.store().pool())
        .await
        .unwrap()
}

async fn agents(w: &TestService) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM project_agents")
        .fetch_one(w.board.store().pool())
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_start_opens_a_project_agent_on_its_roles_model_with_the_flow_skill() {
    let World { w, _tmp } = world(true).await;
    let role = w
        .board
        .store()
        .role_agent(Role::ProjectAgent)
        .await
        .unwrap()
        .expect("a Project agent");
    w.set_agent_model(
        &role.name,
        AgentModel::Tool {
            tool: Tool::Claude,
            model: Some("opus".into()),
        },
    )
    .await;

    let agent = start_flow(&w, "SHOP").await.unwrap();
    assert_eq!(agent.state, AgentState::Running);
    assert_eq!(agent.epic, None);
    assert_eq!(
        (agent.tool, agent.model.as_deref()),
        (Tool::Claude, Some("opus"))
    );
    let cmd = naming::COMMAND_NAME;
    let message = format!(
        "This is shop-web's first flow. Look at how the user works: their Claude Code history (`{cmd} history`), \
         this repo. Then propose a whole flow."
    );
    assert_eq!(
        first_request(&w, &agent.id).await,
        Some(format!("/flow {message}"))
    );
    let session = agent.session.as_ref().expect("a session to attach to");
    let out = w.wait_for_output(&session.id, "ARGS ").await;
    let args = out
        .lines()
        .find(|l| l.starts_with("ARGS "))
        .unwrap_or_default()
        .trim_end();
    assert!(args.contains("--model opus"), "{out}");
    assert!(
        args.ends_with(&format!("/{} {message}", naming::copy_name("flow"))),
        "{out}"
    );

    // A second start while it runs returns it; once it is closed, a start opens a new one.
    let again = start_flow(&w, "SHOP").await.unwrap();
    assert_eq!(again.id, agent.id);
    assert_eq!(agents(&w).await, 1);
    // An ordinary Project agent of the project isn't the onboarding agent.
    w.project_agents
        .open("SHOP", "Why is checkout slow?")
        .await
        .unwrap();
    assert_eq!(start_flow(&w, "SHOP").await.unwrap().id, agent.id);
    w.project_agents.close(&agent.id, true).await.unwrap();
    let next = start_flow(&w, "SHOP").await.unwrap();
    assert_ne!(next.id, agent.id);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_start_is_refused_at_the_limit_and_with_nothing_to_look_at() {
    let World { w, _tmp } = world(true).await;
    let hit = LimitHit {
        kind: LimitKind::Session,
        text: "You've hit your session limit · resets 3:45pm".into(),
    };
    w.plan_limit
        .record_hit(Tool::Claude, &hit, now_ms())
        .await
        .unwrap();
    let e = start_flow(&w, "SHOP").await.unwrap_err();
    assert_eq!(e.message, LIMIT_REFUSAL);
    assert_eq!(agents(&w).await, 0, "nothing stored");
    w.plan_limit.resume_tool(Tool::Claude).await.unwrap();
    start_flow(&w, "SHOP").await.unwrap();
    w.stop().await;

    // No history, no `gh` or `glab`, and a repo with no commits: nothing to look at.
    let World { w, _tmp } = world(false).await;
    let repo = w.repos.repo("empty-api", 0);
    w.board
        .add_project(&otaq_core::board::projects::AddProject {
            path: repo.display().to_string(),
            key: Some("API".into()),
            base_branch: Some("main".into()),
        })
        .await
        .unwrap();
    let e = start_flow(&w, "API").await.unwrap_err();
    assert_eq!(e.message, NOTHING_TO_LOOK_AT);
    assert_eq!(agents(&w).await, 0);
    w.stop().await;
}
