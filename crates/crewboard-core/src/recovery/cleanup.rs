//! Cleanup (REC-04; d0018, d0026): leftovers no ticket owns, listed for the user to delete by hand. **Nothing here
//! deletes anything on its own**: [`Cleanup::scan`] only reads, and [`Cleanup::delete`] runs only on the user's
//! confirmed request naming the items, each checked again against a fresh scan first.
//!
//! Groups:
//! - **Sessions**: running sessions whose ticket is gone, finished (Done) or back in Backlog.
//! - **Worktrees**: folders under `<data>/wt/<repo>/` that no unfinished ticket uses.
//! - **Branches**: `crewboard/*` branches in a project's repository that no ticket names. The user's own branches
//!   are never candidates.
//! - **Ports**: ports still held by tickets that are Done or in Backlog.
//! - **Kept worktrees**: Done / Closed tickets whose worktree was kept because of uncommitted changes (REC-05).
//!   Deleting one discards those changes, so it is deleted only when listed explicitly.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError, Refusal};
use crate::data_dir::DataDir;
use crate::domain::{Column, TicketKey};
use crate::naming;
use crate::ports::PortPool;
use crate::session::Sessions;
use crate::store::now_ms;

/// The Cleanup view's groups, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupGroup {
    Sessions,
    Worktrees,
    Branches,
    Ports,
    KeptWorktrees,
}

/// One leftover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupItem {
    pub group: CleanupGroup,
    /// Names the item in a delete request: a session id, a folder, `<project>:<branch>`, or a ticket id.
    pub id: String,
    /// What the user reads: "shop-web · crewboard/SHOP-9", "SHOP-26 · ports 4031, 4032".
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticket: Option<TicketKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_ms: Option<i64>,
    /// Why it is listed: "no ticket uses this worktree", "SHOP-26 was closed with uncommitted changes".
    pub why: String,
    /// Kept worktrees: the files that aren't committed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uncommitted: Vec<String>,
}

/// An item to delete, as the scan named it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CleanupTarget {
    pub group: CleanupGroup,
    pub id: String,
}

/// What a delete did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupOutcome {
    pub deleted: Vec<CleanupTarget>,
    /// Items that weren't deleted, with why (no longer a leftover, or the deletion failed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed: Vec<(CleanupTarget, String)>,
}

impl CleanupItem {
    pub fn target(&self) -> CleanupTarget {
        CleanupTarget {
            group: self.group,
            id: self.id.clone(),
        }
    }
}

/// Scans for leftovers and deletes the ones the user picked. Cheap to clone.
#[derive(Clone)]
pub struct Cleanup {
    board: Board,
    data: DataDir,
    sessions: Sessions,
    ports: PortPool,
}

impl Cleanup {
    pub fn new(board: Board, data: DataDir, sessions: Sessions, ports: PortPool) -> Self {
        Self {
            board,
            data,
            sessions,
            ports,
        }
    }

    /// Every leftover, grouped. Reads only.
    pub async fn scan(&self) -> Result<Vec<CleanupItem>, BoardError> {
        let now = now_ms();
        let projects = self.board.projects().await?;
        let mut tickets: Vec<Ticket> = Vec::new();
        for project in &projects {
            tickets.extend(self.board.tickets(&project.key).await?);
        }
        let mut items = Vec::new();

        // Sessions of tickets that are gone, done or in Backlog.
        for session in self.sessions.in_memory() {
            if !session.is_running() {
                continue;
            }
            let Some(key) = &session.ticket else {
                continue;
            };
            let owner = tickets.iter().find(|t| &t.key == key);
            let why = match owner {
                None => format!("{key} no longer exists"),
                Some(t) if matches!(t.column, Column::Done | Column::Backlog) => {
                    format!("{key} is {}", t.status.label())
                }
                Some(_) => continue,
            };
            items.push(CleanupItem {
                group: CleanupGroup::Sessions,
                id: session.id.clone(),
                label: format!("{key} · {} session", session.kind),
                project: Some(session.project.clone()),
                ticket: Some(key.clone()),
                path: None,
                size_bytes: None,
                age_ms: Some((now - session.started_at).max(0)),
                why,
                uncommitted: Vec::new(),
            });
        }

        // Worktree folders no unfinished ticket uses; kept ones of finished tickets are their own group.
        let in_use: HashSet<PathBuf> = tickets
            .iter()
            .filter(|t| t.column != Column::Done)
            .filter_map(|t| t.work.as_ref()?.worktree.as_deref())
            .map(normalized)
            .collect();
        let kept: HashSet<PathBuf> = self
            .kept_tickets(&tickets)
            .await?
            .iter()
            .filter_map(|(t, _)| t.work.as_ref()?.worktree.as_deref())
            .map(normalized)
            .collect();
        for folder in worktree_folders(&self.data.worktrees_dir()) {
            let path = normalized(&folder.display().to_string());
            if in_use.contains(&path) || kept.contains(&path) {
                continue;
            }
            let repo = folder
                .parent()
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned());
            items.push(CleanupItem {
                group: CleanupGroup::Worktrees,
                id: folder.display().to_string(),
                label: folder
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                project: repo,
                ticket: None,
                path: Some(folder.display().to_string()),
                size_bytes: Some(folder_size(&folder)),
                age_ms: modified_age(&folder, now),
                why: "no ticket uses this worktree".into(),
                uncommitted: Vec::new(),
            });
        }

        // Branches named by Crewboard that no ticket names.
        let git = self.board.git()?;
        let prefix = naming::branch_prefix();
        for project in &projects {
            let named: HashSet<&str> = tickets
                .iter()
                .filter(|t| t.key.project.as_str() == project.key)
                .filter_map(|t| t.work.as_ref()?.branch.as_deref())
                .collect();
            let refs = git
                .run(
                    Path::new(&project.repo_path),
                    &[
                        "for-each-ref",
                        "--format=%(refname:short) %(committerdate:unix)",
                        &format!("refs/heads/{prefix}"),
                    ],
                )
                .await
                .unwrap_or_default();
            for line in refs.lines() {
                let mut parts = line.split_whitespace();
                let (Some(branch), at) = (parts.next(), parts.next()) else {
                    continue;
                };
                if !branch.starts_with(&prefix) || named.contains(branch) {
                    continue;
                }
                let at = at.and_then(|s| s.parse::<i64>().ok());
                items.push(CleanupItem {
                    group: CleanupGroup::Branches,
                    id: format!("{}:{branch}", project.key),
                    label: format!("{} · {branch}", project.name),
                    project: Some(project.key.clone()),
                    ticket: None,
                    path: Some(project.repo_path.clone()),
                    size_bytes: None,
                    age_ms: at.map(|secs| (now - secs * 1000).max(0)),
                    why: "no ticket uses this branch".into(),
                    uncommitted: Vec::new(),
                });
            }
        }

        // Ports held by tickets that are finished or back in Backlog.
        for ticket in &tickets {
            let Some(work) = &ticket.work else { continue };
            let ports: Vec<u16> = [work.agent_port, work.run_port]
                .into_iter()
                .flatten()
                .collect();
            if ports.is_empty() || !matches!(ticket.column, Column::Done | Column::Backlog) {
                continue;
            }
            items.push(CleanupItem {
                group: CleanupGroup::Ports,
                id: ticket.id.clone(),
                label: format!(
                    "{} · {} {}",
                    ticket.key,
                    if ports.len() == 1 { "port" } else { "ports" },
                    ports
                        .iter()
                        .map(u16::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                project: Some(ticket.key.project.to_string()),
                ticket: Some(ticket.key.clone()),
                path: None,
                size_bytes: None,
                age_ms: ticket.status_since.map(|s| (now - s).max(0)),
                why: format!("{} is {}", ticket.key, ticket.status.label()),
                uncommitted: Vec::new(),
            });
        }

        // Worktrees kept because of uncommitted changes.
        for (ticket, uncommitted) in self.kept_tickets(&tickets).await? {
            let Some(path) = ticket.work.as_ref().and_then(|w| w.worktree.clone()) else {
                continue;
            };
            let folder = PathBuf::from(&path);
            items.push(CleanupItem {
                group: CleanupGroup::KeptWorktrees,
                id: ticket.id.clone(),
                label: format!("{} · {}", ticket.key, ticket.title),
                project: Some(ticket.key.project.to_string()),
                ticket: Some(ticket.key.clone()),
                size_bytes: folder.is_dir().then(|| folder_size(&folder)),
                age_ms: modified_age(&folder, now),
                path: Some(path),
                why: format!(
                    "{} is {} with uncommitted changes",
                    ticket.key,
                    ticket.status.label()
                ),
                uncommitted,
            });
        }

        items.sort_by(|a, b| a.group.cmp(&b.group).then(a.label.cmp(&b.label)));
        Ok(items)
    }

    /// Deletes exactly the `targets` the user confirmed. Each one must still be a leftover in a fresh scan;
    /// anything else is reported, never deleted. Without `confirm`, refused with what would be deleted.
    pub async fn delete(
        &self,
        targets: &[CleanupTarget],
        confirm: bool,
    ) -> Result<CleanupOutcome, BoardError> {
        let scan = self.scan().await?;
        if !confirm {
            let names: Vec<String> = targets
                .iter()
                .filter_map(|t| scan.iter().find(|i| i.target() == *t))
                .map(|i| i.label.clone())
                .collect();
            return Err(BoardError::refused(
                Refusal::NeedsConfirmation,
                format!("Delete {}? This can't be undone.", names.join(", ")),
            ));
        }
        let mut outcome = CleanupOutcome::default();
        let mut repos: HashSet<PathBuf> = HashSet::new();
        for target in targets {
            let Some(item) = scan.iter().find(|i| i.target() == *target) else {
                outcome
                    .failed
                    .push((target.clone(), "not a leftover".into()));
                continue;
            };
            match self.delete_one(item, &mut repos).await {
                Ok(()) => {
                    tracing::info!(group = ?item.group, item = %item.label, "cleanup deleted");
                    outcome.deleted.push(target.clone());
                }
                Err(e) => outcome.failed.push((target.clone(), e.to_string())),
            }
        }
        // Worktrees removed by hand leave records in their repositories.
        if let Ok(git) = self.board.git() {
            for repo in repos {
                let _ = git.run(&repo, &["worktree", "prune"]).await;
            }
        }
        Ok(outcome)
    }

    async fn delete_one(
        &self,
        item: &CleanupItem,
        repos: &mut HashSet<PathBuf>,
    ) -> Result<(), BoardError> {
        let git = self.board.git()?;
        match item.group {
            CleanupGroup::Sessions => self
                .sessions
                .stop(&item.id)
                .await
                .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string())),
            CleanupGroup::Worktrees => {
                let folder = PathBuf::from(&item.id);
                self.remove_worktree(&folder, repos).await
            }
            CleanupGroup::Branches => {
                let (project, branch) = item.id.split_once(':').unwrap_or_default();
                if !branch.starts_with(&naming::branch_prefix()) {
                    return Err(BoardError::refused(
                        Refusal::NotAllowed,
                        format!("{branch} isn't a branch {} made", naming::PRODUCT_NAME),
                    ));
                }
                let repo = self.board.project(project).await?.repo_path;
                git.run(Path::new(&repo), &["branch", "-D", branch]).await?;
                Ok(())
            }
            CleanupGroup::Ports => self
                .ports
                .free(&item.id)
                .await
                .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string())),
            CleanupGroup::KeptWorktrees => {
                if let Some(path) = &item.path {
                    self.remove_worktree(Path::new(path), repos).await?;
                }
                sqlx::query(
                    "UPDATE tickets SET worktree_path = NULL, worktree_kept = FALSE WHERE id = ?",
                )
                .bind(&item.id)
                .execute(self.board.store().pool())
                .await?;
                Ok(())
            }
        }
    }

    /// Removes a worktree folder inside the data folder: through its repository when it is a worktree of one
    /// (changes included: the user picked it), else the folder itself.
    async fn remove_worktree(
        &self,
        folder: &Path,
        repos: &mut HashSet<PathBuf>,
    ) -> Result<(), BoardError> {
        if !folder.starts_with(self.data.worktrees_dir()) {
            return Err(BoardError::refused(
                Refusal::NotAllowed,
                format!(
                    "{} isn't in {}'s worktree folder",
                    folder.display(),
                    naming::PRODUCT_NAME
                ),
            ));
        }
        if !folder.exists() {
            return Ok(());
        }
        let git = self.board.git()?;
        let path = folder.display().to_string();
        let common = git
            .run(
                folder,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            )
            .await
            .ok()
            .map(|dir| PathBuf::from(dir.trim()));
        if let Some(repo) = common.as_deref().and_then(Path::parent) {
            repos.insert(repo.to_owned());
            if git
                .run(repo, &["worktree", "remove", "--force", "--force", &path])
                .await
                .is_ok()
            {
                return Ok(());
            }
        }
        std::fs::remove_dir_all(folder).map_err(|e| {
            BoardError::refused(
                Refusal::NotAllowed,
                format!("can't remove {}: {e}", folder.display()),
            )
        })
    }

    /// Done / Closed tickets whose worktree was kept, with their uncommitted files.
    async fn kept_tickets(
        &self,
        tickets: &[Ticket],
    ) -> Result<Vec<(Ticket, Vec<String>)>, BoardError> {
        let ids: HashSet<String> =
            sqlx::query_scalar::<_, String>("SELECT id FROM tickets WHERE worktree_kept = TRUE")
                .fetch_all(self.board.store().pool())
                .await?
                .into_iter()
                .collect();
        let mut kept = Vec::new();
        for ticket in tickets.iter().filter(|t| ids.contains(&t.id)) {
            let uncommitted = match ticket.work.as_ref().and_then(|w| w.worktree.as_deref()) {
                Some(path) if Path::new(path).is_dir() => self
                    .board
                    .git()?
                    .uncommitted_files(Path::new(path))
                    .await
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            kept.push((ticket.clone(), uncommitted));
        }
        Ok(kept)
    }
}

/// `<root>/<repo>/<KEY>` folders.
fn worktree_folders(root: &Path) -> Vec<PathBuf> {
    let dirs = |dir: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                    .map(|e| e.path())
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut folders: Vec<PathBuf> = dirs(root).iter().flat_map(|repo| dirs(repo)).collect();
    folders.sort();
    folders
}

/// A path compared without symlinks (macOS's `/var` is `/private/var`).
fn normalized(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    path.canonicalize().unwrap_or(path)
}

/// The size of everything in a folder (symlinks not followed).
fn folder_size(folder: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![folder.to_owned()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total += meta.len();
            }
        }
    }
    total
}

fn modified_age(path: &Path, now: i64) -> Option<i64> {
    let modified = path.metadata().ok()?.modified().ok()?;
    let ms = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    Some((now - i64::try_from(ms).ok()?).max(0))
}
