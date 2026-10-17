//! The ticket environment (M19, d0085…d0087): a ticket's build and resources across Prepare, Free, Bring back and
//! Release.
//!
//! - [`handoff`]: how a stock entry is handed to a worktree on this disk ([`handoff::HandoffMode`]).
//! - [`stock`]: the stock of builds per repository: its limit, the nearest build for a ticket, and eviction.
//! - [`free`]: the Free at Review entry (M19-c, d0086).
//!
//! [`TicketEnv`] holds a lock per ticket, so a Free and a Release (Done) of the same ticket never both move its
//! build: whichever comes second reads the ticket again and sees what the first did. **Lock order**: the ticket's
//! lock first, then the stock's (a std mutex, never held across an `.await`).
//!
//! [`TicketEnv::run`] listens to the service's events: a ticket entering [`free::FREED_IN`] is freed; a Run or
//! Shell closing frees its ticket's build if it was held for it; a ticket entering or leaving Ready for Dev changes
//! the stock limit. A slow tick (and once at startup) frees what no event announced ([`TicketEnv::sweep`]).

pub mod free;
pub mod handoff;
pub mod stock;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{OwnedMutexGuard, watch};
use tokio::task::JoinSet;

use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError};
use crate::domain::{BuildState, Column, TicketKey};
use crate::protocol::Event;
use crate::service::EventBus;
use crate::session::{SessionKind, Sessions};
use crate::store::projects::Project;
use crate::ticket_env::free::{FREED_IN, FreeOutcome};
use crate::ticket_env::handoff::HandoffMode;
use crate::ticket_env::stock::Stock;
use crate::warm::store::{WarmBuilds, repo_name};

/// How often [`TicketEnv::run`] sweeps for builds to free (the scheduler's slow tick).
pub const SWEEP_TICK: Duration = crate::scheduler::DEFAULT_TICK;

/// The four moments of a ticket's environment. Cheap to clone.
#[derive(Clone)]
pub struct TicketEnv(Arc<Inner>);

struct Inner {
    board: Board,
    warm: WarmBuilds,
    stock: Stock,
    sessions: Sessions,
    events: EventBus,
    /// One lock per ticket id: Free and Release never run at once for a ticket.
    locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl std::fmt::Debug for TicketEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TicketEnv")
            .field("stock", &self.0.stock)
            .finish_non_exhaustive()
    }
}

impl TicketEnv {
    pub fn new(board: Board, warm: WarmBuilds, sessions: Sessions, events: EventBus) -> Self {
        let stock = Stock::new(warm.clone(), board.clone());
        Self(Arc::new(Inner {
            board,
            warm,
            stock,
            sessions,
            events,
            locks: Mutex::new(HashMap::new()),
        }))
    }

    pub(crate) fn board(&self) -> &Board {
        &self.0.board
    }

    pub(crate) fn warm(&self) -> &WarmBuilds {
        &self.0.warm
    }

    pub fn stock(&self) -> &Stock {
        &self.0.stock
    }

    pub(crate) fn sessions(&self) -> &Sessions {
        &self.0.sessions
    }

    pub(crate) fn events(&self) -> &EventBus {
        &self.0.events
    }

    /// Waits for the ticket's lock.
    pub(crate) async fn lock(&self, ticket_id: &str) -> OwnedMutexGuard<()> {
        let lock = self
            .0
            .locks
            .lock()
            .expect("ticket locks")
            .entry(ticket_id.to_owned())
            .or_default()
            .clone();
        lock.lock_owned().await
    }

    /// **Free** (M19-c): moves `key`'s build out of its worktree into the stock, if it is in Review with its build
    /// in the worktree and no Run or Shell open ([`free`]).
    pub async fn free(&self, key: &TicketKey) -> FreeOutcome {
        free::free_one(self, key).await
    }

    /// **Release** at Done (REC-05, WRM-03): under the ticket's lock, keeps the clean `worktree`'s build in the stock
    /// unless it was freed already (never a reason for Done to fail), removes the worktree (the branch stays), and
    /// resets the ticket's build state, so a reopened ticket is never taken for freed.
    pub(crate) async fn release(
        &self,
        ticket: &Ticket,
        project: &Project,
        worktree: &Path,
    ) -> Result<(), BoardError> {
        let board = &self.0.board;
        let _held = self.lock(&ticket.id).await;
        let state = board
            .ticket_by_id(&ticket.id)
            .await?
            .work
            .map(|w| w.build_state)
            .unwrap_or_default();
        if state == BuildState::Present {
            crate::warm::keep::keep(board, &self.0.warm, project, &ticket.key, worktree).await;
        } else {
            tracing::debug!(ticket = %ticket.key, state = state.as_str(), "the build was freed: nothing to keep");
        }
        let git = board.project_git(&project.key).await?;
        crate::review::finish::remove_worktree(&git, Path::new(&project.repo_path), worktree)
            .await?;
        sqlx::query(
            "UPDATE tickets SET worktree_path = NULL, worktree_kept = FALSE, build_state = ?, build_problem = NULL
             WHERE id = ?",
        )
        .bind(BuildState::Present.as_str())
        .bind(&ticket.id)
        .execute(board.store().pool())
        .await?;
        Ok(())
    }

    /// Frees every ticket in [`FREED_IN`] whose build is still in its worktree and whose last Free didn't fail
    /// (Cleanup's "could not free" retries those): what no event announced.
    pub async fn sweep(&self) {
        let projects = match self.0.board.projects().await {
            Ok(projects) => projects,
            Err(e) => {
                tracing::warn!(error = %e, "can't read the projects to free builds");
                return;
            }
        };
        for project in projects {
            let tickets = match self.0.board.tickets(&project.key).await {
                Ok(tickets) => tickets,
                Err(e) => {
                    tracing::warn!(project = %project.key, error = %e, "can't read the tickets to free builds");
                    continue;
                }
            };
            for ticket in tickets {
                let failed = ticket
                    .work
                    .as_ref()
                    .is_some_and(|w| w.build_problem.is_some());
                if !failed && free::to_free(&ticket).is_some() {
                    self.free(&ticket.key).await;
                }
            }
        }
    }

    /// Applies the stock limit of `key`'s repository (a ticket entered or left Ready for Dev).
    async fn limit_changed(&self, key: &TicketKey) {
        if self.0.warm.mode() != HandoffMode::Move {
            return;
        }
        let Ok(project) = self.0.board.project(key.project.as_str()).await else {
            return;
        };
        let repo = repo_name(&project);
        if let Err(e) = self.0.stock.enforce_limit(&repo).await {
            tracing::warn!(warm_build = %repo, error = %e, "can't apply the stock limit");
        }
    }

    /// The ids of the tickets in Ready for Dev now.
    async fn in_ready_for_dev(&self) -> HashSet<String> {
        let mut ready = HashSet::new();
        let Ok(projects) = self.0.board.projects().await else {
            return ready;
        };
        for project in projects {
            if let Ok(tickets) = self.0.board.tickets(&project.key).await {
                ready.extend(
                    tickets
                        .into_iter()
                        .filter(|t| t.status.column() == Column::ReadyForDev)
                        .map(|t| t.id),
                );
            }
        }
        ready
    }

    /// Listens to the service's events until `stop` is set, and sweeps at once and then on a slow tick
    /// ([`SWEEP_TICK`]).
    pub async fn run(self, mut stop: watch::Receiver<bool>) {
        let mut listener = self.0.events.listen();
        let mut tick = tokio::time::interval(SWEEP_TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut tasks = JoinSet::new();
        // The tickets last seen in Ready for Dev (those there at startup too): leaving it changes the stock limit.
        let mut ready = self.in_ready_for_dev().await;
        loop {
            tokio::select! {
                _ = stop.wait_for(|s| *s) => break,
                _ = tick.tick() => {
                    let env = self.clone();
                    tasks.spawn(async move { env.sweep().await });
                }
                Some(_) = tasks.join_next(), if !tasks.is_empty() => {}
                event = listener.receiver.recv() => match event {
                    Ok(Event::CardStatusChanged { id, key, status, .. }) => {
                        if FREED_IN.contains(&status) {
                            let (env, key) = (self.clone(), key.clone());
                            tasks.spawn(async move {
                                env.free(&key).await;
                            });
                        }
                        let entered = status.column() == Column::ReadyForDev;
                        let left = !entered && ready.remove(&id);
                        if entered {
                            ready.insert(id);
                        }
                        if entered || left {
                            let env = self.clone();
                            tasks.spawn(async move { env.limit_changed(&key).await });
                        }
                    }
                    Ok(Event::SessionState { session })
                        if !session.is_running()
                            && matches!(session.kind, SessionKind::Run | SessionKind::Shell) =>
                    {
                        if let Some(key) = session.ticket {
                            let env = self.clone();
                            tasks.spawn(async move {
                                env.free(&key).await;
                            });
                        }
                    }
                    Ok(_) => {}
                    Err(RecvError::Lagged(missed)) => {
                        tracing::warn!(missed, "the ticket environment fell behind on events; sweeping");
                        let env = self.clone();
                        tasks.spawn(async move { env.sweep().await });
                    }
                    Err(RecvError::Closed) => break,
                },
            }
        }
        // A Free under way finishes: it never leaves a build half moved.
        while tasks.join_next().await.is_some() {}
    }
}
