//! Agent-to-agent questions (MSG-01…MSG-06; d0030, d0048). `crewboard ask <KEY> --stdin` works only between
//! directly linked tickets (one depends on the other) and never blocks the asker. Every question and answer is
//! stored in `messages` and logged on both tickets.
//!
//! - The target has a **live session**: the question is an automatic delivery, typed when its agent is idle and
//!   its input line is empty (d0048); it answers with `crewboard answer <ID> --stdin`.
//! - The target **never had a session**: the ask is refused ("SHOP-18 hasn't started yet …", MSG-04).
//! - The target has only a **stored conversation** (normally a merged parent): the question waits here until the
//!   answer-only resume (MSG-03) or the inbox (MSG-05) takes it.
//!
//! The answer goes back to the asker the same way, and "asked SHOP-40.1 · waiting" lasts until it was typed
//! there (MSG-06).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::board::tickets::{EventKind, Ticket, record_event};
use crate::board::{Board, BoardError, Refusal};
use crate::domain::{Actor, TicketKey, stored_enum};
use crate::naming;
use crate::service::EventBus;
use crate::session::deliveries::Deliveries;
use crate::session::{SessionKind, SessionTicket, Sessions};
use crate::store::{new_id, now_ms};

stored_enum! {
    /// How a question reaches its target (`messages.delivery`).
    pub enum Route ("message route") {
        /// Typed into the target's live session.
        Live = "live",
        /// The target's stored conversation was resumed answer-only (MSG-03).
        Resumed = "resumed",
        /// The user answers from the inbox (MSG-05).
        Inbox = "inbox",
    }
}

/// Where a question stands, for the Messages section and the cards (MSG-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageState {
    /// Waiting to be typed into the target (or for its answer-only resume).
    Asked,
    /// Typed into the target's session; no answer yet.
    Delivered,
    /// In the user's inbox (the target has no live session and finished tickets don't answer).
    InInbox,
    /// Answered; the answer waits to be typed into the asker's session.
    Answered,
    /// The answer reached the asker.
    AnswerDelivered,
}

/// One question between two linked tickets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub from: TicketKey,
    pub to: TicketKey,
    pub question: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Route>,
    pub asked_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivered_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answered_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer_delivered_at: Option<i64>,
}

impl Message {
    pub fn state(&self) -> MessageState {
        if self.answer_delivered_at.is_some() {
            MessageState::AnswerDelivered
        } else if self.answer.is_some() {
            MessageState::Answered
        } else if self.route == Some(Route::Inbox) {
            MessageState::InInbox
        } else if self.delivered_at.is_some() {
            MessageState::Delivered
        } else {
            MessageState::Asked
        }
    }
}

/// Which part of a message a delivery carries (`pending_deliveries.message_part`).
pub const PART_QUESTION: &str = "question";
pub const PART_ANSWER: &str = "answer";

/// Whether two tickets are directly linked: one depends on the other (MSG-01).
pub fn linked(a: &Ticket, b: &Ticket) -> bool {
    a.depends_on.contains(&b.key) || b.depends_on.contains(&a.key)
}

/// "SHOP-18 hasn't started yet, so there is no progress to ask about. Ask the human instead (`crewboard ask
/// human`)." (MSG-04).
pub fn never_started_message(target: &TicketKey) -> String {
    format!(
        "{target} hasn't started yet, so there is no progress to ask about. Ask the human instead (`{} ask human`).",
        naming::COMMAND_NAME
    )
}

/// What is typed into the target's session.
pub fn question_text(m: &Message) -> String {
    let cmd = naming::COMMAND_NAME;
    format!(
        "{}'s agent asks you (question {}): {}\n\nAnswer only this, with `{cmd} answer {} --stdin`, then carry on with \
         your own work.",
        m.from, m.id, m.question, m.id
    )
}

/// What is typed into the asker's session.
pub fn answer_text(m: &Message) -> String {
    format!(
        "{} answered your question {} (\u{201c}{}\u{201d}):\n\n{}",
        m.to,
        m.id,
        first_line(&m.question),
        m.answer.as_deref().unwrap_or_default()
    )
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

type MessageRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    i64,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

const SELECT_MESSAGES: &str = "SELECT m.id, m.from_ticket_id, m.to_ticket_id, m.question, m.answer, m.delivery, \
     m.asked_at, m.delivered_at, m.answered_at, m.answer_delivered_at FROM messages m";

impl Board {
    async fn message_from_row(&self, row: MessageRow) -> Result<Message, BoardError> {
        let (
            id,
            from,
            to,
            question,
            answer,
            route,
            asked_at,
            delivered_at,
            answered_at,
            answer_delivered_at,
        ) = row;
        Ok(Message {
            id,
            from: self.ticket_by_id(&from).await?.key,
            to: self.ticket_by_id(&to).await?.key,
            question,
            answer,
            route: route.and_then(|r| r.parse().ok()),
            asked_at,
            delivered_at,
            answered_at,
            answer_delivered_at,
        })
    }

    async fn messages_where(&self, filter: &str, id: &str) -> Result<Vec<Message>, BoardError> {
        let rows: Vec<MessageRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "{SELECT_MESSAGES} WHERE {filter} ORDER BY m.asked_at, m.id"
        )))
        .bind(id)
        .bind(id)
        .fetch_all(self.store().pool())
        .await?;
        let mut out = Vec::new();
        for row in rows {
            out.push(self.message_from_row(row).await?);
        }
        Ok(out)
    }

    /// A message by its ID.
    pub async fn message(&self, id: &str) -> Result<Message, BoardError> {
        self.messages_where("m.id = ? OR m.id = ?", id.trim())
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| BoardError::NotFound(format!("question {id}")))
    }

    /// Every question to or from a ticket, oldest first (the Messages section, MSG-01).
    pub async fn messages_of(&self, ticket_id: &str) -> Result<Vec<Message>, BoardError> {
        self.messages_where("m.from_ticket_id = ? OR m.to_ticket_id = ?", ticket_id)
            .await
    }

    /// The questions waiting for a ticket's answer (`crewboard questions`, prime).
    pub async fn pending_questions(&self, ticket_id: &str) -> Result<Vec<Message>, BoardError> {
        let key = self.ticket_by_id(ticket_id).await?.key;
        Ok(self
            .messages_of(ticket_id)
            .await?
            .into_iter()
            .filter(|m| m.answer.is_none() && m.to == key)
            .collect())
    }
}

/// Runs the questions between agents. Cheap to clone.
#[derive(Clone)]
pub struct Messages {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    sessions: Sessions,
    deliveries: Deliveries,
    events: EventBus,
}

impl Messages {
    pub fn new(board: Board, sessions: Sessions, deliveries: Deliveries, events: EventBus) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                sessions,
                deliveries,
                events,
            }),
        }
    }

    pub fn board(&self) -> &Board {
        &self.inner.board
    }

    /// `crewboard ask <TARGET> --stdin` from `asker`'s agent (MSG-01, MSG-02, MSG-04).
    pub async fn ask(
        &self,
        asker: &str,
        target: &str,
        question: &str,
        actor: &Actor,
    ) -> Result<Message, BoardError> {
        let board = &self.inner.board;
        let question = question.trim();
        if question.is_empty() {
            return Err(BoardError::refused(
                Refusal::BadQuestion,
                "Write the question on stdin",
            ));
        }
        let from = board.ticket(asker).await?;
        let to = board.ticket(target).await?;
        if from.id == to.id {
            return Err(BoardError::refused(
                Refusal::BadQuestion,
                format!("{} can't ask itself", from.key),
            ));
        }
        if !linked(&from, &to) {
            return Err(BoardError::refused(
                Refusal::NotAllowed,
                format!(
                    "{} and {} aren't linked: ask only a ticket yours depends on, or one that depends on yours. \
                     Ask the human instead (`{} ask human`).",
                    from.key,
                    to.key,
                    naming::COMMAND_NAME
                ),
            ));
        }
        let sessions = self
            .inner
            .sessions
            .history(&SessionTicket {
                id: to.id.clone(),
                key: to.key.clone(),
            })
            .await
            .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        if !sessions.iter().any(|s| s.kind == SessionKind::Agent) {
            return Err(BoardError::refused(
                Refusal::NotAllowed,
                never_started_message(&to.key),
            ));
        }
        let id = new_id();
        let now = now_ms();
        let live = self.inner.sessions.running_of(&to.id, SessionKind::Agent);
        let route = live.as_ref().map(|_| Route::Live);
        let mut tx = board.store().pool().begin().await?;
        sqlx::query(
            "INSERT INTO messages (id, from_ticket_id, to_ticket_id, question, delivery, asked_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&from.id)
        .bind(&to.id)
        .bind(question)
        .bind(route.map(|r| r.as_str()))
        .bind(now)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            &from.id,
            EventKind::QuestionAsked,
            actor,
            json!({ "id": id, "to": to.key, "question": question }),
        )
        .await?;
        record_event(
            &mut tx,
            &to.id,
            EventKind::QuestionAsked,
            actor,
            json!({ "id": id, "from": from.key, "question": question }),
        )
        .await?;
        tx.commit().await?;
        let message = board.message(&id).await?;
        if let Some(session) = live {
            self.inner
                .deliveries
                .queue_message(&session.id, &question_text(&message), &id, PART_QUESTION)
                .await
                .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        }
        tracing::info!(id, from = %from.key, to = %to.key, "question asked");
        self.changed(&[&from, &to]);
        board.message(&id).await
    }

    /// `crewboard answer <ID> --stdin` from the target's agent: stored, logged, and typed into the asker's session
    /// when it is idle with an empty input line.
    pub async fn answer(
        &self,
        id: &str,
        answer: &str,
        actor: &Actor,
    ) -> Result<Message, BoardError> {
        let board = &self.inner.board;
        let answer = answer.trim();
        if answer.is_empty() {
            return Err(BoardError::refused(
                Refusal::BadQuestion,
                "Write the answer on stdin",
            ));
        }
        let message = board.message(id).await?;
        if message.answer.is_some() {
            return Err(BoardError::refused(
                Refusal::BadQuestion,
                format!("Question {} was answered already", message.id),
            ));
        }
        let from = board.ticket(&message.from.to_string()).await?;
        let to = board.ticket(&message.to.to_string()).await?;
        let now = now_ms();
        let mut tx = board.store().pool().begin().await?;
        sqlx::query(
            "UPDATE messages SET answer = ?, answered_at = ?, delivered_at = COALESCE(delivered_at, ?) WHERE id = ?",
        )
        .bind(answer)
        .bind(now)
        .bind(now)
        .bind(&message.id)
        .execute(&mut *tx)
        .await?;
        for (ticket, other) in [(&from, &to.key), (&to, &from.key)] {
            record_event(
                &mut tx,
                &ticket.id,
                EventKind::QuestionAnswered,
                actor,
                json!({ "id": message.id, "with": other, "answer": answer }),
            )
            .await?;
        }
        tx.commit().await?;
        let answered = board.message(&message.id).await?;
        if let Some(session) = self.inner.sessions.running_of(&from.id, SessionKind::Agent) {
            self.inner
                .deliveries
                .queue_message(
                    &session.id,
                    &answer_text(&answered),
                    &answered.id,
                    PART_ANSWER,
                )
                .await
                .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        }
        tracing::info!(id = answered.id, from = %from.key, to = %to.key, "question answered");
        self.changed(&[&from, &to]);
        board.message(&answered.id).await
    }

    /// Clients refetch both tickets (their Messages sections and card flags).
    fn changed(&self, tickets: &[&Ticket]) {
        for t in tickets {
            self.inner
                .events
                .publish(crate::protocol::Event::TicketChanged {
                    project: t.key.project.to_string(),
                    id: t.id.clone(),
                    key: t.key.clone(),
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message() -> Message {
        Message {
            id: "0199".into(),
            from: "SHOP-20".parse().unwrap(),
            to: "SHOP-18".parse().unwrap(),
            question: "What does POST /checkout return on a declined card?".into(),
            answer: None,
            route: Some(Route::Live),
            asked_at: 1,
            delivered_at: None,
            answered_at: None,
            answer_delivered_at: None,
        }
    }

    #[test]
    fn states_follow_the_timestamps() {
        let mut m = message();
        assert_eq!(m.state(), MessageState::Asked);
        m.delivered_at = Some(2);
        assert_eq!(m.state(), MessageState::Delivered);
        m.answer = Some("402 with a code".into());
        assert_eq!(m.state(), MessageState::Answered);
        m.answer_delivered_at = Some(3);
        assert_eq!(m.state(), MessageState::AnswerDelivered);
        let mut inbox = message();
        inbox.route = Some(Route::Inbox);
        assert_eq!(inbox.state(), MessageState::InInbox);
    }

    #[test]
    fn texts() {
        let cmd = naming::COMMAND_NAME;
        assert_eq!(
            never_started_message(&"SHOP-18".parse().unwrap()),
            format!(
                "SHOP-18 hasn't started yet, so there is no progress to ask about. Ask the human instead (`{cmd} ask human`)."
            )
        );
        let text = question_text(&message());
        assert!(text.starts_with("SHOP-20's agent asks you (question 0199): What does POST"));
        assert!(text.contains(&format!("`{cmd} answer 0199 --stdin`")));
        let mut answered = message();
        answered.answer = Some("402 with {code}".into());
        assert_eq!(
            answer_text(&answered),
            "SHOP-18 answered your question 0199 (\u{201c}What does POST /checkout return on a declined card?\u{201d}):\n\n402 with {code}"
        );
    }
}
