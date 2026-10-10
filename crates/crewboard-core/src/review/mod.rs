//! The user's review actions that reach a ticket's agent session (REV-04, GIT-03; d0034, d0038, d0048).
//!
//! **Send back** returns a ticket from Ready for Review straight to Dev with the round's comments and note
//! (d0034). The click is the go-ahead (d0018), so it is never held, except by the optional agent limit:
//! - the session is **live**: `/crewboard:address-review` is typed into it at once through the user-click path
//!   (unsent text in its input line answers "needs confirmation" first, d0048) and the ticket is Agent Working;
//! - the session is **gone**: the same click resumes the stored conversation, `claude --resume <id>`, in the
//!   ticket's worktree with the skill as its first prompt;
//! - the agent limit is **on and full**: the ticket waits in Queued; the scheduler starts it when a slot frees,
//!   and the skill is typed (or the conversation resumed) then.
//!
//! The agent then fetches the feedback itself with `crewboard review feedback` (REV-05).

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::board::conflicts::{Conflict, ConflictReason};
use crate::board::review::EPIC_NO_SEND_BACK;
use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError, Refusal};
use crate::data_dir::DataDir;
use crate::domain::{Actor, Status, TicketKey};
use crate::git::merge::{MergeOutcome, MergeRequest};
use crate::naming;
use crate::scheduler::Scheduler;
use crate::session::deliveries::{Deliveries, Delivery};
use crate::session::{SessionKind, Sessions};
use crate::start::{StartOutcome, Starter};

/// The skill Send back runs (REV-04).
pub const ADDRESS_REVIEW_SKILL: &str = "address-review";
/// The skill Send to agent to fix runs: rebase onto the target and resolve the conflicts (GIT-03).
pub const RESTACK_SKILL: &str = "restack";

/// The answer to a click that returns a ticket to its agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ReturnOutcome {
    /// The ticket is back in Dev: Agent Working with the skill typed or resumed, or Queued while the agent
    /// limit is full (or Start interrupted if the resume couldn't start).
    Done { ticket: Box<Ticket> },
    /// The session's input line has unsent text: ask "Keep it and cancel / Clear it and send" (d0048). Nothing
    /// was changed.
    NeedsConfirmation { ticket: TicketKey },
}

/// The answer to a Local merge click (GIT-03, REV-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum LocalMergeOutcome {
    /// Done · Merged into `target`; `started` are the waiting tickets the scheduler started after it.
    Merged {
        ticket: Box<Ticket>,
        target: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        started: Vec<TicketKey>,
    },
    /// "SHOP-19 has 2 uncommitted files" · View · Send back.
    RefusedUncommitted { files: Vec<String>, message: String },
    /// "`develop` and `origin/develop` have diverged. Push or pull, then retry".
    RefusedDiverged { message: String },
    /// "shop-web has uncommitted changes on main. Clean up and retry."
    RefusedDirtyCheckout { message: String },
    /// The free rebase conflicted: nothing was merged; the ticket stays in Ready for Review with Send to agent
    /// to fix.
    Conflict { target: String, files: Vec<String> },
}

/// Runs review actions. Cheap to clone.
#[derive(Clone)]
pub struct Review {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    data: DataDir,
    sessions: Sessions,
    deliveries: Deliveries,
    starter: Starter,
    scheduler: Scheduler,
    /// One Local merge at a time: they share the scratch worktree and move base branches.
    merging: tokio::sync::Mutex<()>,
}

impl Review {
    pub fn new(
        board: Board,
        data: DataDir,
        sessions: Sessions,
        deliveries: Deliveries,
        starter: Starter,
        scheduler: Scheduler,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                data,
                merging: tokio::sync::Mutex::new(()),
                sessions,
                deliveries,
                starter,
                scheduler,
            }),
        }
    }

    pub fn board(&self) -> &Board {
        &self.inner.board
    }

    /// **Send back** (REV-04): the pending comments and `note` go to the agent, which continues in Dev.
    /// `clear_first` is the user's "Clear it and send" after a "needs confirmation" answer.
    pub async fn send_back(
        &self,
        key: &str,
        note: &str,
        clear_first: bool,
        actor: &Actor,
    ) -> Result<ReturnOutcome, BoardError> {
        let ticket = self.inner.board.ticket(key).await?;
        if ticket.is_epic() {
            return Err(BoardError::refused(Refusal::MoveRefused, EPIC_NO_SEND_BACK));
        }
        if ticket.status != Status::ReadyForReview {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!(
                    "{} is {}; Send back works on a ticket in Ready for Review",
                    ticket.key,
                    ticket.status.label()
                ),
            ));
        }
        self.return_to_agent(&ticket, Return::SendBack { note }, clear_first, actor)
            .await
    }

    /// **Local merge** (GIT-03): the user's approve action that spends no tokens. `message` is the confirmed
    /// squash message ([`Board::merge_proposal`] proposes one).
    pub async fn local_merge(
        &self,
        key: &str,
        message: &str,
        actor: &Actor,
    ) -> Result<LocalMergeOutcome, BoardError> {
        let inner = &self.inner;
        let board = &inner.board;
        let _one_at_a_time = inner.merging.lock().await;
        let ticket = board.ticket(key).await?;
        if ticket.status != Status::ReadyForReview {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!(
                    "{} is {}; Local merge works on a ticket in Ready for Review",
                    ticket.key,
                    ticket.status.label()
                ),
            ));
        }
        if message.trim().is_empty() {
            return Err(BoardError::refused(
                Refusal::EmptyField,
                "Write the squash message first",
            ));
        }
        let target = board.merge_target(&ticket).await?;
        let project = board.project(ticket.key.project.as_str()).await?;
        let repo = PathBuf::from(&project.repo_path);
        let repo_name = repo
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| project.name.clone());
        let request = MergeRequest {
            scratch: inner.data.scratch_path(&repo_name).map_err(other)?,
            repo,
            project_name: project.name.clone(),
            key: ticket.key.to_string(),
            branch: target.branch.clone(),
            worktree: ticket
                .work
                .as_ref()
                .and_then(|w| w.worktree.as_ref())
                .map(PathBuf::from),
            target: target.target.clone(),
            sync_target: !target.into_epic,
            message: message.trim().to_owned(),
        };
        let outcome = board.git()?.local_merge(&request).await?;
        Ok(match outcome {
            MergeOutcome::Merged { commit, onto } => {
                let merged = board
                    .record_local_merge(&ticket, &target.target, &commit, &onto, actor)
                    .await?;
                // Waiting tickets whose last parent this was start now (T-0081).
                let started = match inner.scheduler.on_parent_merged(key).await {
                    Ok(plan) => plan.start.into_iter().map(|t| t.key).collect(),
                    Err(e) => {
                        tracing::warn!(ticket = %ticket.key, error = %e, "can't start the waiting tickets");
                        Vec::new()
                    }
                };
                LocalMergeOutcome::Merged {
                    ticket: Box::new(merged),
                    target: target.target,
                    started,
                }
            }
            MergeOutcome::RefusedUncommitted { files, message } => {
                LocalMergeOutcome::RefusedUncommitted { files, message }
            }
            MergeOutcome::RefusedDiverged { message } => {
                LocalMergeOutcome::RefusedDiverged { message }
            }
            MergeOutcome::RefusedDirtyCheckout { message } => {
                LocalMergeOutcome::RefusedDirtyCheckout { message }
            }
            MergeOutcome::Conflict { files } => {
                board
                    .set_conflict(
                        &ticket.id,
                        &Conflict {
                            target: target.target.clone(),
                            files: files.clone(),
                            reason: ConflictReason::Merge,
                        },
                    )
                    .await?;
                LocalMergeOutcome::Conflict {
                    target: target.target,
                    files,
                }
            }
        })
    }

    /// **Send to agent to fix** (GIT-03, d0038): a ticket whose branch conflicts with its target goes back to Dev
    /// with the `restack` skill, like Send back (the click spends the tokens).
    pub async fn send_to_fix(
        &self,
        key: &str,
        clear_first: bool,
        actor: &Actor,
    ) -> Result<ReturnOutcome, BoardError> {
        let board = &self.inner.board;
        let ticket = board.ticket(key).await?;
        let conflict = board.conflict(&ticket.id).await?;
        let target = match (&conflict, ticket.status) {
            (Some(c), Status::ReadyForReview) if c.reason == ConflictReason::Merge => {
                c.target.clone()
            }
            _ => {
                return Err(BoardError::refused(
                    Refusal::MoveRefused,
                    format!("{} has no conflict to fix in Ready for Review", ticket.key),
                ));
            }
        };
        self.return_to_agent(&ticket, Return::Fix { target: &target }, clear_first, actor)
            .await
    }

    /// Whether the optional agent limit is on and every slot is taken (d0016). Back-pressure doesn't hold a
    /// user's click.
    pub async fn limit_full(&self) -> Result<bool, BoardError> {
        let status = self.inner.scheduler.status().await?;
        Ok(status.limits.enabled && status.holding >= status.limits.max_concurrency.max(1))
    }

    /// The action's bookkeeping: moves the ticket to `to` (Agent Working or Queued) with the skill still to
    /// run, if any.
    async fn record(
        &self,
        ticket: &Ticket,
        action: &Return<'_>,
        to: Status,
        skill: Option<&str>,
        actor: &Actor,
    ) -> Result<(), BoardError> {
        let board = &self.inner.board;
        match action {
            Return::SendBack { note } => board
                .record_send_back(ticket, note, to, skill, actor)
                .await
                .map(|_| ()),
            Return::Fix { target } => {
                board
                    .record_send_to_fix(ticket, target, to, skill, actor)
                    .await
            }
        }
    }

    /// Returns a ticket to its agent with the action's skill: types it into the live session, resumes the
    /// conversation with it, or queues the ticket while the limit is full.
    async fn return_to_agent(
        &self,
        ticket: &Ticket,
        action: Return<'_>,
        clear_first: bool,
        actor: &Actor,
    ) -> Result<ReturnOutcome, BoardError> {
        let inner = &self.inner;
        let skill = action.skill();
        let full = self.limit_full().await?;
        let live = inner.sessions.running_of(&ticket.id, SessionKind::Agent);
        if !full
            && !clear_first
            && let Some(session) = &live
            && inner
                .sessions
                .has_unsent_input(&session.id)
                .map_err(other)?
        {
            return Ok(ReturnOutcome::NeedsConfirmation {
                ticket: ticket.key.clone(),
            });
        }
        let key = ticket.key.to_string();
        match (&live, full) {
            (_, true) => {
                self.record(ticket, &action, Status::Queued, Some(skill), actor)
                    .await?;
                inner.scheduler.wake();
            }
            (Some(session), false) => {
                self.record(ticket, &action, Status::Working, None, actor)
                    .await?;
                let text = naming::skill(skill);
                let typed = if clear_first {
                    inner
                        .deliveries
                        .clear_and_send(&session.id, Some(&text))
                        .await
                } else {
                    inner
                        .deliveries
                        .deliver_from_click(&session.id, &text)
                        .await
                };
                match typed.map_err(other)? {
                    Delivery::Sent { .. } => {}
                    // Text was typed in the moment between the check and the send: it stays as the user typed it.
                    Delivery::NeedsConfirmation { .. } => tracing::warn!(
                        ticket = %ticket.key,
                        "unsent text appeared before the skill was typed"
                    ),
                }
            }
            (None, false) => {
                self.record(ticket, &action, Status::Queued, Some(skill), actor)
                    .await?;
                match inner.starter.start(&key, actor).await? {
                    StartOutcome::Started(_) | StartOutcome::Skipped => {}
                    StartOutcome::Held { message } | StartOutcome::Interrupted { message, .. } => {
                        tracing::warn!(ticket = %ticket.key, %message, "resuming the agent stopped");
                    }
                }
            }
        }
        Ok(ReturnOutcome::Done {
            ticket: Box::new(inner.board.ticket(&key).await?),
        })
    }
}

/// What returns a ticket to its agent.
enum Return<'a> {
    SendBack { note: &'a str },
    Fix { target: &'a str },
}

impl Return<'_> {
    fn skill(&self) -> &'static str {
        match self {
            Self::SendBack { .. } => ADDRESS_REVIEW_SKILL,
            Self::Fix { .. } => RESTACK_SKILL,
        }
    }
}

fn other(e: impl std::fmt::Display) -> BoardError {
    BoardError::refused(Refusal::SessionEnded, e.to_string())
}
