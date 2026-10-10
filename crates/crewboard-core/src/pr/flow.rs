//! The Open PR / Push to PR flow of a delivery unit (REV-10, REV-14; d0050, d0041, d0059), from the user's click
//! until the skill reports the PR or the user cancels: `pr_flows` (migration 0018).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::proposal::{self, Proposal};
use crate::board::tickets::{EventKind, Ticket, record_event};
use crate::board::{Board, BoardError, Refusal};
use crate::domain::{Actor, TicketKey, stored_enum};
use crate::git::reword::CommitInfo;
use crate::store::now_ms;

stored_enum! {
    pub enum FlowKind ("PR flow kind") {
        /// Open PR: the first push, the branch gets its real name.
        Open = "open",
        /// Push to PR #N: new commits to the PR's locked branch.
        Push = "push",
    }
}

stored_enum! {
    pub enum FlowState ("PR flow state") {
        /// The skill runs; no proposal yet ("Open PR skill is preparing…").
        Preparing = "preparing",
        /// The dialog shows the proposal.
        Proposed = "proposed",
        /// The user confirmed: the edits are applied and the skill was told to push.
        Pushing = "pushing",
    }
}

/// A ticket's PR flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrFlow {
    pub kind: FlowKind,
    pub state: FlowState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal: Option<Proposal>,
    /// What the user confirmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed: Option<Proposal>,
    /// What the skill reported instead of a PR (a failed push, a missing login).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported: Option<String>,
    pub started_at: i64,
}

type FlowRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
);

impl Board {
    /// The ticket's PR flow, if one is under way.
    pub async fn pr_flow(&self, ticket_id: &str) -> Result<Option<PrFlow>, BoardError> {
        let row: Option<FlowRow> = sqlx::query_as(
            "SELECT kind, state, proposal_json, confirmed_json, reported, started_at FROM pr_flows WHERE ticket_id = ?",
        )
        .bind(ticket_id)
        .fetch_optional(self.store().pool())
        .await?;
        let Some((kind, state, proposal, confirmed, reported, started_at)) = row else {
            return Ok(None);
        };
        let parse = |json: Option<String>| json.and_then(|j| serde_json::from_str(&j).ok());
        Ok(Some(PrFlow {
            kind: kind.parse().map_err(unknown)?,
            state: state.parse().map_err(unknown)?,
            proposal: parse(proposal),
            confirmed: parse(confirmed),
            reported,
            started_at,
        }))
    }
}

fn unknown(e: crate::domain::UnknownValue) -> BoardError {
    BoardError::NotFound(e.to_string())
}

/// One commit of the PR as the dialog shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowCommit {
    pub sha: String,
    /// The current message (the proposal's, while one is shown, comes separately).
    pub message: String,
    /// Already on the PR's remote branch: read-only (Push to PR, REV-14).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pushed: bool,
}

/// What the Open PR / Push to PR dialog shows (`ticket.pr_flow`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrFlowView {
    pub key: TicketKey,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<PrFlow>,
    /// The branch the PR targets: the ticket's or epic's base branch.
    pub target: String,
    /// The ticket's branch now: `crewboard/SHOP-18`, or the pushed name.
    pub branch: String,
    /// The branch was pushed: its name can't change (GIT-07).
    pub branch_locked: bool,
    /// Every commit of the PR, oldest first.
    pub commits: Vec<FlowCommit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
}

/// The answer to a live branch-name check (REV-10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum BranchCheck {
    Ok,
    /// Not a valid ref name, or taken: the reason, for the dialog.
    Problem {
        message: String,
    },
}

/// The ticket's PR commits, and how many of the first ones are already on the remote branch.
pub(crate) struct PrCommits {
    pub repo: PathBuf,
    pub branch: String,
    pub head: String,
    pub commits: Vec<CommitInfo>,
    pub pushed: usize,
}

impl PrCommits {
    /// The commits the proposal names and the user may reword: all of them for Open PR, the unpushed ones for
    /// Push to PR.
    pub fn editable(&self) -> &[CommitInfo] {
        &self.commits[self.pushed..]
    }
}

/// The message of a delivery unit with no commits to propose.
fn no_commits(key: &TicketKey) -> BoardError {
    BoardError::refused(
        Refusal::BadProposal,
        format!("{key} has no commits to push"),
    )
}

impl Board {
    /// The PR's commits: `base_sha..branch`; for a ticket with a PR, the ones already on `origin/<branch>` are
    /// marked pushed (after a fetch of that branch).
    pub(crate) async fn pr_commits(&self, ticket: &Ticket) -> Result<PrCommits, BoardError> {
        let row = self
            .store()
            .ticket(&ticket.id)
            .await?
            .ok_or_else(|| BoardError::NotFound(ticket.key.to_string()))?;
        let (Some(branch), Some(base_sha)) = (row.branch_name, row.base_sha) else {
            return Err(BoardError::refused(
                Refusal::NoBranch,
                format!("{} has no branch to push", ticket.key),
            ));
        };
        let repo = PathBuf::from(self.project(ticket.key.project.as_str()).await?.repo_path);
        let git = self.git()?;
        let head = git
            .resolve(&repo, &format!("refs/heads/{branch}"))
            .await?
            .ok_or_else(|| {
                BoardError::refused(
                    Refusal::NoBranch,
                    format!("{}'s branch {branch} doesn't exist any more", ticket.key),
                )
            })?;
        let commits = git.commits_between(&repo, &base_sha, &head).await?;
        let mut pushed = 0;
        if row.pr_number.is_some() {
            let tracking = format!("refs/remotes/origin/{branch}");
            let refspec = format!("+refs/heads/{branch}:{tracking}");
            if let Err(e) = git
                .run(&repo, &["fetch", "--quiet", "origin", &refspec])
                .await
            {
                tracing::warn!(key = %ticket.key, error = %e, "can't fetch the PR's branch");
            }
            if let Some(remote) = git.resolve(&repo, &tracking).await? {
                let new = git.commits_between(&repo, &remote, &head).await?;
                pushed = commits.len().saturating_sub(new.len());
            }
        }
        Ok(PrCommits {
            repo,
            branch,
            head,
            commits,
            pushed,
        })
    }

    /// What the Open PR / Push to PR dialog shows.
    pub async fn pr_flow_view(&self, key: &str) -> Result<PrFlowView, BoardError> {
        let ticket = self.ticket(key).await?;
        let commits = self.pr_commits(&ticket).await?;
        let row = self
            .store()
            .ticket(&ticket.id)
            .await?
            .ok_or_else(|| BoardError::NotFound(key.to_owned()))?;
        let target = self.merge_target(&ticket).await?.target;
        Ok(PrFlowView {
            flow: self.pr_flow(&ticket.id).await?,
            target,
            branch: commits.branch.clone(),
            branch_locked: row.branch_pushed,
            commits: commits
                .commits
                .iter()
                .enumerate()
                .map(|(n, c)| FlowCommit {
                    sha: c.sha.clone(),
                    message: c.message.clone(),
                    pushed: n < commits.pushed,
                })
                .collect(),
            pr_number: row.pr_number.and_then(|n| u32::try_from(n).ok()),
            pr_url: row.pr_url,
            external_id: ticket.external_id.clone(),
            key: ticket.key,
        })
    }

    /// Whether `name` can be the PR's branch: a valid ref name, not taken locally (by another branch) and not on
    /// the remote (`git ls-remote`). Used live by the dialog and again on confirm.
    pub async fn check_branch_name(
        &self,
        key: &str,
        name: &str,
    ) -> Result<BranchCheck, BoardError> {
        let ticket = self.ticket(key).await?;
        let name = name.trim();
        if let Some(message) = proposal::branch_name_problem(name) {
            return Ok(BranchCheck::Problem { message });
        }
        let current = ticket.work.as_ref().and_then(|w| w.branch.clone());
        if current.as_deref() == Some(name) {
            return Ok(BranchCheck::Ok);
        }
        let repo = PathBuf::from(self.project(ticket.key.project.as_str()).await?.repo_path);
        let git = self.git()?;
        if git
            .resolve(&repo, &format!("refs/heads/{name}"))
            .await?
            .is_some()
        {
            return Ok(BranchCheck::Problem {
                message: format!("{name} is taken: a local branch has that name"),
            });
        }
        let heads = format!("refs/heads/{name}");
        match git
            .run(&repo, &["ls-remote", "--heads", "origin", &heads])
            .await
        {
            Ok(out) if !out.trim().is_empty() => Ok(BranchCheck::Problem {
                message: format!("{name} is taken on origin"),
            }),
            Ok(_) => Ok(BranchCheck::Ok),
            Err(e) => Ok(BranchCheck::Problem {
                message: format!("Can't check origin for {name}: {e}"),
            }),
        }
    }

    /// Starts the flow on the user's click: "preparing" until the skill proposes. A flow already under way
    /// starts over.
    pub(crate) async fn start_pr_flow(
        &self,
        ticket: &Ticket,
        kind: FlowKind,
    ) -> Result<(), BoardError> {
        let now = now_ms();
        sqlx::query(
            "INSERT INTO pr_flows (ticket_id, kind, state, started_at, updated_at) VALUES (?, ?, 'preparing', ?, ?)
             ON CONFLICT (ticket_id) DO UPDATE SET kind = excluded.kind, state = 'preparing', proposal_json = NULL,
                 confirmed_json = NULL, reported = NULL, started_at = excluded.started_at,
                 updated_at = excluded.updated_at",
        )
        .bind(&ticket.id)
        .bind(kind.as_str())
        .bind(now)
        .bind(now)
        .execute(self.store().pool())
        .await?;
        Ok(())
    }

    /// Ends the flow (Cancel, or the PR was reported): the proposal is discarded.
    pub(crate) async fn end_pr_flow(&self, ticket_id: &str) -> Result<(), BoardError> {
        sqlx::query("DELETE FROM pr_flows WHERE ticket_id = ?")
            .bind(ticket_id)
            .execute(self.store().pool())
            .await?;
        Ok(())
    }

    /// Checks a proposal against the ticket's commits and its flow: the same commits, in order (short IDs
    /// resolved to full ones); Push to PR keeps the locked branch; Open PR needs a title. Returns it with full IDs.
    pub(crate) async fn check_proposal(
        &self,
        ticket: &Ticket,
        kind: FlowKind,
        proposal: &Proposal,
    ) -> Result<Proposal, BoardError> {
        let bad = |message: String| BoardError::refused(Refusal::BadProposal, message);
        let commits = self.pr_commits(ticket).await?;
        let editable = commits.editable();
        if editable.is_empty() {
            return Err(no_commits(&ticket.key));
        }
        if kind == FlowKind::Push && proposal.branch != commits.branch {
            return Err(BoardError::refused(
                Refusal::BranchLocked,
                super::commands::branch_locked_message(&ticket.key.to_string(), &commits.branch),
            ));
        }
        if kind == FlowKind::Open && proposal.title.trim().is_empty() {
            return Err(bad("the proposal has no PR title".to_owned()));
        }
        let listed: Vec<String> = editable.iter().map(|c| short(&c.sha).to_owned()).collect();
        if proposal.commits.len() != editable.len() {
            return Err(bad(format!(
                "the proposal has {} commit messages, but {} has {} commits to push: {}",
                proposal.commits.len(),
                ticket.key,
                editable.len(),
                listed.join(", ")
            )));
        }
        let mut checked = proposal.clone();
        for (n, (proposed, actual)) in checked.commits.iter_mut().zip(editable).enumerate() {
            if !actual.sha.starts_with(&proposed.sha) {
                return Err(bad(format!(
                    "commit {} of the proposal is {}, but the commits to push are, in order: {}",
                    n + 1,
                    proposed.sha,
                    listed.join(", ")
                )));
            }
            proposed.sha = actual.sha.clone();
        }
        Ok(checked)
    }

    /// The user's confirmed edits, applied by Crewboard itself (REV-10): the commits are reworded (trees
    /// unchanged, checked) and the branch renamed; then the name is locked and the flow is "pushing". Any failure
    /// leaves the branch as it was.
    pub(crate) async fn apply_confirmed(
        &self,
        ticket: &Ticket,
        kind: FlowKind,
        confirmed: &Proposal,
        actor: &Actor,
    ) -> Result<(), BoardError> {
        let commits = self.pr_commits(ticket).await?;
        let git = self.git()?;
        let repo = commits.repo.as_path();
        let parent = match commits.pushed {
            0 => self
                .store()
                .ticket(&ticket.id)
                .await?
                .and_then(|r| r.base_sha)
                .ok_or_else(|| no_commits(&ticket.key))?,
            n => commits.commits[n - 1].sha.clone(),
        };
        let pairs: Vec<(CommitInfo, String)> = commits
            .editable()
            .iter()
            .cloned()
            .zip(confirmed.commits.iter().map(|c| c.message.clone()))
            .collect();
        let tip = git.reword(repo, &parent, &pairs).await?;
        // No file changes: every new commit has its old commit's tree.
        let new = git.commits_between(repo, &parent, &tip).await?;
        for (old, new) in pairs.iter().map(|(c, _)| c).zip(&new) {
            if git.tree_of(repo, &old.sha).await? != git.tree_of(repo, &new.sha).await? {
                return Err(BoardError::refused(
                    Refusal::BadProposal,
                    "rewording changed a commit's files; nothing was changed",
                ));
            }
        }
        let branch_ref = format!("refs/heads/{}", commits.branch);
        if tip != commits.head {
            git.run(repo, &["update-ref", &branch_ref, &tip, &commits.head])
                .await?;
        }
        let renamed = kind == FlowKind::Open && confirmed.branch != commits.branch;
        if renamed
            && let Err(e) = git
                .run(repo, &["branch", "-m", &commits.branch, &confirmed.branch])
                .await
        {
            // Put the old commits back.
            if tip != commits.head {
                let _ = git
                    .run(repo, &["update-ref", &branch_ref, &commits.head, &tip])
                    .await;
            }
            return Err(e.into());
        }
        // The reviewed head follows the reworded commits (no new content, GIT-05).
        self.record_crewboard_rebase(&ticket.id, &commits.head, &tip)
            .await?;
        let mut tx = self.store().pool().begin().await?;
        sqlx::query(
            "UPDATE tickets SET branch_name = ?, branch_pushed = TRUE, updated_at = ? WHERE id = ?",
        )
        .bind(&confirmed.branch)
        .bind(now_ms())
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE pr_flows SET state = 'pushing', confirmed_json = ?, updated_at = ? WHERE ticket_id = ?",
        )
        .bind(serde_json::to_string(confirmed).expect("proposal serializes"))
        .bind(now_ms())
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        if renamed {
            record_event(
                &mut tx,
                &ticket.id,
                EventKind::BranchRenamed,
                actor,
                json!({ "from": commits.branch, "to": confirmed.branch }),
            )
            .await?;
        }
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::PrConfirmed,
            actor,
            json!({ "kind": kind, "branch": confirmed.branch, "title": confirmed.title }),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(key = %ticket.key, branch = %confirmed.branch, "PR edits applied");
        Ok(())
    }
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}
