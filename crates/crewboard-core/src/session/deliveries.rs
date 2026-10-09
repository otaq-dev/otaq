//! Typing into live sessions (SES-06, SES-07; d0048).
//!
//! **Automatic deliveries** (another agent's question or answer, the ticket-change note, "continue" after the
//! limit reset) are stored in `pending_deliveries` and typed only when the agent is **idle between turns and
//! the input line is empty**. Until then the card and the terminal strip show "1 message waiting · Deliver now".
//!
//! **The user's own clicks** (Deliver now, Send back, an inbox answer) are never held: if the input line has
//! unsent text the request answers [`Delivery::NeedsConfirmation`] instead, and the client asks *"SHOP-23 has
//! unsent text in its terminal"* → **Keep it and cancel** (nothing is sent) / **Clear it and send**
//! ([`Deliveries::clear_and_send`]). Crewboard never clears the user's text silently.
//!
//! "Idle between turns" comes from hook signals (M4, T-0066/T-0067); here it is an injected [`IdleSignal`].
//! Until those exist, the service uses [`NeverIdle`], so automatic deliveries wait for Deliver now.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{SessionError, Sessions};
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
    }
}

/// Whether an agent is idle between turns. Hook signals answer this from M4.
pub trait IdleSignal: Send + Sync {
    fn is_idle(&self, session: &str) -> bool;
}

/// Until hook signals exist: never idle, so automatic deliveries are held until the user clicks Deliver now.
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
}

/// Lets the program take a typed message in before Enter submits it.
const SUBMIT_PAUSE: Duration = Duration::from_millis(80);

/// Queues and types deliveries. Cheap to clone.
#[derive(Clone)]
pub struct Deliveries {
    store: Store,
    sessions: Sessions,
    events: EventBus,
    idle: Arc<dyn IdleSignal>,
}

impl Deliveries {
    pub fn new(
        store: Store,
        sessions: Sessions,
        events: EventBus,
        idle: Arc<dyn IdleSignal>,
    ) -> Self {
        Self {
            store,
            sessions,
            events,
            idle,
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
        self.publish_waiting(session).await?;
        self.deliver_if_ready(session).await.map(|_| ())
    }

    /// Types every pending delivery, oldest first, if the agent is idle **and** the input line is empty.
    /// Called when something may have made that true (a queued delivery, an Enter, a hook signal later).
    /// Returns how many were sent.
    pub async fn deliver_if_ready(&self, session: &str) -> Result<usize, SessionError> {
        let running = self.sessions.info(session).is_ok_and(|i| i.is_running());
        if !running || !self.idle.is_idle(session) || self.sessions.has_unsent_input(session)? {
            return Ok(0);
        }
        self.send_pending(session).await
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
        Ok(Delivery::Sent { count: 1 })
    }

    /// "Clear it and send": clears the input line (only ever on this click), then sends `text`, or the waiting
    /// messages when `text` is `None` (Deliver now).
    pub async fn clear_and_send(
        &self,
        session: &str,
        text: Option<&str>,
    ) -> Result<Delivery, SessionError> {
        self.sessions.clear_line(session)?;
        tokio::time::sleep(SUBMIT_PAUSE).await;
        let count = match text {
            Some(text) => {
                self.type_and_submit(session, text).await?;
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
            "SELECT id, session_id, kind, text, created_at FROM pending_deliveries
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
        }
        if !pending.is_empty() {
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
        tokio::time::sleep(SUBMIT_PAUSE).await;
        self.sessions.write(session, b"\r")
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
