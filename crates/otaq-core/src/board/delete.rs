//! Deleting tickets that never went to Dev (TKT-08; d0060, d0029, d0018, d0044). Nothing exists on disk for
//! them (no branch, worktree or conversation), so a delete is pure bookkeeping, but it is still a deletion: the
//! user confirms it, from what [`Board::delete_preview`] names (the links that go with it, and for an epic the
//! never-started children that go too). Anything that has been to Dev can only be Closed (d0033). The number
//! is never handed out again (d0029); the key and its aliases stop resolving.

use serde::{Deserialize, Serialize};

use super::tickets::Ticket;
use super::{Board, BoardError, Refusal};
use crate::domain::TicketKey;

/// What the delete confirmation names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletePreview {
    pub key: TicketKey,
    pub title: String,
    pub is_epic: bool,
    /// An epic's children, deleted with it.
    pub children: Vec<(TicketKey, String)>,
    /// Links that go with it: (dependent, dependency) where one side stays.
    pub links: Vec<(TicketKey, TicketKey)>,
}

impl DeletePreview {
    /// "Delete SHOP-40 and its 2 tickets (SHOP-40.1, SHOP-40.2)? None of them started." /
    /// "Delete SHOP-21 Order confirmation email?"
    pub fn question(&self) -> String {
        if self.is_epic && !self.children.is_empty() {
            let keys: Vec<String> = self.children.iter().map(|(k, _)| k.to_string()).collect();
            let n = keys.len();
            format!(
                "Delete {} and its {n} ticket{} ({})? None of them started.",
                self.key,
                if n == 1 { "" } else { "s" },
                keys.join(", ")
            )
        } else {
            format!("Delete {} {}?", self.key, self.title)
        }
    }

    /// One line per link that will be removed: "SHOP-21 is a dependency of SHOP-22; that link will be removed".
    pub fn link_lines(&self) -> Vec<String> {
        self.links
            .iter()
            .map(|(dependent, dependency)| {
                format!("{dependency} is a dependency of {dependent}; that link will be removed")
            })
            .collect()
    }
}

impl Board {
    /// What deleting `key` would remove, or why it can't be deleted.
    pub async fn delete_preview(&self, key: &str) -> Result<DeletePreview, BoardError> {
        let (ticket, doomed, tickets) = self.deletion(key).await?;
        let doomed_keys: Vec<&TicketKey> = doomed.iter().map(|t| &t.key).collect();
        let mut links = Vec::new();
        for t in &tickets {
            for parent in &t.depends_on {
                let child_gone = doomed_keys.contains(&&t.key);
                let parent_gone = doomed_keys.contains(&parent);
                if child_gone != parent_gone {
                    links.push((t.key.clone(), parent.clone()));
                }
            }
        }
        links.sort();
        Ok(DeletePreview {
            key: ticket.key.clone(),
            title: ticket.title.clone(),
            is_epic: ticket.is_epic(),
            children: doomed
                .iter()
                .filter(|t| t.id != ticket.id)
                .map(|t| (t.key.clone(), t.title.clone()))
                .collect(),
            links,
        })
    }

    /// Deletes a never-started ticket (with its links, aliases, review and message rows), or
    /// an epic together with its never-started children, in one transaction.
    pub async fn delete_ticket(&self, key: &str) -> Result<DeletePreview, BoardError> {
        let preview = self.delete_preview(key).await?;
        let (ticket, doomed, _) = self.deletion(key).await?;
        let mut tx = self.store.begin_write().await?;
        // Children first, then the epic (they reference it).
        for t in doomed.iter().filter(|t| t.id != ticket.id) {
            sqlx::query("DELETE FROM tickets WHERE id = ?")
                .bind(&t.id)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("DELETE FROM tickets WHERE id = ?")
            .bind(&ticket.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        tracing::info!(key = %ticket.key, with = doomed.len() - 1, "ticket deleted");
        Ok(preview)
    }

    /// The ticket, everything that goes with it, and the project's tickets; or the refusal.
    async fn deletion(&self, key: &str) -> Result<(Ticket, Vec<Ticket>, Vec<Ticket>), BoardError> {
        let ticket = self.ticket(key).await?;
        let tickets = self.tickets(ticket.key.project.as_str()).await?;
        if self.has_been_to_dev(&ticket).await? {
            return Err(BoardError::refused(
                Refusal::Started,
                format!("{} has been to Dev. Close it instead.", ticket.key),
            ));
        }
        let mut doomed = vec![ticket.clone()];
        if ticket.is_epic() {
            for child in tickets
                .iter()
                .filter(|t| t.epic.as_ref() == Some(&ticket.key))
            {
                if self.has_been_to_dev(child).await? {
                    return Err(BoardError::refused(
                        Refusal::Started,
                        format!("{} has started tickets. Close it instead.", ticket.key),
                    ));
                }
                doomed.push(child.clone());
            }
        }
        Ok((ticket, doomed, tickets))
    }
}
