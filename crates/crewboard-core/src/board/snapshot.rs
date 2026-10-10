//! What the app's board and project switcher read: a project's columns with sorted cards, and per-project
//! counts.

use serde::{Deserialize, Serialize};

use super::order::sort_column;
use super::tickets::Ticket;
use super::{Board, BoardError};
use crate::domain::{Column, TicketKey};
use crate::store::projects::Project;

/// A project as the switcher shows it (UI-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub key: String,
    pub name: String,
    pub repo_path: String,
    pub base_branch: String,
    /// Agents working right now (live agent sessions).
    pub agents_working: u32,
    /// Tickets that need the user (orange family, Interrupted, Start interrupted, Resume failed, Parent closed).
    pub needs_you: u32,
}

/// One column, cards in board order (SDD §7.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSnapshot {
    pub column: Column,
    /// The column's cards; for Dev, the Agent Working section with every card that needs the user.
    pub cards: Vec<Ticket>,
    /// Dev only: the Queued section.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued: Vec<Ticket>,
}

/// A project's board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardSnapshot {
    pub project: ProjectInfo,
    /// The five columns, left to right.
    pub columns: Vec<ColumnSnapshot>,
}

impl BoardSnapshot {
    /// Every ticket on the board, in no particular order.
    pub fn tickets(&self) -> impl Iterator<Item = &Ticket> {
        self.columns
            .iter()
            .flat_map(|c| c.cards.iter().chain(c.queued.iter()))
    }

    pub fn ticket(&self, key: &TicketKey) -> Option<&Ticket> {
        self.tickets().find(|t| &t.key == key)
    }
}

/// Builds the columns from a project's tickets.
pub fn columns(tickets: &[Ticket]) -> Vec<ColumnSnapshot> {
    Column::ALL
        .iter()
        .map(|column| {
            let order = sort_column(*column, tickets);
            let pick = |keys: &[TicketKey]| {
                keys.iter()
                    .filter_map(|k| tickets.iter().find(|t| &t.key == k).cloned())
                    .collect()
            };
            ColumnSnapshot {
                column: *column,
                cards: pick(&order.cards),
                queued: pick(&order.queued),
            }
        })
        .collect()
}

impl Board {
    pub async fn board_snapshot(&self, project: &str) -> Result<BoardSnapshot, BoardError> {
        let tickets = self.tickets(project).await?;
        let project = self.project(project).await?;
        Ok(BoardSnapshot {
            project: self.project_info(&project, &tickets).await?,
            columns: columns(&tickets),
        })
    }

    /// Every project with its counts, oldest first.
    pub async fn project_infos(&self) -> Result<Vec<ProjectInfo>, BoardError> {
        let mut infos = Vec::new();
        for project in self.projects().await? {
            let tickets = self.tickets(&project.key).await?;
            infos.push(self.project_info(&project, &tickets).await?);
        }
        Ok(infos)
    }

    pub async fn project_info_of(&self, key: &str) -> Result<ProjectInfo, BoardError> {
        let project = self.project(key).await?;
        let tickets = self.tickets(key).await?;
        self.project_info(&project, &tickets).await
    }

    async fn project_info(
        &self,
        project: &Project,
        tickets: &[Ticket],
    ) -> Result<ProjectInfo, BoardError> {
        let working: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sessions s JOIN tickets t ON t.id = s.ticket_id
             WHERE t.project_id = ? AND s.kind = 'agent' AND s.ended_at IS NULL",
        )
        .bind(&project.id)
        .fetch_one(self.store.pool())
        .await?;
        let needs_you = tickets.iter().filter(|t| t.needs_user()).count();
        Ok(ProjectInfo {
            key: project.key.clone(),
            name: project.name.clone(),
            repo_path: project.repo_path.clone(),
            base_branch: project.base_branch.clone(),
            agents_working: u32::try_from(working).unwrap_or(0),
            needs_you: u32::try_from(needs_you).unwrap_or(u32::MAX),
        })
    }
}
