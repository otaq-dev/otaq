//! The plan limit being hit (USG-03, USG-04; d0020, d0046): which limit, and when it resets.
//!
//! "It happened" comes from the plugin's `StopFailure` hook: error `rate_limit` with Claude Code's limit text in
//! `last_assistant_message` (or `error_details`). The wording follows the Claude Code docs, "Error reference →
//! usage limits" (code.claude.com/docs/en/errors, read 2026-10-10; Claude Code 2.1.296):
//!
//! ```text
//! You've hit your session limit · resets 3:45pm
//! You've hit your weekly limit · resets Mon 12:00am
//! You've hit your Opus limit · resets 3:45pm
//! ```
//!
//! Other `rate_limit` failures ("API Error: Request rejected (429) · this may be a temporary capacity issue…",
//! "Server is temporarily limiting requests (not your usage limit)") are not the plan limit: Claude Code retries
//! those itself, and they keep their existing handling.
//!
//! The reset time, in this order: the status line's `resets_at` for that window ([`super::plan`]), else the time in
//! the limit text (the next local occurrence of "3:45pm", or of "Mon 12:00am"), else unknown.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::plan::{self, ActiveLimit, PlanLimits, RateLimits};
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
        /// The status line's `rate_limits.*.resets_at`.
        StatusLine = "status_line",
        /// Parsed from the limit text.
        LimitText = "limit_text",
    }
}

/// A recognised limit message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitHit {
    pub kind: LimitKind,
    /// The message as Claude Code showed it.
    pub text: String,
}

/// Whether a `StopFailure` with `error` and `message` is the plan limit, and which one.
pub fn recognise(error: &str, message: &str) -> Option<LimitHit> {
    if error != "rate_limit" {
        return None;
    }
    let lower = message.to_lowercase();
    // "You've hit your … limit"; the apostrophe may be typographic.
    let hit = lower.contains("hit your") && lower.contains("limit");
    if !hit {
        return None;
    }
    let kind = if lower.contains("weekly limit") {
        LimitKind::Weekly
    } else if lower.contains("session limit") {
        LimitKind::Session
    } else {
        LimitKind::Other
    };
    Some(LimitHit {
        kind,
        text: message.trim().to_owned(),
    })
}

/// The time named after "resets" in a limit text: an optional weekday (0 = Sunday) and a time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetText {
    pub weekday: Option<u32>,
    pub hour: u32,
    pub minute: u32,
}

/// Reads "resets 3:45pm", "resets Mon 12:00am", "resets 3pm (Europe/Istanbul)".
pub fn parse_reset(text: &str) -> Option<ResetText> {
    let lower = text.to_lowercase();
    let after = &lower[lower.find("resets")? + "resets".len()..];
    let mut words = after.split_whitespace().peekable();
    let weekday = words.peek().and_then(|w| weekday(w));
    if weekday.is_some() {
        words.next();
    }
    let time = words.next()?;
    let (clock, pm) = match time.strip_suffix("pm") {
        Some(t) => (t, true),
        None => (time.strip_suffix("am")?, false),
    };
    let (h, m) = match clock.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (clock.parse::<u32>().ok()?, 0),
    };
    if !(1..=12).contains(&h) || m > 59 {
        return None;
    }
    let hour = match (h, pm) {
        (12, false) => 0,
        (12, true) => 12,
        (h, false) => h,
        (h, true) => h + 12,
    };
    Some(ResetText {
        weekday,
        hour,
        minute: m,
    })
}

fn weekday(word: &str) -> Option<u32> {
    let names = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
    let w = word.trim_end_matches(',');
    names
        .iter()
        .position(|n| {
            w.starts_with(n) && w.len() <= 9 && w.chars().all(|c| c.is_ascii_alphabetic())
        })
        .map(|i| i as u32)
}

/// The next local moment after `now` that matches `reset` (Unix milliseconds), within eight days.
pub fn next_local(reset: ResetText, now: i64) -> Option<i64> {
    let start_of_minute = (now.div_euclid(60_000) + 1) * 60_000;
    // Walk whole minutes is too slow over a week; walk days and set the time of day with the local clock.
    (0..=8).find_map(|day| {
        let at =
            crate::local_time::on_same_day_at(now + day * 86_400_000, reset.hour, reset.minute)?;
        let weekday_ok = reset
            .weekday
            .is_none_or(|w| crate::signals::background::local(at).is_some_and(|t| t.weekday == w));
        (at >= start_of_minute && weekday_ok).then_some(at)
    })
}

/// When the limit resets, and how that is known: the status line's window for that limit if it lies ahead,
/// else the time in the text, else unknown.
pub fn resolve_reset(
    hit: &LimitHit,
    windows: Option<&RateLimits>,
    now: i64,
) -> Option<(i64, ResetSource)> {
    let window = windows.and_then(|w| match hit.kind {
        LimitKind::Session => w.five_hour,
        LimitKind::Weekly => w.seven_day,
        LimitKind::Other => None,
    });
    if let Some(w) = window.filter(|w| w.resets_at > now) {
        return Some((w.resets_at, ResetSource::StatusLine));
    }
    let parsed = parse_reset(&hit.text)?;
    Some((next_local(parsed, now)?, ResetSource::LimitText))
}

/// Whether an active limit has reset by `now` (bookkeeping only; nothing resumes): its reset time has passed
/// and the status line doesn't still show that window full; with an unknown reset time, once the status line
/// (observed after the hit) shows the window below 100 %.
pub fn reset_seen(limit: &ActiveLimit, windows: &RateLimits, observed_at: i64, now: i64) -> bool {
    if limit.reset_seen_at.is_some() {
        return false;
    }
    let window = match limit.kind {
        LimitKind::Session => windows.five_hour,
        LimitKind::Weekly => windows.seven_day,
        LimitKind::Other => None,
    };
    let still_full = window.is_some_and(|w| w.used_percentage >= 100.0 && w.resets_at > now);
    let has_room = observed_at > limit.hit_at && window.is_some_and(|w| w.used_percentage < 100.0);
    match limit.resets_at {
        Some(at) => now >= at && !still_full,
        None => has_room,
    }
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
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                store: board.store().clone(),
                board,
                events,
                inbox,
                sessions,
                deliveries,
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
        if self.state().await?.is_some_and(|s| s.limit_active()) {
            self.inner.inbox.set_suppress_limit_stopped(true);
        }
        Ok(())
    }

    /// The current state, if anything was ever recorded.
    pub async fn state(&self) -> Result<Option<PlanLimits>, StoreError> {
        plan::latest(&self.inner.store).await
    }

    /// `ticket_id`'s agent hit the plan limit (USG-03): it is marked as stopped by the limit (no inbox item, no
    /// notification), and the limit becomes active with its kind and reset time (status line, else the text, else
    /// unknown). A limit that is already active stays as it is, unless it had already reset (a new hit after the
    /// reset starts a new limit). Returns whether the limit's state changed.
    pub async fn hit(&self, hit: &LimitHit, ticket_id: &str, now: i64) -> Result<bool, StoreError> {
        let _one = self.inner.lock.lock().await;
        self.inner.inbox.mark_limit_stopped(ticket_id, true);
        self.record_locked(hit, now).await
    }

    /// The limit stopped a session that isn't a ticket's (a Project agent, d0068): the limit becomes active as
    /// for agents. Returns whether it became active now.
    pub async fn record_hit(&self, hit: &LimitHit, now: i64) -> Result<bool, StoreError> {
        let _one = self.inner.lock.lock().await;
        self.record_locked(hit, now).await
    }

    async fn record_locked(&self, hit: &LimitHit, now: i64) -> Result<bool, StoreError> {
        self.inner.inbox.set_suppress_limit_stopped(true);
        let previous = plan::latest(&self.inner.store).await?;
        if previous
            .as_ref()
            .and_then(|p| p.limit.as_ref())
            .is_some_and(|l| l.reset_seen_at.is_none())
        {
            return Ok(false);
        }
        let windows = previous.as_ref().map(|p| p.windows).unwrap_or_default();
        let reset = resolve_reset(hit, previous.as_ref().map(|p| &p.windows), now);
        plan::insert(
            &self.inner.store,
            &PlanLimits {
                windows,
                limit: Some(ActiveLimit {
                    kind: hit.kind,
                    text: hit.text.clone(),
                    hit_at: now,
                    resets_at: reset.map(|(at, _)| at),
                    reset_source: reset.map(|(_, source)| source),
                    reset_seen_at: None,
                }),
                observed_at: now,
            },
        )
        .await?;
        tracing::warn!(kind = %hit.kind, resets_at = ?reset, "plan limit reached");
        self.inner.events.publish(Event::UsageChanged);
        Ok(true)
    }

    /// A status-line update (USG-04): stores the windows, then looks whether the limit has reset.
    pub async fn record_windows(&self, limits: &RateLimits, now: i64) -> Result<bool, StoreError> {
        let changed = {
            let _one = self.inner.lock.lock().await;
            plan::record(&self.inner.store, limits, now).await?
        };
        let reset = self.check_reset(now).await?;
        if changed && !reset {
            self.inner.events.publish(Event::UsageChanged);
        }
        Ok(changed || reset)
    }

    /// Marks an active limit as reset once [`reset_seen`] says so. Returns whether it did.
    pub async fn check_reset(&self, now: i64) -> Result<bool, StoreError> {
        let _one = self.inner.lock.lock().await;
        let Some(mut state) = plan::latest(&self.inner.store).await? else {
            return Ok(false);
        };
        let Some(limit) = state.limit.as_mut() else {
            return Ok(false);
        };
        if !reset_seen(limit, &state.windows, state.observed_at, now) {
            return Ok(false);
        }
        limit.reset_seen_at = Some(now);
        state.observed_at = now;
        plan::insert(&self.inner.store, &state).await?;
        tracing::info!("plan limit reset (nothing resumes until Resume all)");
        self.inner.events.publish(Event::UsageChanged);
        Ok(true)
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
        if let Some(mut state) = plan::latest(&inner.store).await?
            && state.limit.take().is_some()
        {
            state.observed_at = now_ms();
            plan::insert(&inner.store, &state).await?;
        }
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
    /// the limit becomes active, reset time unknown unless the status line shows a full window with its reset.
    /// Returns the tickets it folded into the banner.
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
        for (id,) in idle {
            let waiting = Status::WaitingForLimitReset;
            let mut tx = inner.store.pool().begin().await?;
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
        let state = plan::latest(&inner.store).await?;
        let windows = state.as_ref().map(|s| s.windows).unwrap_or_default();
        let full =
            |w: Option<plan::Window>| w.filter(|w| w.used_percentage >= 100.0 && w.resets_at > now);
        let (kind, resets_at) = match (full(windows.five_hour), full(windows.seven_day)) {
            (_, Some(w)) => (LimitKind::Weekly, Some(w.resets_at)),
            (Some(w), None) => (LimitKind::Session, Some(w.resets_at)),
            (None, None) => (LimitKind::Other, None),
        };
        let hit = LimitHit {
            kind,
            text: "Marked as the usage limit by you".to_owned(),
        };
        let _one = inner.lock.lock().await;
        inner.inbox.set_suppress_limit_stopped(true);
        let active = state
            .as_ref()
            .and_then(|s| s.limit.as_ref())
            .is_some_and(|l| l.reset_seen_at.is_none());
        if !active {
            plan::insert(
                &inner.store,
                &PlanLimits {
                    windows,
                    limit: Some(ActiveLimit {
                        kind: hit.kind,
                        text: hit.text,
                        hit_at: now,
                        resets_at,
                        reset_source: resets_at.map(|_| ResetSource::StatusLine),
                        reset_seen_at: None,
                    }),
                    observed_at: now,
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
    use crate::signals::background::local;
    use crate::usage::plan::Window;

    #[test]
    fn only_the_limit_text_with_rate_limit_is_the_plan_limit() {
        let session = recognise(
            "rate_limit",
            "You've hit your session limit · resets 3:45pm",
        )
        .unwrap();
        assert_eq!(session.kind, LimitKind::Session);
        assert_eq!(
            recognise(
                "rate_limit",
                "You’ve hit your weekly limit · resets Mon 12:00am"
            )
            .unwrap()
            .kind,
            LimitKind::Weekly
        );
        assert_eq!(
            recognise("rate_limit", "You've hit your Opus limit · resets 3:45pm")
                .unwrap()
                .kind,
            LimitKind::Other
        );
        assert_eq!(
            recognise(
                "rate_limit",
                "API Error: Request rejected (429) · this may be a temporary capacity issue."
            ),
            None
        );
        assert_eq!(
            recognise(
                "rate_limit",
                "API Error: Server is temporarily limiting requests (not your usage limit)"
            ),
            None
        );
        assert_eq!(
            recognise(
                "overloaded",
                "You've hit your session limit · resets 3:45pm"
            ),
            None
        );
    }

    #[test]
    fn reset_times_are_read_from_the_text() {
        let t = |s: &str| parse_reset(s);
        assert_eq!(
            t("You've hit your session limit · resets 3:45pm"),
            Some(ResetText {
                weekday: None,
                hour: 15,
                minute: 45
            })
        );
        assert_eq!(
            t("You've hit your weekly limit · resets Mon 12:00am"),
            Some(ResetText {
                weekday: Some(1),
                hour: 0,
                minute: 0
            })
        );
        assert_eq!(
            t("resets 12pm (Europe/Istanbul)"),
            Some(ResetText {
                weekday: None,
                hour: 12,
                minute: 0
            })
        );
        assert_eq!(
            t("resets 9am"),
            Some(ResetText {
                weekday: None,
                hour: 9,
                minute: 0
            })
        );
        assert_eq!(t("You've hit your session limit"), None);
        assert_eq!(t("resets soon"), None);
        assert_eq!(t("resets 13:00pm"), None);
    }

    #[test]
    fn the_next_local_occurrence_is_after_now() {
        let now = 1_791_547_200_000; // 2026-10-09 12:00 UTC
        let at = next_local(
            ResetText {
                weekday: None,
                hour: 15,
                minute: 45,
            },
            now,
        )
        .unwrap();
        assert!(at > now && at - now <= 86_400_000);
        let lt = local(at).unwrap();
        assert_eq!((lt.hour, lt.minute), (15, 45));
        let monday = next_local(
            ResetText {
                weekday: Some(1),
                hour: 0,
                minute: 0,
            },
            now,
        )
        .unwrap();
        assert!(monday > now && monday - now <= 7 * 86_400_000);
        let lt = local(monday).unwrap();
        assert_eq!((lt.weekday, lt.hour, lt.minute), (1, 0, 0));
    }

    #[test]
    fn the_status_line_comes_first_then_the_text_then_unknown() {
        let now = 1_791_547_200_000;
        let hit = recognise(
            "rate_limit",
            "You've hit your session limit · resets 3:45pm",
        )
        .unwrap();
        let windows = RateLimits {
            five_hour: Some(Window {
                used_percentage: 100.0,
                resets_at: now + 3_600_000,
            }),
            seven_day: None,
        };
        assert_eq!(
            resolve_reset(&hit, Some(&windows), now),
            Some((now + 3_600_000, ResetSource::StatusLine))
        );
        // A window that already reset doesn't count: the text decides.
        let old = RateLimits {
            five_hour: Some(Window {
                used_percentage: 100.0,
                resets_at: now - 1,
            }),
            seven_day: None,
        };
        assert_eq!(
            resolve_reset(&hit, Some(&old), now).map(|(_, s)| s),
            Some(ResetSource::LimitText)
        );
        let weekly = recognise("rate_limit", "You've hit your weekly limit").unwrap();
        assert_eq!(resolve_reset(&weekly, Some(&windows), now), None);
    }

    #[test]
    fn a_reset_is_seen_after_its_time_unless_the_window_is_still_full() {
        let limit = |resets_at| ActiveLimit {
            kind: LimitKind::Session,
            text: String::new(),
            hit_at: 100,
            resets_at,
            reset_source: None,
            reset_seen_at: None,
        };
        let windows = |used: f64, resets_at: i64| RateLimits {
            five_hour: Some(Window {
                used_percentage: used,
                resets_at,
            }),
            seven_day: None,
        };
        let none = RateLimits::default();
        // Before the time: no.
        assert!(!reset_seen(&limit(Some(1000)), &none, 50, 999));
        // After it, with no window (Claude Code dropped it at the reset): yes.
        assert!(reset_seen(&limit(Some(1000)), &none, 50, 1000));
        // After it, but the status line still shows the window full with a later reset: no.
        assert!(!reset_seen(
            &limit(Some(1000)),
            &windows(100.0, 5000),
            1100,
            1200
        ));
        assert!(reset_seen(
            &limit(Some(1000)),
            &windows(4.0, 5000),
            1100,
            1200
        ));
        // Unknown time: once the status line (after the hit) shows room.
        assert!(!reset_seen(&limit(None), &windows(4.0, 5000), 50, 1200));
        assert!(reset_seen(&limit(None), &windows(4.0, 5000), 150, 1200));
        // Seen once only.
        let mut seen = limit(Some(1000));
        seen.reset_seen_at = Some(1000);
        assert!(!reset_seen(&seen, &none, 50, 2000));
    }
}
