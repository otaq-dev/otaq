//! Typed settings, global and per project (SET-01; d0003, d0023; SDD §20). Everything lives in the app's
//! database and nothing is ever written into a repository (d0023). Invalid values are stored as given and
//! flagged ([`SettingProblem`]), never silently fixed.
//!
//! Global settings are rows of the `settings` table, one per top-level field below (JSON values; a missing row
//! means the default). Project settings are the project's `settings_json`. Empty per-project values fall back
//! to the global ones (agent instructions, workflow skills, validation, rounds, Auto-merge, Project agent
//! instructions; d0040, d0050, d0063, d0064, d0068).
//!
//! The features that use these values arrive in their own milestones (limits T-0080, notifications T-0117,
//! Run T-0133, workflow skills T-0139, "let finished tickets answer" T-0153).

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{Board, BoardError};
use crate::domain::stored_enum;
use crate::naming;
use crate::store::now_ms;

/// The optional agent limit and back-pressure (d0016): off by default; when on, at most 5 agents, and new starts
/// wait while 3 items need the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentLimits {
    pub enabled: bool,
    pub max_concurrency: u32,
    pub back_pressure: u32,
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            enabled: false,
            max_concurrency: 5,
            back_pressure: 3,
        }
    }
}

/// The Commit, Push, Open PR and Validate workflow skills (d0050, d0063).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkflowSkills {
    pub commit: String,
    pub push: String,
    pub open_pr: String,
    /// What the validator runs (d0063).
    pub validate: String,
}

impl Default for WorkflowSkills {
    /// Crewboard's own skills: `/crewboard:commit`, `/crewboard:push`, `/crewboard:open-pr`,
    /// `/crewboard:validate`.
    fn default() -> Self {
        Self {
            commit: naming::skill("commit"),
            push: naming::skill("push"),
            open_pr: naming::skill("open-pr"),
            validate: naming::skill("validate"),
        }
    }
}

/// A workflow step whose skill the user can replace (d0050, d0063).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStep {
    Commit,
    Push,
    OpenPr,
    Validate,
}

impl WorkflowStep {
    pub const ALL: [WorkflowStep; 4] = [Self::Commit, Self::Push, Self::OpenPr, Self::Validate];

    /// The settings field: `commit`, `push`, `open_pr`, `validate`.
    pub fn field(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Push => "push",
            Self::OpenPr => "open_pr",
            Self::Validate => "validate",
        }
    }
}

impl WorkflowSkills {
    pub fn get(&self, step: WorkflowStep) -> &str {
        match step {
            WorkflowStep::Commit => &self.commit,
            WorkflowStep::Push => &self.push,
            WorkflowStep::OpenPr => &self.open_pr,
            WorkflowStep::Validate => &self.validate,
        }
    }
}

impl WorkflowSkillOverrides {
    pub fn get(&self, step: WorkflowStep) -> &str {
        match step {
            WorkflowStep::Commit => &self.commit,
            WorkflowStep::Push => &self.push,
            WorkflowStep::OpenPr => &self.open_pr,
            WorkflowStep::Validate => &self.validate,
        }
    }
}

stored_enum! {
    /// Auto-merge (d0064): validated tickets Local-merge without a click. Off by default.
    #[derive(Default)]
    pub enum AutoMerge ("auto-merge") {
        /// A validated ticket waits in Ready for Review.
        #[default]
        Off = "off",
        /// A validated epic child merges into its epic branch (local, never pushed).
        EpicChildren = "epic_children",
        /// Standalone tickets merge into their base branch too (local, never pushed).
        All = "all",
    }
}

impl AutoMerge {
    /// The settings screen's words (*15 · Settings*).
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::EpicChildren => "Epic children only",
            Self::All => "All tickets",
        }
    }
}

/// Validation rounds the user may pick (d0063); anything else is flagged and never used.
pub const VALIDATION_ROUNDS: std::ops::RangeInclusive<u32> = 1..=5;
/// The rounds when none is set.
pub const DEFAULT_VALIDATION_ROUNDS: u32 = 3;

/// The skill a step runs (REV-09): the project's override, else the global value, else Crewboard's own skill.
/// Empty values fall through; values are trimmed.
pub fn resolve_skill(
    global: &WorkflowSkills,
    own: &WorkflowSkillOverrides,
    step: WorkflowStep,
) -> String {
    [own.get(step), global.get(step)]
        .into_iter()
        .map(str::trim)
        .find(|v| !v.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| WorkflowSkills::default().get(step).to_owned())
}

/// Why a workflow skill value isn't a skill name, if it isn't: a slash and a name such as `/my-pr` or
/// `/crewboard:open-pr`. Empty is fine (it falls back).
pub fn skill_name_problem(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let ok = value.strip_prefix('/').is_some_and(|name| {
        name.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
    });
    (!ok).then(|| format!("{value} isn't a skill name; use something like /my-pr"))
}

stored_enum! {
    /// The mode (d0069): Dark uses the dark theme, Light the light theme, System follows the OS.
    #[derive(Default)]
    pub enum Appearance ("appearance") {
        Light = "light",
        Dark = "dark",
        #[default]
        System = "system",
    }
}

stored_enum! {
    /// The theme used while the window is dark (d0069). Stored as the design's palette key; a name this version
    /// doesn't know reads as Crewboard.
    #[derive(Default)]
    pub enum DarkTheme ("dark theme") {
        #[default]
        Crewboard = naming::THEME_KEY,
        CatppuccinFrappe = "catppuccin-frappe",
        CatppuccinMacchiato = "catppuccin-macchiato",
        CatppuccinMocha = "catppuccin-mocha",
        TokyoNight = "tokyo-night",
        Dracula = "dracula",
        Nord = "nord",
        GruvboxDark = "gruvbox-dark",
        RosePine = "rose-pine",
        OneDark = "one-dark",
    }
}

stored_enum! {
    /// The theme used while the window is light (d0069). Stored as the design's palette key; a name this version
    /// doesn't know reads as Crewboard.
    #[derive(Default)]
    pub enum LightTheme ("light theme") {
        #[default]
        Crewboard = naming::THEME_KEY,
        CatppuccinLatte = "catppuccin-latte",
        Daylight = "daylight",
        SolarizedLight = "solarized-light",
        RosePineDawn = "rose-pine-dawn",
    }
}

/// Settings for every project (SDD §20, left column).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalSettings {
    pub agent_limits: AgentLimits,
    /// The user's rules for agents, printed by `crewboard prime` (d0040). Empty by default.
    pub agent_instructions: String,
    pub workflow_skills: WorkflowSkills,
    /// A separate validator checks every submit (d0063). On by default.
    pub validation: bool,
    /// Rounds before "Validation failed N times" (d0063): 1–5, 3 by default.
    pub validation_rounds: u32,
    /// Off by default (d0064).
    pub auto_merge: AutoMerge,
    /// What every Project agent follows, printed by `crewboard prime --project` (d0068). Empty by default.
    pub project_agent_instructions: String,
    /// Show a dollar estimate next to tokens (d0020). Off by default.
    pub dollar_estimate: bool,
    /// OS notifications for items that block an agent (d0016). On by default.
    pub notifications: bool,
    /// The mode: Light, Dark or System (d0069).
    pub appearance: Appearance,
    /// The theme of a dark window, and of a light one (d0069). Crewboard Dark and Crewboard Light by default.
    pub dark_theme: DarkTheme,
    pub light_theme: LightTheme,
    pub terminal_font_size: u32,
}

/// The terminal font size when none is set.
pub const DEFAULT_TERMINAL_FONT_SIZE: u32 = 13;
/// The terminal font sizes accepted without a warning.
pub const TERMINAL_FONT_SIZES: std::ops::RangeInclusive<u32> = 8..=32;

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            agent_limits: AgentLimits::default(),
            agent_instructions: String::new(),
            workflow_skills: WorkflowSkills::default(),
            validation: true,
            validation_rounds: DEFAULT_VALIDATION_ROUNDS,
            auto_merge: AutoMerge::Off,
            project_agent_instructions: String::new(),
            dollar_estimate: false,
            notifications: true,
            appearance: Appearance::System,
            dark_theme: DarkTheme::Crewboard,
            light_theme: LightTheme::Crewboard,
            terminal_font_size: DEFAULT_TERMINAL_FONT_SIZE,
        }
    }
}

/// Per-project workflow skill overrides; empty means "use the global one".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkflowSkillOverrides {
    pub commit: String,
    pub push: String,
    pub open_pr: String,
    pub validate: String,
}

/// Settings of one project (SDD §20, right column). Name and base branch are project fields (`update_project`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectSettings {
    pub setup_command: String,
    pub run_command: String,
    pub open_url: String,
    /// Files copied from the repository into each worktree, relative to the repository root.
    pub env_files: Vec<String>,
    /// Overrides the global agent instructions when not empty.
    pub agent_instructions: String,
    pub workflow_skills: WorkflowSkillOverrides,
    /// Let finished tickets answer other agents' questions (d0030). On by default.
    pub let_finished_tickets_answer: bool,
    /// Validation on or off for this project; `None` uses the global setting (d0063). Stored as `null` so the
    /// field-by-field read keeps a set value.
    pub validation: Option<bool>,
    /// `None` uses the global rounds.
    pub validation_rounds: Option<u32>,
    /// `None` uses the global Auto-merge (d0064).
    pub auto_merge: Option<AutoMerge>,
    /// Overrides the global Project agent instructions when not empty (d0068).
    pub project_agent_instructions: String,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            setup_command: String::new(),
            run_command: String::new(),
            open_url: String::new(),
            env_files: Vec::new(),
            agent_instructions: String::new(),
            workflow_skills: WorkflowSkillOverrides::default(),
            let_finished_tickets_answer: true,
            validation: None,
            validation_rounds: None,
            auto_merge: None,
            project_agent_instructions: String::new(),
        }
    }
}

/// The values a project's agents actually use, after falling back to the global settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveSettings {
    pub agent_instructions: String,
    pub workflow_skills: WorkflowSkills,
    pub validation: bool,
    /// Always within 1–5: a flagged value falls back to the global one, then to 3.
    pub validation_rounds: u32,
    pub auto_merge: AutoMerge,
    /// Auto-merge is set but does nothing because validation is off for the project (the settings say so,
    /// d0064).
    pub auto_merge_inactive: bool,
    pub project_agent_instructions: String,
}

/// The project's override when it isn't empty, else the global value.
fn own_or_global(own: &str, global: &str) -> String {
    if own.trim().is_empty() {
        global.to_owned()
    } else {
        own.to_owned()
    }
}

/// The rounds a project uses: its own when valid, else the global value when valid, else 3 (a flagged value is
/// never used).
pub fn effective_rounds(own: Option<u32>, global: u32) -> u32 {
    own.into_iter()
        .chain([global])
        .find(|r| VALIDATION_ROUNDS.contains(r))
        .unwrap_or(DEFAULT_VALIDATION_ROUNDS)
}

/// Why a rounds value can't be used, if it can't.
fn rounds_problem(rounds: u32) -> Option<SettingProblem> {
    (!VALIDATION_ROUNDS.contains(&rounds)).then(|| {
        problem(
            "validation_rounds",
            &format!(
                "Use {} to {} validation rounds",
                VALIDATION_ROUNDS.start(),
                VALIDATION_ROUNDS.end()
            ),
        )
    })
}

/// A stored value that is flagged as invalid. `field` names the setting (`env_files`, `open_url`, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingProblem {
    pub field: String,
    pub message: String,
}

impl Board {
    pub async fn global_settings(&self) -> Result<GlobalSettings, BoardError> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value_json FROM settings")
            .fetch_all(self.store.pool())
            .await?;
        let mut object = Map::new();
        for (key, value) in rows {
            if let Ok(value) = serde_json::from_str::<Value>(&value) {
                object.insert(key, value);
            }
        }
        Ok(read_or_default(Value::Object(object)))
    }

    /// Stores every global setting.
    pub async fn set_global_settings(&self, settings: &GlobalSettings) -> Result<(), BoardError> {
        let Value::Object(fields) = serde_json::to_value(settings).expect("settings serialize")
        else {
            unreachable!("settings are an object")
        };
        let mut tx = self.store.pool().begin().await?;
        for (key, value) in fields {
            sqlx::query(
                "INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, ?)
                 ON CONFLICT (key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            )
            .bind(key)
            .bind(value.to_string())
            .bind(now_ms())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn project_settings(&self, project: &str) -> Result<ProjectSettings, BoardError> {
        let project = self.project(project).await?;
        let value: Value = serde_json::from_str(&project.settings_json).unwrap_or_default();
        Ok(read_or_default(value))
    }

    pub async fn set_project_settings(
        &self,
        project: &str,
        settings: &ProjectSettings,
    ) -> Result<(), BoardError> {
        let project = self.project(project).await?;
        let json = serde_json::to_string(settings).expect("settings serialize");
        sqlx::query("UPDATE projects SET settings_json = ? WHERE id = ?")
            .bind(json)
            .bind(&project.id)
            .execute(self.store.pool())
            .await?;
        Ok(())
    }

    /// The values a project's agents, validators and Project agents use.
    pub async fn effective_settings(&self, project: &str) -> Result<EffectiveSettings, BoardError> {
        let global = self.global_settings().await?;
        let own = self.project_settings(project).await?;
        Ok(effective(&global, &own))
    }

    /// Problems with the global settings' values.
    pub async fn global_setting_problems(&self) -> Result<Vec<SettingProblem>, BoardError> {
        let settings = self.global_settings().await?;
        let mut problems = Vec::new();
        if settings.agent_limits.max_concurrency == 0 {
            problems.push(problem(
                "agent_limits.max_concurrency",
                "Allow at least 1 agent",
            ));
        }
        if settings.agent_limits.back_pressure == 0 {
            problems.push(problem(
                "agent_limits.back_pressure",
                "Back-pressure needs at least 1 item",
            ));
        }
        for step in WorkflowStep::ALL {
            if let Some(message) = skill_name_problem(settings.workflow_skills.get(step)) {
                problems.push(problem(
                    &format!("workflow_skills.{}", step.field()),
                    &message,
                ));
            }
        }
        problems.extend(rounds_problem(settings.validation_rounds));
        if !TERMINAL_FONT_SIZES.contains(&settings.terminal_font_size) {
            problems.push(problem(
                "terminal_font_size",
                &format!(
                    "Use a size from {} to {}",
                    TERMINAL_FONT_SIZES.start(),
                    TERMINAL_FONT_SIZES.end()
                ),
            ));
        }
        Ok(problems)
    }

    /// Problems with a project's settings: env files missing from the repository, an Open URL that isn't a URL.
    /// Reads the repository, never writes to it.
    pub async fn project_setting_problems(
        &self,
        project: &str,
    ) -> Result<Vec<SettingProblem>, BoardError> {
        let settings = self.project_settings(project).await?;
        let repo = self.project(project).await?.repo_path;
        let mut problems = Vec::new();
        for file in &settings.env_files {
            if let Some(message) = env_file_problem(Path::new(&repo), file) {
                problems.push(problem("env_files", &message));
            }
        }
        for step in WorkflowStep::ALL {
            if let Some(message) = skill_name_problem(settings.workflow_skills.get(step)) {
                problems.push(problem(
                    &format!("workflow_skills.{}", step.field()),
                    &message,
                ));
            }
        }
        problems.extend(settings.validation_rounds.and_then(rounds_problem));
        if !settings.open_url.trim().is_empty() && !looks_like_url(settings.open_url.trim()) {
            problems.push(problem(
                "open_url",
                &format!(
                    "{} isn't a URL; use something like http://localhost:$PORT",
                    settings.open_url.trim()
                ),
            ));
        }
        Ok(problems)
    }
}

/// Resolves a project's settings against the global ones (pure; [`Board::effective_settings`] reads them).
pub fn effective(global: &GlobalSettings, own: &ProjectSettings) -> EffectiveSettings {
    let skill = |step| resolve_skill(&global.workflow_skills, &own.workflow_skills, step);
    let validation = own.validation.unwrap_or(global.validation);
    let auto_merge = own.auto_merge.unwrap_or(global.auto_merge);
    EffectiveSettings {
        agent_instructions: own_or_global(&own.agent_instructions, &global.agent_instructions),
        workflow_skills: WorkflowSkills {
            commit: skill(WorkflowStep::Commit),
            push: skill(WorkflowStep::Push),
            open_pr: skill(WorkflowStep::OpenPr),
            validate: skill(WorkflowStep::Validate),
        },
        validation,
        validation_rounds: effective_rounds(own.validation_rounds, global.validation_rounds),
        auto_merge,
        auto_merge_inactive: auto_merge != AutoMerge::Off && !validation,
        project_agent_instructions: own_or_global(
            &own.project_agent_instructions,
            &global.project_agent_instructions,
        ),
    }
}

/// Reads settings from stored JSON; anything missing or unreadable gets its default.
fn read_or_default<T: Default + serde::de::DeserializeOwned + Serialize>(stored: Value) -> T {
    // Merge field by field so one bad value doesn't reset the others.
    let mut merged = serde_json::to_value(T::default()).expect("settings serialize");
    if let (Value::Object(merged), Value::Object(stored)) = (&mut merged, stored) {
        for (key, value) in stored {
            if let Some(slot) = merged.get_mut(&key) {
                let previous = std::mem::replace(slot, value);
                let candidate = Value::Object(merged.clone());
                if serde_json::from_value::<T>(candidate).is_err() {
                    merged.insert(key, previous);
                }
            }
        }
    }
    serde_json::from_value(merged).unwrap_or_default()
}

fn problem(field: &str, message: &str) -> SettingProblem {
    SettingProblem {
        field: field.to_owned(),
        message: message.to_owned(),
    }
}

/// Why an env file can't be copied, if it can't: it must be a relative path to a file inside the repository.
fn env_file_problem(repo: &Path, file: &str) -> Option<String> {
    let path = Path::new(file.trim());
    let escapes =
        path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir));
    if file.trim().is_empty() {
        Some("An env file entry is empty".to_owned())
    } else if escapes {
        Some(format!(
            "{file} must be a path inside the repository, like .env.local"
        ))
    } else if !repo.join(path).is_file() {
        Some(format!("{file} doesn't exist in the repository"))
    } else {
        None
    }
}

/// A light check: `http://` or `https://` followed by a host, and no spaces. `$PORT` is allowed (d0035).
fn looks_like_url(url: &str) -> bool {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"));
    rest.is_some_and(|rest| {
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        !host.is_empty() && !url.contains(char::is_whitespace)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        for good in [
            "http://localhost:3000",
            "https://example.com/path?q=1",
            "http://localhost:$PORT/",
        ] {
            assert!(looks_like_url(good), "{good}");
        }
        for bad in [
            "localhost:3000",
            "http://",
            "ftp://x",
            "http://a b",
            "https:///x",
        ] {
            assert!(!looks_like_url(bad), "{bad}");
        }
    }

    #[test]
    fn workflow_skills_resolve_project_then_global_then_built_in() {
        let mut global = WorkflowSkills::default();
        let mut own = WorkflowSkillOverrides::default();
        for step in WorkflowStep::ALL {
            assert_eq!(
                resolve_skill(&global, &own, step),
                WorkflowSkills::default().get(step)
            );
        }
        global.push = "/team-push".into();
        assert_eq!(
            resolve_skill(&global, &own, WorkflowStep::Push),
            "/team-push"
        );
        own.push = " /my-push ".into();
        own.open_pr = "/my-pr".into();
        assert_eq!(resolve_skill(&global, &own, WorkflowStep::Push), "/my-push");
        assert_eq!(resolve_skill(&global, &own, WorkflowStep::OpenPr), "/my-pr");
        assert_eq!(
            resolve_skill(&global, &own, WorkflowStep::Commit),
            naming::skill("commit")
        );
        // An emptied global value falls back to Crewboard's own skill.
        global.commit = "  ".into();
        assert_eq!(
            resolve_skill(&global, &own, WorkflowStep::Commit),
            naming::skill("commit")
        );
    }

    #[test]
    fn skill_names() {
        for good in ["", "/my-pr", "/team:open-pr", "/tdd_2", "/a.b"] {
            assert_eq!(skill_name_problem(good), None, "{good}");
        }
        for bad in ["my-pr", "/", "/my pr", "//x", "/-x", "open pr"] {
            assert!(skill_name_problem(bad).is_some(), "{bad}");
        }
        assert_eq!(
            skill_name_problem("my-pr").unwrap(),
            "my-pr isn't a skill name; use something like /my-pr"
        );
    }

    #[test]
    fn theme_keys_are_the_designs_palette_keys() {
        let dark: Vec<&str> = DarkTheme::ALL.iter().map(|t| t.as_str()).collect();
        assert_eq!(
            dark,
            [
                naming::THEME_KEY,
                "catppuccin-frappe",
                "catppuccin-macchiato",
                "catppuccin-mocha",
                "tokyo-night",
                "dracula",
                "nord",
                "gruvbox-dark",
                "rose-pine",
                "one-dark",
            ]
        );
        let light: Vec<&str> = LightTheme::ALL.iter().map(|t| t.as_str()).collect();
        assert_eq!(
            light,
            [
                naming::THEME_KEY,
                "catppuccin-latte",
                "daylight",
                "solarized-light",
                "rose-pine-dawn",
            ]
        );
        for theme in DarkTheme::ALL {
            let json = serde_json::to_value(theme).unwrap();
            assert_eq!(serde_json::from_value::<DarkTheme>(json).unwrap(), *theme);
        }
        for theme in LightTheme::ALL {
            let json = serde_json::to_value(theme).unwrap();
            assert_eq!(serde_json::from_value::<LightTheme>(json).unwrap(), *theme);
        }
        let defaults = GlobalSettings::default();
        assert_eq!(
            (
                defaults.appearance,
                defaults.dark_theme,
                defaults.light_theme
            ),
            (
                Appearance::System,
                DarkTheme::Crewboard,
                LightTheme::Crewboard
            )
        );
    }

    #[test]
    fn an_unknown_theme_reads_as_crewboard_and_keeps_the_others() {
        let stored = serde_json::json!({
            "appearance": "dark",
            "dark_theme": "monokai",
            "light_theme": "dracula",
            "terminal_font_size": 15,
        });
        let settings: GlobalSettings = read_or_default(stored);
        assert_eq!(settings.dark_theme, DarkTheme::Crewboard);
        assert_eq!(settings.light_theme, LightTheme::Crewboard);
        assert_eq!(settings.appearance, Appearance::Dark);
        assert_eq!(settings.terminal_font_size, 15);
        // A database written before themes existed: the stored mode, Crewboard themes.
        let settings: GlobalSettings = read_or_default(serde_json::json!({"appearance": "light"}));
        assert_eq!(
            (
                settings.appearance,
                settings.dark_theme,
                settings.light_theme
            ),
            (
                Appearance::Light,
                DarkTheme::Crewboard,
                LightTheme::Crewboard
            )
        );
    }

    #[test]
    fn a_bad_stored_value_keeps_the_others() {
        let stored = serde_json::json!({"notifications": "loud", "terminal_font_size": 15});
        let settings: GlobalSettings = read_or_default(stored);
        assert!(settings.notifications);
        assert_eq!(settings.terminal_font_size, 15);
    }
}
