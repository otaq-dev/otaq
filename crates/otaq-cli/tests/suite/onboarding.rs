//! Flow onboarding end to end (T-0432; ONB-11; d0108, d0056, d0079). The onboarding start opens the Project agent;
//! a stand-in `claude` reads the made-up history with the real `otaq history`, proposes a flow file with the real
//! `otaq flow propose`, gets the user's Ask for more changes typed in as its next message, proposes again, and the
//! user's Apply makes the second file the project's flow. A ticket's agent in the same run is refused the history.
//! Then the nothing-to-look-at case: no history, a repository with no commits, no `gh` or `glab`: the start is
//! refused and a preset applies instead. The history is the T-0426 fixture in a temporary Claude Code folder, never
//! the user's own; no real agent tool, `gh` or `glab` runs.

use std::path::{Path, PathBuf};

use otaq_core::agent_tools::LaunchContext;
use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::agent_tools::history::only_a_project_agent;
use otaq_core::agents::FixedModels;
use otaq_core::board::projects::AddProject;
use otaq_core::client::{ClientError, request_once};
use otaq_core::data_dir::DataDir;
use otaq_core::flow::apply::{ApplyParams, ProposeFrom, ProposeFromParams};
use otaq_core::flow::presets::preset;
use otaq_core::flow::proposal::{ApplyChoices, ProposedBy, Source};
use otaq_core::flow::propose::AskForChangesParams;
use otaq_core::git::testing::TempRepos;
use otaq_core::onboarding::Sources;
use otaq_core::project_agents::ProjectAgent;
use otaq_core::project_agents::first_flow::NOTHING_TO_LOOK_AT;
use otaq_core::protocol::{ErrorBody, ProjectParams, Reply, Request};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use otaq_core::tools::CheckEnv;
use serde_json::Value;

const CB: &str = env!("CARGO_BIN_EXE_otaq");

/// What the fixture's pastes hold: never printed.
const PASTE_MARKERS: &[&str] = &[
    "PASTE-MARKER-QUOKKA-1",
    "PASTE-MARKER-NARWHAL-2",
    "PASTE-MARKER-AXOLOTL-3",
];

/// The stand-in's first proposal: Grill first, and a Security check before the user's review.
const FIRST: &str = "\
agents:
  - name: Griller
    skills: [grill]
    model: default
  - name: Developer
    skills: [work-ticket, commit]
    model: default
  - name: Validator
    skills: [validate]
    model: default
flow:
  - id: grill
    name: Grill
    kind: agent
    column: own
    agent: Griller
    skill: grill
    why: Most of your sessions start with /grill
  - id: dev
    name: Dev
    kind: agent
    column: own
    agent: Developer
    skill: work-ticket
  - id: security
    name: Security
    kind: check
    by: agent
    column: own
    agent: Validator
    skill: validate
    rounds: 2
    why: You asked for a security review before 4 of your last merges
  - id: review
    name: Review
    kind: check
    by: you
    column: own
  - id: deliver
    name: Done
    kind: delivery
    column: with-previous
    agent: Developer
    skill: open-pr
    mode: click
    delivers: either
";

/// After "Leave out Grill": the same without it.
const SECOND: &str = "\
agents:
  - name: Developer
    skills: [work-ticket, commit]
    model: default
  - name: Validator
    skills: [validate]
    model: default
flow:
  - id: dev
    name: Dev
    kind: agent
    column: own
    agent: Developer
    skill: work-ticket
  - id: security
    name: Security
    kind: check
    by: agent
    column: own
    agent: Validator
    skill: validate
    rounds: 2
    why: You asked for a security review before 4 of your last merges
  - id: review
    name: Review
    kind: check
    by: you
    column: own
  - id: deliver
    name: Done
    kind: delivery
    column: with-previous
    agent: Developer
    skill: open-pr
    mode: click
    delivers: either
";

/// The stand-in `claude`: started with the first-flow message, it reads the history and proposes [`FIRST`]; each
/// "asks for more changes" typed into it then proposes [`SECOND`]. What it ran lands in `dir`, and a `STAND-IN …`
/// line says each step is done. Started any other way it is an interactive `sh`.
fn stand_in(dir: &Path, data: &Path) -> PathBuf {
    std::fs::write(dir.join("first.yaml"), FIRST).unwrap();
    std::fs::write(dir.join("second.yaml"), SECOND).unwrap();
    let d = dir.display();
    let script = format!(
        "export {home}='{data}'; export HOME='{d}/home'; export PS1=''\n\
         case \"$*\" in *\"first flow\"*) ;; *) exec sh ;; esac\n\
         '{CB}' history --all > '{d}/history.out' 2>&1; echo $? > '{d}/history.status'\n\
         '{CB}' flow propose --stdin --silent < '{d}/first.yaml' > '{d}/first.id' 2>&1\n\
         echo 'STAND-IN proposed the first flow'\n\
         while IFS= read -r line; do\n\
           case \"$line\" in *'asks for more changes'*)\n\
             '{CB}' flow propose --stdin --silent < '{d}/second.yaml' > '{d}/second.id' 2>&1\n\
             echo 'STAND-IN proposed the second flow' ;;\n\
           esac\n\
         done\n",
        home = DataDir::env_var(),
        data = data.display(),
    );
    let path = dir.join("claude-stand-in.sh");
    std::fs::write(&path, script).unwrap();
    path
}

/// The Claude Code folder, the stand-in's files and an empty PATH folder (no `gh`, no `glab`).
struct Mac {
    _tmp: tempfile::TempDir,
    claude: PathBuf,
    bin: PathBuf,
    files: PathBuf,
}

/// `history`: the fixture as the Claude Code folder's `history.jsonl`, or no file at all.
fn mac(history: bool) -> Mac {
    let tmp = tempfile::tempdir().unwrap();
    let claude = tmp.path().join("claude");
    let bin = tmp.path().join("bin");
    let files = tmp.path().join("stand-in");
    for dir in [&claude, &bin, &files, &files.join("home")] {
        std::fs::create_dir_all(dir).unwrap();
    }
    if history {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../otaq-core/tests/fixtures/claude-history.jsonl");
        std::fs::copy(fixture, claude.join("history.jsonl")).unwrap();
    }
    Mac {
        _tmp: tmp,
        claude,
        bin,
        files,
    }
}

async fn world(mac: &Mac) -> TestService {
    let home = tempfile::TempDir::new().unwrap();
    let program = stand_in(&mac.files, &home.path().join("h"));
    let env = CheckEnv {
        path: mac.bin.as_os_str().to_owned(),
        claude_config_dir: Some(mac.claude.clone()),
        managed_settings_dir: mac.claude.join("managed"),
    };
    TestService::start_in(home, TempRepos::new(), move |config| ServiceConfig {
        tools: env,
        launch: Some(LaunchContext {
            bin_dir: "/nonexistent".into(),
            plugin_dir: "/nonexistent/plugin".into(),
            path: std::env::var_os("PATH").unwrap_or_default(),
            claude: ClaudeLaunch {
                program: vec!["sh".into(), program.display().to_string()],
                config_dir: None,
            },
        }),
        ..config
    })
    .await
}

/// `request` as the app sends it.
async fn ask(w: &TestService, request: Request) -> Result<Reply, ErrorBody> {
    match request_once(&w.data.socket_path(), request).await {
        Ok(reply) => Ok(reply),
        Err(ClientError::Service(body)) => Err(body),
        Err(e) => panic!("{e}"),
    }
}

async fn sources(w: &TestService, project: &str) -> Sources {
    let request = Request::OnboardingSources(ProjectParams {
        project: project.into(),
    });
    match ask(w, request).await.unwrap() {
        Reply::OnboardingSources(s) => s,
        other => panic!("{other:?}"),
    }
}

async fn start_flow(w: &TestService, project: &str) -> Result<ProjectAgent, ErrorBody> {
    let request = Request::OnboardingStartFlow(ProjectParams {
        project: project.into(),
    });
    ask(w, request).await.map(|reply| match reply {
        Reply::ProjectAgent(agent) => *agent,
        other => panic!("{other:?}"),
    })
}

async fn stage_names(w: &TestService, project: &str) -> Vec<String> {
    let flow = w.board.effective_flow(project).await.unwrap();
    flow.stages.into_iter().map(|s| s.name).collect()
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[tokio::test(flavor = "multi_thread")]
async fn the_onboarding_agent_reads_the_history_proposes_and_the_second_proposal_applies() {
    let mac = mac(true);
    let w = world(&mac).await;
    let shop = w.add_project("shop-web").await.key;
    let s = sources(&w, &shop).await;
    assert!(s.history && s.commits && !s.gh && !s.glab, "{s:?}");

    let agent = start_flow(&w, &shop).await.unwrap();
    let session = agent.session.as_ref().expect("a session").id.clone();
    w.wait_for_output(&session, "STAND-IN proposed the first flow")
        .await;

    // `otaq history`, from the Project agent's session: allowed, the fixture's totals, no pasted text.
    assert_eq!(read(mac.files.join("history.status")).trim(), "0");
    let history = read(mac.files.join("history.out"));
    assert_eq!(
        history.lines().next(),
        Some("300 of your messages in 3 projects \u{b7} all time"),
        "{history}"
    );
    for marker in PASTE_MARKERS {
        assert!(!history.contains(marker), "{marker} printed");
    }

    // The first proposal: from the agent's document, its why lines, nothing to fix.
    let first: u32 = read(mac.files.join("first.id")).trim().parse().unwrap();
    let open = w
        .board
        .proposal(&shop, &FixedModels::sample())
        .await
        .unwrap()
        .expect("the first proposal");
    assert_eq!(open.id, first);
    assert_eq!(open.source, Source::Document { file: None });
    assert!(
        matches!(&open.proposed_by, ProposedBy::ProjectAgent { session: s, .. } if *s == session),
        "{:?}",
        open.proposed_by
    );
    assert!(open.problems.is_empty(), "{:?}", open.problems);
    let why: Vec<&str> = open.why.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(
        why,
        [
            "Most of your sessions start with /grill",
            "You asked for a security review before 4 of your last merges"
        ]
    );
    // Nothing applies until the user says so.
    assert_eq!(
        stage_names(&w, &shop).await,
        ["Dev", "Validate", "Review", "Done"]
    );

    // A ticket's agent in the same run is refused the history.
    let ticket = w.create(&shop, "Rate limit").await;
    let refused = w
        .agent_shell(&ticket)
        .await
        .run(&format!("'{CB}' history --all --json"))
        .await;
    assert_ne!(refused.status, 0, "{refused:?}");
    let e: Value = serde_json::from_str(&refused.stdout).unwrap();
    assert_eq!(e["error"]["message"], only_a_project_agent(), "{e}");
    assert!(!refused.stdout.contains("/grill"), "{refused:?}");

    // Ask for more changes reaches the agent as its next message; it proposes again.
    let changes = "Leave out Grill: I write my own tickets";
    let reply = ask(
        &w,
        Request::FlowAskForChanges(AskForChangesParams {
            project: shop.clone(),
            proposal: first,
            text: changes.into(),
        }),
    )
    .await
    .unwrap();
    assert!(matches!(reply, Reply::Ok), "{reply:?}");
    w.deliveries.deliver_now(&session).await.unwrap();
    let out = w
        .wait_for_output(&session, "STAND-IN proposed the second flow")
        .await;
    assert!(
        out.contains(&format!(
            "The user asks for more changes to Proposal {first}: {changes}"
        )),
        "{out}"
    );
    let second: u32 = read(mac.files.join("second.id")).trim().parse().unwrap();
    assert_ne!(second, first);

    // Apply: the board is quiet, so the second file is the project's flow at once, and the agent hears so.
    let reply = ask(
        &w,
        Request::FlowApply(ApplyParams {
            project: shop.clone(),
            proposal: second,
            choices: ApplyChoices::default(),
        }),
    )
    .await
    .unwrap();
    let Reply::FlowApplied(applied) = reply else {
        panic!("{reply:?}")
    };
    assert!(applied.waiting.is_none(), "a quiet board applies at once");
    assert_eq!(
        stage_names(&w, &shop).await,
        ["Dev", "Security", "Review", "Done"]
    );
    let file = otaq_core::flow::yaml::read(SECOND).unwrap();
    let applied_flow = w.board.effective_flow(&shop).await.unwrap();
    let security = applied_flow
        .stages
        .iter()
        .find(|s| s.id == "security")
        .unwrap();
    assert_eq!(
        Some(security),
        file.flow.stages.iter().find(|s| s.id == "security")
    );
    w.deliveries.deliver_now(&session).await.unwrap();
    w.wait_for_output(&session, &format!("The user applied Proposal {second}."))
        .await;
    assert!(
        w.board
            .proposal(&shop, &FixedModels::sample())
            .await
            .unwrap()
            .is_none()
    );
    assert!(w.deliveries.pending(&session).await.unwrap().is_empty());
    w.stop().await;
}

/// No history file, a repository with no commits, no `gh` or `glab`: nothing to look at, so the start is refused,
/// no agent opens, and a preset applies instead.
#[tokio::test(flavor = "multi_thread")]
async fn with_nothing_to_look_at_a_preset_applies_instead() {
    let mac = mac(false);
    let w = world(&mac).await;
    let repo = w.repos.repo("empty-api", 0);
    w.board
        .add_project(&AddProject {
            path: repo.display().to_string(),
            key: Some("API".into()),
            base_branch: Some("main".into()),
        })
        .await
        .unwrap();

    let s = sources(&w, "API").await;
    assert!(!s.any(), "{s:?}");
    let e = start_flow(&w, "API").await.unwrap_err();
    assert_eq!(e.message, NOTHING_TO_LOOK_AT);
    let agents = w.project_agents.list("API").await.unwrap();
    assert!(
        agents.open.is_empty() && agents.recent.is_empty(),
        "{agents:?}"
    );
    assert!(!mac.files.join("history.status").exists());

    let file = "grill-first.yaml";
    let reply = ask(
        &w,
        Request::FlowProposeFrom(ProposeFromParams {
            project: "API".into(),
            source: ProposeFrom::Preset { file: file.into() },
        }),
    )
    .await
    .unwrap();
    let Reply::FlowProposal(Some(proposal)) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(proposal.source, Source::Preset { file: file.into() });
    assert!(proposal.problems.is_empty(), "{:?}", proposal.problems);
    let reply = ask(
        &w,
        Request::FlowApply(ApplyParams {
            project: "API".into(),
            proposal: proposal.id,
            choices: ApplyChoices::default(),
        }),
    )
    .await
    .unwrap();
    let Reply::FlowApplied(applied) = reply else {
        panic!("{reply:?}")
    };
    assert!(applied.waiting.is_none(), "a quiet board applies at once");
    assert_eq!(
        w.board.effective_flow("API").await.unwrap(),
        preset(file).unwrap().read().flow
    );
    w.stop().await;
}
