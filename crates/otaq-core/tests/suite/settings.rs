//! Settings (SET-01): defaults, per-project fallback, validation against a temporary repository, and that no
//! settings code writes into the repository.

use crate::common::Fixture;
use otaq_core::board::settings::{
    AutoMerge, GlobalSettings, KeyboardChange, KeyboardSettings, ProjectSettings, ResolvedSkill,
    SkillSetting, Theme, ThemeSetting, WorkflowSkillOverrides, WorkflowSkills, WorkflowStep,
    resource_without_create,
};
use otaq_core::git::testing::{git, snapshot};
use otaq_core::naming;

#[tokio::test]
async fn defaults() {
    let f = Fixture::shop().await;
    let global = f.board.global_settings().await.unwrap();
    assert!(!global.agent_limits.enabled);
    assert_eq!(
        (
            global.agent_limits.max_concurrency,
            global.agent_limits.back_pressure
        ),
        (5, 3)
    );
    assert!(global.notifications);
    assert!(!global.description_source_mode);
    assert_eq!(global.keyboard, KeyboardSettings::default());
    assert_eq!(global.theme, ThemeSetting::default());
    assert_eq!(global.workflow_skills, WorkflowSkills::default());
    let effective = f.board.effective_settings("SHOP").await.unwrap();
    for step in WorkflowStep::ALL {
        assert_eq!(global.workflow_skills.get(step), &SkillSetting::BuiltIn);
        assert_eq!(
            effective.workflow_skills.get(step),
            &ResolvedSkill::BuiltIn(step)
        );
    }
    assert_eq!(
        effective.workflow_skills.open_pr.label(),
        naming::skill("open-pr")
    );

    let project = f.board.project_settings("SHOP").await.unwrap();
    assert!(project.let_finished_tickets_answer);
    assert_eq!(project, ProjectSettings::default());
    assert!(f.board.global_setting_problems().await.unwrap().is_empty());
    assert!(
        f.board
            .project_setting_problems("SHOP")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn global_values_round_trip_and_keep_other_rows() {
    let f = Fixture::shop().await;
    f.board
        .store()
        .set_setting("onboarding.permission_warning_dismissed", "true")
        .await
        .unwrap();
    let mut global = GlobalSettings::default();
    global.agent_limits.enabled = true;
    global.agent_limits.max_concurrency = 8;
    global.agent_instructions = "Ask before coding.".into();
    global.theme = ThemeSetting::Picked(Theme::TokyoNight);
    global.terminal_font_size = 15;
    global.notifications = false;
    global.description_source_mode = true;
    global.keyboard.letters = false;
    global.keyboard.changes.push(KeyboardChange {
        action: "park".into(),
        number: None,
        slot: 0,
        key: Some("g p".into()),
        works_in_terminals: false,
    });
    f.board.set_global_settings(&global).await.unwrap();
    assert_eq!(f.board.global_settings().await.unwrap(), global);
    assert_eq!(
        f.board
            .store()
            .setting("onboarding.permission_warning_dismissed")
            .await
            .unwrap()
            .as_deref(),
        Some("true")
    );
}

#[tokio::test]
async fn a_0069_mode_and_two_themes_become_one_theme() {
    let f = Fixture::shop().await;
    let store = f.board.store();
    store.set_setting("appearance", "\"light\"").await.unwrap();
    store
        .set_setting("dark_theme", "\"tokyo-night\"")
        .await
        .unwrap();
    store.set_setting("light_theme", "\"nord\"").await.unwrap();
    store.set_setting("notifications", "false").await.unwrap();
    let global = f.board.global_settings().await.unwrap();
    // Light mode showed the light theme; `nord` is no light theme, so it reads as Otaq Light.
    assert_eq!(global.theme, ThemeSetting::Picked(Theme::OtaqLight));
    assert!(!global.notifications);

    // Saving stores `theme` and drops the old rows.
    f.board.set_global_settings(&global).await.unwrap();
    for key in ["appearance", "dark_theme", "light_theme"] {
        assert_eq!(store.setting(key).await.unwrap(), None, "{key}");
    }
    assert_eq!(
        store.setting("theme").await.unwrap().as_deref(),
        Some("\"otaq-light\"")
    );
    assert_eq!(f.board.global_settings().await.unwrap(), global);
}

#[tokio::test]
async fn a_stored_dollar_estimate_is_ignored() {
    // The setting was removed (d0086); a database from before still has its row.
    let f = Fixture::shop().await;
    let store = f.board.store();
    store.set_setting("dollar_estimate", "true").await.unwrap();
    store.set_setting("notifications", "false").await.unwrap();
    let global = f.board.global_settings().await.unwrap();
    assert!(!global.notifications);
    assert_eq!(
        global,
        GlobalSettings {
            notifications: false,
            ..Default::default()
        }
    );
}

#[tokio::test]
async fn a_0069_system_mode_waits_for_the_os() {
    let f = Fixture::shop().await;
    let store = f.board.store();
    store.set_setting("appearance", "\"system\"").await.unwrap();
    store
        .set_setting("dark_theme", "\"dracula\"")
        .await
        .unwrap();
    let global = f.board.global_settings().await.unwrap();
    assert_eq!(
        global.theme,
        ThemeSetting::ByOs {
            dark: Theme::Dracula,
            light: Theme::OtaqLight
        }
    );
}

#[tokio::test]
async fn project_values_fall_back_to_global_when_empty() {
    let f = Fixture::shop().await;
    let global = GlobalSettings {
        agent_instructions: "Run tests before submitting.".into(),
        ..Default::default()
    };
    f.board.set_global_settings(&global).await.unwrap();

    let effective = f.board.effective_settings("SHOP").await.unwrap();
    assert_eq!(effective.agent_instructions, "Run tests before submitting.");
    assert_eq!(
        effective.workflow_skills.commit,
        ResolvedSkill::BuiltIn(WorkflowStep::Commit)
    );

    let own = ProjectSettings {
        agent_instructions: "Use my /tdd skill.".into(),
        let_finished_tickets_answer: false,
        ..Default::default()
    };
    f.board.set_project_settings("SHOP", &own).await.unwrap();
    assert_eq!(f.board.project_settings("SHOP").await.unwrap(), own);
    let effective = f.board.effective_settings("SHOP").await.unwrap();
    assert_eq!(effective.agent_instructions, "Use my /tdd skill.");
    assert_eq!(
        effective.workflow_skills.push,
        ResolvedSkill::BuiltIn(WorkflowStep::Push)
    );
}

#[tokio::test]
async fn invalid_values_are_stored_and_flagged_and_the_repo_is_untouched() {
    let f = Fixture::shop().await;
    let repo = std::path::PathBuf::from(f.board.project("SHOP").await.unwrap().repo_path);
    std::fs::write(repo.join(".env.local"), "A=1\n").unwrap();
    let before = (
        snapshot(&repo),
        git(&repo, &["status", "--porcelain", "--ignored"]),
    );

    let settings = ProjectSettings {
        env_files: vec![
            ".env.local".into(),
            ".env.missing".into(),
            "../secrets".into(),
        ],
        open_url: "localhost:3000".into(),
        setup_command: "npm install".into(),
        ..Default::default()
    };
    f.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    assert_eq!(
        f.board.project_settings("SHOP").await.unwrap(),
        settings,
        "stored as given"
    );
    let problems = f.board.project_setting_problems("SHOP").await.unwrap();
    let missing = &problems[0].message;
    assert!(
        missing.starts_with("Can\u{2019}t find .env.missing in ")
            && missing.ends_with(". Add it to the repo root, or remove it from this list."),
        "{missing}"
    );
    let fields: Vec<_> = problems
        .iter()
        .skip(1)
        .map(|p| (p.field.as_str(), p.message.as_str()))
        .collect();
    assert_eq!(
        fields,
        [
            (
                "env_files",
                "../secrets must be a path inside the repository, like .env.local"
            ),
            (
                "open_url",
                "localhost:3000 isn't a URL; use something like http://localhost:$PORT"
            ),
        ]
    );

    let mut global = GlobalSettings::default();
    global.agent_limits.max_concurrency = 0;
    global.terminal_font_size = 99;
    f.board.set_global_settings(&global).await.unwrap();
    let problems = f.board.global_setting_problems().await.unwrap();
    assert_eq!(problems.len(), 2);

    assert_eq!(
        (
            snapshot(&repo),
            git(&repo, &["status", "--porcelain", "--ignored"])
        ),
        before
    );
}

/// T-0181 (VAL-13, AMG-01, PAG-03; d0063, d0064, d0068): the M12 settings' defaults.
#[tokio::test]
async fn m12_defaults() {
    let f = Fixture::shop().await;
    let global = f.board.global_settings().await.unwrap();
    assert!(global.validation);
    assert_eq!(global.validation_rounds, 3);
    assert_eq!(global.auto_merge, AutoMerge::Off);
    assert_eq!(global.workflow_skills.validate, SkillSetting::BuiltIn);
    assert!(global.project_agent_instructions.is_empty());
    let effective = f.board.effective_settings("SHOP").await.unwrap();
    assert!(effective.validation && !effective.auto_merge_inactive);
    assert_eq!(effective.validation_rounds, 3);
    assert_eq!(effective.auto_merge, AutoMerge::Off);
    assert_eq!(
        effective.workflow_skills.validate,
        ResolvedSkill::BuiltIn(WorkflowStep::Validate)
    );
}

/// Project overrides win; empty overrides use the global values; Auto-merge without validation is inactive;
/// rounds outside 1–5 are flagged and never used.
#[tokio::test]
async fn m12_overrides_and_resolution() {
    let f = Fixture::shop().await;
    let global = GlobalSettings {
        validation_rounds: 4,
        auto_merge: AutoMerge::EpicChildren,
        project_agent_instructions: "Use my /import-github skill.".into(),
        ..GlobalSettings::default()
    };
    f.board.set_global_settings(&global).await.unwrap();
    let effective = f.board.effective_settings("SHOP").await.unwrap();
    assert_eq!(
        (effective.validation_rounds, effective.auto_merge),
        (4, AutoMerge::EpicChildren)
    );
    assert_eq!(
        effective.project_agent_instructions,
        "Use my /import-github skill."
    );

    let own = ProjectSettings {
        validation: Some(false),
        validation_rounds: Some(2),
        auto_merge: Some(AutoMerge::All),
        project_agent_instructions: "Split into tickets of about one day.".into(),
        workflow_skills: WorkflowSkillOverrides {
            validate: SkillSetting::Legacy("/my-validate".into()),
            ..WorkflowSkillOverrides::default()
        },
        ..ProjectSettings::default()
    };
    f.board.set_project_settings("SHOP", &own).await.unwrap();
    assert_eq!(f.board.project_settings("SHOP").await.unwrap(), own);
    let effective = f.board.effective_settings("SHOP").await.unwrap();
    assert!(!effective.validation);
    assert_eq!(effective.validation_rounds, 2);
    assert_eq!(effective.auto_merge, AutoMerge::All);
    assert!(
        effective.auto_merge_inactive,
        "Auto-merge does nothing while validation is off"
    );
    // A legacy value isn't run: the global setting (the built-in) is (WSK-07).
    assert_eq!(
        effective.workflow_skills.validate,
        ResolvedSkill::BuiltIn(WorkflowStep::Validate)
    );
    assert_eq!(
        effective.project_agent_instructions,
        "Split into tickets of about one day."
    );

    // Flagged rounds are never used: the project's 6 falls back to the global value, the global 0 to 3.
    let own = ProjectSettings {
        validation_rounds: Some(6),
        ..ProjectSettings::default()
    };
    f.board.set_project_settings("SHOP", &own).await.unwrap();
    assert_eq!(
        f.board
            .effective_settings("SHOP")
            .await
            .unwrap()
            .validation_rounds,
        4
    );
    let problems = f.board.project_setting_problems("SHOP").await.unwrap();
    assert!(
        problems
            .iter()
            .any(|p| p.field == "validation_rounds" && p.message == "Use 1 to 5 validation rounds"),
        "{problems:?}"
    );
    f.board
        .set_global_settings(&GlobalSettings {
            validation_rounds: 0,
            ..GlobalSettings::default()
        })
        .await
        .unwrap();
    assert_eq!(
        f.board
            .effective_settings("SHOP")
            .await
            .unwrap()
            .validation_rounds,
        3
    );
    let problems = f.board.global_setting_problems().await.unwrap();
    assert!(problems.iter().any(|p| p.field == "validation_rounds"));
}

/// Schema paths (M19-e, d0087): each must be a relative glob inside the repository; a Remove or Template refresh
/// command without a Create command is flagged too, and nothing is refused.
#[tokio::test]
async fn schema_path_problems() {
    let f = Fixture::shop().await;
    let repo = f.repos.root().join("shop-web");
    let before = snapshot(&repo);
    let settings = ProjectSettings {
        schema_paths: vec![
            "db/migrations/**".into(),
            "".into(),
            "/etc/schema.sql".into(),
            "../other/db/**".into(),
            "db/[0-9.sql".into(),
            "prisma/schema.prisma".into(),
        ],
        resource_remove: "dropdb x".into(),
        resource_template_refresh: "make template".into(),
        ..ProjectSettings::default()
    };
    f.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    assert_eq!(
        f.board.project_settings("SHOP").await.unwrap(),
        settings,
        "stored as written"
    );
    let problems = f.board.project_setting_problems("SHOP").await.unwrap();
    let of = |field: &str| -> Vec<String> {
        problems
            .iter()
            .filter(|p| p.field == field)
            .map(|p| p.message.clone())
            .collect()
    };
    let schema = of("schema_paths");
    assert_eq!(schema.len(), 4, "{schema:?}");
    assert_eq!(schema[0], "A schema path entry is empty");
    assert!(schema[1].starts_with("/etc/schema.sql must be a path inside"));
    assert!(schema[2].starts_with("../other/db/** must be a path inside"));
    assert!(schema[3].contains("[ has no matching ]"), "{}", schema[3]);
    assert_eq!(of("resource_remove"), [resource_without_create()]);
    assert_eq!(of("resource_template_refresh").len(), 1);

    // With a Create command, Remove and Refresh are fine.
    let settings = ProjectSettings {
        resource_create: "createdb x".into(),
        schema_paths: vec!["db/migrations/**".into()],
        ..settings
    };
    f.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    assert!(
        f.board
            .project_setting_problems("SHOP")
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(snapshot(&repo), before, "the repository is untouched");
}
