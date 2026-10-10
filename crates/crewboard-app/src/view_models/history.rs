//! The History tab (UI-04; d0045): every session the ticket had, with its time, duration, end reason and full
//! Claude session ID (copyable, so `claude --resume <id>` finds the conversation), and the ticket's events
//! (created, moved, start interrupted, held, retried, edited…), newest first.

use crewboard_core::board::tickets::{EventKind, HistoryEvent};
use crewboard_core::domain::{Actor, StartStep, Status};
use crewboard_core::session::{SessionInfo, SessionKind};

use crate::view_models::clock;

/// One line of the History table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    pub title: String,
    pub detail: Option<String>,
    /// Unix milliseconds; shown as HH:MM.
    pub at: i64,
    /// "72 min", "running", or "—".
    pub duration: String,
    /// Tokens arrive with M8; "—" until then.
    pub tokens: String,
    /// The full Claude session ID, for the copy button.
    pub claude_session_id: Option<String>,
    pub is_session: bool,
}

/// Sessions and events merged, newest first. `now` is for running sessions' durations.
pub fn rows(events: &[HistoryEvent], sessions: &[SessionInfo]) -> Vec<HistoryRow> {
    let mut rows: Vec<HistoryRow> = sessions.iter().map(session_row).collect();
    rows.extend(events.iter().filter_map(event_row));
    // Newest first; a session row comes before an event at the same moment.
    rows.sort_by(|a, b| b.at.cmp(&a.at).then(b.is_session.cmp(&a.is_session)));
    rows
}

fn session_row(s: &SessionInfo) -> HistoryRow {
    let title = match s.kind {
        SessionKind::Agent => "Agent session",
        SessionKind::Answer => "Answer session",
        SessionKind::Delivery => "PR session",
        SessionKind::Shell => "Shell",
        SessionKind::Project => "Project terminal",
        SessionKind::Run => "Run",
    };
    let duration = match s.ended_at {
        Some(end) => clock::duration(end - s.started_at),
        None => "running".to_owned(),
    };
    let detail = match (&s.claude_session_id, s.end_reason.as_deref()) {
        (Some(id), Some(reason)) => Some(format!("claude {id} · {reason}")),
        (Some(id), None) => Some(format!("claude {id}")),
        (None, Some(reason)) => Some(reason.to_owned()),
        (None, None) => None,
    };
    HistoryRow {
        title: title.to_owned(),
        detail,
        at: s.started_at,
        duration,
        tokens: "—".to_owned(),
        claude_session_id: s.claude_session_id.clone(),
        is_session: true,
    }
}

fn who(actor: &Actor) -> String {
    match actor {
        Actor::User(_) => "by you".to_owned(),
        Actor::Agent(key) => format!("by {key}'s agent"),
    }
}

fn text(payload: &serde_json::Value, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(|v| v.as_str())
        .map(str::to_owned)
}

fn event_row(e: &HistoryEvent) -> Option<HistoryRow> {
    let p = &e.payload;
    let first_line = |s: String| s.lines().next().unwrap_or_default().to_owned();
    let (title, detail) = match e.kind {
        // The session row stands for a successful start.
        EventKind::Started | EventKind::AgentEditsSeen => return None,
        EventKind::Created => ("Created".to_owned(), Some(who(&e.actor))),
        EventKind::Moved => {
            let to = text(p, "to")
                .and_then(|s| s.parse::<Status>().ok())
                .map_or("moved".to_owned(), |s| s.label().to_owned());
            (to, Some(who(&e.actor)))
        }
        EventKind::StartInterrupted => {
            let step = text(p, "step")
                .and_then(|s| s.parse::<StartStep>().ok())
                .map_or("start stopped", StartStep::failure_label);
            (
                "Start interrupted".to_owned(),
                Some(match text(p, "message") {
                    Some(m) => format!("{step}: {}", first_line(m)),
                    None => step.to_owned(),
                }),
            )
        }
        EventKind::StartRetried => ("Retry start".to_owned(), Some(who(&e.actor))),
        EventKind::Held => ("Held".to_owned(), text(p, "message").map(first_line)),
        EventKind::StartNote => ("Note".to_owned(), text(p, "message").map(first_line)),
        EventKind::Updated => ("Edited".to_owned(), Some(who(&e.actor))),
        EventKind::Pinned => ("Pinned".to_owned(), None),
        EventKind::Unpinned => ("Unpinned".to_owned(), None),
        EventKind::DependencyAdded => (
            "Dependency added".to_owned(),
            text(p, "parent").map(|k| format!("depends on {k}")),
        ),
        EventKind::DependencyRemoved => (
            "Dependency removed".to_owned(),
            text(p, "parent").map(|k| format!("no longer depends on {k}")),
        ),
        EventKind::MovedToEpic => ("Moved to epic".to_owned(), text(p, "epic")),
        EventKind::Closed => ("Closed".to_owned(), Some(who(&e.actor))),
        EventKind::ReviewSubmitted => (
            "Submitted for review".to_owned(),
            text(p, "summary").map(first_line),
        ),
        EventKind::Merged => (
            "Merged".to_owned(),
            text(p, "target").map(|t| format!("into {t}")),
        ),
        EventKind::Reopened => ("Reopened".to_owned(), Some(who(&e.actor))),
        EventKind::Parked => ("Parked".to_owned(), Some(who(&e.actor))),
        EventKind::Rebased => (
            "Rebased".to_owned(),
            text(p, "onto").map(|b| match p.get("conflict") {
                Some(_) => format!("onto {b}: conflicts, the agent resolves them first"),
                None => format!("onto {b}"),
            }),
        ),
        EventKind::SentToFix => (
            "Sent to agent to fix".to_owned(),
            text(p, "target").map(|t| format!("conflicts with {t}")),
        ),
        EventKind::SentBack => {
            let n = p.get("comments").and_then(|v| v.as_u64()).unwrap_or(0);
            let round = p.get("round").and_then(|v| v.as_u64()).unwrap_or(0);
            let plural = if n == 1 { "" } else { "s" };
            (
                "Sent back".to_owned(),
                Some(format!("{n} comment{plural} · round {round}")),
            )
        }
    };
    Some(HistoryRow {
        title,
        detail,
        at: e.at,
        duration: "—".to_owned(),
        tokens: "—".to_owned(),
        claude_session_id: None,
        is_session: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crewboard_core::session::SessionState;
    use serde_json::json;

    fn session(id: &str, start: i64, end: Option<i64>, reason: Option<&str>) -> SessionInfo {
        SessionInfo {
            id: id.into(),
            kind: SessionKind::Agent,
            project: "SHOP".into(),
            ticket: Some("SHOP-28".parse().unwrap()),
            state: if end.is_some() {
                SessionState::Ended
            } else {
                SessionState::Running
            },
            started_at: start,
            ended_at: end,
            end_reason: reason.map(str::to_owned),
            claude_session_id: Some(format!("3be94c10-{id}")),
            size: None,
        }
    }

    fn event(kind: EventKind, at: i64, payload: serde_json::Value) -> HistoryEvent {
        HistoryEvent {
            kind,
            actor: Actor::User("me".into()),
            at,
            payload,
        }
    }

    const MIN: i64 = 60_000;

    #[test]
    fn sessions_and_events_newest_first() {
        let sessions = vec![
            session("b", 100 * MIN, None, None),
            session("a", 10 * MIN, Some(82 * MIN), Some("service stopped")),
        ];
        let events = vec![
            event(EventKind::Started, 10 * MIN, json!({})),
            event(
                EventKind::StartInterrupted,
                5 * MIN,
                json!({"step": "setup", "message": "setup command failed (exit 1):\nERR"}),
            ),
            event(
                EventKind::Moved,
                2 * MIN,
                json!({"from": "backlog", "to": "queued"}),
            ),
            event(EventKind::Created, MIN, json!({})),
        ];
        let rows = rows(&events, &sessions);
        let titles: Vec<_> = rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Agent session",
                "Agent session",
                "Start interrupted",
                "Queued",
                "Created"
            ]
        );
        assert_eq!(rows[0].duration, "running");
        assert_eq!(rows[1].duration, "72 min");
        assert_eq!(
            rows[1].detail.as_deref(),
            Some("claude 3be94c10-a · service stopped")
        );
        assert_eq!(rows[1].claude_session_id.as_deref(), Some("3be94c10-a"));
        assert_eq!(
            rows[2].detail.as_deref(),
            Some("setup command failed: setup command failed (exit 1):")
        );
        assert_eq!(rows[4].detail.as_deref(), Some("by you"));
        assert!(rows.iter().all(|r| r.tokens == "—"));
    }

    #[test]
    fn durations_round_to_minutes() {
        let r = session_row(&session("x", 0, Some(125 * MIN + 30_000), Some("exit 0")));
        assert_eq!(r.duration, "2 h 5 min");
        let r = session_row(&session("y", 0, Some(30_000), None));
        assert_eq!(r.duration, "0 min");
    }
}
