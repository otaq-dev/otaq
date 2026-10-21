//! Usage-limit windows and the active usage limit, per agent tool (USG-03…USG-06, USG-09; d0020, d0046, d0088).
//!
//! **Windows** (`limit_windows`, one row per tool and key): a tool's current windows are the set from its latest
//! report, and a report replaces the set, so a window the tool drops after its reset disappears; nothing is written
//! when the set is unchanged. Each adapter reads its own source ([`crate::agent_tools::AgentTool::windows_source`]);
//! Claude Code's is its status line. API-key users get no windows, so nothing is recorded for them.
//!
//! **The active limit** (`active_limits`) is stored with its tool, one row per tool, while its banner line is up
//! (USG-10): it holds only the agents of that tool (SCH-04 `[M23]`), and that tool's Resume clears it.

use serde::{Deserialize, Serialize};

use super::limit::{LimitKind, ResetSource};
use crate::agent_tools::{Credits, LimitWindow, Tool};
use crate::store::{Store, StoreError};

/// One window as stored: the tool that reported it and when.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolWindow {
    pub tool: Tool,
    pub window: LimitWindow,
    /// When the tool's current set was reported, in Unix milliseconds.
    pub observed_at: i64,
}

/// The current limit state: every tool's windows and the active limit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanLimits {
    /// Every tool's current windows, by tool, then length.
    #[serde(default)]
    pub windows: Vec<ToolWindow>,
    /// The usage limit that was hit, while the banner is up (until the user's Resume, d0020). With several tools
    /// limited at once: one that hasn't reset before one that has, then the newest (each tool's line is
    /// [`crate::agents::limits::ToolLine`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<ActiveLimit>,
}

impl PlanLimits {
    /// Whether a usage limit is active.
    pub fn limit_active(&self) -> bool {
        self.limit.is_some()
    }

    /// `tool`'s current windows.
    pub fn windows_of(&self, tool: Tool) -> impl Iterator<Item = &LimitWindow> {
        self.windows
            .iter()
            .filter(move |w| w.tool == tool)
            .map(|w| &w.window)
    }
}

/// The limit that was hit (USG-03, USG-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveLimit {
    /// The tool whose limit it is.
    pub tool: Tool,
    pub kind: LimitKind,
    /// The tool's message.
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

/// One `limit_windows` row.
#[derive(sqlx::FromRow)]
struct WindowRow {
    tool: String,
    key: String,
    length_minutes: Option<i64>,
    used_percent: f64,
    resets_at: Option<i64>,
    credits_used: Option<f64>,
    credits_total: Option<f64>,
    observed_at: i64,
}

impl WindowRow {
    fn into_window(self) -> Option<ToolWindow> {
        let tool = self
            .tool
            .parse()
            .inspect_err(|e| tracing::warn!(error = %e, "a limit window of an unknown tool"))
            .ok()?;
        Some(ToolWindow {
            tool,
            window: LimitWindow {
                key: self.key,
                length_minutes: self.length_minutes.and_then(|m| u32::try_from(m).ok()),
                used_percent: self.used_percent,
                resets_at: self.resets_at,
                credits: self.credits_used.map(|used| Credits {
                    used,
                    total: self.credits_total,
                }),
            },
            observed_at: self.observed_at,
        })
    }
}

/// One `active_limits` row.
#[derive(sqlx::FromRow)]
struct LimitRow {
    tool: String,
    kind: String,
    text: String,
    hit_at: i64,
    resets_at: Option<i64>,
    reset_source: Option<String>,
    reset_seen_at: Option<i64>,
}

/// The current state; `None` when no tool has windows and no limit is active.
pub async fn latest(store: &Store) -> Result<Option<PlanLimits>, StoreError> {
    let windows = all_windows(store).await?;
    let mut limits = limits(store).await?;
    // Not reset first, then the newest.
    limits.sort_by_key(|l| (l.reset_seen_at.is_none(), l.hit_at));
    let limit = limits.pop();
    Ok((!windows.is_empty() || limit.is_some()).then_some(PlanLimits { windows, limit }))
}

async fn all_windows(store: &Store) -> Result<Vec<ToolWindow>, StoreError> {
    let rows: Vec<WindowRow> = sqlx::query_as(
        "SELECT tool, key, length_minutes, used_percent, resets_at, credits_used, credits_total, observed_at
         FROM limit_windows ORDER BY tool, length_minutes IS NULL, length_minutes, key",
    )
    .fetch_all(store.pool())
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(WindowRow::into_window)
        .collect())
}

/// `tool`'s current windows.
pub async fn windows(store: &Store, tool: Tool) -> Result<Vec<ToolWindow>, StoreError> {
    Ok(all_windows(store)
        .await?
        .into_iter()
        .filter(|w| w.tool == tool)
        .collect())
}

/// Records `tool`'s report: its windows replace the tool's set. Nothing is written when the set is unchanged.
/// Returns whether anything changed.
pub async fn record(
    store: &Store,
    tool: Tool,
    report: &[LimitWindow],
    now: i64,
) -> Result<bool, StoreError> {
    let by_key = |set: &mut Vec<LimitWindow>| set.sort_by(|a, b| a.key.cmp(&b.key));
    let mut current: Vec<LimitWindow> = windows(store, tool)
        .await?
        .into_iter()
        .map(|w| w.window)
        .collect();
    let mut new = report.to_vec();
    by_key(&mut current);
    by_key(&mut new);
    if current == new {
        return Ok(false);
    }
    let mut tx = store.begin_write().await?;
    sqlx::query("DELETE FROM limit_windows WHERE tool = ?")
        .bind(tool.as_str())
        .execute(&mut *tx)
        .await?;
    for w in &new {
        sqlx::query(
            "INSERT INTO limit_windows (tool, key, length_minutes, used_percent, resets_at, credits_used,
                                        credits_total, observed_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (tool, key) DO NOTHING",
        )
        .bind(tool.as_str())
        .bind(&w.key)
        .bind(w.length_minutes.map(i64::from))
        .bind(w.used_percent)
        .bind(w.resets_at)
        .bind(w.credits.map(|c| c.used))
        .bind(w.credits.and_then(|c| c.total))
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(true)
}

/// Every active limit.
pub async fn limits(store: &Store) -> Result<Vec<ActiveLimit>, StoreError> {
    let rows: Vec<LimitRow> = sqlx::query_as(
        "SELECT tool, kind, text, hit_at, resets_at, reset_source, reset_seen_at FROM active_limits
         ORDER BY hit_at, tool",
    )
    .fetch_all(store.pool())
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some(ActiveLimit {
                tool: r.tool.parse().ok()?,
                kind: r.kind.parse().unwrap_or(LimitKind::Other),
                text: r.text,
                hit_at: r.hit_at,
                resets_at: r.resets_at,
                reset_source: r.reset_source.and_then(|s| s.parse().ok()),
                reset_seen_at: r.reset_seen_at,
            })
        })
        .collect())
}

/// `tool`'s active limit.
pub async fn limit_of(store: &Store, tool: Tool) -> Result<Option<ActiveLimit>, StoreError> {
    Ok(limits(store).await?.into_iter().find(|l| l.tool == tool))
}

/// Stores `limit` as its tool's active limit (replacing the one before).
pub async fn set_limit(store: &Store, limit: &ActiveLimit) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO active_limits (tool, kind, text, hit_at, resets_at, reset_source, reset_seen_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (tool) DO UPDATE SET kind = excluded.kind, text = excluded.text, hit_at = excluded.hit_at,
             resets_at = excluded.resets_at, reset_source = excluded.reset_source,
             reset_seen_at = excluded.reset_seen_at",
    )
    .bind(limit.tool.as_str())
    .bind(limit.kind.as_str())
    .bind(&limit.text)
    .bind(limit.hit_at)
    .bind(limit.resets_at)
    .bind(limit.reset_source.map(|s| s.as_str()))
    .bind(limit.reset_seen_at)
    .execute(store.pool())
    .await?;
    Ok(())
}

/// Clears `tool`'s active limit (its Resume). Returns whether there was one.
pub async fn clear_limit(store: &Store, tool: Tool) -> Result<bool, StoreError> {
    let cleared = sqlx::query("DELETE FROM active_limits WHERE tool = ?")
        .bind(tool.as_str())
        .execute(store.pool())
        .await?
        .rows_affected();
    Ok(cleared > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_dir::DataDir;

    async fn temp_store() -> (tempfile::TempDir, Store) {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open_in(&DataDir::new(tmp.path().join("home")))
            .await
            .unwrap();
        (tmp, store)
    }

    fn window(key: &str, minutes: u32, used: f64) -> LimitWindow {
        LimitWindow {
            key: key.into(),
            length_minutes: Some(minutes),
            used_percent: used,
            resets_at: Some(1_000_000),
            credits: None,
        }
    }

    #[tokio::test]
    async fn a_report_replaces_the_set_and_an_unchanged_one_writes_nothing() {
        let (_tmp, store) = temp_store().await;
        assert_eq!(latest(&store).await.unwrap(), None);
        let both = [window("long", 10_080, 40.0), window("short", 300, 82.0)];
        assert!(record(&store, Tool::Claude, &both, 10).await.unwrap());
        let state = latest(&store).await.unwrap().unwrap();
        // Shortest first.
        let keys: Vec<&str> = state
            .windows_of(Tool::Claude)
            .map(|w| w.key.as_str())
            .collect();
        assert_eq!(keys, ["short", "long"]);
        assert!(state.windows.iter().all(|w| w.observed_at == 10));
        assert!(!state.limit_active());

        // The same set in another order: nothing written.
        let reordered = [both[1].clone(), both[0].clone()];
        assert!(!record(&store, Tool::Claude, &reordered, 20).await.unwrap());
        let again = latest(&store).await.unwrap().unwrap();
        assert!(again.windows.iter().all(|w| w.observed_at == 10));

        // The tool dropped a window after its reset: it disappears.
        assert!(record(&store, Tool::Claude, &both[..1], 30).await.unwrap());
        let dropped = windows(&store, Tool::Claude).await.unwrap();
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].window, both[0]);
        assert_eq!(dropped[0].observed_at, 30);

        // Credits round-trip.
        let credited = LimitWindow {
            length_minutes: None,
            resets_at: None,
            credits: Some(Credits {
                used: 0.4,
                total: Some(200.0),
            }),
            ..window("month", 0, 0.2)
        };
        record(&store, Tool::Claude, std::slice::from_ref(&credited), 40)
            .await
            .unwrap();
        assert_eq!(
            windows(&store, Tool::Claude).await.unwrap()[0].window,
            credited
        );
    }

    #[tokio::test]
    async fn the_active_limit_is_stored_with_its_tool_until_cleared() {
        let (_tmp, store) = temp_store().await;
        let limit = ActiveLimit {
            tool: Tool::Claude,
            kind: LimitKind::Weekly,
            text: "You've hit your weekly limit".into(),
            hit_at: 5,
            resets_at: Some(100),
            reset_source: Some(ResetSource::LimitText),
            reset_seen_at: None,
        };
        set_limit(&store, &limit).await.unwrap();
        assert_eq!(
            limit_of(&store, Tool::Claude).await.unwrap(),
            Some(limit.clone())
        );
        let state = latest(&store).await.unwrap().unwrap();
        assert!(state.windows.is_empty());
        assert_eq!(state.limit, Some(limit.clone()));

        let seen = ActiveLimit {
            reset_seen_at: Some(120),
            ..limit.clone()
        };
        set_limit(&store, &seen).await.unwrap();
        assert_eq!(limits(&store).await.unwrap(), [seen]);

        // Another tool's limit stays when one tool's is cleared.
        let other = ActiveLimit {
            tool: Tool::ALL[1],
            ..limit
        };
        set_limit(&store, &other).await.unwrap();
        assert!(clear_limit(&store, Tool::Claude).await.unwrap());
        assert!(!clear_limit(&store, Tool::Claude).await.unwrap());
        assert_eq!(limits(&store).await.unwrap(), [other]);
        assert!(clear_limit(&store, Tool::ALL[1]).await.unwrap());
        assert_eq!(latest(&store).await.unwrap(), None);
    }
}
