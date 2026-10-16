//! Keeping the build at Done (WRM-03, REC-05; d0082): before a clean worktree is removed, its gitignored files are
//! **moved** (renamed, never copied) into the project's warm build, so the next new worktree starts from them.
//!
//! - Only when warm builds are on for the project (the switch, and a disk that can clone).
//! - The gitignored paths come from `git ls-files --others --ignored --exclude-standard --directory`: a folder is one
//!   entry, so a folder of a hundred thousand files is one rename. Every gitignored file is treated the same.
//! - The project's env files are left out: they are copied fresh at each start (SES-01 step 4).
//! - The record: built at (`HEAD`), base (`merge-base HEAD <base branch>`, for "forward only"), from (the ticket)
//!   and the time. The store's replace decides whether it is newer than the warm build there.
//! - The times file ([`TrackedTimes`]): every tracked file's modified time in the worktree (a symlink's own), so a
//!   worktree seeded from this build gives its unchanged files the times they had here (Q71): a file committed after
//!   the last build still looks newer than the build's output.
//! - **Nothing here can make Done fail.** Any failure is logged and the worktree is removed as before; a half-filled
//!   `.incoming` is deleted (dropping the store's [`Incoming`](super::store::Incoming) does that).

use std::path::{Component, Path, PathBuf};

use super::clone::move_tree;
use super::store::{Replaced, TrackedTimes, WarmBuilds, WarmRecord};
use crate::board::Board;
use crate::domain::TicketKey;
use crate::store::projects::Project;

/// Keeps the gitignored files of `worktree`, the clean worktree of `ticket` about to be removed, as the project's
/// warm build. Returns what the store did with them; `None` when nothing was kept (warm builds off, nothing
/// gitignored, or a failure, which is logged).
pub(crate) async fn keep(
    board: &Board,
    warm: &WarmBuilds,
    project: &Project,
    ticket: &TicketKey,
    worktree: &Path,
) -> Option<Replaced> {
    match try_keep(board, warm, project, ticket, worktree).await {
        Ok(replaced) => replaced,
        Err(error) => {
            tracing::warn!(
                ticket = %ticket,
                project = %project.key,
                error = %error,
                "the build isn't kept; the worktree is removed as before"
            );
            None
        }
    }
}

async fn try_keep(
    board: &Board,
    warm: &WarmBuilds,
    project: &Project,
    ticket: &TicketKey,
    worktree: &Path,
) -> Result<Option<Replaced>, String> {
    if !warm
        .enabled(&project.key)
        .await
        .map_err(|e| e.to_string())?
    {
        return Ok(None);
    }
    let settings = board
        .project_settings(&project.key)
        .await
        .map_err(|e| e.to_string())?;
    let env_files: Vec<PathBuf> = settings
        .env_files
        .iter()
        .map(|f| normal(Path::new(f.trim())))
        .filter(|f| !f.as_os_str().is_empty())
        .collect();
    let git = board
        .project_git(&project.key)
        .await
        .map_err(|e| e.to_string())?;
    let listed = git
        .run(
            worktree,
            &[
                "ls-files",
                "-z",
                "--others",
                "--ignored",
                "--exclude-standard",
                "--directory",
            ],
        )
        .await
        .map_err(|e| e.to_string())?;
    let paths = ignored_paths(&listed, &env_files);
    if paths.is_empty() {
        tracing::debug!(ticket = %ticket, "nothing gitignored to keep");
        return Ok(None);
    }
    let built_at = git
        .run(worktree, &["rev-parse", "HEAD"])
        .await
        .map_err(|e| e.to_string())?
        .trim()
        .to_owned();
    let base = git
        .run(worktree, &["merge-base", "HEAD", &project.base_branch])
        .await
        .map_err(|e| e.to_string())?
        .trim()
        .to_owned();
    let tracked: Vec<String> = git
        .run(worktree, &["ls-files", "-z"])
        .await
        .map_err(|e| e.to_string())?
        .split('\0')
        .filter(|f| !f.is_empty())
        .map(str::to_owned)
        .collect();
    // On an error from here on, dropping `incoming` deletes what was moved into it.
    let incoming = warm.incoming(project).await.map_err(|e| e.to_string())?;
    let (from, to) = (worktree.to_owned(), incoming.path().to_owned());
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        move_tree(&from, &to, Some(&paths)).map_err(|e| e.to_string())?;
        // An env file inside a moved folder came along: it doesn't stay.
        for file in &env_files {
            let moved = to.join(file);
            if std::fs::symlink_metadata(&moved).is_ok() {
                std::fs::remove_file(&moved).map_err(|e| format!("{}: {e}", moved.display()))?;
            }
        }
        TrackedTimes::measure(&from, &tracked)
            .write(&to)
            .map_err(|e| format!("{}: {e}", to.display()))
    })
    .await
    .map_err(|e| e.to_string())??;
    let record = WarmRecord {
        built_at,
        base,
        from: ticket.to_string(),
        kept_at: crate::store::now_ms(),
        project: String::new(),
    };
    warm.replace(project, incoming, record)
        .await
        .map(Some)
        .map_err(|e| e.to_string())
}

/// The paths of `git ls-files -z --others --ignored --directory` output, relative to the worktree: folders (listed
/// with a trailing `/`) as one entry, without what lies inside a folder already listed, and without the env files.
fn ignored_paths(listed: &str, env_files: &[PathBuf]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in listed.split('\0').filter(|e| !e.is_empty()) {
        let path = normal(Path::new(entry));
        if path.as_os_str().is_empty()
            || env_files.contains(&path)
            || paths.iter().any(|p| path.starts_with(p))
        {
            continue;
        }
        paths.push(path);
    }
    paths
}

/// `path` with only its plain parts (`./.env.local` and `.env.local` are the same file; `out/` is `out`).
fn normal(path: &Path) -> PathBuf {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_are_one_entry_and_env_files_stay_out() {
        let listed = ".env.local\0out/\0sub/\0sub/y.log\0x.log\0config/.env\0";
        let env = [normal(Path::new("./.env.local"))];
        assert_eq!(
            ignored_paths(listed, &env),
            ["out", "sub", "x.log", "config/.env"]
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_sibling_with_a_longer_name_is_its_own_entry() {
        let listed = "out/\0out-old/\0";
        assert_eq!(
            ignored_paths(listed, &[]),
            [PathBuf::from("out"), PathBuf::from("out-old")]
        );
    }
}
