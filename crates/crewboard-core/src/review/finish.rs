//! Done cleanup (REC-05; d0018, d0035, d0026): when a ticket reaches Done (Local merge, Close ticket, later Mark
//! merged), pure bookkeeping follows without asking, because the user's click finished it: its agent session
//! ends, its **ports return to the pool**, its worktree is **removed when clean** (the branch is kept), and a
//! worktree with uncommitted changes is **kept** and flagged for the Cleanup view. Run and Shell (M9) stop
//! through the [`on_ticket_finished`](super::Review::on_ticket_finished) hook.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::Review;
use crate::board::tickets::Ticket;
use crate::board::{BoardError, Refusal};
use crate::domain::TicketKey;
use crate::session::SessionKind;

/// A finished ticket, for the hooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedTicket {
    pub id: String,
    pub key: TicketKey,
}

/// Called for every finished ticket (Run stops with a toast and the Shell closes from M9).
pub type FinishedHook = Arc<dyn Fn(&FinishedTicket) + Send + Sync>;

/// What the cleanup did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cleanup {
    pub session_stopped: bool,
    pub worktree_removed: bool,
    /// The worktree was kept: these files aren't committed.
    pub kept_dirty: Vec<String>,
}

impl Review {
    /// Registers a hook run for every ticket that reaches Done.
    pub fn on_ticket_finished(&self, hook: FinishedHook) {
        self.inner
            .finished_hooks
            .lock()
            .expect("finished hooks")
            .push(hook);
    }

    /// The cleanup after Done (REC-05); also used by Close ticket.
    pub async fn finish(&self, ticket: &Ticket) -> Result<Cleanup, BoardError> {
        let inner = &self.inner;
        let mut cleanup = Cleanup::default();
        if let Some(session) = inner.sessions.running_of(&ticket.id, SessionKind::Agent) {
            inner
                .sessions
                .stop(&session.id)
                .await
                .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
            cleanup.session_stopped = true;
        }
        let finished = FinishedTicket {
            id: ticket.id.clone(),
            key: ticket.key.clone(),
        };
        let hooks = inner.finished_hooks.lock().expect("finished hooks").clone();
        for hook in hooks {
            hook(&finished);
        }
        inner
            .ports
            .free(&ticket.id)
            .await
            .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        let worktree = ticket
            .work
            .as_ref()
            .and_then(|w| w.worktree.clone())
            .map(PathBuf::from)
            .filter(|w| w.is_dir());
        if let Some(worktree) = worktree {
            let board = &inner.board;
            let git = board.git()?;
            let dirty = git.uncommitted_files(&worktree).await?;
            if dirty.is_empty() {
                let project = board.project(ticket.key.project.as_str()).await?;
                remove_worktree(git, Path::new(&project.repo_path), &worktree).await?;
                sqlx::query(
                    "UPDATE tickets SET worktree_path = NULL, worktree_kept = FALSE WHERE id = ?",
                )
                .bind(&ticket.id)
                .execute(board.store().pool())
                .await?;
                cleanup.worktree_removed = true;
            } else {
                sqlx::query("UPDATE tickets SET worktree_kept = TRUE WHERE id = ?")
                    .bind(&ticket.id)
                    .execute(board.store().pool())
                    .await?;
                tracing::info!(ticket = %ticket.key, files = dirty.len(), "worktree kept: uncommitted changes");
                cleanup.kept_dirty = dirty;
            }
        }
        Ok(cleanup)
    }
}

/// `git worktree remove` (it refuses a worktree with changes by itself, a second guard); the branch stays.
async fn remove_worktree(
    git: &crate::git::Git,
    repo: &Path,
    worktree: &Path,
) -> Result<(), BoardError> {
    let path = worktree.display().to_string();
    git.run(repo, &["worktree", "remove", &path]).await?;
    Ok(())
}
