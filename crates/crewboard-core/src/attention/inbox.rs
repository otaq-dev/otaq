//! The global "Needs you" inbox in the service (ATT-06, ATT-08; d0016, d0045, d0048): answering from it.
//!
//! **Answering** an Asks you or Idle item types the answer into the agent's session as the user's click (d0048): it
//! is never held, but unsent text in the input line asks *Keep it and cancel / Clear it and send* first. For an
//! **Interrupted** ticket that was waiting for an answer (d0045) the same click resumes the conversation and
//! delivers the answer in one step: the answer becomes the resumed conversation's first prompt, so it arrives once
//! the CLI is ready. The answer clears the orange state (ATT-02) and so the inbox item; an answered question is
//! recorded with its answer.

use crate::board::questions::{Choice, format_answer};
use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError, Refusal};
use crate::domain::{Actor, Status};
use crate::review::Review;
use crate::session::deliveries::{Deliveries, Delivery};
use crate::session::{SessionKind, Sessions};
use crate::signals::Signals;

/// The inbox's actions. Cheap to clone.
#[derive(Clone)]
pub struct Inbox {
    board: Board,
    sessions: Sessions,
    deliveries: Deliveries,
    signals: Signals,
    review: Review,
}

impl Inbox {
    pub fn new(
        board: Board,
        sessions: Sessions,
        deliveries: Deliveries,
        signals: Signals,
        review: Review,
    ) -> Self {
        Self {
            board,
            sessions,
            deliveries,
            signals,
            review,
        }
    }

    /// **Answer** an Asks you or Idle item (or an Interrupted one that was waiting for an answer). `clear` is the
    /// user's "Clear it and send" after [`Delivery::NeedsConfirmation`].
    pub async fn answer(
        &self,
        key: &str,
        choice: &Choice,
        clear: bool,
        actor: &Actor,
    ) -> Result<Delivery, BoardError> {
        let ticket = self.board.ticket(key).await?;
        let waiting = answerable(&ticket).ok_or_else(|| {
            BoardError::refused(
                Refusal::MoveRefused,
                format!("{} isn't waiting for an answer", ticket.key),
            )
        })?;
        let question = self.board.open_question(&ticket.id).await?;
        let text = answer_text(question.as_ref(), choice)?;
        let session = self.sessions.running_of(&ticket.id, SessionKind::Agent);
        let delivery = match (waiting, session) {
            (Waiting::Live, Some(session)) => {
                let typed = if clear {
                    self.deliveries
                        .clear_and_send(&session.id, Some(&text))
                        .await
                } else {
                    self.deliveries.deliver_from_click(&session.id, &text).await
                };
                let delivery =
                    typed.map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
                if matches!(delivery, Delivery::Sent { .. }) {
                    self.signals.answered(&ticket.id).await?;
                }
                delivery
            }
            (Waiting::Live, None) => {
                return Err(BoardError::refused(
                    Refusal::SessionEnded,
                    format!("{}'s session has ended", ticket.key),
                ));
            }
            (Waiting::Interrupted, _) => {
                self.review.resume_with_answer(key, &text, actor).await?;
                Delivery::Sent { count: 1 }
            }
        };
        if matches!(delivery, Delivery::Sent { .. })
            && let Some(question) = question
        {
            self.board.record_answer(&question.id, &text).await?;
        }
        tracing::info!(ticket = %ticket.key, ?delivery, "inbox answer");
        Ok(delivery)
    }
}

/// How an item waits for an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Waiting {
    /// Asks you or Idle, with its session.
    Live,
    /// Interrupted while it asked or was idle (d0045).
    Interrupted,
}

/// Whether the ticket takes an answer: Asks you and Idle (a permission prompt is answered in the terminal, and
/// simply asks again after a resume).
fn answerable(ticket: &Ticket) -> Option<Waiting> {
    let takes_answer = |s: Status| matches!(s, Status::AsksYou | Status::Idle);
    if takes_answer(ticket.status) {
        Some(Waiting::Live)
    } else if ticket.status == Status::Interrupted
        && ticket.interrupted_from.is_some_and(takes_answer)
    {
        Some(Waiting::Interrupted)
    } else {
        None
    }
}

/// The text typed for an answer: a question with options gets [`format_answer`]'s wording ("Answer to your
/// question: Postmark (option 1)."); otherwise the free text as written.
pub fn answer_text(
    question: Option<&crate::board::questions::HumanQuestion>,
    choice: &Choice,
) -> Result<String, BoardError> {
    let text = match (question.filter(|q| !q.options.is_empty()), choice) {
        (Some(q), Choice::Option(n)) if *n >= 1 && (*n as usize) <= q.options.len() => {
            format_answer(q, choice)
        }
        (_, Choice::Option(n)) => {
            return Err(BoardError::refused(
                Refusal::BadQuestion,
                format!("There is no option {n} to choose"),
            ));
        }
        (Some(q), Choice::Other(_)) => format_answer(q, choice),
        (None, Choice::Other(text)) => text.trim().to_owned(),
    };
    if text.trim().is_empty() || matches!(choice, Choice::Other(t) if t.trim().is_empty()) {
        return Err(BoardError::refused(
            Refusal::EmptyField,
            "Write the answer first",
        ));
    }
    Ok(text)
}
