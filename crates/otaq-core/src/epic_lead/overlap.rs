//! Children changing the same file (LEAD-09, d0083): for each epic, the files each **running** child has changed
//! so far (its worktree against the merge base with the epic branch, committed or not, plus untracked files that
//! aren't ignored), and the pairs of running children with a file in common.
//!
//! Both cards of a pair show the quiet "same file as SHOP-40.4" ([`crate::domain::Flags::same_file`]); the lead
//! sees the pairs in `prime --epic` ([`overlaps`]). An overlap **doesn't wake the lead**, pause anyone or change a
//! dependency. A child that stops running (merged, parked, back in Ready for Review…) drops out.
//!
//! The files are recomputed on each scheduler pass (every ticket change, including a child's `otaq` command or
//! hook, and a merge into the epic branch) and on the slow tick. The result is cached per child and reused while
//! its HEAD, the epic branch and the worktree's index mtime are unchanged. Edits the agent hasn't staged move
//! none of those, so the child's own `otaq` command or turn end marks it for recomputing ([`Overlaps::touch`])
//! (implementation choice, SDD §26). The cache lives in memory: a restart ends every session anyway.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError};
use crate::domain::{Column, Kind, SameFile, Status, TicketKey};
use crate::git::{Git, GitError};

/// Two running children of one epic that changed the same files so far. `a` sorts before `b`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overlap {
    pub a: TicketKey,
    pub b: TicketKey,
    /// The files both changed, sorted.
    pub files: Vec<String>,
}

/// Whether a ticket in this status is a running child's: in Dev with an agent session (working, turquoise,
/// background or validating). Interrupted and Resume failed have no session.
pub fn is_running(status: Status) -> bool {
    status.column() == Column::Dev && !matches!(status, Status::Interrupted | Status::ResumeFailed)
}

/// The cache of every running child's changed files. The board reads it when it builds tickets; [`refresh`]
/// keeps it. Cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct Overlaps {
    inner: Arc<Mutex<State>>,
}

#[derive(Debug, Default)]
struct State {
    /// Running children by ticket id.
    children: HashMap<String, Child>,
    /// Children whose agent ran a command or ended a turn since their files were last read.
    touched: HashSet<String>,
    /// How many times a child's files were read from git (the cache's misses).
    recomputes: u64,
}

#[derive(Debug, Clone)]
struct Child {
    key: TicketKey,
    epic: TicketKey,
    stamp: Stamp,
    files: Vec<String>,
}

/// What must stay the same for a child's cached files to stay right.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    head: String,
    epic_tip: String,
    index_mtime: Option<SystemTime>,
}

impl Overlaps {
    /// The child's agent ran a `otaq` command or ended a turn: its files are read again at the next refresh,
    /// whatever its stamp says.
    pub fn touch(&self, ticket_id: &str) {
        self.lock().touched.insert(ticket_id.to_owned());
    }

    /// The other running children of this child's epic that changed a file it changed, by key.
    pub fn same_file(&self, ticket_id: &str) -> Vec<SameFile> {
        let state = self.lock();
        let Some(child) = state.children.get(ticket_id) else {
            return Vec::new();
        };
        let mut found: Vec<SameFile> = state
            .children
            .iter()
            .filter(|(id, other)| *id != ticket_id && other.epic == child.epic)
            .filter_map(|(_, other)| {
                let files = common(&child.files, &other.files);
                (!files.is_empty()).then(|| SameFile {
                    other: other.key.clone(),
                    files,
                })
            })
            .collect();
        found.sort_by(|x, y| x.other.cmp(&y.other));
        found
    }

    /// Every pair of running children of `epic` with a file in common, by their keys.
    pub fn of_epic(&self, epic: &TicketKey) -> Vec<Overlap> {
        let state = self.lock();
        let mut children: Vec<&Child> = state
            .children
            .values()
            .filter(|c| &c.epic == epic)
            .collect();
        children.sort_by(|x, y| x.key.cmp(&y.key));
        let mut pairs = Vec::new();
        for (i, a) in children.iter().enumerate() {
            for b in &children[i + 1..] {
                let files = common(&a.files, &b.files);
                if !files.is_empty() {
                    pairs.push(Overlap {
                        a: a.key.clone(),
                        b: b.key.clone(),
                        files,
                    });
                }
            }
        }
        pairs
    }

    /// How many times a child's files were read from git so far (for tests and logs).
    pub fn recomputes(&self) -> u64 {
        self.lock().recomputes
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner.lock().expect("overlaps")
    }
}

/// The paths in both sorted lists.
fn common(a: &[String], b: &[String]) -> Vec<String> {
    a.iter()
        .filter(|f| b.binary_search(f).is_ok())
        .cloned()
        .collect()
}

/// The running children of `epic`'s that changed the same files, refreshed first (for `prime --epic`).
pub async fn overlaps(board: &Board, epic: &str) -> Result<Vec<Overlap>, BoardError> {
    let epic = board.ticket(epic).await?;
    refresh_project(board, epic.key.project.as_str()).await?;
    Ok(board.overlaps().of_epic(&epic.key))
}

/// Refreshes every project's running children. Returns the tickets whose "same file" flag changed, so the caller
/// tells clients.
pub async fn refresh(board: &Board) -> Result<Vec<(String, TicketKey)>, BoardError> {
    let mut changed = Vec::new();
    for project in board.projects().await? {
        changed.extend(refresh_project(board, &project.key).await?);
    }
    Ok(changed)
}

/// Refreshes one project's running children; see [`refresh`].
async fn refresh_project(
    board: &Board,
    project: &str,
) -> Result<Vec<(String, TicketKey)>, BoardError> {
    let Ok(git) = board.git() else {
        return Ok(Vec::new());
    };
    let cache = board.overlaps();
    let tickets = board.tickets(project).await?;
    let in_project = |key: &TicketKey| key.project.as_str() == project;
    let before = flags_of(cache, project);

    let epic_branches: HashMap<&TicketKey, &str> = tickets
        .iter()
        .filter(|t| t.kind == Kind::Epic)
        .filter_map(|t| Some((&t.key, t.work.as_ref()?.branch.as_deref()?)))
        .collect();
    let mut children = HashMap::new();
    for ticket in tickets.iter().filter(|t| is_running(t.status)) {
        let (Some(epic), Some(worktree)) = (
            ticket.epic.as_ref(),
            ticket.work.as_ref().and_then(|w| w.worktree.as_deref()),
        ) else {
            continue;
        };
        let Some(branch) = epic_branches.get(epic) else {
            continue;
        };
        match read_child(git, cache, ticket, epic, Path::new(worktree), branch).await {
            Ok(child) => {
                children.insert(ticket.id.clone(), child);
            }
            Err(e) => {
                tracing::debug!(ticket = %ticket.key, error = %e, "can't read the changed files")
            }
        }
    }
    {
        let mut state = cache.lock();
        state.children.retain(|_, c| !in_project(&c.key));
        state.children.extend(children);
    }

    let after = flags_of(cache, project);
    let mut changed: Vec<(String, TicketKey)> = before
        .iter()
        .chain(after.iter())
        .filter(|(id, _)| flag(&before, id) != flag(&after, id))
        .map(|(id, (key, _))| (id.clone(), key.clone()))
        .collect();
    changed.sort();
    changed.dedup();
    Ok(changed)
}

/// A child's files: the cached ones while its stamp holds and it wasn't touched, else read from git.
async fn read_child(
    git: &Git,
    cache: &Overlaps,
    ticket: &Ticket,
    epic: &TicketKey,
    worktree: &Path,
    branch: &str,
) -> Result<Child, GitError> {
    let stamp = stamp(git, worktree, branch).await?;
    let cached = {
        let mut state = cache.lock();
        let touched = state.touched.remove(&ticket.id);
        state
            .children
            .get(&ticket.id)
            .filter(|c| !touched && c.stamp == stamp && &c.epic == epic)
            .map(|c| c.files.clone())
    };
    let files = match cached {
        Some(files) => files,
        None => {
            let files = git.worktree_changed_paths(worktree, branch).await?;
            cache.lock().recomputes += 1;
            files
        }
    };
    Ok(Child {
        key: ticket.key.clone(),
        epic: epic.clone(),
        stamp,
        files,
    })
}

/// The child's HEAD, the epic branch's tip and the worktree's index mtime, in one `git` call.
async fn stamp(git: &Git, worktree: &Path, branch: &str) -> Result<Stamp, GitError> {
    let branch_ref = format!("refs/heads/{branch}");
    let out = git
        .run(
            worktree,
            &["rev-parse", "HEAD", &branch_ref, "--git-path", "index"],
        )
        .await?;
    let mut lines = out.lines();
    let head = lines.next().unwrap_or_default().to_owned();
    let epic_tip = lines.next().unwrap_or_default().to_owned();
    let index = worktree.join(lines.next().unwrap_or_default());
    let index_mtime = tokio::fs::metadata(&index)
        .await
        .and_then(|m| m.modified())
        .ok();
    Ok(Stamp {
        head,
        epic_tip,
        index_mtime,
    })
}

/// One child's flag in a [`flags_of`] map; a child that isn't there has none.
fn flag<'a>(flags: &'a HashMap<String, (TicketKey, Vec<SameFile>)>, id: &str) -> &'a [SameFile] {
    flags.get(id).map_or(&[], |(_, flag)| flag)
}

/// The "same file" flag of every cached child of a project, by ticket id.
fn flags_of(cache: &Overlaps, project: &str) -> HashMap<String, (TicketKey, Vec<SameFile>)> {
    let ids: Vec<(String, TicketKey)> = cache
        .lock()
        .children
        .iter()
        .filter(|(_, c)| c.key.project.as_str() == project)
        .map(|(id, c)| (id.clone(), c.key.clone()))
        .collect();
    ids.into_iter()
        .map(|(id, key)| {
            let flag = cache.same_file(&id);
            (id, (key, flag))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn child(key: &str, files: &[&str]) -> Child {
        Child {
            key: key.parse().unwrap(),
            epic: "SHOP-40".parse().unwrap(),
            stamp: Stamp {
                head: String::new(),
                epic_tip: String::new(),
                index_mtime: None,
            },
            files: files.iter().map(|f| (*f).to_owned()).collect(),
        }
    }

    #[test]
    fn pairs_with_a_common_file_flag_each_other() {
        let cache = Overlaps::default();
        {
            let mut state = cache.lock();
            state.children.insert(
                "a".into(),
                child("SHOP-40.2", &["lib/wishlist.ts", "src/button.tsx"]),
            );
            state.children.insert(
                "b".into(),
                child("SHOP-40.4", &["lib/share.ts", "lib/wishlist.ts"]),
            );
            state
                .children
                .insert("c".into(), child("SHOP-40.5", &["README.md"]));
        }
        assert_eq!(
            cache.of_epic(&"SHOP-40".parse().unwrap()),
            vec![Overlap {
                a: "SHOP-40.2".parse().unwrap(),
                b: "SHOP-40.4".parse().unwrap(),
                files: vec!["lib/wishlist.ts".into()],
            }]
        );
        assert_eq!(
            cache.same_file("a"),
            vec![SameFile {
                other: "SHOP-40.4".parse().unwrap(),
                files: vec!["lib/wishlist.ts".into()],
            }]
        );
        assert_eq!(cache.same_file("b")[0].other.to_string(), "SHOP-40.2");
        assert!(cache.same_file("c").is_empty());
        assert!(cache.same_file("unknown").is_empty());
    }

    #[test]
    fn children_of_different_epics_never_overlap() {
        let cache = Overlaps::default();
        let mut other = child("SHOP-45.1", &["lib/wishlist.ts"]);
        other.epic = "SHOP-45".parse().unwrap();
        {
            let mut state = cache.lock();
            state
                .children
                .insert("a".into(), child("SHOP-40.2", &["lib/wishlist.ts"]));
            state.children.insert("b".into(), other);
        }
        assert!(cache.same_file("a").is_empty());
        assert!(cache.of_epic(&"SHOP-40".parse().unwrap()).is_empty());
    }

    #[test]
    fn running_means_in_dev_with_a_session() {
        assert!(is_running(Status::Working));
        assert!(is_running(Status::AsksYou));
        assert!(is_running(Status::Validating));
        assert!(is_running(Status::WorkingInBackground));
        assert!(!is_running(Status::Interrupted));
        assert!(!is_running(Status::ResumeFailed));
        assert!(!is_running(Status::Parked));
        assert!(!is_running(Status::ReadyForReview));
        assert!(!is_running(Status::Merged));
    }
}
