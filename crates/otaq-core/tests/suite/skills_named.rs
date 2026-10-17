//! Skills named through the adapter (T-0315; ADP-05, PLG-03, REV-09; d0088, d0086): `prime` names Otaq's skills in
//! the caller's tool's syntax (Claude Code's by hand), and a workflow skill the user chose goes by its SKILL.md path
//! when Claude Code would find it: the project's `.claude/skills` first, then the user's Claude folder. Only
//! temporary folders: the Claude folder is the service's launch setting, never the real one; no test starts
//! claude (its stand-in is `sh` printing its arguments).

use std::path::{Path, PathBuf};

use otaq_core::agent_tools::LaunchContext;
use otaq_core::agent_tools::claude::ClaudeLaunch;
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

    // The user's, not found: by name as stored.
    set_skill(&w, |s| s.workflow_skills.commit = "/my-commit".into()).await;
    let primed = prime_as(&w, &agent, &key).await;
    assert_eq!(primed["commit_skill"], "/my-commit");

    // Found in the user's Claude folder: by its path.
    let user = write_skill(claude.path(), "my-commit");
    let primed = prime_as(&w, &agent, &key).await;
    assert_eq!(primed["commit_skill"], json!({"file": user}));
    assert_eq!(
        commit_rule(&primed),
        format!(
            "2. Commit everything by following `{}` before you submit. Never push and never open a PR: the user \
             does that.",
            user.display()
        )
    );

    // Found in the project too (the ticket's worktree): the project's wins, also by hand.
    let own = write_skill(&worktree.join(".claude"), "my-commit");
    let primed = prime_as(&w, &agent, &key).await;
    assert_eq!(primed["commit_skill"], json!({"file": own}));
    let by_hand = prime_by_hand(&w, &key).await;
    assert_eq!(
        serde_json::to_value(&by_hand.commit_skill).unwrap(),
        json!({"file": own})
    );
    w.stop().await;
}

/// A found Validate skill is the validator's first prompt by path; a not-found one by name (VAL-13, d0086).
#[tokio::test(flavor = "multi_thread")]
async fn the_validator_follows_a_found_validate_skill() {
    let claude = TempDir::new().unwrap();
    let w = world(claude.path()).await;
    let (t, _) = working(&w).await;
    set_skill(&w, |s| s.workflow_skills.validate = "/my-validate".into()).await;
    let file = write_skill(claude.path(), "my-validate");
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

/// Open PR types a found Open PR skill into the live session by path; Push's not-found skill by name (REV-10).
#[tokio::test(flavor = "multi_thread")]
async fn open_pr_types_a_found_skill_by_path_and_a_missing_one_by_name() {
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
    set_skill(&w, |s| s.workflow_skills.open_pr = "/my-pr".into()).await;
    let file = write_skill(claude.path(), "my-pr");
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

    // The same skill, gone: Retry types it by name as stored.
    std::fs::remove_file(&file).unwrap();
    let outcome = w.prs.retry("SHOP-1", false).await.unwrap();
    assert!(matches!(outcome, PrOutcome::Started { .. }), "{outcome:?}");
    w.wait_for_output(&agent.session, "/my-pr: ").await;
    w.stop().await;
}
