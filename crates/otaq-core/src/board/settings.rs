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
    /// Otaq's own skills: `/otaq:commit`, `/otaq:push`, `/otaq:open-pr`,
    /// `/otaq:validate`.
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

/// The skill a step runs (REV-09): the project's override, else the global value, else Otaq's own skill.
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
/// `/otaq:open-pr`. Empty is fine (it falls back).
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
    /// One of the 15 bundled themes (d0069, d0070), stored as its name: the design's palette key, with Otaq's
    /// two versions told apart. The dark themes first, then the light ones, as the pickers list them.
    pub enum Theme ("theme") {
        OtaqDark = naming::THEME_DARK,
        CatppuccinFrappe = "catppuccin-frappe",
        CatppuccinMacchiato = "catppuccin-macchiato",
        CatppuccinMocha = "catppuccin-mocha",
        TokyoNight = "tokyo-night",
        Dracula = "dracula",
        Nord = "nord",
        GruvboxDark = "gruvbox-dark",
        RosePine = "rose-pine",
        OneDark = "one-dark",
        OtaqLight = naming::THEME_LIGHT,
        CatppuccinLatte = "catppuccin-latte",
        Daylight = "daylight",
        SolarizedLight = "solarized-light",
        RosePineDawn = "rose-pine-dawn",
    }
}

impl Theme {
    /// Whether the theme draws a dark window.
    pub fn is_dark(self) -> bool {
        !matches!(
            self,
            Self::OtaqLight
                | Self::CatppuccinLatte
                | Self::Daylight
                | Self::SolarizedLight
                | Self::RosePineDawn
        )
    }

    /// A theme stored by d0069 as a dark or light theme, where both Otaq versions were `otaq`. A name
    /// this version doesn't know, or one of the other kind, reads as Otaq.
    fn from_0069(value: Option<&Value>, dark: bool) -> Self {
        let otaq = if dark {
            Self::OtaqDark
        } else {
            Self::OtaqLight
        };
        match value.and_then(Value::as_str) {
            Some(naming::THEME_KEY) | None => otaq,
            Some(name) => name
                .parse()
                .ok()
                .filter(|theme: &Self| theme.is_dark() == dark)
                .unwrap_or(otaq),
        }
    }
}

/// The theme setting (d0070): one theme, used all the time. Until one is picked (a fresh install, or System mode
/// from before d0070) it holds a theme for a dark OS and one for a light OS; the app picks by the OS at its next
/// start and stores that theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ThemeSetting {
    Picked(Theme),
    ByOs { dark: Theme, light: Theme },
}

impl Default for ThemeSetting {
    /// Otaq Dark on a dark OS, Otaq Light on a light one.
    fn default() -> Self {
        Self::ByOs {
            dark: Theme::OtaqDark,
            light: Theme::OtaqLight,
        }
    }
}

impl ThemeSetting {
    /// The theme to use, given whether the OS is dark.
    pub fn resolve(self, os_dark: bool) -> Theme {
        match self {
            Self::Picked(theme) => theme,
            Self::ByOs { dark, light } => {
                if os_dark {
                    dark
                } else {
                    light
                }
            }
        }
    }

    /// The setting d0069 stored as a mode (`appearance`) and two themes: Dark and Light mode keep the theme they
    /// showed, System mode keeps both until the OS decides. `None` when nothing of it was stored.
    fn from_0069(stored: &Map<String, Value>) -> Option<Self> {
        if !THEME_KEYS_0069.iter().any(|key| stored.contains_key(*key)) {
            return None;
        }
        let dark = Theme::from_0069(stored.get("dark_theme"), true);
        let light = Theme::from_0069(stored.get("light_theme"), false);
        Some(match stored.get("appearance").and_then(Value::as_str) {
            Some("dark") => Self::Picked(dark),
            Some("light") => Self::Picked(light),
            _ => Self::ByOs { dark, light },
        })
    }
}

/// The settings rows d0069 used for the theme, replaced by `theme` (d0070).
const THEME_KEYS_0069: [&str; 3] = ["appearance", "dark_theme", "light_theme"];

/// Settings for every project (SDD §20, left column).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalSettings {
    pub agent_limits: AgentLimits,
    /// The user's rules for agents, printed by `otaq prime` (d0040). Empty by default.
    pub agent_instructions: String,
    pub workflow_skills: WorkflowSkills,
    /// A separate validator checks every submit (d0063). On by default.
    pub validation: bool,
    /// Rounds before "Validation failed N times" (d0063): 1–5, 3 by default.
    pub validation_rounds: u32,
    /// Off by default (d0064).
    pub auto_merge: AutoMerge,
    /// What every Project agent follows, printed by `otaq prime --project` (d0068). Empty by default.
    pub project_agent_instructions: String,
    /// Show a dollar estimate next to tokens (d0020). Off by default.
    pub dollar_estimate: bool,
    /// OS notifications for items that block an agent (d0016). On by default.
    pub notifications: bool,
    /// One theme, used all the time (d0070).
    pub theme: ThemeSetting,
    pub terminal_font_size: u32,
    /// How terminals draw (Q37, Q38): one setting for every terminal.
    pub terminal_renderer: TerminalRenderer,
    /// The description editor shows the raw Markdown source instead of formatted text (d0078 D9): every editor opens
    /// in the mode last chosen with its "Markdown" toggle. Off by default; not a row of the Settings page.
    pub description_source_mode: bool,
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
            theme: ThemeSetting::default(),
            terminal_font_size: DEFAULT_TERMINAL_FONT_SIZE,
            terminal_renderer: TerminalRenderer::default(),
            description_source_mode: false,
        }
    }
}

/// The terminal renderer: xterm.js's two (UIT-05). WebGL draws on the GPU and keeps the CPU low; DOM draws with the
/// page's elements. A broken WebGL driver draws garbage without any error (the M14 test build, in a Linux VM), so
/// nothing falls back by itself: this setting picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalRenderer {
    Webgl,
    Dom,
}

impl Default for TerminalRenderer {
    /// DOM on Linux, WebGL on macOS and Windows (Q37, deferred as option 1). The service and the app run on the same
    /// machine, so the service's platform is the app's.
    fn default() -> Self {
        if cfg!(target_os = "linux") {
            Self::Dom
        } else {
            Self::Webgl
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
        if !object.contains_key("theme")
            && let Some(theme) = ThemeSetting::from_0069(&object)
        {
            object.insert(
                "theme".to_owned(),
                serde_json::to_value(theme).expect("theme"),
            );
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
        for key in THEME_KEYS_0069 {
            sqlx::query("DELETE FROM settings WHERE key = ?")
                .bind(key)
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

    /// The effective validation rounds of the project with this id (cards show "round 1 of 3").
    pub(crate) async fn validation_rounds_of(&self, project_id: &str) -> Result<u32, BoardError> {
        let global = self.global_settings().await?;
        let json: Option<String> =
            sqlx::query_scalar("SELECT settings_json FROM projects WHERE id = ?")
                .bind(project_id)
                .fetch_optional(self.store.pool())
                .await?;
        let own: ProjectSettings =
            read_or_default(serde_json::from_str(&json.unwrap_or_default()).unwrap_or_default());
        Ok(effective_rounds(
            own.validation_rounds,
            global.validation_rounds,
        ))
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
        // An emptied global value falls back to Otaq's own skill.
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
    fn theme_names_are_the_designs_palette_keys() {
        let names: Vec<&str> = Theme::ALL.iter().map(|t| t.as_str()).collect();
        assert_eq!(
            names,
            [
                naming::THEME_DARK,
                "catppuccin-frappe",
                "catppuccin-macchiato",
                "catppuccin-mocha",
                "tokyo-night",
                "dracula",
                "nord",
                "gruvbox-dark",
                "rose-pine",
                "one-dark",
                naming::THEME_LIGHT,
                "catppuccin-latte",
                "daylight",
                "solarized-light",
                "rose-pine-dawn",
            ]
        );
        let dark = Theme::ALL.iter().filter(|t| t.is_dark()).count();
        assert_eq!(dark, 10);
        assert!(Theme::ALL[..dark].iter().all(|t| t.is_dark()));
        for theme in Theme::ALL {
            let json = serde_json::to_value(ThemeSetting::Picked(*theme)).unwrap();
            assert_eq!(json, serde_json::json!(theme.as_str()));
            assert_eq!(
                serde_json::from_value::<ThemeSetting>(json).unwrap(),
                ThemeSetting::Picked(*theme)
            );
        }
        let defaults = GlobalSettings::default();
        assert_eq!(defaults.theme.resolve(true), Theme::OtaqDark);
        assert_eq!(defaults.theme.resolve(false), Theme::OtaqLight);
        let by_os = serde_json::to_value(defaults.theme).unwrap();
        assert_eq!(
            by_os,
            serde_json::json!({"dark": naming::THEME_DARK, "light": naming::THEME_LIGHT})
        );
        assert_eq!(
            serde_json::from_value::<ThemeSetting>(by_os).unwrap(),
            defaults.theme
        );
    }

    #[test]
    fn a_picked_theme_is_used_whatever_the_os() {
        let picked = ThemeSetting::Picked(Theme::Dracula);
        assert_eq!(picked.resolve(true), Theme::Dracula);
        assert_eq!(picked.resolve(false), Theme::Dracula);
    }

    #[test]
    fn an_unknown_theme_reads_as_unpicked_and_keeps_the_others() {
        let stored = serde_json::json!({"theme": "monokai", "terminal_font_size": 15});
        let settings: GlobalSettings = read_or_default(stored);
        assert_eq!(settings.theme, ThemeSetting::default());
        assert_eq!(settings.terminal_font_size, 15);
    }

    #[test]
    fn a_0069_mode_keeps_the_theme_it_showed() {
        let read = |stored: Value| {
            let Value::Object(stored) = stored else {
                unreachable!()
            };
            ThemeSetting::from_0069(&stored)
        };
        let both = |appearance: &str| {
            read(serde_json::json!({
                "appearance": appearance,
                "dark_theme": "tokyo-night",
                "light_theme": "daylight",
            }))
        };
        assert_eq!(both("dark"), Some(ThemeSetting::Picked(Theme::TokyoNight)));
        assert_eq!(both("light"), Some(ThemeSetting::Picked(Theme::Daylight)));
        assert_eq!(
            both("system"),
            Some(ThemeSetting::ByOs {
                dark: Theme::TokyoNight,
                light: Theme::Daylight
            })
        );
        // Both Otaq versions were stored as `otaq`; unknown names read as Otaq.
        assert_eq!(
            read(serde_json::json!({"appearance": "light", "light_theme": naming::THEME_KEY})),
            Some(ThemeSetting::Picked(Theme::OtaqLight))
        );
        assert_eq!(
            read(serde_json::json!({"appearance": "dark", "dark_theme": "monokai"})),
            Some(ThemeSetting::Picked(Theme::OtaqDark))
        );
        assert_eq!(
            read(serde_json::json!({"dark_theme": "nord"})),
            Some(ThemeSetting::ByOs {
                dark: Theme::Nord,
                light: Theme::OtaqLight
            })
        );
        assert_eq!(read(serde_json::json!({"notifications": false})), None);
    }

    #[test]
    fn the_terminal_renderer_defaults_to_dom_on_linux_only() {
        let default = GlobalSettings::default().terminal_renderer;
        if cfg!(target_os = "linux") {
            assert_eq!(default, TerminalRenderer::Dom);
        } else {
            assert_eq!(default, TerminalRenderer::Webgl);
        }
        let stored = serde_json::json!({"terminal_renderer": "dom"});
        let settings: GlobalSettings = read_or_default(stored);
        assert_eq!(settings.terminal_renderer, TerminalRenderer::Dom);
        let stored = serde_json::json!({"terminal_renderer": "vulkan", "terminal_font_size": 15});
        let settings: GlobalSettings = read_or_default(stored);
        assert_eq!(settings.terminal_renderer, default);
        assert_eq!(settings.terminal_font_size, 15);
    }

    #[test]
    fn the_description_source_mode_is_off_until_chosen() {
        assert!(!GlobalSettings::default().description_source_mode);
        // Settings stored before the field existed load with it off and keep their own values.
        let settings: GlobalSettings =
            read_or_default(serde_json::json!({"terminal_font_size": 15}));
        assert!(!settings.description_source_mode);
        assert_eq!(settings.terminal_font_size, 15);
        let old: GlobalSettings =
            serde_json::from_value(serde_json::json!({"validation": false})).unwrap();
        assert!(!old.description_source_mode);
        let settings: GlobalSettings =
            read_or_default(serde_json::json!({"description_source_mode": true}));
        assert!(settings.description_source_mode);
    }

    #[test]
    fn a_bad_stored_value_keeps_the_others() {
        let stored = serde_json::json!({"notifications": "loud", "terminal_font_size": 15});
        let settings: GlobalSettings = read_or_default(stored);
        assert!(settings.notifications);
        assert_eq!(settings.terminal_font_size, 15);
    }
}
