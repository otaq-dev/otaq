//! Worktrees and temporary branches (SES-01 steps 1 and 3, SES-02, EPC-02, GIT-07; d0010, d0023, d0029, d0050,
//! d0059).
//!
//! Each ticket works in its own git worktree **outside the user's repo**, at `wt/<repo-name>/<KEY>` in the data
//! folder, on the temporary branch `crewboard/<KEY>` (the real name is chosen at the first push, M10). The base
//! is resolved first: standalone tickets use their base branch; **epic children use the epic's branch**, which
//! the first child's start creates as `crewboard/SHOP-40` from the epic's base. The epic itself gets no agent
//! and no worktree.
//!
//! Everything converges (SES-02): a branch or worktree that exists is reused, a worktree registered with git
//! whose folder is gone is added again, and the ticket row is filled in whatever happened before. Nothing is
//! written into the user's repository except git's own branch refs and worktree metadata under `.git/`.

use std::path::{Path, PathBuf};

use super::StartError;
use crate::board::Board;
use crate::board::tickets::{Base, Ticket};
use crate::data_dir::DataDir;
use crate::git::{Git, GitError};
use crate::naming;
use crate::store::now_ms;

/// Where a ticket works.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    /// `crewboard/SHOP-18`.
    pub branch: String,
    /// The branch it started from: `main`, or the epic's `crewboard/SHOP-40`.
    pub base_branch: String,
    /// The commit where the ticket left its base.
    pub base_sha: String,
}

/// Resolves the base, creates the branch and the worktree (or reuses them), and stores `branch_name`,
/// `base_sha` and `worktree_path` on the ticket, locking its base (d0051).
pub async fn prepare_worktree(
    board: &Board,
    data: &DataDir,
    key: &str,
) -> Result<Worktree, StartError> {
    let ticket = board.ticket(key).await?;
    if ticket.is_epic() {
        return Err(StartError::Other(format!(
            "{} is an epic; epics get no worktree of their own",
            ticket.key
        )));
    }
    let project = board.project(ticket.key.project.as_str()).await?;
    let git = board.git()?;
    let repo = PathBuf::from(&project.repo_path);

    // Step 1: the base.
    let (base_branch, base_commit) = match &ticket.base {
        Base::Branch(branch) => {
            let commit = branch_commit(git, &repo, branch).await?;
            (branch.clone(), commit)
        }
        Base::Epic(epic) => {
            let epic = board.ticket(&epic.to_string()).await?;
            let branch = epic_branch(board, git, &repo, &epic).await?;
            let commit = branch_commit(git, &repo, &branch).await?;
            (branch, commit)
        }
    };

    // Step 3: the ticket's branch.
    let branch = naming::branch(&ticket.key.to_string());
    let branch_ref = format!("refs/heads/{branch}");
    let stored = board.store().ticket(&ticket.id).await?;
    let stored_base_sha = stored.and_then(|row| row.base_sha);
    let base_sha = if git.resolve(&repo, &branch_ref).await?.is_some() {
        match stored_base_sha {
            Some(sha) => sha,
            // The branch was made but the start stopped before storing where it left its base.
            None => git
                .run(&repo, &["merge-base", &branch_ref, &base_commit])
                .await?
                .trim()
                .to_owned(),
        }
    } else {
        git.run(&repo, &["branch", "--no-track", &branch, &base_commit])
            .await?;
        base_commit.clone()
    };

    let repo_name = repo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| project.name.clone());
    let path = data.worktree_path(&repo_name, &ticket.key.to_string())?;
    add_worktree(git, &repo, &path, &branch).await?;

    sqlx::query(
        "UPDATE tickets SET branch_name = ?, base_sha = ?, worktree_path = ?, branch_locked = TRUE,
                            updated_at = ?
         WHERE id = ?",
    )
    .bind(&branch)
    .bind(&base_sha)
    .bind(path.display().to_string())
    .bind(now_ms())
    .bind(&ticket.id)
    .execute(board.store().pool())
    .await
    .map_err(crate::store::StoreError::from)?;

    Ok(Worktree {
        path,
        branch,
        base_branch,
        base_sha,
    })
}

/// The epic's integration branch, created from the epic's base on the first child's start (EPC-02).
async fn epic_branch(
    board: &Board,
    git: &Git,
    repo: &Path,
    epic: &Ticket,
) -> Result<String, StartError> {
    let Base::Branch(epic_base) = &epic.base else {
        return Err(StartError::Other(format!(
            "{} has no base branch",
            epic.key
        )));
    };
    let branch = naming::branch(&epic.key.to_string());
    let branch_ref = format!("refs/heads/{branch}");
    let base_sha = match git.resolve(repo, &branch_ref).await? {
        Some(_) => None,
        None => {
            let from = branch_commit(git, repo, epic_base).await?;
            git.run(repo, &["branch", "--no-track", &branch, &from])
                .await?;
            Some(from)
        }
    };
    // Fill in the epic's row (also after a start that stopped before doing so); the first base_sha stays.
    sqlx::query(
        "UPDATE tickets SET branch_name = ?, base_sha = COALESCE(base_sha, ?), branch_locked = TRUE,
                            updated_at = ?
         WHERE id = ?",
    )
    .bind(&branch)
    .bind(base_sha)
    .bind(now_ms())
    .bind(&epic.id)
    .execute(board.store().pool())
    .await
    .map_err(crate::store::StoreError::from)?;
    Ok(branch)
}

/// The commit of a base branch: the local branch, or the remote's when there is no local one (`origin/develop`).
pub(crate) async fn branch_commit(
    git: &Git,
    repo: &Path,
    branch: &str,
) -> Result<String, StartError> {
    if let Some(sha) = git.resolve(repo, &format!("refs/heads/{branch}")).await? {
        return Ok(sha);
    }
    let remotes = git
        .run(
            repo,
            &[
                "for-each-ref",
                "--format=%(refname)",
                &format!("refs/remotes/*/{branch}"),
            ],
        )
        .await?;
    let mut candidates: Vec<&str> = remotes.lines().collect();
    // Prefer origin when several remotes have it.
    candidates.sort_by_key(|r| !r.starts_with("refs/remotes/origin/"));
    for candidate in candidates {
        if let Some(sha) = git.resolve(repo, candidate).await? {
            return Ok(sha);
        }
    }
    Err(StartError::Other(format!(
        "the base branch `{branch}` doesn't exist in {}",
        repo.display()
    )))
}

/// Adds the worktree at `path` on `branch`, or reuses the one already there.
pub(crate) async fn add_worktree(
    git: &Git,
    repo: &Path,
    path: &Path,
    branch: &str,
) -> Result<(), StartError> {
    let registered = registered_worktrees(git, repo).await?;
    let is_registered = registered.iter().any(|p| same_path(p, path));
    if is_registered && path.is_dir() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        DataDir::ensure_dir(parent)
            .map_err(|e| StartError::Other(format!("can't create {}: {e}", parent.display())))?;
    }
    if !is_registered && path.exists() && !is_empty_dir(path) {
        return Err(StartError::Other(format!(
            "{} exists and isn't a worktree of {}; move it away and retry",
            path.display(),
            repo.display()
        )));
    }
    let path_arg = path.display().to_string();
    let mut args = vec!["worktree", "add", "--quiet"];
    if is_registered {
        // Registered with git but its folder is gone: `--force` lets git add it again.
        args.push("--force");
    }
    args.extend([path_arg.as_str(), branch]);
    git.run(repo, &args).await?;
    Ok(())
}

async fn registered_worktrees(git: &Git, repo: &Path) -> Result<Vec<PathBuf>, GitError> {
    let list = git.run(repo, &["worktree", "list", "--porcelain"]).await?;
    Ok(list
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .map(PathBuf::from)
        .collect())
}

/// Compares paths after resolving symlinks where possible (`/tmp` and `/private/tmp` on macOS).
fn same_path(a: &Path, b: &Path) -> bool {
    let canonical = |p: &Path| {
        crate::paths::canonical(p).unwrap_or_else(|_| {
            // A missing folder: canonicalize its parent and keep the name.
            match (
                p.parent().and_then(|d| crate::paths::canonical(d).ok()),
                p.file_name(),
            ) {
                (Some(dir), Some(name)) => dir.join(name),
                _ => p.to_path_buf(),
            }
        })
    };
    canonical(a) == canonical(b)
}

fn is_empty_dir(path: &Path) -> bool {
    std::fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none())
}
