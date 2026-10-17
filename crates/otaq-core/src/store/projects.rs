//! Thin access to the `projects` table. Validation and key rules live in `board::projects` (T-0021).

use serde_json::Value;
use sqlx::FromRow;

use super::{Store, StoreError, new_id, now_ms};
use crate::named_agents::RoleDefaults;

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct Project {
    pub id: String,
    pub key: String,
    pub name: String,
    pub repo_path: String,
    pub base_branch: String,
    pub next_ticket_number: i64,
    pub settings_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewProject {
    pub key: String,
    pub name: String,
    pub repo_path: String,
    pub base_branch: String,
}

impl Store {
    pub async fn insert_project(&self, new: &NewProject) -> Result<Project, StoreError> {
        let id = new_id();
        sqlx::query(
            "INSERT INTO projects (id, key, name, repo_path, base_branch, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.key)
        .bind(&new.name)
        .bind(&new.repo_path)
        .bind(&new.base_branch)
        .bind(now_ms())
        .execute(self.pool())
        .await?;
        self.project(&id)
            .await?
            .ok_or(StoreError::Database(sqlx::Error::RowNotFound))
    }

    pub async fn project(&self, id: &str) -> Result<Option<Project>, StoreError> {
        Ok(sqlx::query_as("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool())
            .await?)
    }

    pub async fn project_by_key(&self, key: &str) -> Result<Option<Project>, StoreError> {
        Ok(sqlx::query_as("SELECT * FROM projects WHERE key = ?")
            .bind(key)
            .fetch_optional(self.pool())
            .await?)
    }

    pub async fn project_by_repo_path(
        &self,
        repo_path: &str,
    ) -> Result<Option<Project>, StoreError> {
        Ok(sqlx::query_as("SELECT * FROM projects WHERE repo_path = ?")
            .bind(repo_path)
            .fetch_optional(self.pool())
            .await?)
    }

    /// Every project, oldest first.
    pub async fn projects(&self) -> Result<Vec<Project>, StoreError> {
        Ok(
            sqlx::query_as("SELECT * FROM projects ORDER BY created_at, key")
                .fetch_all(self.pool())
                .await?,
        )
    }

    /// Changes the editable fields. The key never changes (d0029).
    pub async fn update_project(
        &self,
        id: &str,
        name: &str,
        base_branch: &str,
    ) -> Result<(), StoreError> {
        sqlx::query("UPDATE projects SET name = ?, base_branch = ? WHERE id = ?")
            .bind(name)
            .bind(base_branch)
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Deletes the project row; its tickets, sessions and everything hanging off them go with it (ON DELETE
    /// CASCADE). Never touches the repository.
    pub async fn delete_project(&self, id: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}

/// The key of a project's role default overrides in its `settings_json`: `{"developer": id, …}` as the app-wide
/// ones, each absent = "Use app default" (AGT-03, d0088).
pub const PROJECT_AGENT_DEFAULTS: &str = "agents";

impl Project {
    /// The project's role default overrides; each absent = "Use app default".
    pub fn agent_defaults(&self) -> RoleDefaults {
        serde_json::from_str::<Value>(&self.settings_json)
            .ok()
            .and_then(|mut settings| settings.get_mut(PROJECT_AGENT_DEFAULTS).map(Value::take))
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default()
    }
}

impl Store {
    /// Stores the project's role default overrides, keeping its other settings.
    pub async fn set_project_agent_defaults(
        &self,
        id: &str,
        defaults: &RoleDefaults,
    ) -> Result<(), StoreError> {
        let mut tx = self.begin_write().await?;
        let json: Option<String> =
            sqlx::query_scalar("SELECT settings_json FROM projects WHERE id = ?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let mut settings = json
            .and_then(|json| serde_json::from_str::<Value>(&json).ok())
            .filter(Value::is_object)
            .unwrap_or_else(|| Value::Object(Default::default()));
        settings[PROJECT_AGENT_DEFAULTS] =
            serde_json::to_value(defaults).expect("defaults serialize");
        sqlx::query("UPDATE projects SET settings_json = ? WHERE id = ?")
            .bind(settings.to_string())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
