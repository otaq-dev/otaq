//! The scheduler (SCH-01, SCH-02; d0007, d0016, d0049): a loop in the service that starts Queued tickets once
//! every dependency is merged (beads-style readiness, [`crate::board::readiness`]), **oldest first** by the time
//! the user queued them.
//!
//! It runs a pass on every state change (the service's event bus) plus a slow tick ([`DEFAULT_TICK`]) that
//! catches anything missed. A pass is [`plan`]: pure, from all projects' tickets. The scheduler:
//! - **never queues a ticket by itself**: only the user's drag puts a ticket in Queued (SCH-02). It only looks at
//!   tickets already in Queued's own sub-states and moves them among those (Queued ↔ Parent closed);
//! - never touches **Start interrupted** or **Held** (diverged base) tickets: only the user's Retry start
//!   puts them back to Queued (d0018, d0059);
//! - starts each ticket once: passes never overlap, tickets whose start is running are skipped, and the start
//!   sequence claims a ticket before it reads its status.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::{Mutex, Notify, watch};

use crate::board::readiness::{ReadyState, readiness};
use crate::board::tickets::{EventKind, Ticket, record_event};
use crate::board::{Board, BoardError};
use crate::domain::{Actor, Status, TicketKey};
use crate::protocol::Event;
use crate::service::EventBus;
use crate::start::Starter;
use crate::store::now_ms;
use crate::store::tickets::set_state_since;

/// The slow tick: a pass at least this often, whatever happened.
pub const DEFAULT_TICK: Duration = Duration::from_secs(30);

/// A Queued sub-state the scheduler moves a ticket into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusChange {
    pub id: String,
    pub key: TicketKey,
    pub from: Status,
    pub to: Status,
}

/// A ticket a pass starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToStart {
    pub id: String,
    pub key: TicketKey,
}

/// What one pass does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Sub-state bookkeeping, applied first.
    pub changes: Vec<StatusChange>,
    /// Tickets to start, oldest first.
    pub start: Vec<ToStart>,
}

/// The Queued sub-states the scheduler looks at. Held and Start interrupted are left alone (user's Retry).
const SCHEDULED: &[Status] = &[Status::Queued, Status::WaitingForSlot, Status::ParentClosed];

/// Decides one pass from every project's tickets. `starting` holds the ids of tickets whose start runs now.
pub fn plan(tickets: &[Ticket], starting: &HashSet<String>) -> Plan {
    let mut plan = Plan::default();
    let mut ready: Vec<&Ticket> = Vec::new();
    for ticket in tickets {
        if !SCHEDULED.contains(&ticket.status) || starting.contains(&ticket.id) {
            continue;
        }
        let to = match readiness(ticket, tickets) {
            ReadyState::ParentClosed { .. } => Status::ParentClosed,
            ReadyState::Waiting { .. } => Status::Queued,
            ReadyState::Ready => {
                ready.push(ticket);
                // A ticket whose closed parent was unlinked starts from Queued.
                if ticket.status == Status::ParentClosed {
                    Status::Queued
                } else {
                    ticket.status
                }
            }
        };
        if to != ticket.status {
            plan.changes.push(StatusChange {
                id: ticket.id.clone(),
                key: ticket.key.clone(),
                from: ticket.status,
                to,
            });
        }
    }
    ready.sort_by_key(|t| (queue_time(t), t.created_at, t.key.clone()));
    plan.start = ready
        .into_iter()
        .map(|t| ToStart {
            id: t.id.clone(),
            key: t.key.clone(),
        })
        .collect();
    plan
}

/// When the user queued the ticket (older tickets start first).
fn queue_time(ticket: &Ticket) -> i64 {
    ticket
        .queued_at
        .or(ticket.status_since)
        .unwrap_or(ticket.created_at)
}

/// Starts tickets for the scheduler. The service uses the start sequence ([`Starter`]); tests inject their own.
pub trait StartTickets: Send + Sync + 'static {
    /// Starts the ticket in the background and returns at once.
    fn spawn_start(&self, ticket: &ToStart);
    /// The ids of tickets whose start is running.
    fn starting(&self) -> HashSet<String>;
}

impl StartTickets for Starter {
    fn spawn_start(&self, ticket: &ToStart) {
        let starter = self.clone();
        let key = ticket.key.to_string();
        tokio::spawn(async move {
            // The user's drag queued it; the scheduler only picks the moment (SCH-02).
            if let Err(e) = starter.start(&key, &Actor::current_user()).await {
                tracing::error!(ticket = %key, error = %e, "start failed");
            }
        });
    }

    fn starting(&self) -> HashSet<String> {
        Starter::starting(self)
    }
}

/// The scheduler of one service. Cheap to clone.
#[derive(Clone)]
pub struct Scheduler {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    starts: Arc<dyn StartTickets>,
    events: EventBus,
    tick: Duration,
    /// One pass at a time.
    pass: Mutex<()>,
    wake: Notify,
}

impl Scheduler {
    pub fn new(
        board: Board,
        starts: Arc<dyn StartTickets>,
        events: EventBus,
        tick: Duration,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                starts,
                events,
                tick,
                pass: Mutex::new(()),
                wake: Notify::new(),
            }),
        }
    }

    /// Asks the loop for a pass soon (for changes that publish no ticket event, such as settings).
    pub fn wake(&self) {
        self.inner.wake.notify_one();
    }

    /// Runs one pass now: applies the sub-state changes and starts the ready tickets, oldest first.
    pub async fn pass(&self) -> Result<Plan, BoardError> {
        let _one_at_a_time = self.inner.pass.lock().await;
        let board = &self.inner.board;
        let mut tickets = Vec::new();
        for project in board.projects().await? {
            tickets.extend(board.tickets(&project.key).await?);
        }
        let plan = plan(&tickets, &self.inner.starts.starting());
        for change in &plan.changes {
            if let Err(e) = self.apply(change).await {
                tracing::warn!(ticket = %change.key, error = %e, "can't change the queued state");
            }
        }
        for ticket in &plan.start {
            tracing::info!(ticket = %ticket.key, "scheduler starts it");
            self.inner.starts.spawn_start(ticket);
        }
        Ok(plan)
    }

    /// Runs passes on every state change and every tick until `stop` is set.
    pub async fn run(self, mut stop: watch::Receiver<bool>) {
        let mut listener = self.inner.events.listen();
        let events = &mut listener.receiver;
        let mut tick = tokio::time::interval(self.inner.tick);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = stop.wait_for(|s| *s) => return,
                _ = tick.tick() => {}
                _ = self.inner.wake.notified() => {}
                event = events.recv() => match event {
                    Ok(event) if !triggers_pass(&event) => continue,
                    Ok(_) | Err(RecvError::Lagged(_)) => {
                        // One pass covers everything that arrived meanwhile.
                        while matches!(events.try_recv(), Ok(_) | Err(TryRecvError::Lagged(_))) {}
                    }
                    Err(RecvError::Closed) => return,
                },
            }
            if let Err(e) = self.pass().await {
                tracing::warn!(error = %e, "scheduler pass failed");
            }
        }
    }

    /// Moves a ticket between Queued's sub-states, only if it is still where the plan found it.
    async fn apply(&self, change: &StatusChange) -> Result<(), BoardError> {
        let pool = self.inner.board.store().pool();
        let now = now_ms();
        let mut tx = pool.begin().await?;
        let updated = sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ? AND state = ?"
        ))
        .bind(change.to.column().as_str())
        .bind(change.to.as_str())
        .bind(now)
        .bind(change.to.as_str())
        .bind(now)
        .bind(&change.id)
        .bind(change.from.as_str())
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if updated == 0 {
            return Ok(());
        }
        // Parent closed is worth a History line; waiting for a slot comes and goes (T-0080).
        if change.from == Status::ParentClosed || change.to == Status::ParentClosed {
            record_event(
                &mut tx,
                &change.id,
                EventKind::Moved,
                &Actor::current_user(),
                json!({ "from": change.from, "to": change.to }),
            )
            .await?;
        }
        tx.commit().await?;
        let project = change.key.project.to_string();
        self.inner.events.publish(Event::TicketChanged {
            project: project.clone(),
            id: change.id.clone(),
            key: change.key.clone(),
        });
        self.inner.events.publish(Event::CardStatusChanged {
            project,
            id: change.id.clone(),
            key: change.key.clone(),
            status: change.to,
        });
        Ok(())
    }
}

/// Events that can change what a pass decides: any ticket change (a status, a link) or a project going.
fn triggers_pass(event: &Event) -> bool {
    matches!(
        event,
        Event::TicketChanged { .. }
            | Event::CardStatusChanged { .. }
            | Event::TicketDeleted { .. }
            | Event::ProjectsChanged
    )
}
