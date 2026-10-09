//! Typed settings, global and per project (SET-01; d0003, d0023; SDD §20). Everything lives in the app's
//! database and nothing is ever written into a repository (d0023). Invalid values are stored as given and
//! flagged ([`SettingProblem`]), never silently fixed.
//!
//! Global settings are rows of the `settings` table, one per top-level field below (JSON values; a missing row
//! means the default). Project settings are the project's `settings_json`. Empty per-project values fall back
//! to the global ones (agent instructions, workflow skills).
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

/// The Commit, Push and Open PR workflow skills (d0050).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkflowSkills {
    pub commit: String,
    pub push: String,
    pub open_pr: String,
}

impl Default for WorkflowSkills {
    /// Crewboard's own skills: `/crewboard:commit`, `/crewboard:push`, `/crewboard:open-pr`.
    fn default() -> Self {
        Self {
            commit: naming::skill("commit"),
            push: naming::skill("push"),
            open_pr: naming::skill("open-pr"),
        }
    }
}

stored_enum! {
    #[derive(Default)]
    pub enum Appearance ("appearance") {
        Light = "light",
        Dark = "dark",
        #[default]
        System = "system",
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
    /// Show a dollar estimate next to tokens (d0020). Off by default.
    pub dollar_estimate: bool,
    /// OS notifications for items that block an agent (d0016). On by default.
    pub notifications: bool,
    pub appearance: Appearance,
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
            dollar_estimate: false,
            notifications: true,
            appearance: Appearance::System,
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
        }
    }
}

/// The values a project's agents actually use, after falling back to the global settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveSettings {
    pub agent_instructions: String,
    pub workflow_skills: WorkflowSkills,
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

    /// The agent instructions and workflow skills a project's agents use.
    pub async fn effective_settings(&self, project: &str) -> Result<EffectiveSettings, BoardError> {
        let global = self.global_settings().await?;
        let own = self.project_settings(project).await?;
        let pick = |own: &str, global: &str| {
            if own.trim().is_empty() {
                global.to_owned()
            } else {
                own.to_owned()
            }
        };
        Ok(EffectiveSettings {
            agent_instructions: pick(&own.agent_instructions, &global.agent_instructions),
            workflow_skills: WorkflowSkills {
                commit: pick(&own.workflow_skills.commit, &global.workflow_skills.commit),
                push: pick(&own.workflow_skills.push, &global.workflow_skills.push),
                open_pr: pick(
                    &own.workflow_skills.open_pr,
                    &global.workflow_skills.open_pr,
                ),
            },
        })
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
    fn a_bad_stored_value_keeps_the_others() {
        let stored = serde_json::json!({"notifications": "loud", "terminal_font_size": 15});
        let settings: GlobalSettings = read_or_default(stored);
        assert!(settings.notifications);
        assert_eq!(settings.terminal_font_size, 15);
    }
}
