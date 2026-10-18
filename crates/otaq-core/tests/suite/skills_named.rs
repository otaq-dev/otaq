//! Skills named through the adapter (T-0315; ADP-05, PLG-03, REV-09; d0088, d0086, d0092): `prime` names Otaq's
//! skills in the caller's tool's syntax (Claude Code's by hand), and an imported workflow skill goes by its copy's
//! SKILL.md path. A typed name from before d0092 isn't run, even where Claude Code would find it: the step uses the
//! built-in (WSK-07). Only temporary folders: the Claude folder is the service's launch setting, never the real one;
//! no test starts claude (its stand-in is `sh` printing its arguments).

use std::path::{Path, PathBuf};

use otaq_core::agent_tools::LaunchContext;
use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::board::settings::{SkillSetting, WorkflowStep};
use otaq_core::board::tickets::Ticket;
use otaq_core::client::{Client, ClientConfig};
use otaq_core::domain::{Actor, Status, ValidationTrigger};
use otaq_core::git::testing::commit;
use otaq_core::naming;
use otaq_core::pr::service::PrOutcome;
use otaq_core::protocol::{Reply, Request};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::{AgentShell, TestService};
use otaq_core::session::SessionKind;
use otaq_core::workflow_skills::testing::skill_folder;
use serde_json::{Value, json};
use tempfile::TempDir;

/// A service whose `claude` prints its arguments then waits, and whose user Claude folder is `claude`.
async fn world(claude: &Path) -> TestService {
    let config_dir = claude.to_owned();
    TestService::start_with(move |config| ServiceConfig {
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
                config_dir: Some(config_dir),
            },
        }),
        ..config
    })
    .await
}

/// A SKILL.md at `<root>/skills/<name>/`.
fn write_skill(root: &Path, name: &str) -> PathBuf {
    let file = root.join("skills").join(name).join("SKILL.md");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(
        &file,
        "---\nname: mine\ndescription: mine\n---\nCommit it my way.\n",
    )
    .unwrap();
    file
}

async fn set_skill(
    w: &TestService,
    set: impl FnOnce(&mut otaq_core::board::settings::ProjectSettings),
) {
    let mut settings = w.board.project_settings("SHOP").await.unwrap();
    set(&mut settings);
    w.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
}

/// Imports a skill folder `name` as the project's `step`: the copy's SKILL.md.
async fn import(w: &TestService, step: WorkflowStep, name: &str) -> PathBuf {
    let folder = skill_folder(w.data.root(), name);
    let status = w
        .board
        .import_workflow_skill(Some("SHOP"), step, &folder)
        .await
        .unwrap();
    status.copy.unwrap().join("SKILL.md")
}

/// `ticket.prime` from inside `shell`'s session: the context as JSON.
async fn prime_as(w: &TestService, shell: &AgentShell, key: &str) -> Value {
    let request = json!({"protocol_version": 1, "id": 1,
                         "request": {"method": "ticket.prime", "params": {"key": key}}});
    let ran = shell
        .send_raw(&w.data.socket_path(), &request.to_string())
        .await;
    let answer: Value = serde_json::from_str(ran.stdout.lines().next().unwrap_or_default())
        .unwrap_or_else(|e| panic!("{e}: {ran:?}"));
    assert!(answer.get("error").is_none(), "{answer}");
    answer["reply"]["prime"].clone()
}

/// `otaq prime` by hand: the user's caller, no agent session.
async fn prime_by_hand(w: &TestService, key: &str) -> otaq_core::prime::PrimeContext {
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let request: Request =
        serde_json::from_value(json!({"method": "ticket.prime", "params": {"key": key}})).unwrap();
    match client.request(request).await.unwrap() {
        Reply::Prime(context) => *context,
        other => panic!("{other:?}"),
    }
}

fn commit_rule(context: &Value) -> String {
    let context: otaq_core::prime::PrimeContext = serde_json::from_value(context.clone()).unwrap();
    otaq_core::prime::render(&context)
        .lines()
        .find(|l| l.contains("before you submit"))
        .expect("the commit rule")
        .to_owned()
}

/// A working ticket with a worktree.
async fn working(w: &TestService) -> (Ticket, PathBuf) {
    w.add_project("shop-web").await;
    let t = w.create("SHOP", "Cart totals").await;
    let worktree = w.give_worktree(&t).await;
    w.force_status(&t, Status::Working).await;
    (w.board.ticket(&t.key.to_string()).await.unwrap(), worktree)
}

#[tokio::test(flavor = "multi_thread")]
async fn prime_names_claudes_skills_and_the_users_commit_skill_by_path() {
    let claude = TempDir::new().unwrap();
    let w = world(claude.path()).await;
    let (t, worktree) = working(&w).await;
    let key = t.key.to_string();
    let agent = w.agent_shell(&t).await;

    // Otaq's own: Claude Code's syntax, for an agent and by hand.
    let primed = prime_as(&w, &agent, &key).await;
    assert_eq!(primed["commit_skill"], json!(naming::skill("commit")));
    assert_eq!(primed["restack_skill"], json!(naming::skill("restack")));
    let by_hand = prime_by_hand(&w, &key).await;
    assert_eq!(
        serde_json::to_value(&by_hand.commit_skill).unwrap(),
        json!(naming::skill("commit"))
    );
    assert_eq!(by_hand.restack_skill, naming::skill("restack"));

    // A typed name from before d0092, even where Claude Code would find it: the built-in (WSK-07).
    set_skill(&w, |s| {
        s.workflow_skills.commit = SkillSetting::Legacy("/my-commit".into())
    })
    .await;
    write_skill(claude.path(), "my-commit");
    write_skill(&worktree.join(".claude"), "my-commit");
    let primed = prime_as(&w, &agent, &key).await;
    assert_eq!(primed["commit_skill"], json!(naming::skill("commit")));

    // Imported: by its copy's path, also by hand.
    let copy = import(&w, WorkflowStep::Commit, "my-commit").await;
    let primed = prime_as(&w, &agent, &key).await;
    assert_eq!(primed["commit_skill"], json!({"file": copy}));
    assert_eq!(
        commit_rule(&primed),
        format!(
            "2. Commit everything by following `{}` before you submit. Never push and never open a PR: the user \
             does that.",
            copy.display()
        )
    );
    let by_hand = prime_by_hand(&w, &key).await;
    assert_eq!(
        serde_json::to_value(&by_hand.commit_skill).unwrap(),
        json!({"file": copy})
    );
    w.stop().await;
}

/// An imported Validate skill is the validator's first prompt by its copy's path (VAL-13, d0092).
#[tokio::test(flavor = "multi_thread")]
async fn the_validator_follows_an_imported_validate_skill() {
    let claude = TempDir::new().unwrap();
    let w = world(claude.path()).await;
    let (t, _) = working(&w).await;
    let file = import(&w, WorkflowStep::Validate, "my-validate").await;
    w.validation
        .submit(
            &t.key.to_string(),
            "Cart totals add up.",
            &Actor::Agent(t.key.clone()),
            ValidationTrigger::Agent,
        )
        .await
        .unwrap();
    let validator = w
        .sessions
        .running_of(&t.id, SessionKind::Validator)
        .unwrap();
    let out = w
        .wait_for_output(&validator.id, &format!("Follow `{}`", file.display()))
        .await;
    assert!(!out.contains(" /my-validate"), "{out}");
    w.stop().await;
}

/// Open PR types an imported Open PR skill into the live session by its copy's path (REV-10, d0092).
#[tokio::test(flavor = "multi_thread")]
async fn open_pr_types_an_imported_skill_by_path() {
    let claude = TempDir::new().unwrap();
    let w = world(claude.path()).await;
    let repo = w.repos.clone_with_origin("shop-web", "main", 1);
    w.board
        .add_project(&otaq_core::board::projects::AddProject {
            path: repo.display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let file = import(&w, WorkflowStep::OpenPr, "my-pr").await;
    let ticket = w.create("SHOP", "Login redirect").await;
    let worktree = w.start_worktree(&ticket).await;
    commit(&worktree, "login.ts", "redirect\n");
    w.force_status(&ticket, Status::Working).await;
    let me = Actor::User("me".into());
    w.board
        .submit_for_review("SHOP-1", "Redirects after login.", &me)
        .await
        .unwrap();
    let ticket = w.board.ticket("SHOP-1").await.unwrap();
    let agent = w.agent_shell(&ticket).await;

    let outcome = w.prs.open("SHOP-1", false, false, &me).await.unwrap();
    assert!(matches!(outcome, PrOutcome::Started { .. }), "{outcome:?}");
    w.wait_for_output(&agent.session, &format!("Follow `{}`", file.display()))
        .await;
    w.stop().await;
}
