//! Finished tickets answer automatically (MSG-03; d0030, the one explicit exception to d0018's "nothing launches
//! an agent without a click"). A question to a ticket with no live session but a stored conversation (normally a
//! merged parent), with "Let finished tickets answer questions" on: Otaq recreates the worktree **at the same
//! path** on the ticket's kept branch, resumes the conversation answer-only on the tool it ran on, with the
//! answer-question skill as its first prompt (session kind `answer`), and once `otaq answer` arrives ends the session and
//! removes the worktree, unless files changed there (then it is kept and listed in Cleanup). While the plan limit is
//! active the question waits.

use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::watch;

use super::{Messages, PART_QUESTION, Route, question_text};
use crate::agent_tools::{FirstPrompt, LaunchRequest, LaunchRole};
use crate::board::tickets::Ticket;
use crate::board::{BoardError, Refusal};
use crate::naming;
use crate::session::{
    SessionInfo, SessionKind, SessionTicket, SpawnSpec, StoredConversation, TerminalSize,
};
use crate::store::now_ms;

/// The skill an answer session starts with.
pub const ANSWER_SKILL: &str = "answer-question";

/// How often waiting questions are looked at again (a limit that reset, a target that came back).
pub const ROUTE_TICK: Duration = Duration::from_secs(30);

const INITIAL_SIZE: TerminalSize = TerminalSize {
    rows: 40,
    cols: 120,
};

/// "SHOP-18's conversation is no longer available" (MSG-04).
pub fn no_longer_available(key: &crate::domain::TicketKey) -> String {
    format!(
        "{key}'s conversation is no longer available. Ask the human instead (`{} ask human`).",
        naming::COMMAND_NAME
    )
}

impl Messages {
    /// The target's stored conversation, if it can be resumed: the newest agent session's tool, tool session id
    /// and model, which its tool's adapter says is still there to resume.
    pub(super) async fn stored_conversation(
        &self,
        target: &Ticket,
    ) -> Result<Option<StoredConversation>, BoardError> {
        let history = self
            .inner
            .sessions
            .history(&SessionTicket {
                id: target.id.clone(),
                key: target.key.clone(),
            })
            .await
            .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        let Some(conversation) = history
            .iter()
            .filter(|s| s.kind == SessionKind::Agent)
            .find_map(StoredConversation::of)
        else {
            return Ok(None);
        };
        let cwd = self.worktree_path(target).await?;
        let adapter = self.inner.adapters.get(conversation.tool);
        Ok(adapter
            .can_resume(&conversation.id, &cwd)
            .then_some(conversation))
    }

    /// The ticket's worktree folder, where its conversation ran: `<data>/wt/<repo>/<KEY>`.
    async fn worktree_path(&self, ticket: &Ticket) -> Result<PathBuf, BoardError> {
        let board = &self.inner.board;
        let project = board.project(ticket.key.project.as_str()).await?;
        let repo = PathBuf::from(&project.repo_path);
        let repo_name = repo
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| project.name.clone());
        self.inner
            .data
            .worktree_path(&repo_name, &ticket.key.to_string())
            .map_err(|e| BoardError::refused(Refusal::NoWorktree, e.to_string()))
    }

    /// Whether the plan limit holds new agent work (USG-03): the question waits.
    async fn limit_active(&self) -> bool {
        crate::usage::plan::latest(self.inner.board.store())
            .await
            .ok()
            .flatten()
            .is_some_and(|p| p.limit_active())
    }

    /// Takes the questions that wait for a target without a live session: typed into a target that is live again,
    /// put in the inbox when finished tickets don't answer (MSG-05), or answered by resuming the target (MSG-03).
    pub async fn route_waiting(&self) -> Result<(), BoardError> {
        let _one = self.inner.routing.lock().await;
        let board = &self.inner.board;
        let waiting: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, to_ticket_id FROM messages WHERE delivery IS NULL AND answer IS NULL ORDER BY asked_at, id",
        )
        .fetch_all(board.store().pool())
        .await?;
        let mut busy: Vec<String> = Vec::new();
        for (id, target_id) in waiting {
            if busy.contains(&target_id) {
                continue;
            }
            let target = board.ticket_by_id(&target_id).await?;
            let message = board.message(&id).await?;
            let sessions = &self.inner.sessions;
            if let Some(live) = sessions.running_of(&target.id, SessionKind::Agent) {
                self.set_route(&id, Route::Live).await?;
                self.inner
                    .deliveries
                    .queue_message(&live.id, &question_text(&message), &id, PART_QUESTION)
                    .await
                    .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
                continue;
            }
            // One answer session per ticket at a time; the next question waits for it.
            if sessions
                .running_of(&target.id, SessionKind::Answer)
                .is_some()
            {
                busy.push(target_id);
                continue;
            }
            let settings = board.project_settings(target.key.project.as_str()).await?;
            if !settings.let_finished_tickets_answer {
                self.set_route(&id, Route::Inbox).await?;
                self.changed_message(&message).await;
                continue;
            }
            if self.limit_active().await {
                continue;
            }
            busy.push(target_id);
            match self.start_answer_session(&target).await {
                Ok(_) => {
                    sqlx::query(
                        "UPDATE messages SET delivery = 'resumed', delivered_at = ? WHERE id = ?",
                    )
                    .bind(now_ms())
                    .bind(&id)
                    .execute(board.store().pool())
                    .await?;
                    self.changed_message(&message).await;
                }
                Err(e) => {
                    tracing::warn!(id, target = %target.key, error = %e, "can't resume the target to answer")
                }
            }
        }
        Ok(())
    }

    /// **Resume SHOP-17** on a question sent to the user (MSG-05): the user's click lets the finished ticket answer
    /// it itself, as MSG-03 would with the setting on. The plan limit still holds it.
    pub async fn resume_target(&self, id: &str) -> Result<super::Message, BoardError> {
        let board = &self.inner.board;
        let message = board.message(id).await?;
        if message.answer.is_some() {
            return Err(BoardError::refused(
                Refusal::BadQuestion,
                format!("Question {} was answered already", message.id),
            ));
        }
        let target = board.ticket(&message.to.to_string()).await?;
        if self.limit_active().await {
            return Err(BoardError::refused(
                Refusal::NotAllowed,
                format!(
                    "The usage limit is active: {} answers once it resets",
                    target.key
                ),
            ));
        }
        if let Some(live) = self
            .inner
            .sessions
            .running_of(&target.id, SessionKind::Agent)
        {
            self.set_route(id, Route::Live).await?;
            self.inner
                .deliveries
                .queue_message(&live.id, &question_text(&message), id, PART_QUESTION)
                .await
                .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        } else {
            if self.stored_conversation(&target).await?.is_none() {
                return Err(BoardError::refused(
                    Refusal::NotAllowed,
                    no_longer_available(&target.key),
                ));
            }
            self.start_answer_session(&target).await?;
            sqlx::query("UPDATE messages SET delivery = 'resumed', delivered_at = ? WHERE id = ?")
                .bind(now_ms())
                .bind(id)
                .execute(board.store().pool())
                .await?;
        }
        self.changed_message(&message).await;
        board.message(id).await
    }

    async fn set_route(&self, id: &str, route: Route) -> Result<(), BoardError> {
        sqlx::query("UPDATE messages SET delivery = ? WHERE id = ?")
            .bind(route.as_str())
            .bind(id)
            .execute(self.inner.board.store().pool())
            .await?;
        Ok(())
    }

    /// Recreates the worktree at its path on the kept branch and resumes the conversation answer-only.
    async fn start_answer_session(&self, target: &Ticket) -> Result<SessionInfo, BoardError> {
        let inner = &self.inner;
        let board = &inner.board;
        inner.adapters.unavailable(None)?;
        let conversation = self.stored_conversation(target).await?.ok_or_else(|| {
            BoardError::refused(Refusal::SessionEnded, no_longer_available(&target.key))
        })?;
        let row = board
            .store()
            .ticket(&target.id)
            .await?
            .ok_or_else(|| BoardError::NotFound(target.key.to_string()))?;
        let branch = row.branch_name.ok_or_else(|| {
            BoardError::refused(
                Refusal::NoBranch,
                format!("{} has no branch to answer from", target.key),
            )
        })?;
        let project = board.project(target.key.project.as_str()).await?;
        let repo = PathBuf::from(&project.repo_path);
        let path = self.worktree_path(target).await?;
        crate::start::worktree::add_worktree(board.git()?, &repo, &path, &branch)
            .await
            .map_err(|e| BoardError::refused(Refusal::NoWorktree, e.to_string()))?;
        sqlx::query("UPDATE tickets SET worktree_path = ? WHERE id = ?")
            .bind(path.display().to_string())
            .bind(&target.id)
            .execute(board.store().pool())
            .await?;
        let launched = inner.adapters.launch(
            Some(conversation.tool),
            &LaunchRequest {
                role: LaunchRole::Answer,
                cwd: path,
                project: project.key.clone(),
                ticket: Some(target.key.clone()),
                epic: None,
                port: 0,
                first_prompt: Some(FirstPrompt::OtaqSkill {
                    name: ANSWER_SKILL.to_owned(),
                    message: None,
                }),
                resume: Some(conversation.id),
                model: conversation.model,
            },
        )?;
        let session = inner
            .sessions
            .spawn(SpawnSpec {
                kind: SessionKind::Answer,
                project_id: project.id,
                project_key: project.key,
                ticket: Some(SessionTicket {
                    id: target.id.clone(),
                    key: target.key.clone(),
                }),
                program: launched.plan.program,
                args: launched.plan.args,
                cwd: launched.plan.cwd,
                env: launched.plan.env,
                size: INITIAL_SIZE,
                tool: Some(launched.tool),
                tool_session_id: launched.tool_session_id,
                model: launched.model,
            })
            .await
            .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        tracing::info!(key = %target.key, session = session.id, "resumed to answer a question");
        Ok(session)
    }

    /// After the answer: the answer session ends and its worktree goes, unless files changed there (kept, listed in
    /// Cleanup).
    pub(super) async fn end_answer_session(&self, target: &Ticket) -> Result<(), BoardError> {
        let inner = &self.inner;
        let Some(session) = inner.sessions.running_of(&target.id, SessionKind::Answer) else {
            return Ok(());
        };
        inner
            .sessions
            .stop(&session.id)
            .await
            .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        let board = &inner.board;
        let path = self.worktree_path(target).await?;
        if !path.is_dir() {
            return Ok(());
        }
        let git = board.git()?;
        let dirty = git.uncommitted_files(&path).await?;
        if dirty.is_empty() {
            let project = board.project(target.key.project.as_str()).await?;
            crate::review::finish::remove_worktree(
                &board.project_git(&project.key).await?,
                std::path::Path::new(&project.repo_path),
                &path,
            )
            .await?;
            sqlx::query(
                "UPDATE tickets SET worktree_path = NULL, worktree_kept = FALSE WHERE id = ?",
            )
            .bind(&target.id)
            .execute(board.store().pool())
            .await?;
        } else {
            sqlx::query("UPDATE tickets SET worktree_kept = TRUE WHERE id = ?")
                .bind(&target.id)
                .execute(board.store().pool())
                .await?;
            tracing::info!(key = %target.key, files = dirty.len(), "answer worktree kept: files changed");
        }
        Ok(())
    }

    /// Looks at waiting questions every [`ROUTE_TICK`] until the service stops.
    pub async fn run(self, mut stop: watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(ROUTE_TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = stop.wait_for(|s| *s) => return,
                _ = tick.tick() => {}
            }
            if let Err(e) = self.route_waiting().await {
                tracing::warn!(error = %e, "can't route the waiting questions");
            }
        }
    }
}
