//! `crewboard review submit` (REV-07; d0050, d0004): the agent's work goes to **Ready for Review** with its
//! summary.
//!
//! It is **refused while the worktree has uncommitted files**, tracked or untracked: "2 files are not committed.
//! Commit them, or discard them if they're scratch files". The agent commits through the Commit skill; Crewboard
//! never commits by itself (GIT-08). Crewboard runs no tests or checks of its own (P5).
//!
//! On submit the service records the review head (`review_head_sha`, the worktree's `HEAD`) and when the ticket
//! entered review (`review_entered_at`), for "changed since you reviewed it" later (T-0097), and the summary in the
//! ticket's history (`review_submitted`), where `crewboard prime` finds it for the tickets that depend on it.

use std::path::Path;

use serde_json::json;

use super::tickets::{EventKind, Ticket, record_event};
use super::{Board, BoardError, Refusal};
use crate::domain::{Actor, Status};
use crate::signals::LIVE;
use crate::store::now_ms;
use crate::store::tickets::set_state_since;

/// Epics take no Send back and no comments: their changes come from their children (EPC-05, d0059).
pub const EPIC_NO_SEND_BACK: &str = "Add or reopen a child ticket to change it";

/// The statuses the user can submit from: the agent isn't mid-turn (CLS-05).
pub const USER_SUBMITS_FROM: &[Status] = &[
    Status::Idle,
    Status::AsksYou,
    Status::NeedsYou,
    Status::Interrupted,
];

/// The review summary of a user's submit: "Submitted by you", then the note.
pub fn user_summary(note: &str) -> String {
    let note = note.trim();
    if note.is_empty() {
        "Submitted by you".to_owned()
    } else {
        format!("Submitted by you\n\n{note}")
    }
}

/// The note of a user's submit from its summary (what follows "Submitted by you").
pub fn user_note(summary: &str) -> String {
    summary
        .strip_prefix("Submitted by you")
        .unwrap_or(summary)
        .trim()
        .to_owned()
}

/// "2 files are not committed. Commit them, or discard them if they're scratch files".
pub fn uncommitted_message(count: usize) -> String {
    if count == 1 {
        "1 file is not committed. Commit it, or discard it if it's a scratch file".to_owned()
    } else {
        format!(
            "{count} files are not committed. Commit them, or discard them if they're scratch files"
        )
    }
}

impl Board {
    /// Moves a working ticket to Ready for Review with `summary`, unless its worktree has uncommitted files.
    pub async fn submit_for_review(
        &self,
        key: &str,
        summary: &str,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let ticket = self.ticket(key).await?;
        if !LIVE.contains(&ticket.status) {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!(
                    "{} is {}; only a ticket whose agent is working can be submitted",
                    ticket.key,
                    ticket.status.label()
                ),
            ));
        }
        self.submit(ticket, summary, actor).await
    }

    /// **Submit for review** by the user (CLS-05, d0052): when the agent isn't mid-turn (Idle, Asks you, Needs
    /// you, Interrupted), with the same checks as the agent's submit. The summary reads "Submitted by you" and
    /// the optional note; the session isn't stopped, so Send back works as usual.
    pub async fn submit_by_user(
        &self,
        key: &str,
        note: &str,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let ticket = self.ticket(key).await?;
        if !USER_SUBMITS_FROM.contains(&ticket.status) {
            let why = match ticket.status {
                Status::Working | Status::WorkingInBackground => {
                    "its agent is in the middle of a turn".to_owned()
                }
                other => format!("it is {}", other.label()),
            };
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!("{} can't be submitted for review now: {why}", ticket.key),
            ));
        }
        self.submit(ticket, &user_summary(note), actor).await
    }

    /// A submit's checks (REV-07): the ticket has a worktree with nothing uncommitted. Returns its head.
    pub(crate) async fn submit_head(&self, ticket: &Ticket) -> Result<String, BoardError> {
        let worktree = ticket
            .work
            .as_ref()
            .and_then(|w| w.worktree.clone())
            .ok_or_else(|| {
                BoardError::refused(
                    Refusal::MoveRefused,
                    format!("{} has no worktree to submit", ticket.key),
                )
            })?;
        let git = self.git()?;
        let uncommitted = git.uncommitted_files(Path::new(&worktree)).await?;
        if !uncommitted.is_empty() {
            return Err(BoardError::refused(
                Refusal::Uncommitted,
                uncommitted_message(uncommitted.len()),
            ));
        }
        Ok(git
            .run(Path::new(&worktree), &["rev-parse", "HEAD"])
            .await?
            .trim()
            .to_owned())
    }

    async fn submit(
        &self,
        ticket: Ticket,
        summary: &str,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let head = self.submit_head(&ticket).await?;
        let now = now_ms();
        let status = Status::ReadyForReview;
        let mut tx = self.store.pool().begin().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, review_head_sha = ?, review_entered_at = ?,
                                signal_text = NULL, signal_since = NULL, wakeup_at = NULL, sent_back_at = NULL,
                                resume_skill = NULL, conflicts_with = NULL, conflict_files = NULL,
                                conflict_reason = NULL, pr_rework = FALSE, updated_by = ?,
                                updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(status.column().as_str())
        .bind(status.as_str())
        .bind(&head)
        .bind(now)
        .bind(actor.to_string())
        .bind(now)
        .bind(status.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::ReviewSubmitted,
            actor,
            json!({ "summary": summary.trim(), "head": head }),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(key = %ticket.key, %actor, "submitted for review");
        self.ticket_by_id(&ticket.id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_refusal_names_the_count() {
        assert_eq!(
            uncommitted_message(2),
            "2 files are not committed. Commit them, or discard them if they're scratch files"
        );
        assert_eq!(
            uncommitted_message(1),
            "1 file is not committed. Commit it, or discard it if it's a scratch file"
        );
    }
}

/// One commit made after the review began.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewCommit {
    pub sha: String,
    pub subject: String,
}

/// Has the ticket changed since it entered Ready for Review (REV-06, d0043)?
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ApproveCheck {
    Unchanged,
    /// "SHOP-18 changed since you reviewed it · 1 new commit": the new commits, oldest first, and the files
    /// they changed (the diff from the reviewed head to the current head).
    Changed {
        new_commits: Vec<NewCommit>,
        files: Vec<crate::git::diff::ChangedFile>,
        /// The reviewed head, for `diff.file` of only the new commits.
        reviewed: String,
        head: String,
    },
}

/// "SHOP-18 changed since you reviewed it · 1 new commit".
pub fn changed_message(key: &str, commits: usize) -> String {
    let n = if commits == 1 {
        "1 new commit".to_owned()
    } else {
        format!("{commits} new commits")
    };
    format!("{key} changed since you reviewed it \u{b7} {n}")
}

impl Board {
    /// The check before any approve: new commits on the branch since its review head.
    pub async fn approve_check(&self, key: &str) -> Result<ApproveCheck, BoardError> {
        let ticket = self.ticket(key).await?;
        let Some(row) = self.store.ticket(&ticket.id).await? else {
            return Ok(ApproveCheck::Unchanged);
        };
        let (Some(reviewed), Some(branch)) = (row.review_head_sha, row.branch_name) else {
            return Ok(ApproveCheck::Unchanged);
        };
        let project = self.project(ticket.key.project.as_str()).await?;
        let repo = Path::new(&project.repo_path);
        let git = self.git()?;
        let Some(head) = git.resolve(repo, &format!("refs/heads/{branch}")).await? else {
            return Ok(ApproveCheck::Unchanged);
        };
        if head == reviewed {
            return Ok(ApproveCheck::Unchanged);
        }
        let range = format!("{reviewed}..{head}");
        let log = git
            .run(repo, &["log", "--reverse", "--format=%H%x09%s", &range])
            .await?;
        let new_commits: Vec<NewCommit> = log
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(sha, subject)| NewCommit {
                sha: sha.to_owned(),
                subject: subject.to_owned(),
            })
            .collect();
        let files = git
            .changed_files(repo, &crate::git::diff::DiffSpec::between(&reviewed, &head))
            .await?;
        if new_commits.is_empty() && files.is_empty() {
            return Ok(ApproveCheck::Unchanged);
        }
        Ok(ApproveCheck::Changed {
            new_commits,
            files,
            reviewed,
            head,
        })
    }

    /// After Crewboard itself rebased the ticket's branch from `old_head` to `new_head` (GIT-05): the rebase
    /// isn't a change to review. The reviewed head moves to its rebased counterpart: the new head when nothing
    /// was added after the review, else the commit as many commits below the new head as were added.
    pub async fn record_crewboard_rebase(
        &self,
        ticket_id: &str,
        old_head: &str,
        new_head: &str,
    ) -> Result<(), BoardError> {
        if old_head == new_head {
            return Ok(());
        }
        let Some(row) = self.store.ticket(ticket_id).await? else {
            return Ok(());
        };
        let Some(reviewed) = row.review_head_sha else {
            return Ok(());
        };
        let project_key: String = sqlx::query_scalar(
            "SELECT p.key FROM projects p JOIN tickets t ON t.project_id = p.id WHERE t.id = ?",
        )
        .bind(ticket_id)
        .fetch_one(self.store.pool())
        .await?;
        let project = self.project(&project_key).await?;
        let repo = Path::new(&project.repo_path);
        let git = self.git()?;
        let added: u32 = if reviewed == old_head {
            0
        } else {
            let range = format!("{reviewed}..{old_head}");
            match git.run(repo, &["rev-list", "--count", &range]).await {
                Ok(n) => n.trim().parse().unwrap_or(0),
                // The reviewed head isn't on the branch any more: leave it, the check reports the change.
                Err(_) => return Ok(()),
            }
        };
        let rebased = git
            .run(repo, &["rev-parse", &format!("{new_head}~{added}")])
            .await?
            .trim()
            .to_owned();
        sqlx::query("UPDATE tickets SET review_head_sha = ? WHERE id = ?")
            .bind(rebased)
            .bind(ticket_id)
            .execute(self.store.pool())
            .await?;
        Ok(())
    }
}
