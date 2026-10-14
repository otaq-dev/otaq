//! Pushed-content verification (REV-10 step 4, REV-06; d0043, d0050): after a push, the PR branch's **content** on
//! the remote must equal what the user reviewed. Trees are compared, not commit IDs, because the dialog rewords
//! commits. A difference shows the "changed since you reviewed it" warning with only the extra content.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError};
use crate::git::diff::DiffSpec;
use crate::store::now_ms;

/// What was pushed beyond the reviewed content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushedDiff {
    /// The reviewed head.
    pub reviewed: String,
    /// The remote branch's tip.
    pub pushed: String,
    /// The files whose content differs.
    pub files: Vec<String>,
}

impl PushedDiff {
    /// "SHOP-18 changed since you reviewed it · the pushed content differs in 2 files".
    pub fn message(&self, key: &str) -> String {
        let n = self.files.len();
        let files = if n == 1 { "1 file" } else { "files" };
        if n == 1 {
            format!(
                "{key} changed since you reviewed it \u{b7} the pushed content differs in {files}"
            )
        } else {
            format!(
                "{key} changed since you reviewed it \u{b7} the pushed content differs in {n} {files}"
            )
        }
    }
}

impl Board {
    /// Fetches the PR branch and compares its tree with the reviewed head's; stores the difference (or clears
    /// it). `None` when they match or nothing can be compared (no reviewed head, the branch isn't on origin).
    pub async fn verify_pushed(&self, ticket: &Ticket) -> Result<Option<PushedDiff>, BoardError> {
        let row = self
            .store()
            .ticket(&ticket.id)
            .await?
            .ok_or_else(|| BoardError::NotFound(ticket.key.to_string()))?;
        let (Some(branch), Some(reviewed)) = (row.branch_name, row.review_head_sha) else {
            return Ok(None);
        };
        let repo = PathBuf::from(self.project(ticket.key.project.as_str()).await?.repo_path);
        let git = self.git()?;
        let tracking = format!("refs/remotes/origin/{branch}");
        let refspec = format!("+refs/heads/{branch}:{tracking}");
        if let Err(e) = git
            .run(&repo, &["fetch", "--quiet", "origin", &refspec])
            .await
        {
            tracing::warn!(key = %ticket.key, error = %e, "can't fetch the pushed branch to compare it");
            return Ok(None);
        }
        let Some(pushed) = git.resolve(&repo, &tracking).await? else {
            return Ok(None);
        };
        let differs = if git.tree_of(&repo, &pushed).await? == git.tree_of(&repo, &reviewed).await?
        {
            None
        } else {
            let files = git
                .changed_files(&repo, &DiffSpec::between(&reviewed, &pushed))
                .await?
                .into_iter()
                .map(|f| f.path)
                .collect();
            Some(PushedDiff {
                reviewed,
                pushed,
                files,
            })
        };
        sqlx::query("UPDATE tickets SET pushed_differs = ?, updated_at = ? WHERE id = ?")
            .bind(
                differs
                    .as_ref()
                    .map(|d| serde_json::to_string(d).expect("diff serializes")),
            )
            .bind(now_ms())
            .bind(&ticket.id)
            .execute(self.store().pool())
            .await?;
        Ok(differs)
    }
}
