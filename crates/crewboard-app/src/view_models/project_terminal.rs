//! The Project terminal drawer (RUN-06; *17 · Project terminal*): its header state, the path line and the hint in
//! the empty scrollback. The terminal itself is a plain shell in the project's folder, owned by the service.

use crewboard_core::session::{END_STOPPED_BY_RESTART, SessionInfo};

/// The drawer's height when it opens (*Empty · shell prompt*: 380 px).
pub const DRAWER_HEIGHT: f32 = 380.;

/// How long a ticket created live keeps its highlight on the board (*Running · creating tickets*).
pub const FRESH_FOR_MS: u64 = 4_000;

/// The header's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Opening,
    /// The shell runs: "Shell".
    Live,
    /// The user exited the shell: "Ended", with Reopen.
    Ended,
    /// A restart ended it: "Stopped by restart", with Reopen.
    StoppedByRestart,
}

impl State {
    pub fn of(session: Option<&SessionInfo>) -> Self {
        match session {
            None => Self::Opening,
            Some(s) if s.is_running() => Self::Live,
            Some(s) if s.end_reason.as_deref() == Some(END_STOPPED_BY_RESTART) => {
                Self::StoppedByRestart
            }
            Some(_) => Self::Ended,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Opening => "Opening…",
            Self::Live => "Shell",
            Self::Ended => "Ended",
            Self::StoppedByRestart => "Stopped by restart",
        }
    }

    /// Reopen is a click (nothing reopens on its own, d0045).
    pub fn offers_reopen(self) -> bool {
        matches!(self, Self::Ended | Self::StoppedByRestart)
    }
}

/// "~/code/shop-web · plain shell · full crewboard access".
pub fn path_line(short_repo: &str) -> String {
    format!(
        "{short_repo} · plain shell · full {} access",
        crewboard_core::naming::COMMAND_NAME
    )
}

/// The hint in the empty scrollback, until the first keystroke.
pub fn hint(project_name: &str, short_repo: &str) -> Vec<String> {
    let cb = crewboard_core::naming::COMMAND_NAME;
    vec![
        format!("Project terminal · {project_name} — a shell in {short_repo}"),
        format!("Run `claude` to plan with your skills, or `{cb} create`:"),
        "  › claude".to_owned(),
        format!("  › {cb} create \"Wishlist share link\""),
        "Tickets you create here land in Backlog. Nothing starts until you drag them.".to_owned(),
    ]
}

/// Whether the hint shows: an open shell nobody typed into yet.
pub fn shows_hint(state: State, typed: bool) -> bool {
    state == State::Live && !typed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crewboard_core::session::{SessionKind, SessionState};

    fn session(state: SessionState, reason: Option<&str>) -> SessionInfo {
        SessionInfo {
            id: "p".into(),
            kind: SessionKind::Project,
            project: "SHOP".into(),
            ticket: None,
            state,
            started_at: 0,
            ended_at: None,
            end_reason: reason.map(str::to_owned),
            claude_session_id: None,
            size: None,
        }
    }

    #[test]
    fn states() {
        assert_eq!(State::of(None), State::Opening);
        let live = State::of(Some(&session(SessionState::Running, None)));
        assert_eq!((live, live.label()), (State::Live, "Shell"));
        assert!(!live.offers_reopen());
        let ended = State::of(Some(&session(SessionState::Ended, Some("exit 0"))));
        assert_eq!((ended, ended.label()), (State::Ended, "Ended"));
        assert!(ended.offers_reopen());
        let restart = State::of(Some(&session(
            SessionState::Ended,
            Some(END_STOPPED_BY_RESTART),
        )));
        assert_eq!(restart, State::StoppedByRestart);
        assert!(restart.offers_reopen());
    }

    #[test]
    fn hint_until_the_first_keystroke() {
        assert!(shows_hint(State::Live, false));
        assert!(!shows_hint(State::Live, true));
        assert!(!shows_hint(State::Ended, false));
        let lines = hint("shop-web", "~/code/shop-web");
        assert!(lines[1].starts_with("Run `claude` to plan with your skills, or `"));
        assert_eq!(
            path_line("~/code/shop-web"),
            format!(
                "~/code/shop-web · plain shell · full {} access",
                crewboard_core::naming::COMMAND_NAME
            )
        );
    }
}
