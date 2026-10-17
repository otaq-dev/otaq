//! The Shell tab's strip and states (RUN-05; *13 · Run & Shell → Shell tab · BILL-4*): "Shell · not the agent", the
//! worktree, "Run port · $PORT=4004", and the stopped-by-restart state with its Reopen click (d0045).

use otaq_core::session::{END_STOPPED_BY_RESTART, SessionInfo};

/// The strip's label: the Shell is never the agent.
pub const SHELL_LABEL: &str = "Shell · not the agent";

/// "Run port · $PORT=4004".
pub fn port_label(port: u16) -> String {
    format!("Run port · $PORT={port}")
}

/// A terminal a restart stopped: its title and note, for the state shown instead of an empty terminal.
pub fn stopped_note(
    session: &SessionInfo,
    hhmm: impl Fn(i64) -> String,
) -> Option<(String, String)> {
    if session.is_running() || session.end_reason.as_deref() != Some(END_STOPPED_BY_RESTART) {
        return None;
    }
    Some((
        format!(
            "Stopped by restart · was open at {}",
            hhmm(session.started_at)
        ),
        "A restart ended this shell. It never reopens on its own.".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use otaq_core::session::{SessionKind, SessionState};

    fn shell(state: SessionState, reason: Option<&str>) -> SessionInfo {
        SessionInfo {
            id: "s".into(),
            kind: SessionKind::Shell,
            project: "BILL".into(),
            ticket: Some("BILL-4".parse().unwrap()),
            state,
            started_at: 0,
            ended_at: None,
            end_reason: reason.map(str::to_owned),
            tool: None,
            tool_session_id: None,
            model: None,
            size: None,
        }
    }

    #[test]
    fn strip_and_stopped_state() {
        assert_eq!(port_label(4004), "Run port · $PORT=4004");
        let hhmm = |_| "08:24".to_owned();
        assert_eq!(
            stopped_note(
                &shell(SessionState::Ended, Some(END_STOPPED_BY_RESTART)),
                hhmm
            )
            .unwrap()
            .0,
            "Stopped by restart · was open at 08:24"
        );
        assert_eq!(
            stopped_note(&shell(SessionState::Running, None), hhmm),
            None
        );
        assert_eq!(
            stopped_note(&shell(SessionState::Ended, Some("exit 0")), hhmm),
            None
        );
    }
}
