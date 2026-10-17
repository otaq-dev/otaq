//! Typing into live sessions (SES-06, SES-07; d0048).
//!
//! **Automatic deliveries** (another agent's question or answer, the ticket-change note, "continue" after the
//! limit reset) are stored in `pending_deliveries` and typed only when the agent is **idle between turns and
//! the input line is empty**. Until then the card and the terminal strip show "1 message waiting · Deliver now".
//!
//! **The user's own clicks** (Deliver now, Send back, an inbox answer) are never held: if the input line has
//! unsent text the request answers [`Delivery::NeedsConfirmation`] instead, and the client asks *"SHOP-23 has
//! unsent text in its terminal"* → **Keep it and cancel** (nothing is sent) / **Clear it and send**
//! ([`Deliveries::clear_and_send`]). Otaq never clears the user's text silently.
//!
//! "Idle between turns" comes from hook signals (T-0067): the service passes its [`crate::signals::Signals`],
//! whose answer is "the agent's last hook ended its turn". [`NeverIdle`] stays for tests.
//!
//! How the line is cleared and how long Enter waits after the text are the session's agent tool's input quirks
//! (ADP-06, [`crate::agent_tools::InputQuirks`]).

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{SessionError, Sessions};
use crate::agent_tools::{Adapters, InputQuirks};
use crate::domain::{TicketKey, stored_enum};
use crate::protocol::Event;
use crate::service::EventBus;
use crate::store::{Store, StoreError, new_id, now_ms};

stored_enum! {
    /// What an automatic delivery carries (the `pending_deliveries.kind` column).
    pub enum DeliveryKind ("delivery kind") {
        /// Another agent's question or answer (d0030, M10).
        AgentMessage = "agent_message",
        /// What the user changed in the ticket while the agent works (SES-08, M4).
        TicketChange = "ticket_change",
        /// "continue" after the plan limit resets (d0046, M8).
        ContinueAfterLimit = "continue_after_limit",
        /// A skill a user's click asked for (Send back's address-review) while the agent limit was full: typed
        /// once the agent has a slot and its input line is empty (d0034).
        Skill = "skill",
        /// The validator's findings, or "Validation was cancelled…", for the agent (d0063).
        Validation = "validation",
        /// A wake-up for an epic's lead: a child merged, was closed, asked or got stuck (d0083, LEAD-04).
        LeadWake = "lead_wake",
        /// The lead's unread notes, typed in when the child goes idle (LEAD-10, d0083).
        LeadNote = "lead_note",
    }
}

/// Whether an agent is idle between turns. Hook signals answer this from M4.
pub trait IdleSignal: Send + Sync {
    fn is_idle(&self, session: &str) -> bool;

    /// Messages were typed into the session (they start the agent's next turn).
    fn delivered(&self, _session: &str) {}
}

/// Never idle: automatic deliveries are held until the user clicks Deliver now (tests).
pub struct NeverIdle;

impl IdleSignal for NeverIdle {
    fn is_idle(&self, _session: &str) -> bool {
        false
    }
}

/// The answer to a user's click that types into a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Delivery {
    /// Typed and submitted; `count` messages.
    Sent { count: usize },
    /// The input line has unsent text: ask "Keep it and cancel / Clear it and send".
    NeedsConfirmation {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ticket: Option<TicketKey>,
    },
}

/// A message waiting to be typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct PendingDelivery {
    pub id: String,
    pub session_id: String,
    pub kind: String,
    pub text: String,
    pub created_at: i64,
    /// The agent-to-agent message it carries, if any, and which part (`question` / `answer`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_part: Option<String>,
}

/// Lets the program take a typed message in before Enter submits it (a tool may ask for a longer pause), and
/// a cleared line settle before the text.
const SUBMIT_PAUSE: Duration = Duration::from_millis(80);

/// Queues and types deliveries. Cheap to clone.
#[derive(Clone)]
pub struct Deliveries {
    store: Store,
    sessions: Sessions,
    events: EventBus,
    idle: Arc<dyn IdleSignal>,
    /// Each session's agent tool's input quirks.
    adapters: Adapters,
}

impl Deliveries {
    pub fn new(
        store: Store,
        sessions: Sessions,
        events: EventBus,
        idle: Arc<dyn IdleSignal>,
        adapters: Adapters,
    ) -> Self {
        Self {
            store,
            sessions,
            events,
            idle,
            adapters,
        }
    }

    /// Stores an automatic delivery, tells clients the waiting count, and types it right away if the session
    /// is idle with an empty input line.
    pub async fn queue_delivery(
        &self,
        session: &str,
        kind: DeliveryKind,
        text: &str,
    ) -> Result<(), SessionError> {
        self.hold_delivery(session, kind, text).await?;
        self.deliver_if_ready(session).await.map(|_| ())
    }

    /// Stores an automatic delivery without typing it, even if the session is idle: it waits for the plan's usage
    /// limit to pass (the limit's Resume types it with [`deliver_if_ready`](Self::deliver_if_ready)).
    pub async fn hold_delivery(
        &self,
        session: &str,
        kind: DeliveryKind,
        text: &str,
    ) -> Result<(), SessionError> {
        sqlx::query(
            "INSERT INTO pending_deliveries (id, session_id, kind, text, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(new_id())
        .bind(session)
        .bind(kind.as_str())
        .bind(text)
        .bind(now_ms())
        .execute(self.store.pool())
        .await
        .map_err(store_error)?;
        self.publish_waiting(session).await
    }

    /// Like [`queue_delivery`](Self::queue_delivery) for an agent-to-agent question or answer: once it is typed,
    /// the message records that it reached the other agent (MSG-02, MSG-06).
    pub async fn queue_message(
        &self,
        session: &str,
        text: &str,
        message_id: &str,
        part: &str,
    ) -> Result<(), SessionError> {
        sqlx::query(
            "INSERT INTO pending_deliveries (id, session_id, kind, text, created_at, message_id, message_part)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(new_id())
        .bind(session)
        .bind(DeliveryKind::AgentMessage.as_str())
        .bind(text)
        .bind(now_ms())
        .bind(message_id)
        .bind(part)
        .execute(self.store.pool())
        .await
        .map_err(store_error)?;
        self.publish_waiting(session).await?;
        self.deliver_if_ready(session).await.map(|_| ())
    }

    /// Types every pending delivery, oldest first, if the agent is idle **and** the input line is empty.
    /// Called when something may have made that true (a queued delivery, an Enter, a hook signal later).
    /// Returns how many were sent.
    pub async fn deliver_if_ready(&self, session: &str) -> Result<usize, SessionError> {
        if !self.is_ready(session)? {
            return Ok(0);
        }
        self.send_pending(session).await
    }

    /// Whether an automatic delivery would be typed now: the session runs, its agent is idle between turns and its
    /// input line is empty.
    pub fn is_ready(&self, session: &str) -> Result<bool, SessionError> {
        let running = self.sessions.info(session).is_ok_and(|i| i.is_running());
        Ok(running && self.idle.is_idle(session) && !self.sessions.has_unsent_input(session)?)
    }

    /// **Deliver now** (the user's click): sends the waiting messages even if the agent is busy; asks first if
    /// the input line has unsent text.
    pub async fn deliver_now(&self, session: &str) -> Result<Delivery, SessionError> {
        if self.sessions.has_unsent_input(session)? {
            return Ok(self.needs_confirmation(session));
        }
        let count = self.send_pending(session).await?;
        Ok(Delivery::Sent { count })
    }

    /// Text from a user's click (Send back, an inbox answer): never held; asks first if the input line has
    /// unsent text.
    pub async fn deliver_from_click(
        &self,
        session: &str,
        text: &str,
    ) -> Result<Delivery, SessionError> {
        if self.sessions.has_unsent_input(session)? {
            return Ok(self.needs_confirmation(session));
        }
        self.type_and_submit(session, text).await?;
        self.idle.delivered(session);
        Ok(Delivery::Sent { count: 1 })
    }

    /// "Clear it and send": clears the input line (only ever on this click), then sends `text`, or the waiting
    /// messages when `text` is `None` (Deliver now).
    pub async fn clear_and_send(
        &self,
        session: &str,
        text: Option<&str>,
    ) -> Result<Delivery, SessionError> {
        let input = self.input(session);
        // A tool whose clear-line keys leave the rest of a multi-line draft (M23) can't clear it safely; the
        // tracker can't tell a multi-line draft, so any draft is the user's to clear (SES-07, ADP-06).
        if !input.multi_line_clearable && self.sessions.has_unsent_input(session)? {
            return Ok(self.needs_confirmation(session));
        }
        self.sessions.clear_line(session, input.clear_line)?;
        tokio::time::sleep(SUBMIT_PAUSE).await;
        let count = match text {
            Some(text) => {
                self.type_and_submit(session, text).await?;
                self.idle.delivered(session);
                1
            }
            None => self.send_pending(session).await?,
        };
        Ok(Delivery::Sent { count })
    }

    /// "Keep it and cancel": nothing is sent, nothing is cleared.
    pub fn keep(&self, _session: &str) -> Delivery {
        Delivery::Sent { count: 0 }
    }

    /// The messages waiting for a session, oldest first.
    pub async fn pending(&self, session: &str) -> Result<Vec<PendingDelivery>, SessionError> {
        sqlx::query_as(
            "SELECT id, session_id, kind, text, created_at, message_id, message_part FROM pending_deliveries
             WHERE session_id = ? ORDER BY created_at, id",
        )
        .bind(session)
        .fetch_all(self.store.pool())
        .await
        .map_err(store_error)
    }

    async fn send_pending(&self, session: &str) -> Result<usize, SessionError> {
        let pending = self.pending(session).await?;
        for delivery in &pending {
            self.type_and_submit(session, &delivery.text).await?;
            sqlx::query("DELETE FROM pending_deliveries WHERE id = ?")
                .bind(&delivery.id)
                .execute(self.store.pool())
                .await
                .map_err(store_error)?;
            // A question or answer between agents: it reached the other agent now.
            if let (Some(message), Some(part)) = (&delivery.message_id, &delivery.message_part) {
                let column = if part == crate::messages::PART_ANSWER {
                    "answer_delivered_at"
                } else {
                    "delivered_at"
                };
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "UPDATE messages SET {column} = COALESCE({column}, ?) WHERE id = ?"
                )))
                .bind(now_ms())
                .bind(message)
                .execute(self.store.pool())
                .await
                .map_err(store_error)?;
                if let Ok(info) = self.sessions.info(session)
                    && let Some(ticket) = info.ticket
                {
                    self.events.publish(Event::MessageDelivered {
                        project: info.project,
                        key: ticket,
                        message: message.clone(),
                    });
                }
            }
        }
        if !pending.is_empty() {
            self.idle.delivered(session);
            self.publish_waiting(session).await?;
        }
        Ok(pending.len())
    }

    /// Types `text` (as a bracketed paste when it has line breaks, so they stay inside the message), then Enter.
    async fn type_and_submit(&self, session: &str, text: &str) -> Result<(), SessionError> {
        let typed = if text.contains('\n') {
            format!("\x1b[200~{text}\x1b[201~")
        } else {
            text.to_owned()
        };
        self.sessions.write(session, typed.as_bytes())?;
        tokio::time::sleep(SUBMIT_PAUSE.max(self.input(session).enter_delay)).await;
        self.sessions.write(session, b"\r")
    }

    /// One of Otaq's own skills as the session's agent tool names it (ADP-05): `/otaq:address-review` for
    /// Claude Code.
    pub fn skill_reference(&self, session: &str, skill: &str) -> String {
        let tool = self.sessions.info(session).ok().and_then(|info| info.tool);
        self.adapters.of_session(tool).skill_reference(skill)
    }

    /// How the session's agent tool takes typed text (ADP-06).
    fn input(&self, session: &str) -> InputQuirks {
        let tool = self.sessions.info(session).ok().and_then(|info| info.tool);
        self.adapters.of_session(tool).input()
    }

    fn needs_confirmation(&self, session: &str) -> Delivery {
        Delivery::NeedsConfirmation {
            ticket: self.sessions.info(session).ok().and_then(|i| i.ticket),
        }
    }

    async fn publish_waiting(&self, session: &str) -> Result<(), SessionError> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM pending_deliveries WHERE session_id = ?")
                .bind(session)
                .fetch_one(self.store.pool())
                .await
                .map_err(store_error)?;
        let info = self.sessions.info(session).ok();
        self.events.publish(Event::MessagesWaiting {
            session: session.to_owned(),
            project: info.as_ref().map(|i| i.project.clone()),
            ticket: info.and_then(|i| i.ticket),
            count: u32::try_from(count).unwrap_or(u32::MAX),
        });
        Ok(())
    }
}

fn store_error(e: sqlx::Error) -> SessionError {
    SessionError::Store(StoreError::from(e))
}
