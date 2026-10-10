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
//!
//! **Optional limits** (SCH-03; d0016, d0042), off by default, global across projects (d0024): with
//! `max_concurrency` on, a pass starts at most `max − running` tickets ([`HOLDS_A_SLOT`]); with back-pressure
//! reached (that many needs-you items), it starts none. Ready tickets that don't start show "Waiting for a slot".
//! Limits only hold back **starts**: nothing running is stopped or paused, and an agent the user unblocks
//! always continues. Settings changes wake the scheduler, so they apply at once.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::{Mutex, Notify, watch};

use crate::board::needs_you::{ProjectTickets, needs_you_count, needs_you_items};
use crate::board::readiness::{ReadyState, readiness};
use crate::board::settings::AgentLimits;
use crate::board::tickets::{Base, EventKind, Ticket, record_event};
use crate::board::{Board, BoardError};
use crate::domain::{Actor, StartStep, Status, TicketKey};
use crate::git::BaseSync;
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

/// The tickets that hold a slot of the optional agent limit (d0016, d0042): Agent Working **including** the
/// orange family, Working in background and Waiting for limit reset. Review and Parked tickets don't, and
/// neither do Interrupted or Resume failed ones (no agent runs). Tickets whose start is running hold one too.
pub const HOLDS_A_SLOT: &[Status] = &[
    Status::Working,
    Status::WorkingInBackground,
    Status::AsksYou,
    Status::NeedsYou,
    Status::Idle,
    Status::WaitingForLimitReset,
];

/// What may hold new starts back (SCH-03): the optional limits and the number of needs-you items
/// ([`crate::board::needs_you::needs_you_count`]). The default lets everything start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gate {
    pub limits: AgentLimits,
    pub needs_you: usize,
}

impl Gate {
    /// How many tickets may start now; `None` when the limits are off. Limits only control **starting**:
    /// nothing running is ever stopped, so this is never below zero. A stored 0 (flagged in Settings) acts as 1.
    pub fn free_slots(&self, running: usize) -> Option<usize> {
        if !self.limits.enabled {
            return None;
        }
        let back_pressure = self.limits.back_pressure.max(1) as usize;
        if self.needs_you >= back_pressure {
            return Some(0);
        }
        Some((self.limits.max_concurrency.max(1) as usize).saturating_sub(running))
    }
}

/// How many agents hold a slot: tickets in [`HOLDS_A_SLOT`] plus those whose start is running.
pub fn running_count(tickets: &[Ticket], starting: &HashSet<String>) -> usize {
    tickets
        .iter()
        .filter(|t| HOLDS_A_SLOT.contains(&t.status) || starting.contains(&t.id))
        .count()
}

/// Decides one pass from every project's tickets. `starting` holds the ids of tickets whose start runs now.
/// Ready tickets beyond the free slots wait in Queued as "waiting for a slot", oldest first when slots free up.
pub fn plan(tickets: &[Ticket], starting: &HashSet<String>, gate: &Gate) -> Plan {
    let mut plan = Plan::default();
    let mut ready: Vec<&Ticket> = Vec::new();
    for ticket in tickets {
        if !SCHEDULED.contains(&ticket.status) || starting.contains(&ticket.id) {
            continue;
        }
        match readiness(ticket, tickets) {
            ReadyState::ParentClosed { .. } => change(&mut plan, ticket, Status::ParentClosed),
            ReadyState::Waiting { .. } => change(&mut plan, ticket, Status::Queued),
            ReadyState::Ready => ready.push(ticket),
        }
    }
    ready.sort_by_key(|t| (queue_time(t), t.created_at, t.key.clone()));
    let slots = gate
        .free_slots(running_count(tickets, starting))
        .unwrap_or(usize::MAX);
    for (i, ticket) in ready.into_iter().enumerate() {
        if i < slots {
            // A ticket whose closed parent was unlinked starts from Queued.
            if ticket.status == Status::ParentClosed {
                change(&mut plan, ticket, Status::Queued);
            }
            plan.start.push(ToStart {
                id: ticket.id.clone(),
                key: ticket.key.clone(),
            });
        } else {
            change(&mut plan, ticket, Status::WaitingForSlot);
        }
    }
    plan
}

fn change(plan: &mut Plan, ticket: &Ticket, to: Status) {
    if ticket.status != to {
        plan.changes.push(StatusChange {
            id: ticket.id.clone(),
            key: ticket.key.clone(),
            from: ticket.status,
            to,
        });
    }
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
        self.pass_locked().await
    }

    /// **A parent was merged** (SCH-05; d0049): the user's Local merge (M6) or Mark merged (M10) calls this after
    /// setting the parent Done · Merged. Before its waiting children start, the base they start from is brought
    /// up to date: the parent's base branch is fetched (and fast-forwarded when only behind); an epic's branch is
    /// local and isn't fetched. Then a pass starts the children that became ready, from the merged code.
    ///
    /// If the base diverged from its remote, or the fetch fails, those children are **held** in Queued with the
    /// message; only the user's Retry start runs them again (nothing retries silently, d0018).
    pub async fn on_parent_merged(&self, parent: &str) -> Result<Plan, BoardError> {
        let _one_at_a_time = self.inner.pass.lock().await;
        let board = &self.inner.board;
        let parent = board.ticket(parent).await?;
        let tickets = board.tickets(parent.key.project.as_str()).await?;
        let starting = self.inner.starts.starting();
        let children: Vec<&Ticket> = tickets
            .iter()
            .filter(|t| {
                t.depends_on.contains(&parent.key)
                    && SCHEDULED.contains(&t.status)
                    && !starting.contains(&t.id)
                    && readiness(t, &tickets).is_ready()
            })
            .collect();
        if let (false, Base::Branch(branch)) = (children.is_empty(), &parent.base) {
            let project = board.project(parent.key.project.as_str()).await?;
            let synced = board
                .git()?
                .sync_base(Path::new(&project.repo_path), branch)
                .await;
            let problem = match synced {
                Ok(BaseSync::Diverged { message }) => Some(message),
                Ok(_) => None,
                Err(e) => Some(format!("Fetching `{branch}` failed: {e}")),
            };
            if let Some(message) = problem {
                for child in children {
                    self.hold(child, &message).await?;
                }
                tracing::warn!(parent = %parent.key, %message, "children held after the parent merged");
            }
        }
        self.pass_locked().await
    }

    /// Holds a ticket in Queued with the base's problem (step "sync base"), as the start sequence does.
    async fn hold(&self, ticket: &Ticket, message: &str) -> Result<(), BoardError> {
        let now = now_ms();
        let held = Status::Held;
        let mut tx = self.inner.board.store().pool().begin().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, start_problem_step = ?, start_problem_message = ?,
                                updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(held.column().as_str())
        .bind(held.as_str())
        .bind(StartStep::SyncBase.as_str())
        .bind(message)
        .bind(now)
        .bind(held.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::Held,
            &Actor::current_user(),
            json!({ "step": StartStep::SyncBase, "message": message }),
        )
        .await?;
        tx.commit().await?;
        self.publish(&ticket.id, &ticket.key, held);
        Ok(())
    }

    async fn pass_locked(&self) -> Result<Plan, BoardError> {
        let board = &self.inner.board;
        let projects = board.projects().await?;
        let mut loaded = Vec::with_capacity(projects.len());
        for project in &projects {
            loaded.push(board.tickets(&project.key).await?);
        }
        let limits = board.global_settings().await?.agent_limits;
        let needs_you = if limits.enabled {
            let views: Vec<ProjectTickets<'_>> = projects
                .iter()
                .zip(&loaded)
                .map(|(p, tickets)| ProjectTickets {
                    key: &p.key,
                    name: &p.name,
                    tickets,
                })
                .collect();
            needs_you_count(&needs_you_items(&views, now_ms()))
        } else {
            0
        };
        let tickets: Vec<Ticket> = loaded.into_iter().flatten().collect();
        let plan = plan(
            &tickets,
            &self.inner.starts.starting(),
            &Gate { limits, needs_you },
        );
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
        // Parent closed is worth a History line; waiting for a slot comes and goes.
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
        self.publish(&change.id, &change.key, change.to);
        Ok(())
    }

    fn publish(&self, id: &str, key: &TicketKey, status: Status) {
        let project = key.project.to_string();
        self.inner.events.publish(Event::TicketChanged {
            project: project.clone(),
            id: id.to_owned(),
            key: key.clone(),
        });
        self.inner.events.publish(Event::CardStatusChanged {
            project,
            id: id.to_owned(),
            key: key.clone(),
            status,
        });
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
