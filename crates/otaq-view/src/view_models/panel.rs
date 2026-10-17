//! The ticket side panel's logic (UI-04; d0058, d0035): its tabs and their indicators, previous/next ticket in
//! the column's order (⌘[ / ⌘]), its width over the main area (SHL-05), and the header's texts.
//!
//! An epic's panel shows **Ticket · Lead · History**, with **Diff** first once the epic is in review (d0083, d0059;
//! *05 → Row · Epic panel · Lead tab (0083)*); it has no Terminal, Run or Shell tab. ⌘⌥N follows the shown order.

use otaq_core::board::order::sort_column;
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Column, TicketKey};
use otaq_core::project_agents::lead::LeadStatus;
use otaq_core::session::SessionInfo;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::layout::{ClassWidths, SizeClass};

/// The panel's tabs; a ticket shows the first six in this order, ⌘⌥1–6 switch to them (d0058). An epic shows
/// [`epic_tabs`] (with Lead), numbered in that order. `PanelTab` on the page, where each tab's screen
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
    /// An epic's lead (d0083).
    Lead,
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

    /// ⌘⌥1 is the Terminal tab (a ticket's tabs).
    pub fn from_number(n: u8) -> Option<Tab> {
        Self::ALL.get(usize::from(n).checked_sub(1)?).copied()
    }

    /// The tab ⌘⌥`n` shows among `shown` (the panel's tabs, in order).
    pub fn numbered(shown: &[Tab], n: u8) -> Option<Tab> {
        shown.get(usize::from(n).checked_sub(1)?).copied()
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Diff => "Diff",
            Self::Run => "Run",
            Self::Shell => "Shell",
            Self::Ticket => "Ticket",
            Self::History => "History",
            Self::Lead => "Lead",
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
            Self::Lead => IconName::Bot,
        }
    }
}

/// The small marker after a tab's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indicator {
    None,
    /// The live-session dot, in the status's colour (blue working, turquoise waiting for you).
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

/// An epic's tabs (d0083, d0059 and accepted Q9): Ticket · Lead · History, with Diff first in review.
pub fn epic_tabs(epic: &Ticket) -> Vec<Tab> {
    let review = matches!(epic.column, Column::ReadyForReview | Column::Approved);
    review
        .then_some(Tab::Diff)
        .into_iter()
        .chain([Tab::Ticket, Tab::Lead, Tab::History])
        .collect()
}

/// The tabs the panel shows for `ticket`, in order.
pub fn tabs(ticket: &Ticket) -> Vec<Tab> {
    if ticket.is_epic() {
        epic_tabs(ticket)
    } else {
        Tab::ALL.to_vec()
    }
}

/// What the tab bar shows for a ticket. `sessions` is the ticket's session history (newest first); `lead` is an
/// epic's lead status (the Lead tab's dot: violet while it runs, turquoise while it asks you).
pub fn indicators(
    ticket: &Ticket,
    sessions: &[SessionInfo],
    lead: Option<&LeadStatus>,
) -> Vec<(Tab, Indicator)> {
    // The ticket's newest agent session (Run and Shell sessions are listed too).
    let live = sessions
        .iter()
        .find(|s| s.kind == otaq_core::session::SessionKind::Agent)
        .is_some_and(SessionInfo::is_running);
    tabs(ticket)
        .iter()
        .map(|tab| {
            let indicator = match tab {
                Tab::Terminal if live => Indicator::Dot {
                    attention: ticket.shows_needs_you(),
                },
                Tab::Lead => match lead {
                    Some(status) if super::lead_tab::is_live(status) => Indicator::Dot {
                        attention: *status == LeadStatus::AsksYou,
                    },
                    _ => Indicator::None,
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

/// The panel's width until the user drags it, per window size class (SHL-05, *Row · Panel at 1280 × 800,
/// remembered width (0087)*): 600 / 660 / 760 px. A dragged width is kept per class.
pub const DEFAULT_WIDTHS: ClassWidths = ClassWidths {
    w1280: 600.,
    w1440: 660.,
    w1920: 760.,
};
/// The narrowest panel (*Narrow panel · 480 px*).
pub const MIN_WIDTH: f32 = 480.;
/// The panel opens over the main area and leaves at least this much of it showing on its left (SHL-05: "480 to
/// main area − 240"); ⇧⌘E takes all of it.
pub const MAIN_KEEPS: f32 = 240.;
/// Below this width the tabs show icons only.
pub const ICON_TABS_BELOW: f32 = 560.;

/// A width kept within its limits over a main area `main_width` px wide; a small main area still gets the minimum.
pub fn clamp_width(width: f32, main_width: f32) -> f32 {
    let max = (main_width - MAIN_KEEPS).max(MIN_WIDTH);
    width.clamp(MIN_WIDTH, max)
}

/// The panel's width in a window of `class` over a main area `main_width` px wide: the dragged one for that class,
/// else its default.
pub fn width(class: SizeClass, dragged: Option<f32>, main_width: f32) -> f32 {
    clamp_width(dragged.unwrap_or(DEFAULT_WIDTHS.get(class)), main_width)
}

/// "The start didn’t finish" above a Start interrupted ticket's tab (*16 → Start interrupted · SHOP-28 panel*,
/// T-0063): the failed step and its message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartProblem {
    pub heading: String,
    pub message: String,
}

/// What Retry start does, under the callout.
pub const RETRY_START_DOES: [&str; 3] = [
    "Runs the whole start again, the same way as dragging to Dev",
    "Starts a new agent session, so it uses tokens",
    "Leaves the ticket and its dependencies as they are",
];

pub fn start_problem(ticket: &Ticket) -> Option<StartProblem> {
    if ticket.status != otaq_core::domain::Status::StartInterrupted {
        return None;
    }
    Some(match &ticket.start_problem {
        Some(p) => StartProblem {
            heading: format!("The start didn\u{2019}t finish: {}", p.step.failure_label()),
            message: p.message.clone(),
        },
        None => StartProblem {
            heading: "The start didn\u{2019}t finish".to_owned(),
            message: format!(
                "{} stopped before its agent session started, so there is nothing to resume.",
                ticket.key
            ),
        },
    })
}

/// A long branch name shortened in the middle, keeping its start (with the ticket number) and its end:
/// `otaq/SHOP-25-upgr…-app-router`.
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
                .then(|| otaq_core::naming::branch(&ticket.key.to_string()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use otaq_core::domain::Status;
    use otaq_core::session::{SessionKind, SessionState};

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
            tool: Some(otaq_core::agent_tools::Tool::Claude),
            tool_session_id: Some("abc".into()),
            model: None,
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
        let live = indicators(&working, &[session(SessionState::Running)], None);
        assert_eq!(
            live[0],
            (Tab::Terminal, Indicator::Dot { attention: false })
        );
        assert_eq!(live[5], (Tab::History, Indicator::Count(1)));

        working.status = Status::NeedsYou;
        let waiting = indicators(
            &working,
            &[session(SessionState::Running), session(SessionState::Ended)],
            None,
        );
        assert_eq!(waiting[0].1, Indicator::Dot { attention: true });
        assert_eq!(waiting[5].1, Indicator::Count(2));

        let review = ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
        let ended = indicators(&review, &[session(SessionState::Ended)], None);
        assert_eq!(
            ended[0].1,
            Indicator::None,
            "no live dot after the session ended"
        );

        let mut running = ticket("SHOP-19", "Price table", Status::Working);
        running.flags.run_port = Some(4020);
        assert_eq!(
            indicators(&running, &[], None)[2],
            (Tab::Run, Indicator::Text(":4020".into()))
        );
        let backlog = ticket("SHOP-30", "New", Status::Backlog);
        assert!(
            indicators(&backlog, &[], None)
                .iter()
                .all(|(_, i)| *i == Indicator::None)
        );
    }

    /// An epic shows Ticket · Lead · History, with Diff first in review; ⌘⌥N follows that order (d0083).
    #[test]
    fn an_epics_tabs_per_column() {
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = otaq_core::domain::Kind::Epic;
        assert_eq!(tabs(&epic), [Tab::Ticket, Tab::Lead, Tab::History]);
        for status in [Status::EpicInProgress, Status::Queued] {
            epic.status = status;
            epic.column = status.column();
            assert_eq!(tabs(&epic), [Tab::Ticket, Tab::Lead, Tab::History]);
        }
        for status in [Status::ReadyForReview, Status::PrOpen] {
            epic.status = status;
            epic.column = status.column();
            assert_eq!(
                tabs(&epic),
                [Tab::Diff, Tab::Ticket, Tab::Lead, Tab::History]
            );
        }
        epic.status = Status::Merged;
        epic.column = Column::Done;
        assert_eq!(tabs(&epic), [Tab::Ticket, Tab::Lead, Tab::History]);
        let shown = tabs(&epic);
        assert_eq!(Tab::numbered(&shown, 2), Some(Tab::Lead));
        assert_eq!(Tab::numbered(&shown, 4), None);
        assert_eq!(
            tabs(&ticket("SHOP-18", "Checkout API", Status::Working)),
            Tab::ALL
        );
        assert_eq!(Tab::Lead.label(), "Lead");
        assert_eq!(Tab::Lead.icon(), IconName::Bot);
    }

    /// The Lead tab's dot: violet while the lead runs, turquoise while it asks you, none once it ended.
    #[test]
    fn the_lead_tab_has_its_dot() {
        let mut epic = ticket("SHOP-40", "Wishlist", Status::EpicInProgress);
        epic.kind = otaq_core::domain::Kind::Epic;
        let lead = |status: Option<LeadStatus>| {
            indicators(&epic, &[], status.as_ref())
                .into_iter()
                .find(|(t, _)| *t == Tab::Lead)
                .map(|(_, i)| i)
        };
        assert_eq!(
            lead(Some(LeadStatus::Replanning)),
            Some(Indicator::Dot { attention: false })
        );
        assert_eq!(
            lead(Some(LeadStatus::AsksYou)),
            Some(Indicator::Dot { attention: true })
        );
        assert_eq!(
            lead(Some(LeadStatus::EndedByRestart)),
            Some(Indicator::None)
        );
        assert_eq!(lead(None), Some(Indicator::None));
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
    fn width_defaults_per_class_and_limits_against_the_main_area() {
        // The main area at 1280 / 1440 / 1920 with the pane at its default (rail 64 px).
        assert_eq!(width(SizeClass::W1280, None, 1280. - 64. - 320.), 600.);
        assert_eq!(width(SizeClass::W1440, None, 1440. - 64. - 400.), 660.);
        assert_eq!(width(SizeClass::W1920, None, 1920. - 64. - 460.), 760.);
        assert_eq!(width(SizeClass::W1920, Some(900.), 1396.), 900.);
        assert_eq!(clamp_width(660., 976.), 660.);
        assert_eq!(clamp_width(200., 976.), MIN_WIDTH);
        assert_eq!(
            clamp_width(1000., 976.),
            976. - MAIN_KEEPS,
            "the board keeps 240 px"
        );
        assert_eq!(
            clamp_width(1200., 1396.),
            1396. - MAIN_KEEPS,
            "no fixed maximum"
        );
        assert_eq!(
            clamp_width(900., 600.),
            MIN_WIDTH,
            "a small main area still gets the minimum"
        );
    }

    #[test]
    fn a_start_interrupted_ticket_says_which_step_failed() {
        let mut t = ticket("SHOP-28", "Cart badge count", Status::StartInterrupted);
        let p = start_problem(&t).unwrap();
        assert_eq!(p.heading, "The start didn\u{2019}t finish");
        assert!(p.message.starts_with("SHOP-28 stopped before"));
        t.start_problem = Some(otaq_core::board::tickets::StartProblem {
            step: otaq_core::domain::StartStep::SyncBase,
            message: "main has diverged".into(),
        });
        let p = start_problem(&t).unwrap();
        assert!(p.heading.starts_with("The start didn\u{2019}t finish: "));
        assert_eq!(p.message, "main has diverged");
        assert_eq!(start_problem(&ticket("SHOP-1", "t", Status::Working)), None);
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
            Some(otaq_core::naming::branch("SHOP-18"))
        );
    }
}
