//! Activity's data per agent tool (SHL-03, USG-09; d0087, d0088): agents running, today's tokens and the limit
//! windows, keyed by tool (`usage.activity`). Read-only. The view model words it ("No plan limit · tokens only",
//! "Limits not reported"); processes stay `run.list` (RUN-04) and agent limits the scheduler status (SCH-03).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::plan::{self, ActiveLimit};
use super::{DayRow, local_day, tokens};
use crate::agent_tools::{Adapters, LimitWindow, Tool, WindowsSource};
use crate::store::{Store, StoreError, now_ms};

/// The answer to `usage.activity`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    /// Today, as `YYYY-MM-DD` in local time.
    pub day: String,
    /// Every tool, in [`Tool::ALL`] order.
    pub tools: Vec<ToolActivity>,
    /// Today's tokens across every tool (the shown number, [`super::Tokens::shown`]).
    pub tokens_today: u64,
}

impl Activity {
    /// `tool`'s part.
    pub fn tool(&self, tool: Tool) -> Option<&ToolActivity> {
        self.tools.iter().find(|t| t.tool == tool)
    }

    /// Agents running across every tool: the header's "N running".
    pub fn running(&self) -> u32 {
        self.tools.iter().map(|t| t.running).sum()
    }
}

/// One tool's activity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolActivity {
    pub tool: Tool,
    /// The tool's agent sessions running across all projects, counted as the header counts them (live ticket
    /// agent sessions, [`crate::board::snapshot::ProjectInfo::agents_working`]).
    pub running: u32,
    /// The same split by project, for the hover ("pocket-notes 2 · notes-api 1"): projects with at least one,
    /// oldest project first.
    pub by_project: Vec<ProjectRunning>,
    /// Today's tokens of the tool's sessions (the shown number).
    pub tokens_today: u64,
    /// The tool's limit windows, or why there are none.
    pub limits: ToolLimits,
    /// The tool's active usage limit, while its banner is up (USG-03).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<ActiveLimit>,
}

/// A project's running agents of one tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRunning {
    pub key: String,
    pub name: String,
    pub running: u32,
}

/// A tool's limit windows (d0087).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "windows", rename_all = "snake_case")]
pub enum ToolLimits {
    /// The tool's current windows as stored, shortest first.
    Windows(Vec<LimitWindow>),
    /// The tool reports limits but has none now: an API-key login, or before its first report ("No plan limit ·
    /// tokens only").
    NoPlanWindows,
    /// The tool doesn't report limits at all ("Limits not reported", OpenCode).
    NotReported,
}

/// `(tool, SUM(input), SUM(cached), SUM(output), SUM(reasoning))` of a day.
type ToolDayRow = (
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

/// The activity summary now.
pub async fn activity(store: &Store, adapters: &Adapters) -> Result<Activity, StoreError> {
    let day = local_day(now_ms());
    // A session with no tool (or one this build doesn't know) counts for the tool a new session runs on, so the
    // total stays the header's.
    let tool_of = |stored: Option<String>| {
        adapters
            .of_session(stored.and_then(|t| t.parse().ok()))
            .tool()
    };

    // The header's count (`ProjectInfo::agents_working`), split by tool.
    let rows: Vec<(String, String, Option<String>, i64)> = sqlx::query_as(
        "SELECT p.key, p.name, s.tool, COUNT(*)
         FROM sessions s JOIN tickets t ON t.id = s.ticket_id JOIN projects p ON p.id = t.project_id
         WHERE s.kind = 'agent' AND s.ended_at IS NULL
         GROUP BY p.id, s.tool
         ORDER BY p.created_at, p.key",
    )
    .fetch_all(store.pool())
    .await?;
    let mut running: BTreeMap<Tool, Vec<ProjectRunning>> = BTreeMap::new();
    for (key, name, tool, count) in rows {
        let projects = running.entry(tool_of(tool)).or_default();
        let count = u32::try_from(count).unwrap_or(0);
        match projects.iter_mut().find(|p| p.key == key) {
            Some(p) => p.running += count,
            None => projects.push(ProjectRunning {
                key,
                name,
                running: count,
            }),
        }
    }

    // Today's `usage` rows by their session's tool; every row counts, as in `usage.summary`'s total.
    let rows: Vec<ToolDayRow> = sqlx::query_as(
        "SELECT s.tool, SUM(u.input), SUM(u.cached), SUM(u.output), SUM(u.reasoning)
         FROM usage u LEFT JOIN sessions s ON s.id = u.session_id
         WHERE u.day = ? GROUP BY s.tool",
    )
    .bind(&day)
    .fetch_all(store.pool())
    .await?;
    let mut today: BTreeMap<Tool, u64> = BTreeMap::new();
    for (tool, a, b, c, d) in rows {
        let sums: DayRow = (a, b, c, d);
        *today.entry(tool_of(tool)).or_default() += tokens(sums).shown();
    }

    let limits = plan::limits(store).await?;
    let mut tools = Vec::new();
    for &tool in Tool::ALL {
        let by_project = running.remove(&tool).unwrap_or_default();
        let windows: Vec<LimitWindow> = plan::windows(store, tool)
            .await?
            .into_iter()
            .map(|w| w.window)
            .collect();
        let state = match adapters.get(tool).windows_source() {
            WindowsSource::NotReported => ToolLimits::NotReported,
            _ if windows.is_empty() => ToolLimits::NoPlanWindows,
            _ => ToolLimits::Windows(windows),
        };
        tools.push(ToolActivity {
            tool,
            running: by_project.iter().map(|p| p.running).sum(),
            by_project,
            tokens_today: today.get(&tool).copied().unwrap_or(0),
            limits: state,
            limit: limits.iter().find(|l| l.tool == tool).cloned(),
        });
    }
    Ok(Activity {
        day,
        tokens_today: tools.iter().map(|t| t.tokens_today).sum(),
        tools,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limit_states_are_tagged_on_the_wire() {
        let json = |l: ToolLimits| serde_json::to_value(l).unwrap();
        assert_eq!(
            json(ToolLimits::NoPlanWindows),
            serde_json::json!({"state": "no_plan_windows"})
        );
        assert_eq!(
            json(ToolLimits::NotReported),
            serde_json::json!({"state": "not_reported"})
        );
        assert_eq!(
            json(ToolLimits::Windows(Vec::new())),
            serde_json::json!({"state": "windows", "windows": []})
        );
    }
}
