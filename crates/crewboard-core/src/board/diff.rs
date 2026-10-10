//! A ticket's diff for the Diff tab (REV-01, REV-02, EPC-05; d0017, d0049, d0059).
//!
//! A ticket's diff is always **its branch against its `base_sha`** (`base_sha...branch`), where it left its
//! base, never against the base's current head: a ticket never shows someone else's changes. The reply names
//! the base branch and the short `base_sha` ("diff vs `main` at `a1b2c3d`").
//!
//! An **epic's** diff is the epic branch against its base, and is also **grouped by child**: each child that a
//! Local merge put into the epic branch has one squash commit there (`merge_commit_sha`); a child with nothing
//! merged isn't listed.
//!
//! Everything is read from the project's repository by branch name, so a Done ticket whose worktree was
//! removed still shows its diff.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::tickets::{Base, Ticket};
use super::{Board, BoardError, Refusal};
use crate::domain::{Status, TicketKey};
use crate::git::diff::{ChangedFile, DiffSpec, FileDiff};

/// How many characters of `base_sha` the base indicator shows.
pub const SHORT_SHA: usize = 7;

/// The file list of a ticket's diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSummary {
    pub key: TicketKey,
    /// The base branch: `main`, or the epic's branch `crewboard/SHOP-40` for an epic child.
    pub base_branch: String,
    pub base_sha: String,
    /// The first [`SHORT_SHA`] characters of `base_sha`.
    pub base_short: String,
    /// The branch shown: `crewboard/SHOP-18`.
    pub branch: String,
    pub files: Vec<ChangedFile>,
    /// "+412 −38": the sum over the files.
    pub added: u32,
    pub removed: u32,
    /// The review round (1 + the Send backs so far): the Diff tab keeps its viewed ticks per round.
    pub round: u32,
    /// What the agent said at `review submit` (or "Submitted by you" and the note), shown above the diff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_summary: Option<String>,
    /// Epics: the merged children, by key, with what each one's squash commit changed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ChildDiff>,
}

/// What one merged child brought into its epic's branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildDiff {
    pub key: TicketKey,
    pub title: String,
    /// The child's squash commit in the epic branch.
    pub commit: String,
    pub files: Vec<ChangedFile>,
    pub added: u32,
    pub removed: u32,
}

/// Where a ticket's diff comes from.
struct DiffSource {
    repo: PathBuf,
    base_branch: String,
    base_sha: String,
    branch: String,
}

impl Board {
    /// The changed files of a ticket's or epic's branch since its `base_sha`.
    pub async fn diff_summary(&self, key: &str) -> Result<DiffSummary, BoardError> {
        let ticket = self.ticket(key).await?;
        let source = self.diff_source(&ticket).await?;
        let git = self.git()?;
        let files = git
            .changed_files(
                &source.repo,
                &DiffSpec::since(&source.base_sha, &source.branch),
            )
            .await?;
        let (added, removed) = totals(&files);
        let children = if ticket.is_epic() {
            self.child_diffs(&ticket, &source).await?
        } else {
            Vec::new()
        };
        Ok(DiffSummary {
            round: self.current_round(&ticket.id).await?,
            review_summary: self.review_summary(&ticket.id).await?,
            key: ticket.key,
            base_short: short_sha(&source.base_sha),
            base_branch: source.base_branch,
            base_sha: source.base_sha,
            branch: source.branch,
            files,
            added,
            removed,
            children,
        })
    }

    /// One file's hunks. `child` (epics only) limits the diff to that child's squash commit.
    pub async fn diff_file(
        &self,
        key: &str,
        path: &str,
        child: Option<&str>,
    ) -> Result<FileDiff, BoardError> {
        let ticket = self.ticket(key).await?;
        let source = self.diff_source(&ticket).await?;
        let spec = match child {
            None => DiffSpec::since(&source.base_sha, &source.branch),
            Some(child) => {
                let child = self.ticket(child).await?;
                if child.epic.as_ref() != Some(&ticket.key) {
                    return Err(BoardError::refused(
                        Refusal::NotAnEpic,
                        format!("{} isn't a child of {}", child.key, ticket.key),
                    ));
                }
                let commit = self.merge_commit(&child.id).await?.ok_or_else(|| {
                    BoardError::NotFound(format!("{} has nothing merged", child.key))
                })?;
                DiffSpec::commit(&commit)
            }
        };
        self.git()?
            .file_diff(&source.repo, &spec, path)
            .await?
            .ok_or_else(|| BoardError::NotFound(format!("{path} isn't changed in {}", ticket.key)))
    }

    async fn diff_source(&self, ticket: &Ticket) -> Result<DiffSource, BoardError> {
        let project = self.project(ticket.key.project.as_str()).await?;
        let work = ticket.work.clone().unwrap_or_default();
        let (Some(branch), Some(base_sha)) = (work.branch, work.base_sha) else {
            return Err(BoardError::refused(
                Refusal::NoBranch,
                format!("{} has no branch yet: it hasn't started", ticket.key),
            ));
        };
        let base_branch = match &ticket.base {
            Base::Branch(branch) => branch.clone(),
            Base::Epic(epic) => {
                let epic = self.ticket(&epic.to_string()).await?;
                epic.work
                    .and_then(|w| w.branch)
                    .unwrap_or_else(|| crate::naming::branch(&epic.key.to_string()))
            }
        };
        Ok(DiffSource {
            repo: PathBuf::from(project.repo_path),
            base_branch,
            base_sha,
            branch,
        })
    }

    /// The epic's merged children, each with its squash commit's files, by key.
    async fn child_diffs(
        &self,
        epic: &Ticket,
        source: &DiffSource,
    ) -> Result<Vec<ChildDiff>, BoardError> {
        let git = self.git()?;
        let mut children = Vec::new();
        for child in self.epic_children(&epic.key.to_string()).await? {
            if child.status != Status::Merged {
                continue;
            }
            let Some(commit) = self.merge_commit(&child.id).await? else {
                continue;
            };
            let files = git
                .changed_files(&source.repo, &DiffSpec::commit(&commit))
                .await?;
            let (added, removed) = totals(&files);
            children.push(ChildDiff {
                key: child.key,
                title: child.title,
                commit,
                files,
                added,
                removed,
            });
        }
        Ok(children)
    }

    /// The squash commit a Local merge made for the ticket.
    pub(crate) async fn merge_commit(&self, ticket_id: &str) -> Result<Option<String>, BoardError> {
        let sha: Option<Option<String>> =
            sqlx::query_scalar("SELECT merge_commit_sha FROM tickets WHERE id = ?")
                .bind(ticket_id)
                .fetch_optional(self.store.pool())
                .await?;
        Ok(sha.flatten())
    }
}

fn totals(files: &[ChangedFile]) -> (u32, u32) {
    files.iter().fold((0, 0), |(a, r), f| {
        (a.saturating_add(f.added), r.saturating_add(f.removed))
    })
}

pub fn short_sha(sha: &str) -> String {
    sha.chars().take(SHORT_SHA).collect()
}
