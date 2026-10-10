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
//! a service restart ends every session anyway (its tickets become Interrupted, [`crate::recovery`]).
//!
//! [`state::transition`] is the state machine: hook signals, `ask human`, any other `crewboard` command of the
//! agent, and Enter in the ticket's terminal (ATT-01, ATT-02, ATT-04). Every hook and command also moves the
//! ticket's **last signal** (ATT-05). The status, its text and the last signal are stored on the ticket.
//!
//! Hook status changes are not written to the History tab (they come several times per turn).

pub mod background;
pub mod hooks;
pub mod state;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError};
use crate::caller::Caller;
use crate::domain::Status;
use crate::protocol::Event;
use crate::service::EventBus;
use crate::session::deliveries::IdleSignal;
use crate::store::now_ms;
use crate::store::tickets::set_state_since;
pub use hooks::{HELPER_PROMPT_PREFIX, HookEvent, Signal, TurnFacts, map_hook};
pub use state::{Input, STALE_AFTER_MS, SignalState, is_stale, transition};

/// The statuses of a ticket whose agent is at work; only these follow signals. Waiting for limit reset is one:
/// a turn that starts again (Claude Code's own continue after the reset, the user's prompt, Resume all's
/// "continue"; Q27) makes it Working.
pub const LIVE: &[Status] = &[
    Status::Working,
    Status::WorkingInBackground,
    Status::AsksYou,
    Status::NeedsYou,
    Status::Idle,
    Status::WaitingForLimitReset,
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
    /// One signal is applied at a time, so read-modify-write of a ticket's state never interleaves.
    applying: tokio::sync::Mutex<()>,
}

/// One session's current turn.
#[derive(Debug, Clone, Copy, Default)]
struct Turn {
    facts: TurnFacts,
    /// The last hook ended the turn (Stop) and no new turn has started: the agent is idle between turns.
    ended: bool,
}

impl Signals {
    pub fn new(board: Board, events: EventBus) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                events,
                turns: Mutex::new(HashMap::new()),
                applying: tokio::sync::Mutex::new(()),
            }),
        }
    }

    /// The facts of a session's current turn.
    pub fn turn(&self, session: &str) -> TurnFacts {
        self.turn_state(session).facts
    }

    /// Whether the session's agent is idle between turns (its last hook ended the turn).
    pub fn is_idle_between_turns(&self, session: &str) -> bool {
        self.turn_state(session).ended
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
        if let Signal::Failed { error, message } = &signal {
            tracing::warn!(%error, message = ?message, "agent turn failed");
        }
        // The agent is alive in its conversation: its first prompt arrived.
        self.inner.board.first_prompt_delivered(ticket_id).await?;
        self.apply(ticket_id, &Input::Hook(signal.clone())).await?;
        Ok(signal)
    }

    /// The agent ran a `crewboard` command (other than `hook` and `ask human`): it is active, so any orange
    /// label clears (ATT-02) and the last signal moves (ATT-05).
    pub async fn command(&self, caller: &Caller) -> Result<(), BoardError> {
        if let Caller::Agent { ticket_id, .. } = caller {
            self.inner.board.first_prompt_delivered(ticket_id).await?;
            self.apply(ticket_id, &Input::Command).await?;
        }
        Ok(())
    }

    /// `crewboard ask human` for `ticket_id`: the ticket shows Asks you with the question (ATT-01). From the
    /// ticket's agent, this turn's `Stop` won't turn it into Idle.
    pub async fn ask_human(
        &self,
        caller: &Caller,
        ticket_id: &str,
        question: &str,
    ) -> Result<(), BoardError> {
        if let Caller::Agent { session, .. } = caller {
            self.mark_submitted_or_asked(session);
        }
        self.apply(ticket_id, &Input::AskHuman(question.to_owned()))
            .await
    }

    /// Enter was pressed in the ticket's terminal: the user answered, so the orange label clears (ATT-02).
    pub async fn enter(&self, ticket_id: &str) -> Result<(), BoardError> {
        self.apply(ticket_id, &Input::Enter).await
    }

    /// The user answered the agent from the inbox (or the card or panel): the orange label clears (ATT-02).
    pub async fn answered(&self, ticket_id: &str) -> Result<(), BoardError> {
        self.apply(ticket_id, &Input::Answered).await
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

    /// Runs one transition on the stored state, writes what changed and tells clients.
    async fn apply(&self, ticket_id: &str, input: &Input) -> Result<(), BoardError> {
        let _one_at_a_time = self.inner.applying.lock().await;
        let pool = self.inner.board.store().pool();
        let row: Option<StoredSignal> =
            sqlx::query_as(
                "SELECT state, signal_text, signal_since, wakeup_at, last_signal_at FROM tickets WHERE id = ?",
            )
            .bind(ticket_id)
            .fetch_optional(pool)
            .await?;
        let Some((state, text, since, wakeup_at, last_signal_at)) = row else {
            return Ok(());
        };
        let Ok(status) = state.parse::<Status>() else {
            return Ok(());
        };
        let before = SignalState {
            status,
            text,
            since,
            wakeup_at,
            last_signal_at,
        };
        let after = transition(&before, input, now_ms());
        if after == before {
            return Ok(());
        }
        // An agent the plan limit stopped is at work again (Claude Code continued it, or the user did): it is no
        // longer the limit's to hide from the inbox (ATT-09).
        if before.status == Status::WaitingForLimitReset && after.status != before.status {
            self.inner.board.limit_stops().mark(ticket_id, false);
        }
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, signal_text = ?, signal_since = ?, wakeup_at = ?,
                                last_signal_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(after.status.column().as_str())
        .bind(after.status.as_str())
        .bind(&after.text)
        .bind(after.since)
        .bind(after.wakeup_at)
        .bind(after.last_signal_at)
        .bind(after.status.as_str())
        .bind(now_ms())
        .bind(ticket_id)
        .execute(pool)
        .await?;
        let ticket = self.inner.board.ticket_by_id(ticket_id).await?;
        self.publish(&ticket, after.status != before.status);
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

/// `state, signal_text, signal_since, wakeup_at, last_signal_at` of a ticket.
type StoredSignal = (
    String,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

/// The delivery queue's "idle between turns" (SES-06): the agent's last hook ended its turn.
impl IdleSignal for Signals {
    fn is_idle(&self, session: &str) -> bool {
        self.is_idle_between_turns(session)
    }

    /// Typing a message starts the agent's next turn: until its next Stop it isn't idle any more.
    fn delivered(&self, session: &str) {
        if let Some(turn) = self.inner.turns.lock().expect("turns").get_mut(session) {
            turn.ended = false;
        }
    }
}
