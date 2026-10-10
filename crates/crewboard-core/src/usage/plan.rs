//! Plan limits (USG-04, USG-05; d0020): the 5-hour and weekly windows of a Pro or Max plan, as Claude Code's
//! status line reports them, stored in `plan_limits` (the newest row is the current state).
//!
//! Source: Claude Code docs, "Customize your status line → Available data" (code.claude.com/docs/en/statusline,
//! read 2026-10-10; Claude Code 2.1.296): `rate_limits.five_hour` and `rate_limits.seven_day`, each with
//! `used_percentage` (0–100) and `resets_at` (Unix epoch **seconds**). `rate_limits` appears only for claude.ai Pro
//! and Max subscribers (or behind a gateway with a spend limit), only after the session's first API response; each
//! window may be absent on its own, and Claude Code drops a window once its `resets_at` passes. API-key users get
//! no `rate_limits`, so nothing is recorded for them.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::limit::{LimitKind, ResetSource};
use crate::store::{Store, StoreError, new_id};

/// One window: how much of it is used and when it resets.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Window {
    /// 0–100.
    pub used_percentage: f64,
    /// When the window resets, in Unix milliseconds.
    pub resets_at: i64,
}

/// The windows of one status-line update.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct RateLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<Window>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seven_day: Option<Window>,
}

impl RateLimits {
    /// The `rate_limits` of a status-line JSON; `None` when it has no window (API-key users, or before the first
    /// response).
    pub fn from_status_line(payload: &Value) -> Option<Self> {
        let limits = payload.get("rate_limits")?;
        let window = |name: &str| {
            let w = limits.get(name)?;
            Some(Window {
                used_percentage: w.get("used_percentage")?.as_f64()?,
                resets_at: w.get("resets_at")?.as_f64()? as i64 * 1000,
            })
        };
        let found = Self {
            five_hour: window("five_hour"),
            seven_day: window("seven_day"),
        };
        (found.five_hour.is_some() || found.seven_day.is_some()).then_some(found)
    }
}

/// The current plan-limit state: the newest `plan_limits` row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanLimits {
    #[serde(flatten)]
    pub windows: RateLimits,
    /// The usage limit that was hit, while the banner is up (until the user's Resume all, d0020).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<ActiveLimit>,
    /// When this state was observed, in Unix milliseconds.
    pub observed_at: i64,
}

impl PlanLimits {
    /// `plan_limits.limit_active`.
    pub fn limit_active(&self) -> bool {
        self.limit.is_some()
    }
}

/// The limit that was hit (USG-03, USG-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveLimit {
    pub kind: LimitKind,
    /// Claude Code's message.
    pub text: String,
    pub hit_at: i64,
    /// When it resets; `None` = "resets at an unknown time".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_source: Option<ResetSource>,
    /// When the reset was observed (bookkeeping only; nothing resumes by itself).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_seen_at: Option<i64>,
}

/// One `plan_limits` row.
#[derive(sqlx::FromRow)]
struct Row {
    five_hour_used_percent: Option<f64>,
    five_hour_resets_at: Option<i64>,
    seven_day_used_percent: Option<f64>,
    seven_day_resets_at: Option<i64>,
    limit_active: bool,
    limit_kind: Option<String>,
    limit_text: Option<String>,
    limit_hit_at: Option<i64>,
    limit_resets_at: Option<i64>,
    limit_reset_source: Option<String>,
    limit_reset_seen_at: Option<i64>,
    observed_at: i64,
}

/// The newest state, if anything was ever recorded.
pub async fn latest(store: &Store) -> Result<Option<PlanLimits>, StoreError> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT five_hour_used_percent, five_hour_resets_at, seven_day_used_percent, seven_day_resets_at,
                limit_active, limit_kind, limit_text, limit_hit_at, limit_resets_at, limit_reset_source,
                limit_reset_seen_at, observed_at
         FROM plan_limits ORDER BY observed_at DESC, id DESC LIMIT 1",
    )
    .fetch_optional(store.pool())
    .await?;
    let window = |used: Option<f64>, resets: Option<i64>| {
        Some(Window {
            used_percentage: used?,
            resets_at: resets?,
        })
    };
    Ok(row.map(|r| PlanLimits {
        windows: RateLimits {
            five_hour: window(r.five_hour_used_percent, r.five_hour_resets_at),
            seven_day: window(r.seven_day_used_percent, r.seven_day_resets_at),
        },
        limit: r.limit_active.then(|| ActiveLimit {
            kind: r
                .limit_kind
                .and_then(|k| k.parse().ok())
                .unwrap_or(LimitKind::Other),
            text: r.limit_text.unwrap_or_default(),
            hit_at: r.limit_hit_at.unwrap_or(r.observed_at),
            resets_at: r.limit_resets_at,
            reset_source: r.limit_reset_source.and_then(|s| s.parse().ok()),
            reset_seen_at: r.limit_reset_seen_at,
        }),
        observed_at: r.observed_at,
    }))
}

/// Records a status-line update. A new row is written only when the windows changed; the limit carries over.
/// Returns whether anything changed.
pub async fn record(store: &Store, limits: &RateLimits, now: i64) -> Result<bool, StoreError> {
    let previous = latest(store).await?;
    if previous.as_ref().is_some_and(|p| p.windows == *limits) {
        return Ok(false);
    }
    insert(
        store,
        &PlanLimits {
            windows: *limits,
            limit: previous.and_then(|p| p.limit),
            observed_at: now,
        },
    )
    .await?;
    Ok(true)
}

/// Writes a new current state.
pub async fn insert(store: &Store, state: &PlanLimits) -> Result<(), StoreError> {
    let w = &state.windows;
    let l = state.limit.as_ref();
    sqlx::query(
        "INSERT INTO plan_limits (id, five_hour_used_percent, five_hour_resets_at, seven_day_used_percent,
                                  seven_day_resets_at, limit_active, limit_kind, limit_text, limit_hit_at,
                                  limit_resets_at, limit_reset_source, limit_reset_seen_at, observed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(new_id())
    .bind(w.five_hour.map(|x| x.used_percentage))
    .bind(w.five_hour.map(|x| x.resets_at))
    .bind(w.seven_day.map(|x| x.used_percentage))
    .bind(w.seven_day.map(|x| x.resets_at))
    .bind(l.is_some())
    .bind(l.map(|l| l.kind.as_str()))
    .bind(l.map(|l| l.text.clone()))
    .bind(l.map(|l| l.hit_at))
    .bind(l.and_then(|l| l.resets_at))
    .bind(l.and_then(|l| l.reset_source).map(|s| s.as_str()))
    .bind(l.and_then(|l| l.reset_seen_at))
    .bind(state.observed_at)
    .execute(store.pool())
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn rate_limits_come_from_the_status_line() {
        let payload = json!({"model": {"display_name": "Opus"},
            "rate_limits": {"five_hour": {"used_percentage": 23.5, "resets_at": 1738425600},
                            "seven_day": {"used_percentage": 41.2, "resets_at": 1738857600}}});
        assert_eq!(
            RateLimits::from_status_line(&payload),
            Some(RateLimits {
                five_hour: Some(Window {
                    used_percentage: 23.5,
                    resets_at: 1_738_425_600_000
                }),
                seven_day: Some(Window {
                    used_percentage: 41.2,
                    resets_at: 1_738_857_600_000
                }),
            })
        );
        // One window dropped after its reset.
        let one = json!({"rate_limits": {"seven_day": {"used_percentage": 41.2, "resets_at": 1738857600}}});
        assert_eq!(
            RateLimits::from_status_line(&one).map(|r| r.five_hour),
            Some(None)
        );
        // API-key users, and before the first response.
        assert_eq!(RateLimits::from_status_line(&json!({"cwd": "/wt"})), None);
        assert_eq!(
            RateLimits::from_status_line(&json!({"rate_limits": {}})),
            None
        );
    }
}
