//! Settings (SET-01): defaults, per-project fallback, validation against a temporary repository, and that no
//! settings code writes into the repository.

mod common;

use common::Fixture;
use crewboard_core::board::settings::{
    Appearance, DarkTheme, GlobalSettings, LightTheme, ProjectSettings, WorkflowSkillOverrides,
};
use crewboard_core::git::testing::{git, snapshot};
use crewboard_core::naming;

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
    assert!(!global.dollar_estimate);
    assert!(global.notifications);
    assert_eq!(global.appearance, Appearance::System);
    assert_eq!(global.workflow_skills.commit, naming::skill("commit"));
    assert_eq!(global.workflow_skills.push, naming::skill("push"));
    assert_eq!(global.workflow_skills.open_pr, naming::skill("open-pr"));
    assert!(global.workflow_skills.open_pr.ends_with(":open-pr"));

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
    global.appearance = Appearance::Dark;
    global.dark_theme = DarkTheme::TokyoNight;
    global.light_theme = LightTheme::Daylight;
    global.terminal_font_size = 15;
    global.notifications = false;
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
async fn stored_theme_names_the_app_does_not_know_read_as_crewboard() {
    let f = Fixture::shop().await;
    let store = f.board.store();
    store.set_setting("appearance", "\"light\"").await.unwrap();
    store
        .set_setting("dark_theme", "\"monokai\"")
        .await
        .unwrap();
    store.set_setting("light_theme", "\"nord\"").await.unwrap();
    store.set_setting("notifications", "false").await.unwrap();
    let global = f.board.global_settings().await.unwrap();
    assert_eq!(global.appearance, Appearance::Light);
    assert_eq!(global.dark_theme, DarkTheme::Crewboard);
    assert_eq!(global.light_theme, LightTheme::Crewboard);
    assert!(!global.notifications);
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
    assert_eq!(effective.workflow_skills, global.workflow_skills);

    let own = ProjectSettings {
        agent_instructions: "Use my /tdd skill.".into(),
        workflow_skills: WorkflowSkillOverrides {
            commit: "/my-commit".into(),
            ..Default::default()
        },
        let_finished_tickets_answer: false,
        ..Default::default()
    };
    f.board.set_project_settings("SHOP", &own).await.unwrap();
    assert_eq!(f.board.project_settings("SHOP").await.unwrap(), own);
    let effective = f.board.effective_settings("SHOP").await.unwrap();
    assert_eq!(effective.agent_instructions, "Use my /tdd skill.");
    assert_eq!(effective.workflow_skills.commit, "/my-commit");
    assert_eq!(effective.workflow_skills.push, global.workflow_skills.push);
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
    let fields: Vec<_> = problems
        .iter()
        .map(|p| (p.field.as_str(), p.message.as_str()))
        .collect();
    assert_eq!(
        fields,
        [
            ("env_files", ".env.missing doesn't exist in the repository"),
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
