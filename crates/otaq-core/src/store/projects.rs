//! Thin access to the `projects` table. Validation and key rules live in `board::projects` (T-0021).

use sqlx::FromRow;

use super::{Store, StoreError, new_id, now_ms};

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
