//! The Terminals & agents drawer (RUN-06, PAG-01, UI-09; d0068, d0019; *17 · Project terminals*, *18 · Project agents
//! & terminals*): one tab per Project agent and per Project terminal (agents first), the + New menu, which tab ⇧⌘T
//! shows, the header button's dot, a terminal's header state, the path line and the hint in the empty scrollback.
//! Each terminal is a plain shell in the project's folder, owned by the service.

use otaq_core::project_agents::{AgentState, ProjectAgent};
use otaq_core::session::{END_STOPPED_BY_RESTART, SessionInfo};
use otaq_core::terminals::shell::ProjectTerminal;
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::menu::MenuItem;

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

/// "~/code/shop-web · plain shell · full otaq access".
pub fn path_line(short_repo: &str) -> String {
    format!(
        "{short_repo} · plain shell · full {} access",
        otaq_core::naming::COMMAND_NAME
    )
}

/// The hint in the empty scrollback, until the first keystroke.
pub fn hint(project_name: &str, short_repo: &str) -> Vec<String> {
    let cb = otaq_core::naming::COMMAND_NAME;
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

/// What a tab is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "DrawerTabKind")]
pub enum TabKind {
    Agent,
    Terminal,
}

/// The state dot after an agent tab's name: blue while it works, turquoise while it waits for you.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "DrawerTabDot")]
pub enum Dot {
    Working,
    Waiting,
}

/// One tab of the drawer's strip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DrawerTab")]
pub struct Tab {
    /// `agent:<id>` or `terminal:<session>`; what the page selects and closes by.
    pub id: String,
    pub kind: TabKind,
    pub label: String,
    /// " · exited", " · restart", "ended".
    pub note: Option<&'static str>,
    pub icon: IconName,
    pub dot: Option<Dot>,
    /// "New agent" before its first request reads muted.
    pub muted: bool,
    /// The session its terminal attaches to; none for an agent whose session is gone.
    pub session: Option<String>,
    /// Its shell or session ended: the scrollback stays, read-only.
    pub ended: bool,
    /// The line under an ended terminal's scrollback.
    pub ended_line: Option<String>,
}

pub fn agent_tab_id(id: &str) -> String {
    format!("agent:{id}")
}

pub fn terminal_tab_id(session: &str) -> String {
    format!("terminal:{session}")
}

/// A Project agent waits for you (a permission prompt or a question) while its terminal doesn't have your focus.
pub fn agent_waits(agent: &ProjectAgent) -> bool {
    agent.state == AgentState::Running
        && (agent.question.is_some() || agent.needs_you.is_some())
        && !agent.focused
}

/// The header button's turquoise dot: one of the project's open Project agents waits for you.
pub fn button_dot(agents: &[ProjectAgent]) -> bool {
    agents.iter().any(agent_waits)
}

/// The header button tooltip's line about who waits: "Project agent “Import GitHub issues” asks you".
pub fn waiting_line(agents: &[ProjectAgent]) -> Option<String> {
    let waiting: Vec<&ProjectAgent> = agents.iter().filter(|a| agent_waits(a)).collect();
    match waiting.as_slice() {
        [] => None,
        [one] => Some(format!(
            "Project agent \u{201c}{}\u{201d} {}",
            one.name,
            if one.question.is_some() {
                "asks you"
            } else {
                "needs permission"
            }
        )),
        many => Some(format!("{} Project agents wait for you", many.len())),
    }
}

fn agent_tab(agent: &ProjectAgent) -> Tab {
    let running = agent.state == AgentState::Running;
    Tab {
        id: agent_tab_id(&agent.id),
        kind: TabKind::Agent,
        label: agent.name.clone(),
        note: match agent.state {
            AgentState::Running => None,
            AgentState::Ended => Some("ended"),
            AgentState::EndedByRestart => Some(" \u{b7} restart"),
        },
        icon: IconName::Bot,
        dot: running.then(|| {
            if agent_waits(agent) {
                Dot::Waiting
            } else {
                Dot::Working
            }
        }),
        muted: agent.name_source.is_none(),
        session: agent.session.as_ref().map(|s| s.id.clone()),
        ended: !running,
        ended_line: (!running).then(|| agent_ended_line(agent)),
    }
}

/// "── session ended 11:42 · Resume continues this conversation ──" under an ended agent's scrollback
/// (*Ended normally*, *Ended by restart · Resume*).
pub fn agent_ended_line(agent: &ProjectAgent) -> String {
    let how = match agent.state {
        AgentState::EndedByRestart => "session ended by restart",
        _ => "session ended",
    };
    let at = agent
        .ended_at
        .map(|t| format!(" {}", crate::view_models::clock::local_hhmm(t)))
        .unwrap_or_default();
    format!("\u{2500}\u{2500} {how}{at} \u{b7} Resume continues this conversation \u{2500}\u{2500}")
}

/// "── shell exited 11:20 · scrollback kept · close the tab, or + → New terminal ──".
pub fn ended_line(ended_at: Option<i64>) -> String {
    let at = ended_at
        .map(|t| format!(" {}", crate::view_models::clock::local_hhmm(t)))
        .unwrap_or_default();
    format!(
        "\u{2500}\u{2500} shell exited{at} \u{b7} scrollback kept \u{b7} close the tab, or + \u{2192} New terminal \u{2500}\u{2500}"
    )
}

fn terminal_tab(terminal: &ProjectTerminal) -> Tab {
    let ended = !terminal.session.is_running();
    Tab {
        id: terminal_tab_id(&terminal.session.id),
        kind: TabKind::Terminal,
        label: terminal.name.clone(),
        note: ended.then_some(" \u{b7} exited"),
        icon: IconName::SquareTerminal,
        dot: None,
        muted: false,
        session: Some(terminal.session.id.clone()),
        ended,
        ended_line: ended.then(|| ended_line(terminal.session.ended_at)),
    }
}

/// The strip: Project agents first (oldest first, as the service lists them), then terminals by number.
pub fn tabs(agents: &[ProjectAgent], terminals: &[ProjectTerminal]) -> Vec<Tab> {
    let mut terminals: Vec<&ProjectTerminal> = terminals.iter().collect();
    terminals.sort_by_key(|t| t.number);
    agents
        .iter()
        .map(agent_tab)
        .chain(terminals.into_iter().map(terminal_tab))
        .collect()
}

/// What ⇧⌘T shows: the last-used terminal while it is open, else the newest open one, else a new one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LastUsed {
    Show(String),
    OpenNew,
}

pub fn last_used(terminals: &[ProjectTerminal], remembered: Option<&str>) -> LastUsed {
    if let Some(tab) = remembered.filter(|r| {
        terminals
            .iter()
            .any(|t| terminal_tab_id(&t.session.id) == *r)
    }) {
        return LastUsed::Show(tab.to_owned());
    }
    terminals
        .iter()
        .max_by_key(|t| t.number)
        .map_or(LastUsed::OpenNew, |t| {
            LastUsed::Show(terminal_tab_id(&t.session.id))
        })
}

pub const NEW_TERMINAL: &str = "new-terminal";
pub const NEW_AGENT: &str = "new-agent";

/// + New (*18 → + menu · New terminal / New Project agent*).
pub fn new_menu(agent_keys: Option<&str>) -> Vec<MenuItem> {
    let agent_note = match agent_keys {
        Some(keys) => format!("{keys} \u{b7} uses tokens"),
        None => "uses tokens".to_owned(),
    };
    vec![
        MenuItem::new(NEW_TERMINAL, "New terminal")
            .icon(IconName::SquareTerminal)
            .shortcut("plain shell"),
        MenuItem::new(NEW_AGENT, "New Project agent")
            .icon(IconName::Bot)
            .shortcut(agent_note),
    ]
}

/// The empty drawer (no tab open, for example after a restart closed every terminal, *17 → After a restart*).
pub fn empty_lines(short_repo: &str) -> Vec<String> {
    vec![format!(
        "Nothing open. + \u{2192} New terminal opens a fresh shell in {short_repo}."
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use otaq_core::session::{SessionKind, SessionState};

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

    fn terminal(number: u32, id: &str, state: SessionState) -> ProjectTerminal {
        ProjectTerminal {
            number,
            name: format!("Terminal {number}"),
            session: SessionInfo {
                id: id.into(),
                ..session(state, None)
            },
        }
    }

    fn agent(id: &str, name: Option<&str>, state: AgentState) -> ProjectAgent {
        ProjectAgent {
            id: id.into(),
            project: "SHOP".into(),
            name: name.unwrap_or("New agent").into(),
            name_source: name.map(|_| otaq_core::project_agents::NameSource::Agent),
            state,
            claude_session_id: "c".into(),
            got_request: name.is_some(),
            mid_turn: false,
            limit_stopped: false,
            created_at: 0,
            ended_at: None,
            closed: false,
            question: None,
            needs_you: None,
            focused: false,
            tokens: Default::default(),
            session: None,
            epic: None,
        }
    }

    #[test]
    fn agents_come_first_then_terminals_by_number() {
        let mut waiting = agent("a2", Some("Import GitHub issues"), AgentState::Running);
        waiting.needs_you = Some("Allow gh issue list?".into());
        let agents = vec![
            agent("a1", Some("Checkout slowness"), AgentState::Running),
            waiting,
            agent("a3", None, AgentState::Running),
            agent("a4", Some("Old"), AgentState::EndedByRestart),
        ];
        let terminals = vec![
            terminal(2, "t2", SessionState::Ended),
            terminal(1, "t1", SessionState::Running),
        ];
        let strip = tabs(&agents, &terminals);
        let labels: Vec<&str> = strip.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Checkout slowness",
                "Import GitHub issues",
                "New agent",
                "Old",
                "Terminal 1",
                "Terminal 2"
            ]
        );
        assert_eq!(strip[0].dot, Some(Dot::Working));
        assert_eq!(strip[1].dot, Some(Dot::Waiting));
        assert!(strip[2].muted && !strip[0].muted);
        assert_eq!(
            (strip[3].dot, strip[3].note),
            (None, Some(" \u{b7} restart"))
        );
        assert!(
            strip[3]
                .ended_line
                .as_ref()
                .unwrap()
                .contains("session ended by restart")
        );
        assert_eq!(strip[4].id, "terminal:t1");
        assert!(!strip[4].ended && strip[4].ended_line.is_none());
        assert_eq!(strip[5].note, Some(" \u{b7} exited"));
        assert!(
            strip[5]
                .ended_line
                .as_ref()
                .unwrap()
                .contains("scrollback kept")
        );
    }

    #[test]
    fn the_button_dot_follows_a_waiting_agent() {
        let mut asks = agent("a1", Some("Checkout slowness"), AgentState::Running);
        assert!(!button_dot(std::slice::from_ref(&asks)));
        asks.question = Some(otaq_core::board::questions::HumanQuestion {
            id: "q".into(),
            text: "Import closed issues too?".into(),
            options: Vec::new(),
            recommended: None,
            asked_at: 0,
            via_lead: None,
        });
        assert!(button_dot(std::slice::from_ref(&asks)));
        asks.focused = true;
        assert!(
            !button_dot(std::slice::from_ref(&asks)),
            "you are in its terminal"
        );
    }

    #[test]
    fn shift_cmd_t_shows_the_last_used_terminal_or_a_new_one() {
        assert_eq!(last_used(&[], None), LastUsed::OpenNew);
        assert_eq!(last_used(&[], Some("terminal:gone")), LastUsed::OpenNew);
        let open = vec![
            terminal(1, "t1", SessionState::Running),
            terminal(2, "t2", SessionState::Running),
        ];
        assert_eq!(
            last_used(&open, Some("terminal:t1")),
            LastUsed::Show("terminal:t1".into())
        );
        assert_eq!(
            last_used(&open, Some("agent:a1")),
            LastUsed::Show("terminal:t2".into()),
            "an agent tab isn't a terminal: the newest terminal"
        );
        assert_eq!(last_used(&open, None), LastUsed::Show("terminal:t2".into()));
    }

    #[test]
    fn the_new_menu_names_the_shortcut_and_the_tokens() {
        let menu = new_menu(Some("\u{21e7}\u{2318}A"));
        assert_eq!(menu[0].id, NEW_TERMINAL);
        assert_eq!(menu[0].shortcut.as_deref(), Some("plain shell"));
        assert_eq!(menu[1].label, "New Project agent");
        assert_eq!(
            menu[1].shortcut.as_deref(),
            Some("\u{21e7}\u{2318}A \u{b7} uses tokens")
        );
        assert_eq!(new_menu(None)[1].shortcut.as_deref(), Some("uses tokens"));
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
                otaq_core::naming::COMMAND_NAME
            )
        );
    }
}
