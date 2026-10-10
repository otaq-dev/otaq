//! Local merge's board side (GIT-03, EPC-03; d0015, d0038, d0059): where a ticket merges, the squash message the
//! confirmation proposes, and the bookkeeping once it merged. The git work is [`crate::git::merge`]; the click
//! is [`crate::review::Review::local_merge`].

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::tickets::{Base, EventKind, Ticket, record_event};
use super::{Board, BoardError, Refusal};
use crate::domain::{Actor, Status, TicketKey};
use crate::naming;
use crate::store::tickets::set_state_since;
use crate::store::{new_id, now_ms};

/// Where a ticket's Local merge goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeTarget {
    /// The branch merged: `crewboard/SHOP-18`, or an epic's `crewboard/SHOP-40`.
    pub branch: String,
    /// The branch merged into: `main`, or the epic's branch for an epic child (EPC-03).
    pub target: String,
    /// The target is the epic's local integration branch (no fetch, d0059).
    pub into_epic: bool,
}

/// What the Local merge confirmation shows before the click: the target and an editable squash message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeProposal {
    pub key: TicketKey,
    pub target: String,
    pub into_epic: bool,
    /// "SHOP-18: Checkout API" and the agent's summary.
    pub message: String,
}

impl Board {
    /// Where the ticket merges: a standalone ticket or an epic into its base branch, an epic child into its
    /// epic's branch.
    pub async fn merge_target(&self, ticket: &Ticket) -> Result<MergeTarget, BoardError> {
        let branch = ticket
            .work
            .as_ref()
            .and_then(|w| w.branch.clone())
            .ok_or_else(|| {
                BoardError::refused(
                    Refusal::NoBranch,
                    format!("{} has no branch to merge", ticket.key),
                )
            })?;
        Ok(match &ticket.base {
            Base::Branch(target) => MergeTarget {
                branch,
                target: target.clone(),
                into_epic: false,
            },
            Base::Epic(epic) => {
                let epic = self.ticket(&epic.to_string()).await?;
                MergeTarget {
                    branch,
                    target: epic
                        .work
                        .and_then(|w| w.branch)
                        .unwrap_or_else(|| naming::branch(&epic.key.to_string())),
                    into_epic: true,
                }
            }
        })
    }

    /// The proposed squash message: "SHOP-18: Checkout API", then the agent's summary (an epic lists its merged
    /// children).
    pub async fn merge_proposal(&self, key: &str) -> Result<MergeProposal, BoardError> {
        let ticket = self.ticket(key).await?;
        let target = self.merge_target(&ticket).await?;
        let mut message = format!("{}: {}", ticket.key, ticket.title);
        let body = if ticket.is_epic() {
            let children: Vec<String> = self
                .epic_children(key)
                .await?
                .into_iter()
                .filter(|c| c.status == Status::Merged)
                .map(|c| format!("- {} {}", c.key, c.title))
                .collect();
            (!children.is_empty()).then(|| children.join("\n"))
        } else {
            self.review_summary(&ticket.id).await?
        };
        if let Some(body) = body {
            message.push_str("\n\n");
            message.push_str(body.trim());
        }
        Ok(MergeProposal {
            key: ticket.key,
            target: target.target,
            into_epic: target.into_epic,
            message,
        })
    }

    /// After a Local merge: Done · Merged with the squash commit, and the review action recorded. `onto` is the
    /// target's commit the branch was rebased onto: the ticket's diff stays its own changes.
    pub async fn record_local_merge(
        &self,
        ticket: &Ticket,
        target: &str,
        commit: &str,
        onto: &str,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let now = now_ms();
        let to = Status::Merged;
        let round = self.current_round(&ticket.id).await?;
        let mut tx = self.store.pool().begin().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, outcome = 'merged', merge_commit_sha = ?,
                                base_sha = ?, sent_back_at = NULL, resume_skill = NULL, conflicts_with = NULL,
                                conflict_files = NULL, conflict_reason = NULL,
                                signal_text = NULL, signal_since = NULL, wakeup_at = NULL,
                                updated_by = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(to.column().as_str())
        .bind(to.as_str())
        .bind(commit)
        .bind(onto)
        .bind(actor.to_string())
        .bind(now)
        .bind(to.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO reviews (id, ticket_id, round, action, note, at) VALUES (?, ?, ?, 'approve_local', NULL, ?)",
        )
        .bind(new_id())
        .bind(&ticket.id)
        .bind(i64::from(round))
        .bind(now)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::Merged,
            actor,
            json!({ "from": ticket.status, "target": target, "commit": commit }),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(key = %ticket.key, target, commit, "merged locally");
        self.ticket_by_id(&ticket.id).await
    }
}
