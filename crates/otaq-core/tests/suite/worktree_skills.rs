//! Imported workflow skills in the worktree (T-0379; WSK-05, WSK-06, WSK-08, ADP-05, REV-11; d0092, d0023, d0011,
//! d0063): before a session runs a workflow skill, Otaq's copy is written into the worktree as
//! `.agents/skills/otaq-custom-<step>/` and `.claude/skills/otaq-custom-<step>/`, gitignored by its own `.gitignore`;
//! the agent is pointed at that folder, the same on every tool, and built-ins keep their `otaq:` names. Nothing else
//! in the repo or its `.git` is written, and git never sees the copies (status, `add -A`, the validator's read-only
//! check, the uncommitted-files refusals). Skill folders, repos and `OTAQ_HOME` are temporary; `claude` stands in as
//! `sh` printing its arguments and what the worktree's `.agents/skills` holds at launch: no test starts claude.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::agent_tools::{LaunchContext, SkillRef, Tool};
use otaq_core::board::moves::DropTarget;
use otaq_core::board::settings::{ResolvedSkill, WorkflowStep};
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Actor, Status, ValidationTrigger};
use otaq_core::git::testing::{commit, git};
use otaq_core::naming;
use otaq_core::pr::service::PrOutcome;
use otaq_core::review::LocalMergeOutcome;
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use otaq_core::session::SessionKind;
use otaq_core::workflow_skills::ResolvedSkills;
use otaq_core::workflow_skills::testing::skill_folder;
use otaq_core::workflow_skills::worktree::{self, prepare};

/// Prints its arguments and the copies in the launch folder's `.agents/skills`, then echoes what is typed.
const SCRIPT: &str = "printf 'ARGS %s\\n' \"$*\"; printf 'COPIES %s.\\n' \"$(ls .agents/skills 2>/dev/null | \
                      tr '\\n' ' ')\"; exec cat";

/// How every tool's first prompt names an imported skill of `step` (WSK-06).
fn follow(step: &str) -> String {
    format!(
        "Follow the skill in `.agents/skills/otaq-custom-{step}/` (start with its SKILL.md; paths in it are \
         relative to that folder)."
    )
}

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
                program: vec!["sh".into(), "-c".into(), SCRIPT.into(), "sh".into()],
                config_dir: None,
            },
        }),
        ..config
    })
    .await
}

/// shop-web with a local origin (no PR provider: Open PR needs an imported skill, REV-11).
async fn shop_web(w: &TestService) {
    let repo = w.repos.clone_with_origin("shop-web", "main", 1);
    w.board
        .add_project(&otaq_core::board::projects::AddProject {
            path: repo.display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Imports the skill folder `name` as the project's `step`.
async fn import(w: &TestService, step: WorkflowStep, name: &str) {
    let folder = skill_folder(w.data.root(), name);
    w.board
        .import_workflow_skill(Some("SHOP"), step, &folder)
        .await
        .unwrap();
}

fn worktree_of(t: &Ticket) -> PathBuf {
    PathBuf::from(t.work.as_ref().unwrap().worktree.clone().unwrap())
}

/// A ticket in Ready for Review with a started (linked) worktree and one commit of its own.
async fn in_review(w: &TestService, title: &str) -> (Ticket, PathBuf) {
    let t = w.create("SHOP", title).await;
    let worktree = w.start_worktree(&t).await;
    commit(&worktree, "login.ts", "redirect\n");
    w.force_status(&t, Status::Working).await;
    let key = t.key.to_string();
    w.board
        .submit_for_review(&key, "Redirects after login.", &me())
        .await
        .unwrap();
    (w.board.ticket(&key).await.unwrap(), worktree)
}

/// Whether both copies of `step` are in `worktree`.
fn copied(worktree: &Path, step: &str) -> bool {
    [".agents", ".claude"].iter().all(|root| {
        worktree
            .join(root)
            .join("skills")
            .join(format!("otaq-custom-{step}"))
            .join("SKILL.md")
            .is_file()
    })
}

/// Every file under `path` with its contents: "nothing in `.git` was written".
fn files(path: &Path) -> BTreeMap<String, Vec<u8>> {
    otaq_core::git::testing::snapshot(path)
        .into_iter()
        .filter(|rel| path.join(rel).is_file())
        .map(|rel| {
            let contents = std::fs::read(path.join(&rel)).unwrap();
            (rel, contents)
        })
        .collect()
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// Otaq's copy of the example's `my-commit`: SKILL.md (CRLF line ends, a second `name:` in the body), reference.md, an
/// executable script and a `.gitignore` of its own.
fn otaq_copy(root: &Path) -> PathBuf {
    let copy = root.join("skills").join("0192ab");
    std::fs::create_dir_all(copy.join("scripts")).unwrap();
    std::fs::write(
        copy.join("SKILL.md"),
        "---\r\nname: my-commit\r\ndescription: Commits in the team's style.\r\n---\r\nname: stays\r\nRun \
         `bash scripts/check-staged.sh`.\r\n",
    )
    .unwrap();
    std::fs::write(copy.join("reference.md"), "notes: sort by date\n").unwrap();
    std::fs::write(copy.join(".gitignore"), "build/\n").unwrap();
    let script = copy.join("scripts/check-staged.sh");
    std::fs::write(&script, "#!/bin/sh\necho CHECK-STAGED-OK\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    copy
}

fn imported(copy: &Path) -> ResolvedSkill {
    ResolvedSkill::Imported {
        name: "my-commit".into(),
        folder: copy.to_owned(),
    }
}

fn skills(commit: ResolvedSkill, push: ResolvedSkill) -> ResolvedSkills {
    ResolvedSkills {
        commit,
        push,
        open_pr: ResolvedSkill::BuiltIn(WorkflowStep::OpenPr),
        validate: ResolvedSkill::BuiltIn(WorkflowStep::Validate),
    }
}

/// WSK-05 in a linked worktree (`git worktree add`): both folders written with `name:` rewritten, the rest byte for
/// byte, modes kept and `.gitignore` holding `*`; git status and `git add -A` see nothing; nothing in `.git` changes;
/// a second call replaces a changed copy; a step back on the built-in loses its folders; a failure leaves no partial
/// folder.
#[test]
fn the_copy_in_a_linked_worktree() {
    let dir = tempfile::tempdir().unwrap();
    let repos = otaq_core::git::testing::TempRepos::new();
    let repo = repos.repo("pocket-notes", 1);
    let wt = dir.path().join("NOTE-1");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "otaq/NOTE-1",
            wt.to_str().unwrap(),
        ],
    );
    let copy = otaq_copy(dir.path());
    let git_dir = files(&repo.join(".git"));

    let copies = prepare(
        &wt,
        &skills(imported(&copy), ResolvedSkill::BuiltIn(WorkflowStep::Push)),
        &[WorkflowStep::Commit],
    );
    assert!(copies.copied(WorkflowStep::Commit));
    assert!(copies.failures.is_empty(), "{:?}", copies.failures);
    for root in [".agents/skills", ".claude/skills"] {
        let folder = wt.join(root).join("otaq-custom-commit");
        assert_eq!(
            std::fs::read_to_string(folder.join("SKILL.md")).unwrap(),
            "---\r\nname: otaq-custom-commit\r\ndescription: Commits in the team's style.\r\n---\r\nname: \
             stays\r\nRun `bash scripts/check-staged.sh`.\r\n",
            "only the name: line"
        );
        assert_eq!(
            std::fs::read_to_string(folder.join("reference.md")).unwrap(),
            "notes: sort by date\n"
        );
        assert_eq!(
            std::fs::read_to_string(folder.join(".gitignore")).unwrap(),
            "*\n"
        );
        #[cfg(unix)]
        assert_eq!(mode(&folder.join("scripts/check-staged.sh")), 0o755);
        let names: Vec<_> = std::fs::read_dir(wt.join(root))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["otaq-custom-commit"], "nothing partial is left");
    }
    assert_eq!(files(&repo.join(".git")), git_dir, "nothing in .git");
    assert_eq!(
        git(&wt, &["status", "--porcelain", "--untracked-files=all"]),
        ""
    );
    git(&wt, &["add", "-A"]);
    assert_eq!(git(&wt, &["diff", "--cached", "--name-only"]), "");
    let git_dir = files(&repo.join(".git"));

    // A changed copy (Re-import) replaces the one in the worktree.
    std::fs::write(copy.join("reference.md"), "area: what changed\n").unwrap();
    std::fs::remove_dir_all(copy.join("scripts")).unwrap();
    worktree::write(&wt, WorkflowStep::Commit, &imported(&copy)).unwrap();
    let folder = wt.join(".agents/skills/otaq-custom-commit");
    assert_eq!(
        std::fs::read_to_string(folder.join("reference.md")).unwrap(),
        "area: what changed\n"
    );
    assert!(!folder.join("scripts").exists());

    // Another session's imported step is left as it is; a step back on the built-in loses its folders.
    worktree::write(&wt, WorkflowStep::Push, &imported(&copy)).unwrap();
    let copies = prepare(
        &wt,
        &skills(
            ResolvedSkill::BuiltIn(WorkflowStep::Commit),
            imported(&copy),
        ),
        &[WorkflowStep::Validate],
    );
    assert!(!copies.copied(WorkflowStep::Commit));
    assert!(!copied(&wt, "commit"));
    assert!(copied(&wt, "push"));

    // A copy that can't be written: no folder at all, the step on the built-in.
    std::fs::write(copy.join("SKILL.md"), "---\ndescription: no name\n---\n").unwrap();
    let copies = prepare(
        &wt,
        &skills(imported(&copy), imported(&copy)),
        &[WorkflowStep::Push],
    );
    assert!(!copies.copied(WorkflowStep::Push));
    assert_eq!(copies.failures.len(), 1, "{:?}", copies.failures);
    assert_eq!(copies.failures[0].0, WorkflowStep::Push);
    for root in [".agents/skills", ".claude/skills"] {
        let left: Vec<_> = std::fs::read_dir(wt.join(root))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert!(left.is_empty(), "{left:?}");
    }
    let gone = dir.path().join("gone");
    assert!(worktree::write(&wt, WorkflowStep::Commit, &imported(&gone)).is_err());
    assert!(!copied(&wt, "commit"));
    assert_eq!(files(&repo.join(".git")), git_dir, "nothing in .git");
}

/// The ticket's agent (Commit): the copy is in the worktree before the launch, and `prime` names it by its folder on
/// every tool; Otaq's own Commit skill by its `otaq:` name in each tool's syntax (WSK-06, ADP-05).
#[tokio::test(flavor = "multi_thread")]
async fn the_ticket_agent_gets_the_commit_copy_and_prime_names_it() {
    let w = world().await;
    shop_web(&w).await;
    let t = w.create("SHOP", "Sort notes by date").await;
    let key = t.key.to_string();
    for tool in Tool::ALL.iter().copied() {
        let primed = w
            .board
            .prime_context(&key, w.adapters.get(tool))
            .await
            .unwrap();
        assert_eq!(
            primed.commit_skill,
            SkillRef::Own(w.adapters.get(tool).skill_reference("commit"))
        );
    }
    import(&w, WorkflowStep::Commit, "my-commit").await;
    w.board
        .move_ticket(&key, DropTarget::ReadyForDev, true, &me())
        .await
        .unwrap();
    w.starter.start(&key, &me()).await.unwrap();
    let agent = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            if let Some(s) = w.sessions.running_of(&t.id, SessionKind::Agent) {
                return s;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the agent runs");
    let out = w.wait_for_output(&agent.id, "COPIES").await;
    assert!(out.contains("COPIES otaq-custom-commit ."), "{out}");
    let worktree = worktree_of(&w.board.ticket(&key).await.unwrap());
    assert!(copied(&worktree, "commit"));

    for tool in Tool::ALL.iter().copied() {
        let primed = w
            .board
            .prime_context(&key, w.adapters.get(tool))
            .await
            .unwrap();
        assert_eq!(
            primed.commit_skill,
            SkillRef::Folder {
                folder: ".agents/skills/otaq-custom-commit/".into()
            },
            "{tool:?}"
        );
        let rule = otaq_core::prime::render(&primed)
            .lines()
            .find(|l| l.contains("before you submit"))
            .expect("the commit rule")
            .to_owned();
        assert_eq!(
            rule,
            "2. Commit everything by following the skill in `.agents/skills/otaq-custom-commit/` (start with its \
             SKILL.md; paths in it are relative to that folder) before you submit. Never push and never open a PR: \
             the user does that."
        );
    }
    // JSON: the folder, for `prime --json`.
    let json = serde_json::to_value(
        w.board
            .prime_context(&key, w.adapters.get(Tool::Claude))
            .await
            .unwrap()
            .commit_skill,
    )
    .unwrap();
    assert_eq!(
        json,
        serde_json::json!({"folder": ".agents/skills/otaq-custom-commit/"})
    );
    w.stop().await;
}

/// The validator (Validate): the copy is written before its launch and named in its first prompt; only ignored paths
/// change, so its verdict counts (VAL-02, d0063).
#[tokio::test(flavor = "multi_thread")]
async fn the_validator_follows_the_copy_and_stays_read_only() {
    let w = world().await;
    w.add_project("shop-web").await;
    import(&w, WorkflowStep::Validate, "my-validate").await;
    let t = w.create("SHOP", "Cart totals").await;
    let worktree = w.give_worktree(&t).await;
    w.force_status(&t, Status::Working).await;
    let key = t.key.to_string();
    w.validation
        .submit(
            &key,
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
    let out = w.wait_for_output(&validator.id, "COPIES").await;
    assert!(out.contains(&follow("validate")), "{out}");
    assert!(out.contains("COPIES otaq-custom-validate ."), "{out}");
    assert!(!out.contains("/otaq:validate"), "{out}");
    assert!(copied(&worktree, "validate"));
    let after = w
        .validation
        .verdict(&key, true, "Totals are right.", &validator.id)
        .await
        .unwrap();
    assert_eq!(after.status, Status::WaitingForYou, "not discarded");
    w.stop().await;
}

/// The copies in a worktree are no uncommitted files: the submit and Local merge go through (REV-07, REV-08).
#[tokio::test(flavor = "multi_thread")]
async fn submit_and_local_merge_ignore_the_copies() {
    let w = world().await;
    shop_web(&w).await;
    for (step, name) in [
        (WorkflowStep::Commit, "my-commit"),
        (WorkflowStep::Push, "my-push"),
        (WorkflowStep::OpenPr, "my-pr"),
        (WorkflowStep::Validate, "my-validate"),
    ] {
        import(&w, step, name).await;
    }
    let t = w.create("SHOP", "Login redirect").await;
    let worktree = w.start_worktree(&t).await;
    commit(&worktree, "login.ts", "redirect\n");
    let copies = w
        .board
        .copy_skills_into("SHOP", &worktree, &WorkflowStep::ALL)
        .await
        .unwrap();
    assert!(WorkflowStep::ALL.iter().all(|s| copies.copied(*s)));
    w.force_status(&t, Status::Working).await;
    w.board
        .submit_for_review("SHOP-1", "Redirects after login.", &me())
        .await
        .expect("nothing uncommitted");
    let merged = w
        .review
        .local_merge("SHOP-1", "SHOP-1: Login redirect", false, &me())
        .await
        .unwrap();
    assert!(
        matches!(merged, LocalMergeOutcome::Merged { .. }),
        "{merged:?}"
    );
    w.stop().await;
}

/// Open PR into the live session (`run_in_session`): the copy first, then the typed prompt; Push to PR likewise, and
/// the Open PR folders of a step back on the built-in go (WSK-05).
#[tokio::test(flavor = "multi_thread")]
async fn open_pr_and_push_type_the_copy_into_the_live_session() {
    let w = world().await;
    shop_web(&w).await;
    import(&w, WorkflowStep::OpenPr, "my-pr").await;
    let (t, worktree) = in_review(&w, "Login redirect").await;
    let agent = w.agent_shell(&t).await;
    let outcome = w.prs.open("SHOP-1", false, false, &me()).await.unwrap();
    assert!(matches!(outcome, PrOutcome::Started { .. }), "{outcome:?}");
    w.wait_for_output(&agent.session, &follow("open-pr")).await;
    assert!(copied(&worktree, "open-pr"));
    // The flow shows the imported skill's name (WSK-08).
    assert_eq!(w.board.pr_flow_view("SHOP-1").await.unwrap().skill, "my-pr");
    w.prs.cancel("SHOP-1", &me()).await.unwrap();

    // Push to PR #481, the Open PR step back on the built-in.
    sqlx::query("UPDATE tickets SET pr_number = 481 WHERE id = ?")
        .bind(&t.id)
        .execute(w.board.store().pool())
        .await
        .unwrap();
    w.board
        .reset_workflow_skill(Some("SHOP"), WorkflowStep::OpenPr, true)
        .await
        .unwrap();
    import(&w, WorkflowStep::Push, "my-push").await;
    let outcome = w.prs.open("SHOP-1", true, false, &me()).await.unwrap();
    assert!(matches!(outcome, PrOutcome::Started { .. }), "{outcome:?}");
    w.wait_for_output(&agent.session, &follow("push")).await;
    assert!(copied(&worktree, "push"));
    assert!(!worktree.join(".agents/skills/otaq-custom-open-pr").exists());
    assert!(!worktree.join(".claude/skills/otaq-custom-open-pr").exists());
    assert_eq!(
        w.board.pr_flow_view("SHOP-1").await.unwrap().skill,
        "my-push"
    );
    w.stop().await;
}

/// Open PR with no live session resumes the conversation (`resume`): the copy is in the worktree at the launch, and
/// the first prompt names it.
#[tokio::test(flavor = "multi_thread")]
async fn open_pr_resumes_with_the_copy_in_place() {
    let w = world().await;
    shop_web(&w).await;
    import(&w, WorkflowStep::OpenPr, "my-pr").await;
    let (t, worktree) = in_review(&w, "Login redirect").await;
    w.record_ended_agent(&t, "3be94c10-0000-7000-8000-000000000000")
        .await;
    let outcome = w.prs.open("SHOP-1", true, false, &me()).await.unwrap();
    assert!(matches!(outcome, PrOutcome::Started { .. }), "{outcome:?}");
    let session = w
        .sessions
        .running_of(&t.id, SessionKind::Agent)
        .expect("resumed");
    let out = w.wait_for_output(&session.id, "COPIES").await;
    assert!(out.contains(&follow("open-pr")), "{out}");
    assert!(out.contains("COPIES otaq-custom-open-pr ."), "{out}");
    assert!(copied(&worktree, "open-pr"));
    w.stop().await;
}

/// Built-ins: no folder is written and the first prompt is Otaq's own in the tool's syntax.
#[tokio::test(flavor = "multi_thread")]
async fn a_built_in_step_writes_nothing() {
    let w = world().await;
    w.add_project("shop-web").await;
    let t = w.create("SHOP", "Cart totals").await;
    let worktree = w.give_worktree(&t).await;
    w.force_status(&t, Status::Working).await;
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
    let out = w.wait_for_output(&validator.id, "COPIES").await;
    assert!(out.contains(&naming::skill("validate")), "{out}");
    assert!(out.contains("COPIES ."), "{out}");
    assert!(!worktree.join(".agents").exists());
    assert!(!worktree.join(".claude").exists());
    w.stop().await;
}
