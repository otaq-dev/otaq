//! The ticket side panel's logic (UI-04; d0058, d0035): its tabs and their indicators, previous/next ticket in
//! the column's order (⌘[ / ⌘]), its width, and the header's texts.

use crewboard_core::board::order::sort_column;
use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Column, TicketKey};
use crewboard_core::session::SessionInfo;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;

/// The panel's tabs, in order; ⌘⌥1–6 switch to them (d0058). `PanelTab` on the page, where each tab's screen
/// registers it (T-0212).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "PanelTab")]
pub enum Tab {
    Terminal,
    Diff,
    Run,
    Shell,
    Ticket,
    History,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Terminal,
        Tab::Diff,
        Tab::Run,
        Tab::Shell,
        Tab::Ticket,
        Tab::History,
    ];

    /// ⌘⌥1 is the Terminal tab.
    pub fn from_number(n: u8) -> Option<Tab> {
        Self::ALL.get(usize::from(n).checked_sub(1)?).copied()
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Diff => "Diff",
            Self::Run => "Run",
            Self::Shell => "Shell",
            Self::Ticket => "Ticket",
            Self::History => "History",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Terminal => IconName::SquareTerminal,
            Self::Diff => IconName::FileDiff,
            Self::Run => IconName::Play,
            Self::Shell => IconName::Terminal,
            Self::Ticket => IconName::FileText,
            Self::History => IconName::History,
        }
    }
}

/// The small marker after a tab's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indicator {
    None,
    /// The live-session dot, in the status's colour (blue working, orange waiting for you).
    Dot {
        attention: bool,
    },
    /// "+412 −38" (Diff), ":4020" (Run running).
    Text(String),
    /// A count (History: sessions).
    Count(usize),
    /// A red dot: something needs fixing (Run failed to start, its port is in use).
    Problem,
}

/// What the tab bar shows for a ticket. `sessions` is the ticket's session history (newest first).
pub fn indicators(ticket: &Ticket, sessions: &[SessionInfo]) -> Vec<(Tab, Indicator)> {
    // The ticket's newest agent session (Run and Shell sessions are listed too).
    let live = sessions
        .iter()
        .find(|s| s.kind == crewboard_core::session::SessionKind::Agent)
        .is_some_and(SessionInfo::is_running);
    Tab::ALL
        .iter()
        .map(|tab| {
            let indicator = match tab {
                Tab::Terminal if live => Indicator::Dot {
                    attention: ticket.shows_orange(),
                },
                Tab::Run => ticket
                    .flags
                    .run_port
                    .map_or(Indicator::None, |port| Indicator::Text(format!(":{port}"))),
                Tab::History if !sessions.is_empty() => Indicator::Count(sessions.len()),
                _ => Indicator::None,
            };
            (*tab, indicator)
        })
        .collect()
}

/// The ticket before (`-1`) or after (`+1`) `current` in its column's order (both Dev sections, top to bottom).
pub fn neighbour(tickets: &[Ticket], current: &TicketKey, step: i32) -> Option<TicketKey> {
    let ticket = tickets.iter().find(|t| &t.key == current)?;
    let order = sort_column(ticket.column, tickets);
    let keys: Vec<&TicketKey> = order.cards.iter().collect();
    let at = keys.iter().position(|k| *k == current)? as i32;
    let next = at + step;
    (next >= 0 && (next as usize) < keys.len()).then(|| keys[next as usize].clone())
}

/// The panel's width when it opens.
pub const DEFAULT_WIDTH: f32 = 660.;
/// The narrowest panel (*Narrow panel · 480 px*).
pub const MIN_WIDTH: f32 = 480.;
/// The board keeps at least this much of the window next to the panel.
pub const MIN_BOARD: f32 = 420.;
/// Below this width the tabs show icons only.
pub const ICON_TABS_BELOW: f32 = 560.;

/// A dragged width kept within its limits for this window.
pub fn clamp_width(width: f32, window_width: f32) -> f32 {
    let max = (window_width - MIN_BOARD).max(MIN_WIDTH);
    width.clamp(MIN_WIDTH, max)
}

/// A long branch name shortened in the middle, keeping its start (with the ticket number) and its end:
/// `crewboard/SHOP-25-upgr…-app-router`.
pub fn middle_truncate(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars || max_chars < 5 {
        return text.to_owned();
    }
    // Two thirds for the start (prefix and ticket number), one third for the end.
    let keep = max_chars - 1;
    let tail = keep / 3;
    let head = keep - tail;
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);
    out
}

/// The branch the header shows: the stored one once started, else the temporary name it will get.
pub fn branch_name(ticket: &Ticket) -> Option<String> {
    let started = ticket.has_started() || ticket.column != Column::Backlog;
    ticket
        .work
        .as_ref()
        .and_then(|w| w.branch.clone())
        .or_else(|| {
            (!ticket.is_epic() && started)
                .then(|| crewboard_core::naming::branch(&ticket.key.to_string()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use crewboard_core::domain::Status;
    use crewboard_core::session::{SessionKind, SessionState};

    fn session(state: SessionState) -> SessionInfo {
        SessionInfo {
            id: "s".into(),
            kind: SessionKind::Agent,
            project: "SHOP".into(),
            ticket: Some("SHOP-18".parse().unwrap()),
            state,
            started_at: 0,
            ended_at: None,
            end_reason: None,
            claude_session_id: Some("abc".into()),
            size: None,
        }
    }

    #[test]
    fn tabs_and_shortcut_numbers() {
        assert_eq!(Tab::from_number(1), Some(Tab::Terminal));
        assert_eq!(Tab::from_number(6), Some(Tab::History));
        assert_eq!(Tab::from_number(0), None);
        assert_eq!(Tab::from_number(7), None);
        let labels: Vec<_> = Tab::ALL.iter().map(|t| t.label()).collect();
        assert_eq!(
            labels,
            ["Terminal", "Diff", "Run", "Shell", "Ticket", "History"]
        );
    }

    #[test]
    fn indicators_per_state() {
        let mut working = ticket("SHOP-18", "Checkout API", Status::Working);
        let live = indicators(&working, &[session(SessionState::Running)]);
        assert_eq!(
            live[0],
            (Tab::Terminal, Indicator::Dot { attention: false })
        );
        assert_eq!(live[5], (Tab::History, Indicator::Count(1)));

        working.status = Status::NeedsYou;
        let waiting = indicators(
            &working,
            &[session(SessionState::Running), session(SessionState::Ended)],
        );
        assert_eq!(waiting[0].1, Indicator::Dot { attention: true });
        assert_eq!(waiting[5].1, Indicator::Count(2));

        let review = ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
        let ended = indicators(&review, &[session(SessionState::Ended)]);
        assert_eq!(
            ended[0].1,
            Indicator::None,
            "no live dot after the session ended"
        );

        let mut running = ticket("SHOP-19", "Price table", Status::Working);
        running.flags.run_port = Some(4020);
        assert_eq!(
            indicators(&running, &[])[2],
            (Tab::Run, Indicator::Text(":4020".into()))
        );
        let backlog = ticket("SHOP-30", "New", Status::Backlog);
        assert!(
            indicators(&backlog, &[])
                .iter()
                .all(|(_, i)| *i == Indicator::None)
        );
    }

    #[test]
    fn previous_and_next_follow_the_column_order() {
        let mut a = ticket("SHOP-1", "a", Status::Backlog);
        a.created_at = 1;
        let mut b = ticket("SHOP-2", "b", Status::Backlog);
        b.created_at = 2;
        let mut c = ticket("SHOP-3", "c", Status::Backlog);
        c.created_at = 3;
        c.flags.pinned_at = Some(10);
        let other = ticket("SHOP-4", "d", Status::Working);
        let tickets = vec![a, b, c, other];
        let key = |s: &str| s.parse::<TicketKey>().unwrap();
        // Pinned SHOP-3 is first, then SHOP-1, SHOP-2.
        assert_eq!(neighbour(&tickets, &key("SHOP-1"), -1), Some(key("SHOP-3")));
        assert_eq!(neighbour(&tickets, &key("SHOP-1"), 1), Some(key("SHOP-2")));
        assert_eq!(
            neighbour(&tickets, &key("SHOP-2"), 1),
            None,
            "stays in its column"
        );
        assert_eq!(neighbour(&tickets, &key("SHOP-3"), -1), None);
        assert_eq!(neighbour(&tickets, &key("SHOP-4"), 1), None);
    }

    #[test]
    fn neighbours_stay_in_the_column() {
        let mut working = ticket("SHOP-1", "w", Status::Working);
        working.created_at = 5;
        let mut queued = ticket("SHOP-2", "q", Status::Queued);
        queued.created_at = 1;
        let mut waiting = ticket("SHOP-3", "s", Status::WaitingForSlot);
        waiting.created_at = 2;
        let tickets = vec![working, queued, waiting];
        let key = |s: &str| s.parse::<TicketKey>().unwrap();
        assert_eq!(
            neighbour(&tickets, &key("SHOP-1"), 1),
            None,
            "Ready for Dev is its own column"
        );
        assert_eq!(neighbour(&tickets, &key("SHOP-2"), 1), Some(key("SHOP-3")));
        assert_eq!(neighbour(&tickets, &key("SHOP-3"), -1), Some(key("SHOP-2")));
    }

    #[test]
    fn width_limits() {
        assert_eq!(clamp_width(660., 1440.), 660.);
        assert_eq!(clamp_width(200., 1440.), MIN_WIDTH);
        assert_eq!(clamp_width(1400., 1440.), 1440. - MIN_BOARD);
        assert_eq!(
            clamp_width(900., 800.),
            MIN_WIDTH,
            "a small window still gets the minimum"
        );
    }

    #[test]
    fn long_branches_shorten_in_the_middle() {
        let name = "feature/SHOP-25-upgrade-next-15-app-router";
        let short = middle_truncate(name, 28);
        assert_eq!(short.chars().count(), 28);
        assert!(short.starts_with("feature/SHOP-25"));
        assert!(short.ends_with("router"));
        assert!(short.contains('…'));
        assert_eq!(middle_truncate("feature/SHOP-1", 28), "feature/SHOP-1");
    }

    #[test]
    fn branch_shown_in_the_header() {
        let backlog = ticket("SHOP-30", "New", Status::Backlog);
        assert_eq!(branch_name(&backlog), None);
        let queued = ticket("SHOP-18", "Checkout API", Status::Queued);
        assert_eq!(
            branch_name(&queued),
            Some(crewboard_core::naming::branch("SHOP-18"))
        );
    }
}
