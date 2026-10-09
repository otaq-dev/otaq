//! Agent signals: what each agent is doing, from its hooks (SDD §11.4, §12.1; d0042).
//!
//! The plugin's hooks call `crewboard hook <event>` inside the agent's session; the service resolves the caller
//! to the agent (CLI-01), maps the hook to a [`Signal`] ([`hooks::map_hook`]) and sets the ticket's status:
//! **Needs you**, **Working in background**, **Idle** or back to **Working**. Only a ticket whose agent is at work
//! (Working, Working in background, or one of the orange family) changes status this way; a ticket that was
//! submitted, parked or closed keeps its column.
//!
//! Per session, the service remembers the current turn's facts (did `review submit` or `ask human` run?) and
//! whether the turn has ended. Both reset when the next turn starts (`UserPromptSubmit`). They live in memory:
//! a service restart ends every session anyway (until M7).
//!
//! Hook status changes are not written to the History tab (they come several times per turn).

pub mod background;
pub mod hooks;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError};
use crate::caller::Caller;
use crate::domain::Status;
use crate::protocol::Event;
use crate::service::EventBus;
use crate::store::now_ms;
pub use hooks::{HookEvent, Signal, TurnFacts, map_hook};

/// The statuses of a ticket whose agent is at work; only these follow hook signals.
pub const LIVE: &[Status] = &[
    Status::Working,
    Status::WorkingInBackground,
    Status::AsksYou,
    Status::NeedsYou,
    Status::Idle,
];

/// The signal state of every agent session. Cheap to clone.
#[derive(Clone)]
pub struct Signals {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    events: EventBus,
    turns: Mutex<HashMap<String, Turn>>,
}

/// One session's current turn.
#[derive(Debug, Clone, Copy, Default)]
struct Turn {
    facts: TurnFacts,
    /// The last hook ended the turn (Stop) and no new turn has started.
    ended: bool,
}

impl Signals {
    pub fn new(board: Board, events: EventBus) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                events,
                turns: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// The facts of a session's current turn.
    pub fn turn(&self, session: &str) -> TurnFacts {
        self.turn_state(session).facts
    }

    /// `crewboard review submit` or `crewboard ask human` ran in this turn, so its `Stop` isn't Idle.
    pub fn mark_submitted_or_asked(&self, session: &str) {
        self.inner
            .turns
            .lock()
            .expect("turns")
            .entry(session.to_owned())
            .or_default()
            .facts
            .submitted_or_asked = true;
    }

    /// Handles one hook from `caller`. Hooks from outside an agent session (the plugin used by hand) change
    /// nothing.
    pub async fn hook(
        &self,
        caller: &Caller,
        event: HookEvent,
        payload: &Value,
    ) -> Result<Signal, BoardError> {
        tracing::debug!(%event, payload = %payload, agent = ?caller.ticket(), "hook");
        let Caller::Agent {
            ticket_id, session, ..
        } = caller
        else {
            tracing::info!(%event, "hook from outside an agent session ignored");
            return Ok(Signal::Ignored);
        };
        let signal = map_hook(event, payload, self.turn(session));
        {
            let mut turns = self.inner.turns.lock().expect("turns");
            let turn = turns.entry(session.clone()).or_default();
            match &signal {
                Signal::Working => *turn = Turn::default(),
                s if s.ends_turn() => turn.ended = true,
                _ => {}
            }
        }
        let status = match &signal {
            Signal::NeedsYou { .. } => Some(Status::NeedsYou),
            Signal::WorkingInBackground { .. } => Some(Status::WorkingInBackground),
            Signal::Idle { .. } => Some(Status::Idle),
            Signal::Working => Some(Status::Working),
            Signal::Failed { error, message } => {
                tracing::warn!(%error, message = ?message, "agent turn failed");
                None
            }
            Signal::TurnEnded | Signal::Ignored => None,
        };
        if let Some(status) = status {
            self.set_status(ticket_id, status).await?;
        }
        Ok(signal)
    }

    fn turn_state(&self, session: &str) -> Turn {
        self.inner
            .turns
            .lock()
            .expect("turns")
            .get(session)
            .copied()
            .unwrap_or_default()
    }

    /// Sets a live ticket's status and tells clients. Tickets that aren't live keep theirs.
    async fn set_status(&self, ticket_id: &str, status: Status) -> Result<(), BoardError> {
        let board = &self.inner.board;
        let before = board.ticket_by_id(ticket_id).await?;
        if !LIVE.contains(&before.status) || before.status == status {
            return Ok(());
        }
        sqlx::query("UPDATE tickets SET board_column = ?, state = ?, updated_at = ? WHERE id = ?")
            .bind(status.column().as_str())
            .bind(status.as_str())
            .bind(now_ms())
            .bind(ticket_id)
            .execute(board.store().pool())
            .await?;
        let after = board.ticket_by_id(ticket_id).await?;
        self.publish(&after, true);
        Ok(())
    }

    fn publish(&self, ticket: &Ticket, status_changed: bool) {
        let project = ticket.key.project.to_string();
        self.inner.events.publish(Event::TicketChanged {
            project: project.clone(),
            id: ticket.id.clone(),
            key: ticket.key.clone(),
        });
        if status_changed {
            self.inner.events.publish(Event::CardStatusChanged {
                project,
                id: ticket.id.clone(),
                key: ticket.key.clone(),
                status: ticket.status,
            });
        }
    }
}
