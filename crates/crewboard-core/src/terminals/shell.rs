//! The Shell tab (RUN-05; d0035, d0026) and the Project terminal (RUN-06; d0019, d0031): plain terminals running
//! the user's login shell, owned by the service (sessions of kind `shell` and `project`), so they survive closing
//! the window. Neither starts Claude; callers inside them are the user, with full `crewboard` access (CLI-03).
//!
//! - **Shell**: in the ticket's worktree, with `$PORT` = the Run port and `$CREWBOARD_TICKET` set. It starts the
//!   first time the tab opens and closes when the ticket reaches Done or is closed.
//! - **Project terminal**: one per project, in the project's own folder (the user's checkout), no ticket. It ends
//!   only when the user exits the shell, or with a restart.
//!
//! Opening a terminal again shows the one there is: running, or ended (with its output while the service keeps
//! it). A terminal a restart stopped stays "stopped by restart" until the user's Reopen click (d0045); opening the
//! tab never restarts it.

use std::path::PathBuf;

use super::TerminalEnv;
use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError, Refusal};
use crate::data_dir::DataDir;
use crate::review::finish::FinishedTicket;
use crate::session::{
    END_STOPPED_BY_RESTART, SessionInfo, SessionKind, SessionTicket, Sessions, SpawnSpec,
    TerminalSize,
};

/// The size a terminal starts with; the app resizes it.
const START_SIZE: TerminalSize = TerminalSize {
    cols: 120,
    rows: 32,
};

/// Opens Shells and Project terminals. Cheap to clone.
#[derive(Clone)]
pub struct Shells {
    board: Board,
    sessions: Sessions,
    data: DataDir,
    env: TerminalEnv,
}

impl Shells {
    pub fn new(board: Board, sessions: Sessions, data: DataDir, env: TerminalEnv) -> Self {
        Self {
            board,
            sessions,
            data,
            env,
        }
    }

    /// The ticket's Shell: the running one, the last one (ended, or stopped by a restart), or a new one on first
    /// use. `reopen` (the user's click) starts a new one unless one runs.
    pub async fn open_shell(&self, key: &str, reopen: bool) -> Result<SessionInfo, BoardError> {
        let ticket = self.board.ticket(key).await?;
        if let Some(running) = self.sessions.running_of(&ticket.id, SessionKind::Shell) {
            return Ok(running);
        }
        let worktree = ticket
            .work
            .as_ref()
            .and_then(|w| w.worktree.clone())
            .map(PathBuf::from)
            .filter(|w| w.is_dir());
        let Some(worktree) = worktree else {
            return Err(BoardError::refused(
                Refusal::NoWorktree,
                format!(
                    "{} has no worktree. The Shell opens once the ticket has started, until it is done.",
                    ticket.key
                ),
            ));
        };
        if !reopen {
            if let Some(last) = self.sessions.latest_of(&ticket.id, SessionKind::Shell) {
                return Ok(last);
            }
            if let Some(stopped) = self
                .stopped_by_restart(
                    "SELECT id FROM sessions WHERE ticket_id = ? AND kind = 'shell'
                     ORDER BY started_at DESC, id DESC LIMIT 1",
                    &ticket.id,
                )
                .await?
            {
                return Ok(stopped);
            }
        }
        self.spawn_shell(&ticket, worktree).await
    }

    /// The project's Project terminal, like [`open_shell`](Self::open_shell): in the project's folder, no ticket.
    pub async fn open_project(
        &self,
        project: &str,
        reopen: bool,
    ) -> Result<SessionInfo, BoardError> {
        let project = self.board.project(project).await?;
        let last = self.sessions.latest_project_terminal(&project.key);
        if let Some(running) = last.clone().filter(SessionInfo::is_running) {
            return Ok(running);
        }
        if !reopen {
            if let Some(last) = last {
                return Ok(last);
            }
            if let Some(stopped) = self
                .stopped_by_restart(
                    "SELECT id FROM sessions WHERE project_id = ? AND kind = 'project' AND ticket_id IS NULL
                     ORDER BY started_at DESC, id DESC LIMIT 1",
                    &project.id,
                )
                .await?
            {
                return Ok(stopped);
            }
        }
        let info = self
            .sessions
            .spawn(SpawnSpec {
                kind: SessionKind::Project,
                project_id: project.id.clone(),
                project_key: project.key.clone(),
                ticket: None,
                program: self.env.shell.clone(),
                args: self.env.shell_args.clone(),
                cwd: PathBuf::from(&project.repo_path),
                env: self.env.env(&self.data, None, None),
                size: START_SIZE,
                claude_session_id: None,
            })
            .await
            .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        tracing::info!(project = %project.key, "project terminal opened");
        Ok(info)
    }

    /// A ticket reached Done or was closed: its Shell closes (RUN-05).
    pub async fn finished(&self, ticket: &FinishedTicket) {
        if let Some(shell) = self.sessions.running_of(&ticket.id, SessionKind::Shell)
            && let Err(e) = self.sessions.stop(&shell.id).await
        {
            tracing::warn!(ticket = %ticket.key, error = %e, "can't close the shell");
        }
    }

    async fn spawn_shell(
        &self,
        ticket: &Ticket,
        worktree: PathBuf,
    ) -> Result<SessionInfo, BoardError> {
        let project = self.board.project(ticket.key.project.as_str()).await?;
        let port = ticket.work.as_ref().and_then(|w| w.run_port);
        let info = self
            .sessions
            .spawn(SpawnSpec {
                kind: SessionKind::Shell,
                project_id: project.id.clone(),
                project_key: project.key.clone(),
                ticket: Some(SessionTicket {
                    id: ticket.id.clone(),
                    key: ticket.key.clone(),
                }),
                program: self.env.shell.clone(),
                args: self.env.shell_args.clone(),
                cwd: worktree,
                env: self.env.env(&self.data, Some(&ticket.key), port),
                size: START_SIZE,
                claude_session_id: None,
            })
            .await
            .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        tracing::info!(ticket = %ticket.key, "shell opened");
        Ok(info)
    }

    /// The newest stored terminal (by `sql` with one id bound) when a restart stopped it.
    async fn stopped_by_restart(
        &self,
        sql: &'static str,
        id: &str,
    ) -> Result<Option<SessionInfo>, BoardError> {
        let newest: Option<(String,)> = sqlx::query_as(sql)
            .bind(id)
            .fetch_optional(self.board.store().pool())
            .await?;
        let Some((session,)) = newest else {
            return Ok(None);
        };
        Ok(self
            .sessions
            .stored(&session)
            .await
            .ok()
            .filter(|s| s.end_reason.as_deref() == Some(END_STOPPED_BY_RESTART)))
    }
}
