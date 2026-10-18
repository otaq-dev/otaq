//! Imported workflow skills (T-0378; WSK-01…WSK-04, WSK-07, SET-01; d0092): the checks at import, Otaq's copy in
//! `$OTAQ_HOME/skills/<id>/`, Re-import, Import a different folder, going back, the status, the clean-ups and
//! the settings save that refuses a typed name. Skill folders live under the test's own temporary folders, and
//! `OTAQ_HOME` is one too (the fixture's and the test service's data folder).

use std::path::{Path, PathBuf};

use crate::common::Fixture;
use otaq_core::board::settings::{ResolvedSkill, SkillSetting, WorkflowStep};
use otaq_core::board::{BoardError, Refusal};
use otaq_core::client::{Client, ClientConfig, ClientError};
use otaq_core::protocol::{Reply, Request};
use otaq_core::service::testing::TestService;
use otaq_core::workflow_skills::{SkillKind, SkillStatus};
use serde_json::json;

const COMMIT: WorkflowStep = WorkflowStep::Commit;

/// The example's team skill folders under `root/team-skills/`: `my-commit` (SKILL.md, reference.md and an
/// executable `scripts/check-staged.sh`), `commit-helpers` (no SKILL.md), `broken-front-matter` and
/// `missing-script` (its SKILL.md names a script that isn't there).
fn team_skills(root: &Path) -> PathBuf {
    let team = root.join("team-skills");
    let my_commit = team.join("my-commit");
    std::fs::create_dir_all(my_commit.join("scripts")).unwrap();
    std::fs::write(
        my_commit.join("SKILL.md"),
        "---\nname: my-commit\ndescription: Commits in the team's style.\n---\n\
         Read [the style](reference.md), run `bash scripts/check-staged.sh`, then commit with a message in the style \
         `reference.md` gives.\n",
    )
    .unwrap();
    std::fs::write(
        my_commit.join("reference.md"),
        "Commit messages: `<area>: <what changed>`, for example `notes: sort by date`\n",
    )
    .unwrap();
    let script = my_commit.join("scripts/check-staged.sh");
    std::fs::write(&script, "#!/bin/sh\necho CHECK-STAGED-OK\n").unwrap();
    set_mode(&script, 0o755);

    std::fs::create_dir_all(team.join("commit-helpers")).unwrap();
    std::fs::write(team.join("commit-helpers/notes.md"), "no skill here\n").unwrap();

    std::fs::create_dir_all(team.join("broken-front-matter")).unwrap();
    std::fs::write(
        team.join("broken-front-matter/SKILL.md"),
        "---\nname my-commit\ndescription: x\n---\nBody\n",
    )
    .unwrap();

    std::fs::create_dir_all(team.join("missing-script")).unwrap();
    std::fs::write(
        team.join("missing-script/SKILL.md"),
        "---\nname: missing-script\ndescription: x\n---\nRun `bash scripts/check-staged.sh` first.\n",
    )
    .unwrap();
    team
}

fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// The folders under `skills/`.
fn copies(f: &Fixture) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(f.board.data().skills_dir())
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn refusal(result: Result<SkillStatus, BoardError>) -> (Refusal, String) {
    match result {
        Err(BoardError::Refused { code, message }) => (code, message),
        other => panic!("{other:?}"),
    }
}

async fn imported(f: &Fixture, project: Option<&str>, step: WorkflowStep) -> SkillSetting {
    match project {
        None => f.board.global_settings().await.unwrap().workflow_skills,
        Some(_) => {
            let own = f
                .board
                .project_settings("SHOP")
                .await
                .unwrap()
                .workflow_skills;
            return own.get(step).clone();
        }
    }
    .get(step)
    .clone()
}

#[tokio::test]
async fn the_example_skill_is_copied_whole_with_its_modes() {
    let f = Fixture::shop().await;
    let team = team_skills(f.home.path());
    let original = team.join("my-commit");
    // A link inside the folder is copied as its file; one that leaves the folder is a warning, not followed.
    std::os::unix::fs::symlink(original.join("reference.md"), original.join("style.md")).unwrap();
    std::os::unix::fs::symlink(
        team.join("commit-helpers/notes.md"),
        original.join("outside.md"),
    )
    .unwrap();

    let status = f
        .board
        .import_workflow_skill(None, COMMIT, &original)
        .await
        .unwrap();
    assert_eq!(status.kind, SkillKind::Imported);
    assert_eq!(status.name.as_deref(), Some("my-commit"));
    assert_eq!(status.original.as_deref(), Some(original.to_str().unwrap()));
    assert!(status.original_exists && status.copy_ok && !status.edited);
    assert!(status.imported_at.unwrap() > 0);
    assert_eq!(
        status.warnings,
        ["outside.md links outside the folder; not copied"]
    );

    let SkillSetting::Imported(skill) = imported(&f, None, COMMIT).await else {
        panic!("imported")
    };
    let copy = f.board.data().skill_path(&skill.id).unwrap();
    assert_eq!(status.copy.as_deref(), Some(copy.as_path()));
    assert_eq!(copies(&f), [skill.id.as_str()], "an id of Otaq's own");
    assert_ne!(skill.id, "my-commit");
    assert_eq!(mode(&copy), 0o700);
    for file in ["SKILL.md", "reference.md", "scripts/check-staged.sh"] {
        assert_eq!(
            std::fs::read(copy.join(file)).unwrap(),
            std::fs::read(original.join(file)).unwrap(),
            "{file}"
        );
    }
    assert_eq!(mode(&copy.join("scripts/check-staged.sh")), 0o755);
    let style = copy.join("style.md");
    assert!(!std::fs::symlink_metadata(&style).unwrap().is_symlink());
    assert_eq!(
        std::fs::read(style).unwrap(),
        std::fs::read(original.join("reference.md")).unwrap()
    );
    assert!(!copy.join("outside.md").exists());

    // Agents use the copy (WSK-03): a project on "use global" resolves to it too.
    let effective = f.board.effective_settings("SHOP").await.unwrap();
    assert_eq!(
        effective.workflow_skills.commit,
        ResolvedSkill::Imported {
            name: "my-commit".into(),
            folder: copy.clone()
        }
    );
    // A missing or broken copy means the built-in (WSK-04), shown as such.
    std::fs::write(copy.join("SKILL.md"), "no front matter any more\n").unwrap();
    let effective = f.board.effective_settings("SHOP").await.unwrap();
    assert_eq!(
        effective.workflow_skills.commit,
        ResolvedSkill::BuiltIn(COMMIT)
    );
    let status = &f.board.workflow_skills_status(None).await.unwrap()[0];
    assert!(!status.copy_ok && status.edited);
    assert_eq!(
        status.copy_problem.as_deref(),
        Some("SKILL.md's front matter can't be read: it must start with a --- line")
    );
}

#[tokio::test]
async fn refused_folders_change_nothing_and_missing_files_are_a_warning() {
    let f = Fixture::shop().await;
    let team = team_skills(f.home.path());
    f.board
        .import_workflow_skill(Some("SHOP"), COMMIT, &team.join("my-commit"))
        .await
        .unwrap();
    let before = (imported(&f, Some("SHOP"), COMMIT).await, copies(&f));

    for (folder, why) in [
        ("commit-helpers", "The folder has no SKILL.md"),
        (
            "broken-front-matter",
            "SKILL.md's front matter can't be read: line 2 isn't key: value",
        ),
    ] {
        let (code, message) = refusal(
            f.board
                .import_workflow_skill(Some("SHOP"), COMMIT, &team.join(folder))
                .await,
        );
        assert_eq!((code, message.as_str()), (Refusal::BadSkillFolder, why));
        assert_eq!(
            (imported(&f, Some("SHOP"), COMMIT).await, copies(&f)),
            before,
            "{folder}"
        );
    }
    let (code, _) = refusal(
        f.board
            .import_workflow_skill(Some("SHOP"), COMMIT, Path::new("team-skills/my-commit"))
            .await,
    );
    assert_eq!(code, Refusal::BadSkillFolder, "a relative path");
    assert_eq!(copies(&f), before.1);

    let status = f
        .board
        .import_workflow_skill(
            Some("SHOP"),
            WorkflowStep::Push,
            &team.join("missing-script"),
        )
        .await
        .unwrap();
    assert_eq!(
        status.warnings,
        ["SKILL.md mentions scripts/check-staged.sh, which isn't in the folder"]
    );
    assert!(status.copy_ok);
    // The warning stays with the setting for the view.
    let listed = f.board.workflow_skills_status(Some("SHOP")).await.unwrap();
    assert_eq!(listed[1].warnings, status.warnings);
    assert!(listed[0].warnings.is_empty(), "my-commit has everything");
}

#[tokio::test]
async fn one_copy_per_setting_and_another_folder_replaces_it() {
    let f = Fixture::shop().await;
    let team = team_skills(f.home.path());
    let my_commit = team.join("my-commit");
    let global = f
        .board
        .import_workflow_skill(None, COMMIT, &my_commit)
        .await
        .unwrap();
    let project = f
        .board
        .import_workflow_skill(Some("SHOP"), COMMIT, &my_commit)
        .await
        .unwrap();
    assert_ne!(global.copy, project.copy, "two copies of the same folder");
    assert_eq!(copies(&f).len(), 2);

    // Import a different folder: the copy and the remembered original change in one step.
    let replaced = f
        .board
        .import_workflow_skill(Some("SHOP"), COMMIT, &team.join("missing-script"))
        .await
        .unwrap();
    assert_eq!(replaced.name.as_deref(), Some("missing-script"));
    assert_eq!(
        replaced.original.as_deref(),
        Some(team.join("missing-script").to_str().unwrap())
    );
    assert!(!project.copy.unwrap().exists(), "the old copy is deleted");
    assert!(global.copy.unwrap().exists(), "the global one is untouched");
    assert_eq!(copies(&f).len(), 2);
}

#[tokio::test]
async fn re_import_replaces_the_copy_from_the_original() {
    let f = Fixture::shop().await;
    let team = team_skills(f.home.path());
    let original = team.join("my-commit");
    let first = f
        .board
        .import_workflow_skill(None, COMMIT, &original)
        .await
        .unwrap();
    let SkillSetting::Imported(before) = imported(&f, None, COMMIT).await else {
        panic!()
    };

    std::fs::write(original.join("reference.md"), "New style\n").unwrap();
    let again = f.board.reimport_workflow_skill(None, COMMIT).await.unwrap();
    let SkillSetting::Imported(after) = imported(&f, None, COMMIT).await else {
        panic!()
    };
    assert_ne!(after.fingerprint, before.fingerprint, "renewed");
    assert_eq!(after.original, before.original);
    assert!(!first.copy.unwrap().exists());
    let copy = again.copy.unwrap();
    assert_eq!(
        std::fs::read_to_string(copy.join("reference.md")).unwrap(),
        "New style\n"
    );
    assert!(!again.edited);

    // The original gone: refused, the copy kept.
    std::fs::rename(&original, team.join("moved")).unwrap();
    let status = &f.board.workflow_skills_status(None).await.unwrap()[0];
    assert!(!status.original_exists && status.copy_ok);
    let (code, message) = refusal(f.board.reimport_workflow_skill(None, COMMIT).await);
    assert_eq!(code, Refusal::OriginalNotFound);
    assert_eq!(
        message,
        format!("Original not found: {}", original.display())
    );
    assert!(copy.join("SKILL.md").is_file());
    assert_eq!(
        imported(&f, None, COMMIT).await,
        SkillSetting::Imported(after)
    );

    // Nothing imported: nothing to re-import.
    let (code, _) = refusal(
        f.board
            .reimport_workflow_skill(None, WorkflowStep::Push)
            .await,
    );
    assert_eq!(code, Refusal::NotImported);
}

#[tokio::test]
async fn going_back_deletes_the_copy_and_asks_only_for_edits() {
    let f = Fixture::shop().await;
    let team = team_skills(f.home.path());
    let my_commit = team.join("my-commit");

    // Unedited: at once.
    let global = f
        .board
        .import_workflow_skill(None, COMMIT, &my_commit)
        .await
        .unwrap();
    let back = f
        .board
        .reset_workflow_skill(None, COMMIT, false)
        .await
        .unwrap();
    assert_eq!(back.kind, SkillKind::BuiltIn);
    assert_eq!(imported(&f, None, COMMIT).await, SkillSetting::BuiltIn);
    assert!(!global.copy.unwrap().exists());

    // Edited since the import: asks first, then deletes.
    let project = f
        .board
        .import_workflow_skill(Some("SHOP"), COMMIT, &my_commit)
        .await
        .unwrap();
    let copy = project.copy.unwrap();
    std::fs::write(copy.join("reference.md"), "edited in Otaq's copy\n").unwrap();
    assert!(f.board.workflow_skills_status(Some("SHOP")).await.unwrap()[0].edited);
    let (code, message) = refusal(
        f.board
            .reset_workflow_skill(Some("SHOP"), COMMIT, false)
            .await,
    );
    assert_eq!(code, Refusal::NeedsConfirmation);
    assert!(
        message.contains("has edits that aren't in the original"),
        "{message}"
    );
    assert!(copy.is_dir());
    assert!(matches!(
        imported(&f, Some("SHOP"), COMMIT).await,
        SkillSetting::Imported(_)
    ));
    let back = f
        .board
        .reset_workflow_skill(Some("SHOP"), COMMIT, true)
        .await
        .unwrap();
    assert_eq!(back.kind, SkillKind::UseGlobal);
    assert_eq!(
        imported(&f, Some("SHOP"), COMMIT).await,
        SkillSetting::UseGlobal
    );
    assert!(!copy.exists());
    assert!(copies(&f).is_empty());

    // A changed original alone isn't an edit (d0092's changelog).
    f.board
        .import_workflow_skill(None, COMMIT, &my_commit)
        .await
        .unwrap();
    std::fs::write(my_commit.join("reference.md"), "changed original\n").unwrap();
    f.board
        .reset_workflow_skill(None, COMMIT, false)
        .await
        .unwrap();
}

/// WSK-07: a typed name from before d0092 shows as legacy; a settings save can't bring a new one, and keeps the
/// imported records whatever it sends.
#[tokio::test]
async fn a_settings_save_refuses_a_typed_name_and_keeps_imports() {
    let f = Fixture::shop().await;
    let team = team_skills(f.home.path());
    let mut stored = f.board.global_settings().await.unwrap();
    stored.workflow_skills.push = SkillSetting::Legacy("/my-push".into());
    f.board.set_global_settings(&stored).await.unwrap();
    let status = f.board.workflow_skills_status(None).await.unwrap();
    assert_eq!(
        status
            .iter()
            .map(|s| (s.step, s.kind, s.name.as_deref()))
            .collect::<Vec<_>>(),
        [
            (COMMIT, SkillKind::BuiltIn, None),
            (WorkflowStep::Push, SkillKind::Legacy, Some("/my-push")),
            (WorkflowStep::OpenPr, SkillKind::BuiltIn, None),
            (WorkflowStep::Validate, SkillKind::BuiltIn, None),
        ]
    );
    let project = f.board.workflow_skills_status(Some("SHOP")).await.unwrap();
    assert!(project.iter().all(|s| s.kind == SkillKind::UseGlobal));

    // The page sends back what it read (the legacy value unchanged): fine.
    let mut sent = f.board.global_settings().await.unwrap();
    sent.agent_instructions = "Ask first.".into();
    f.board.save_global_settings(&sent).await.unwrap();

    // A new typed name: refused, nothing saved.
    sent.workflow_skills.commit = SkillSetting::Legacy("/my-commit".into());
    sent.agent_instructions = "Changed".into();
    let Err(BoardError::Refused { code, message }) = f.board.save_global_settings(&sent).await
    else {
        panic!("refused")
    };
    assert_eq!(code, Refusal::TypedSkillName);
    assert_eq!(
        message,
        "/my-commit is a typed name; import a skill folder instead"
    );
    assert_eq!(
        f.board.global_settings().await.unwrap().agent_instructions,
        "Ask first."
    );

    // A stale save (read before an import) keeps the import.
    let stale = f.board.project_settings("SHOP").await.unwrap();
    f.board
        .import_workflow_skill(Some("SHOP"), COMMIT, &team.join("my-commit"))
        .await
        .unwrap();
    f.board.save_project_settings("SHOP", &stale).await.unwrap();
    assert!(matches!(
        imported(&f, Some("SHOP"), COMMIT).await,
        SkillSetting::Imported(_)
    ));
    let mut typed = stale.clone();
    typed.workflow_skills.open_pr = SkillSetting::Legacy("/my-pr".into());
    assert!(matches!(
        f.board.save_project_settings("SHOP", &typed).await,
        Err(BoardError::Refused {
            code: Refusal::TypedSkillName,
            ..
        })
    ));
}

/// The requests over the socket, and the clean-ups: a removed project's copies, and at service start the folders
/// no setting holds.
#[tokio::test(flavor = "multi_thread")]
async fn the_requests_and_the_clean_ups() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let team = team_skills(w.home.path());
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let ask = |method: &str, params: serde_json::Value| {
        let request: Request =
            serde_json::from_value(json!({ "method": method, "params": params })).unwrap();
        client.request(request)
    };

    let Reply::WorkflowSkill(status) = ask(
        "workflow_skills.import",
        json!({ "project": "SHOP", "step": "commit", "folder": team.join("my-commit") }),
    )
    .await
    .unwrap() else {
        panic!()
    };
    assert_eq!(status.kind, SkillKind::Imported);
    let project_copy = status.copy.clone().unwrap();
    let Reply::WorkflowSkill(_) = ask(
        "workflow_skills.import",
        json!({ "step": "open_pr", "folder": team.join("missing-script") }),
    )
    .await
    .unwrap() else {
        panic!()
    };
    let Err(ClientError::Service(e)) = ask(
        "workflow_skills.import",
        json!({ "step": "push", "folder": team.join("commit-helpers") }),
    )
    .await
    else {
        panic!("refused")
    };
    assert_eq!(e.message, "The folder has no SKILL.md");
    assert_eq!(e.data.unwrap()["refusal"], "bad_skill_folder");
    let Reply::WorkflowSkills(listed) = ask("workflow_skills.status", json!({})).await.unwrap()
    else {
        panic!()
    };
    assert_eq!(listed[2].kind, SkillKind::Imported);
    let Reply::WorkflowSkill(_) = ask("workflow_skills.reimport", json!({ "step": "open_pr" }))
        .await
        .unwrap()
    else {
        panic!()
    };
    let Reply::WorkflowSkill(back) = ask(
        "workflow_skills.reset",
        json!({ "step": "open_pr", "confirmed": false }),
    )
    .await
    .unwrap() else {
        panic!()
    };
    assert_eq!(back.kind, SkillKind::BuiltIn);

    // A global copy, and a stray folder no setting holds.
    let Reply::WorkflowSkill(global) = ask(
        "workflow_skills.import",
        json!({ "step": "validate", "folder": team.join("my-commit") }),
    )
    .await
    .unwrap() else {
        panic!()
    };
    let global_copy = global.copy.unwrap();
    drop(client);

    // Removing the project deletes its copies.
    w.board.remove_project("SHOP").await.unwrap();
    assert!(!project_copy.exists());
    assert!(global_copy.exists());

    let stray = w.data.skill_path("left-by-a-crash").unwrap();
    std::fs::create_dir_all(&stray).unwrap();
    let w = w.restart().await;
    assert!(!stray.exists(), "removed at start");
    assert!(global_copy.join("SKILL.md").is_file(), "held by a setting");
    w.stop().await;
}
