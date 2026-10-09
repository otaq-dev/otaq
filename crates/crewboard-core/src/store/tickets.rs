//! Thin access to the `tickets` table. Key allocation, fields and events come in M2 (T-0022).

use sqlx::FromRow;

use super::{Store, StoreError, new_id, now_ms};

/// A full `tickets` row.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct TicketRow {
    pub id: String,
    pub project_id: String,
    pub number: i64,
    pub epic_id: Option<String>,
    pub child_number: i64,
    pub next_child_number: i64,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub external_id: Option<String>,
    pub board_column: String,
    pub state: String,
    pub outcome: Option<String>,
    pub pinned_at: Option<i64>,
    pub parked: bool,
    pub base_branch: Option<String>,
    pub branch_name: Option<String>,
    pub branch_locked: bool,
    pub base_sha: Option<String>,
    pub review_head_sha: Option<String>,
    pub review_entered_at: Option<i64>,
    pub worktree_path: Option<String>,
    pub agent_port: Option<i64>,
    pub run_port: Option<i64>,
    pub pr_url: Option<String>,
    pub pr_number: Option<i64>,
    pub sent_back_round: i64,
    pub created_by: String,
    pub updated_by: Option<String>,
    pub started_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub start_problem_step: Option<String>,
    pub start_problem_message: Option<String>,
}

/// The fields needed to insert a ticket. The caller picks the number (allocation is T-0022's job).
#[derive(Debug, Clone)]
pub struct NewTicket {
    pub project_id: String,
    pub number: i64,
    pub epic_id: Option<String>,
    pub child_number: i64,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub board_column: String,
    pub state: String,
    pub created_by: String,
}

impl Store {
    pub async fn insert_ticket(&self, new: &NewTicket) -> Result<TicketRow, StoreError> {
        let id = new_id();
        let now = now_ms();
        sqlx::query(
            "INSERT INTO tickets (id, project_id, number, epic_id, child_number, kind, title, body,
                                  board_column, state, created_by, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.project_id)
        .bind(new.number)
        .bind(&new.epic_id)
        .bind(new.child_number)
        .bind(&new.kind)
        .bind(&new.title)
        .bind(&new.body)
        .bind(&new.board_column)
        .bind(&new.state)
        .bind(&new.created_by)
        .bind(now)
        .bind(now)
        .execute(self.pool())
        .await?;
        self.ticket(&id)
            .await?
            .ok_or(StoreError::Database(sqlx::Error::RowNotFound))
    }

    pub async fn ticket(&self, id: &str) -> Result<Option<TicketRow>, StoreError> {
        Ok(sqlx::query_as("SELECT * FROM tickets WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool())
            .await?)
    }

    /// A ticket by project and number: `(40, 0)` is SHOP-40, `(40, 1)` is SHOP-40.1.
    pub async fn ticket_by_number(
        &self,
        project_id: &str,
        number: i64,
        child_number: i64,
    ) -> Result<Option<TicketRow>, StoreError> {
        Ok(sqlx::query_as(
            "SELECT * FROM tickets WHERE project_id = ? AND number = ? AND child_number = ?",
        )
        .bind(project_id)
        .bind(number)
        .bind(child_number)
        .fetch_optional(self.pool())
        .await?)
    }
}
