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

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::board::review::EPIC_NO_SEND_BACK;
use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError, Refusal};
use crate::domain::{Actor, Status, TicketKey};
use crate::naming;
use crate::scheduler::Scheduler;
use crate::session::deliveries::{Deliveries, Delivery};
use crate::session::{SessionKind, Sessions};
use crate::start::{StartOutcome, Starter};

/// The skill Send back runs (REV-04).
pub const ADDRESS_REVIEW_SKILL: &str = "address-review";

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

/// Runs review actions. Cheap to clone.
#[derive(Clone)]
pub struct Review {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    sessions: Sessions,
    deliveries: Deliveries,
    starter: Starter,
    scheduler: Scheduler,
}

impl Review {
    pub fn new(
        board: Board,
        sessions: Sessions,
        deliveries: Deliveries,
        starter: Starter,
        scheduler: Scheduler,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
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
}

impl Return<'_> {
    fn skill(&self) -> &'static str {
        match self {
            Self::SendBack { .. } => ADDRESS_REVIEW_SKILL,
        }
    }
}

fn other(e: impl std::fmt::Display) -> BoardError {
    BoardError::refused(Refusal::SessionEnded, e.to_string())
}
