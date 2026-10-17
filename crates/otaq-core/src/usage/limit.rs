//! The usage limit being hit (USG-03, USG-04, USG-06, USG-07; d0020, d0046, d0088): which limit, when it resets,
//! and Resume all.
//!
//! "It happened" comes from the tool's adapter as *limit reached* (Claude Code: the plugin's `StopFailure` hook
//! with its limit text, `agent_tools::claude::limits`). The limit is stored with its tool ([`super::plan`]).
//!
//! The reset time, in this order: the tool's window for that limit ([`AgentTool::limit_window_key`]) if its reset
//! lies ahead, else the time in the limit message ([`AgentTool::limit_reset_from_text`]), else unknown.
//!
//! In M20 any active limit holds every start (SCH-04) and Resume all continues every limit-stopped agent; holding
//! only the agents of the limited tool, a banner line per tool and Resume per tool are M23 (USG-10).

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::plan::{self, ActiveLimit, ToolWindow};
use crate::agent_tools::{Adapters, AgentTool, LimitWindow, Tool};
use crate::attention::Inbox;
use crate::board::{Board, BoardError, Refusal};
use crate::domain::{Status, TicketKey, stored_enum};
use crate::protocol::Event;
use crate::service::EventBus;
use crate::session::deliveries::{Deliveries, DeliveryKind};
use crate::session::{SessionKind, Sessions};
use crate::store::{Store, StoreError, now_ms};

/// What Resume all types into each limit-stopped session (an automatic delivery, d0046, d0048).
pub const CONTINUE: &str = "continue";

/// How often the service looks whether an active limit has reset.
pub const RESET_TICK: Duration = Duration::from_secs(30);

stored_enum! {
    /// Which limit was hit; the banner words it (USG-06).
    pub enum LimitKind ("limit kind") {
        /// The 5-hour session limit.
        Session = "session",
        /// The weekly limit.
        Weekly = "weekly",
        /// Another limit (a model's own limit, a spend limit): worded like the session one, reset from the text.
        Other = "other",
    }
}

stored_enum! {
    /// Where the reset time came from.
    pub enum ResetSource ("reset source") {
        /// The tool's window for the limit (Claude Code: the status line's `resets_at`).
        StatusLine = "status_line",
        /// The limit message.
        LimitText = "limit_text",
    }
}

/// A recognised limit message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitHit {
    pub kind: LimitKind,
    /// The message as the tool showed it.
    pub text: String,
}

/// The window a limit of `kind` belongs to among `windows`, as `adapter` matches them.
pub fn matching_window<'a>(
    adapter: &dyn AgentTool,
    kind: LimitKind,
    windows: impl IntoIterator<Item = &'a LimitWindow>,
) -> Option<&'a LimitWindow> {
    let key = adapter.limit_window_key(kind)?;
    windows.into_iter().find(|w| w.key == key)
}

/// When the limit resets, and how that is known: its `window` if its reset lies ahead, else `from_text` (the time
/// in the limit message), else unknown.
pub fn resolve_reset(
    window: Option<&LimitWindow>,
    from_text: Option<i64>,
    now: i64,
) -> Option<(i64, ResetSource)> {
    if let Some(at) = window.and_then(|w| w.resets_at).filter(|&at| at > now) {
        return Some((at, ResetSource::StatusLine));
    }
    Some((from_text?, ResetSource::LimitText))
}

/// Whether an active limit has reset by `now` (bookkeeping only; nothing resumes): its reset time has passed
/// and its `window` (reported at `observed_at`) doesn't still show it full; with an unknown reset time, once the
/// window, reported after the hit, shows room.
pub fn reset_seen(
    limit: &ActiveLimit,
    window: Option<&LimitWindow>,
    observed_at: Option<i64>,
    now: i64,
) -> bool {
    if limit.reset_seen_at.is_some() {
        return false;
    }
    let still_full =
        window.is_some_and(|w| w.used_percent >= 100.0 && w.resets_at.is_none_or(|at| at > now));
    let has_room = observed_at.is_some_and(|at| at > limit.hit_at)
        && window.is_some_and(|w| w.used_percent < 100.0);
    match limit.resets_at {
        Some(at) => now >= at && !still_full,
        None => has_room,
    }
}

/// When `windows` were reported.
fn observed_at(windows: &[ToolWindow]) -> Option<i64> {
    windows.iter().map(|w| w.observed_at).max()
}

/// The plan limit in the service: the hit, the reset time, the reset bookkeeping and Resume all. While the limit
/// is active the inbox leaves out the agents it stopped (ATT-09, d0046) and the scheduler starts nothing (SCH-04).
/// Cheap to clone.
#[derive(Clone)]
pub struct PlanLimit {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    store: Store,
    events: EventBus,
    inbox: Inbox,
    sessions: Sessions,
    deliveries: Deliveries,
    /// Which window a limit belongs to, and the reset time in a limit message (USG-04).
    adapters: Adapters,
    /// The Project agents Resume all continues too (set by the service).
    project_agents: std::sync::Mutex<Option<crate::project_agents::ProjectAgents>>,
    /// One change at a time.
    lock: tokio::sync::Mutex<()>,
}

impl PlanLimit {
    pub fn new(
        board: Board,
        events: EventBus,
        inbox: Inbox,
        sessions: Sessions,
        deliveries: Deliveries,
        adapters: Adapters,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                store: board.store().clone(),
                board,
                events,
                inbox,
                sessions,
                deliveries,
                adapters,
                project_agents: std::sync::Mutex::new(None),
                lock: tokio::sync::Mutex::new(()),
            }),
        }
    }

    /// The Project agents whose limit stops Resume all continues (d0068).
    pub fn set_project_agents(&self, agents: crate::project_agents::ProjectAgents) {
        *self.inner.project_agents.lock().expect("project agents") = Some(agents);
    }

    /// At service start: a limit still active keeps its agents out of the inbox.
    pub async fn restore(&self) -> Result<(), StoreError> {
        if !plan::limits(&self.inner.store).await?.is_empty() {
            self.inner.inbox.set_suppress_limit_stopped(true);
        }
        Ok(())
    }

    /// The current state, if anything was ever recorded.
    pub async fn state(&self) -> Result<Option<plan::PlanLimits>, StoreError> {
        plan::latest(&self.inner.store).await
    }

    /// `ticket_id`'s agent hit `tool`'s usage limit (USG-03): it is marked as stopped by the limit (no inbox item,
    /// no notification), and the tool's limit becomes active with its kind and reset time (the tool's window, else
    /// the limit message, else unknown). A limit of the tool that is already active stays as it is, unless it had
    /// already reset (a new hit after the reset starts a new limit). Returns whether the limit's state changed.
    pub async fn hit(
        &self,
        tool: Tool,
        hit: &LimitHit,
        ticket_id: &str,
        now: i64,
    ) -> Result<bool, StoreError> {
        let _one = self.inner.lock.lock().await;
        self.inner.inbox.mark_limit_stopped(ticket_id, true);
        self.record_locked(tool, hit, now).await
    }

    /// The limit stopped a session that isn't a ticket's (a Project agent, d0068): the limit becomes active as
    /// for agents. Returns whether it became active now.
    pub async fn record_hit(
        &self,
        tool: Tool,
        hit: &LimitHit,
        now: i64,
    ) -> Result<bool, StoreError> {
        let _one = self.inner.lock.lock().await;
        self.record_locked(tool, hit, now).await
    }

    async fn record_locked(
        &self,
        tool: Tool,
        hit: &LimitHit,
        now: i64,
    ) -> Result<bool, StoreError> {
        let store = &self.inner.store;
        self.inner.inbox.set_suppress_limit_stopped(true);
        if plan::limit_of(store, tool)
            .await?
            .is_some_and(|l| l.reset_seen_at.is_none())
        {
            return Ok(false);
        }
        let windows = plan::windows(store, tool).await?;
        let adapter = self.inner.adapters.get(tool);
        let window = matching_window(adapter, hit.kind, windows.iter().map(|w| &w.window));
        let reset = resolve_reset(window, adapter.limit_reset_from_text(&hit.text, now), now);
        plan::set_limit(
            store,
            &ActiveLimit {
                tool,
                kind: hit.kind,
                text: hit.text.clone(),
                hit_at: now,
                resets_at: reset.map(|(at, _)| at),
                reset_source: reset.map(|(_, source)| source),
                reset_seen_at: None,
            },
        )
        .await?;
        tracing::warn!(%tool, kind = %hit.kind, resets_at = ?reset, "plan limit reached");
        self.inner.events.publish(Event::UsageChanged);
        Ok(true)
    }

    /// `tool`'s windows reported (USG-04, USG-09; Claude Code's status line): they replace the tool's set, then
    /// the limit is checked for its reset.
    pub async fn record_windows(
        &self,
        tool: Tool,
        windows: &[LimitWindow],
        now: i64,
    ) -> Result<bool, StoreError> {
        let changed = {
            let _one = self.inner.lock.lock().await;
            plan::record(&self.inner.store, tool, windows, now).await?
        };
        let reset = self.check_reset(now).await?;
        if changed && !reset {
            self.inner.events.publish(Event::UsageChanged);
        }
        Ok(changed || reset)
    }

    /// Marks each active limit as reset once [`reset_seen`] says so. Returns whether it did.
    pub async fn check_reset(&self, now: i64) -> Result<bool, StoreError> {
        let _one = self.inner.lock.lock().await;
        let store = &self.inner.store;
        let mut any = false;
        for mut limit in plan::limits(store).await? {
            let windows = plan::windows(store, limit.tool).await?;
            let adapter = self.inner.adapters.get(limit.tool);
            let window = matching_window(adapter, limit.kind, windows.iter().map(|w| &w.window));
            if !reset_seen(&limit, window, observed_at(&windows), now) {
                continue;
            }
            limit.reset_seen_at = Some(now);
            plan::set_limit(store, &limit).await?;
            tracing::info!(tool = %limit.tool, "plan limit reset (nothing resumes until Resume all)");
            any = true;
        }
        if any {
            self.inner.events.publish(Event::UsageChanged);
        }
        Ok(any)
    }

    /// **Resume all** (USG-06; the user's click, d0020): queues one "continue" for every agent still waiting for
    /// the limit reset, as an automatic delivery (typed once the agent is idle and its input line empty, d0048),
    /// and clears the limit, its banner and the inbox's hiding. Agents Claude Code already continued by itself are
    /// working again and get nothing (Q27). Returns the tickets that got a "continue".
    pub async fn resume_all(&self) -> Result<Vec<TicketKey>, BoardError> {
        let inner = &self.inner;
        let _one = inner.lock.lock().await;
        // A validation that waits for the reset isn't the agent's: its validator starts instead (d0063).
        let waiting: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM tickets WHERE state = ? AND COALESCE(interrupted_from, '') <> ?",
        )
        .bind(Status::WaitingForLimitReset.as_str())
        .bind(Status::Validating.as_str())
        .fetch_all(inner.store.pool())
        .await?;
        let mut continued = Vec::new();
        for (id,) in waiting {
            let Some(session) = inner.sessions.running_of(&id, SessionKind::Agent) else {
                continue;
            };
            inner
                .deliveries
                .queue_delivery(&session.id, DeliveryKind::ContinueAfterLimit, CONTINUE)
                .await
                .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
            continued.push(inner.board.ticket_by_id(&id).await?.key);
        }
        continued.sort();
        // Project agents the limit stopped continue the same way (d0068).
        let agents = inner.project_agents.lock().expect("project agents").clone();
        if let Some(agents) = agents {
            for (agent, session) in agents.limit_stopped().await? {
                if !inner.sessions.info(&session).is_ok_and(|s| s.is_running()) {
                    continue;
                }
                inner
                    .deliveries
                    .queue_delivery(&session, DeliveryKind::ContinueAfterLimit, CONTINUE)
                    .await
                    .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
                tracing::info!(agent, "project agent continues after the limit");
            }
            // The leads' wake-ups held during the limit are typed now (d0083).
            agents.leads().deliver_held().await?;
        }
        plan::clear_limits(&inner.store).await?;
        inner.inbox.set_suppress_limit_stopped(false);
        for id in inner.board.limit_stops().snapshot().marked {
            inner.inbox.mark_limit_stopped(&id, false);
        }
        tracing::info!(
            continued = continued.len(),
            "resume all after the usage limit"
        );
        inner.events.publish(Event::UsageChanged);
        Ok(continued)
    }

    /// **"This is the usage limit"** (USG-07; d0020), the user's click on an Idle inbox item when detection missed:
    /// every Idle agent becomes Waiting for limit reset (logged in its History), stops raising an inbox item, and
    /// the limit becomes active, reset time unknown unless the tool's windows show a full one with its reset.
    /// The limit is the Idle agents' tool's (the first marked agent's; with none, the first tool). Returns the
    /// tickets it folded into the banner. Marking per tool is M23 (USG-10).
    pub async fn mark_idle(
        &self,
        actor: &crate::domain::Actor,
    ) -> Result<Vec<TicketKey>, BoardError> {
        use crate::board::tickets::{EventKind, record_event};
        use crate::store::tickets::set_state_since;
        let inner = &self.inner;
        let now = now_ms();
        let idle: Vec<(String,)> = sqlx::query_as("SELECT id FROM tickets WHERE state = ?")
            .bind(Status::Idle.as_str())
            .fetch_all(inner.store.pool())
            .await?;
        let mut marked = Vec::new();
        let mut tool = None;
        for (id,) in idle {
            let waiting = Status::WaitingForLimitReset;
            let mut tx = inner.store.begin_write().await?;
            let updated = sqlx::query(concat!(
                "UPDATE tickets SET board_column = ?, state = ?, signal_since = ?, updated_at = ?, ",
                set_state_since!(),
                " WHERE id = ? AND state = ?"
            ))
            .bind(waiting.column().as_str())
            .bind(waiting.as_str())
            .bind(now)
            .bind(now)
            .bind(waiting.as_str())
            .bind(now)
            .bind(&id)
            .bind(Status::Idle.as_str())
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if updated == 0 {
                continue;
            }
            record_event(
                &mut tx,
                &id,
                EventKind::LimitMarked,
                actor,
                serde_json::json!({}),
            )
            .await?;
            tx.commit().await?;
            inner.inbox.mark_limit_stopped(&id, true);
            tool = tool.or_else(|| {
                inner
                    .sessions
                    .running_of(&id, SessionKind::Agent)
                    .and_then(|s| s.tool)
            });
            let ticket = inner.board.ticket_by_id(&id).await?;
            let project = ticket.key.project.to_string();
            inner.events.publish(Event::TicketChanged {
                project: project.clone(),
                id: id.clone(),
                key: ticket.key.clone(),
            });
            inner.events.publish(Event::CardStatusChanged {
                project,
                id,
                key: ticket.key.clone(),
                status: waiting,
            });
            marked.push(ticket.key);
        }
        marked.sort();
        // The limit itself, unless one is already active and hasn't reset.
        let tool = tool.unwrap_or(Tool::ALL[0]);
        let windows = plan::windows(&inner.store, tool).await?;
        let adapter = inner.adapters.get(tool);
        let full = |kind| {
            matching_window(adapter, kind, windows.iter().map(|w| &w.window))
                .filter(|w| w.used_percent >= 100.0)
                .and_then(|w| w.resets_at)
                .filter(|&at| at > now)
        };
        let (kind, resets_at) = match (full(LimitKind::Session), full(LimitKind::Weekly)) {
            (_, Some(at)) => (LimitKind::Weekly, Some(at)),
            (Some(at), None) => (LimitKind::Session, Some(at)),
            (None, None) => (LimitKind::Other, None),
        };
        let _one = inner.lock.lock().await;
        inner.inbox.set_suppress_limit_stopped(true);
        let active = plan::limit_of(&inner.store, tool)
            .await?
            .is_some_and(|l| l.reset_seen_at.is_none());
        if !active {
            plan::set_limit(
                &inner.store,
                &ActiveLimit {
                    tool,
                    kind,
                    text: "Marked as the usage limit by you".to_owned(),
                    hit_at: now,
                    resets_at,
                    reset_source: resets_at.map(|_| ResetSource::StatusLine),
                    reset_seen_at: None,
                },
            )
            .await?;
        }
        tracing::info!(
            marked = marked.len(),
            "Idle agents marked as stopped by the usage limit"
        );
        inner.events.publish(Event::UsageChanged);
        Ok(marked)
    }

    /// Looks for the reset every [`RESET_TICK`] until `stop` is set.
    pub async fn run(self, mut stop: watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(RESET_TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = stop.wait_for(|s| *s) => return,
                _ = tick.tick() => {}
            }
            if let Err(e) = self.check_reset(now_ms()).await {
                tracing::warn!(error = %e, "can't check the plan limit");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(used: f64, resets_at: i64) -> LimitWindow {
        LimitWindow {
            key: "w".into(),
            length_minutes: Some(300),
            used_percent: used,
            resets_at: Some(resets_at),
            credits: None,
        }
    }

    #[test]
    fn the_window_comes_first_then_the_text_then_unknown() {
        let now = 1_791_547_200_000;
        let full = window(100.0, now + 3_600_000);
        let from_text = Some(now + 7_200_000);
        assert_eq!(
            resolve_reset(Some(&full), from_text, now),
            Some((now + 3_600_000, ResetSource::StatusLine))
        );
        // A window that already reset doesn't count: the text decides.
        let old = window(100.0, now - 1);
        assert_eq!(
            resolve_reset(Some(&old), from_text, now),
            Some((now + 7_200_000, ResetSource::LimitText))
        );
        assert_eq!(
            resolve_reset(None, from_text, now).map(|(_, s)| s),
            Some(ResetSource::LimitText)
        );
        assert_eq!(resolve_reset(None, None, now), None);
        assert_eq!(resolve_reset(Some(&old), None, now), None);
    }

    #[test]
    fn a_reset_is_seen_after_its_time_unless_the_window_is_still_full() {
        let limit = |resets_at| ActiveLimit {
            tool: Tool::Claude,
            kind: LimitKind::Session,
            text: String::new(),
            hit_at: 100,
            resets_at,
            reset_source: None,
            reset_seen_at: None,
        };
        // Before the time: no.
        assert!(!reset_seen(&limit(Some(1000)), None, None, 999));
        // After it, with no window (the tool dropped it at the reset): yes.
        assert!(reset_seen(&limit(Some(1000)), None, Some(50), 1000));
        // After it, but the window is still full with a later reset: no.
        assert!(!reset_seen(
            &limit(Some(1000)),
            Some(&window(100.0, 5000)),
            Some(1100),
            1200
        ));
        assert!(reset_seen(
            &limit(Some(1000)),
            Some(&window(4.0, 5000)),
            Some(1100),
            1200
        ));
        // Unknown time: once the window (reported after the hit) shows room.
        assert!(!reset_seen(
            &limit(None),
            Some(&window(4.0, 5000)),
            Some(50),
            1200
        ));
        assert!(reset_seen(
            &limit(None),
            Some(&window(4.0, 5000)),
            Some(150),
            1200
        ));
        // Seen once only.
        let mut seen = limit(Some(1000));
        seen.reset_seen_at = Some(1000);
        assert!(!reset_seen(&seen, None, None, 2000));
    }
}
