//! The ticket side panel's logic (UI-04; d0058, d0035), now inside the ticket sheet (`sheet.rs`, T-0464): today's
//! tabs, which the sheet's tabs hold until their tickets move them, and their indicators; previous/next ticket in the
//! column's order (j / k, ⌘[ / ⌘]); the Start interrupted callout and the branch the header shows.
//!
//! An epic has **Ticket · Lead · History**, with **Files** first once the epic is in review (d0083, d0059;
//! *05 → Row · Epic panel · Lead tab (0083)*); it has no Terminal, Run or Shell tab.

use otaq_core::board::order::sort_column;
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::StatusSet;
use otaq_core::domain::TicketKey;
use otaq_core::project_agents::lead::LeadStatus;
use otaq_core::session::SessionInfo;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;

/// Today's panel tabs, in the sheet's tabs ([`super::sheet::SheetTab::contents`]); a ticket has the first six, an
/// epic [`epic_tabs`] (with Lead). `PanelTab` on the page, where each tab's screen registers it (T-0212).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "PanelTab")]
pub enum Tab {
    Terminal,
    /// The Files tab (FTB-01…FTB-05, T-0468), the Diff tab's successor.
    Files,
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
        Tab::Files,
        Tab::Run,
        Tab::Shell,
        Tab::Ticket,
        Tab::History,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Files => "Files",
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
            Self::Files => IconName::FolderOpen,
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
    /// ":4020" (Run running).
    Text(String),
    /// A count (History: sessions).
    Count(usize),
    /// A red dot: something needs fixing (Run failed to start, its port is in use).
    Problem,
}

/// An epic's tabs (d0083, d0059 and accepted Q9): Ticket · Lead · History, with Files first in review.
pub fn epic_tabs(epic: &Ticket) -> Vec<Tab> {
    let review = matches!(
        crate::view_models::flow_columns::place_set(epic),
        StatusSet::HumanCheck | StatusSet::Delivery
    );
    review
        .then_some(Tab::Files)
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
/// epic's lead status (the Lead tab's dot: violet while it runs, turquoise while it asks you); `comments` its ticket
/// comments (the Ticket tab's count, CMT-10).
pub fn indicators(
    ticket: &Ticket,
    sessions: &[SessionInfo],
    lead: Option<&LeadStatus>,
    comments: usize,
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
                Tab::Ticket if comments > 0 => Indicator::Count(comments),
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
    let order = sort_column(ticket.status.set(), tickets);
    let keys: Vec<&TicketKey> = order.cards.iter().collect();
    let at = keys.iter().position(|k| *k == current)? as i32;
    let next = at + step;
    (next >= 0 && (next as usize) < keys.len()).then(|| keys[next as usize].clone())
}

/// Below this width a terminal tab's switch shows icons only.
pub const ICON_TABS_BELOW: f32 = 560.;

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
            heading: format!("Start failed: {}", p.step.failure_label()),
            message: p.message.clone(),
        },
        None => StartProblem {
            heading: "Start failed".to_owned(),
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
    let started = ticket.has_started() || !ticket.place.is_backlog();
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
            agent: None,
            effort: None,
            fallback: None,
            model_ran: None,
            size: None,
        }
    }

    #[test]
    fn the_tabs_in_order() {
        let labels: Vec<_> = Tab::ALL.iter().map(|t| t.label()).collect();
        assert_eq!(
            labels,
            ["Terminal", "Files", "Run", "Shell", "Ticket", "History"]
        );
    }

    #[test]
    fn indicators_per_state() {
        let mut working = ticket("SHOP-18", "Checkout API", Status::Working);
        let live = indicators(&working, &[session(SessionState::Running)], None, 0);
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
            0,
        );
        assert_eq!(waiting[0].1, Indicator::Dot { attention: true });
        assert_eq!(waiting[5].1, Indicator::Count(2));

        let review = ticket("SHOP-18", "Checkout API", Status::WaitingForYou);
        let ended = indicators(&review, &[session(SessionState::Ended)], None, 4);
        assert_eq!(
            ended[0].1,
            Indicator::None,
            "no live dot after the session ended"
        );
        // The Ticket tab counts its comments (CMT-10, `s05-41`).
        assert_eq!(ended[4], (Tab::Ticket, Indicator::Count(4)));
        assert_eq!(
            live[4],
            (Tab::Ticket, Indicator::None),
            "none without comments"
        );

        let mut running = ticket("SHOP-19", "Price table", Status::Working);
        running.flags.run_port = Some(4020);
        assert_eq!(
            indicators(&running, &[], None, 0)[2],
            (Tab::Run, Indicator::Text(":4020".into()))
        );
        let backlog = ticket("SHOP-30", "New", Status::Backlog);
        assert!(
            indicators(&backlog, &[], None, 0)
                .iter()
                .all(|(_, i)| *i == Indicator::None)
        );
    }

    /// An epic shows Ticket · Lead · History, with Files first in review (d0083).
    #[test]
    fn an_epics_tabs_per_column() {
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = otaq_core::domain::Kind::Epic;
        assert_eq!(tabs(&epic), [Tab::Ticket, Tab::Lead, Tab::History]);
        for status in [Status::Backlog, Status::Queued] {
            epic.status = status;
            epic.place = otaq_core::domain::Place::Done;
            assert_eq!(tabs(&epic), [Tab::Ticket, Tab::Lead, Tab::History]);
        }
        for status in [Status::WaitingForYou, Status::PrOpen] {
            epic.status = status;
            epic.place = otaq_core::domain::Place::of(epic.status, None, None);
            assert_eq!(
                tabs(&epic),
                [Tab::Files, Tab::Ticket, Tab::Lead, Tab::History]
            );
        }
        epic.status = Status::Merged;
        epic.place = otaq_core::domain::Place::of(epic.status, None, None);
        assert_eq!(tabs(&epic), [Tab::Ticket, Tab::Lead, Tab::History]);
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
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = otaq_core::domain::Kind::Epic;
        let lead = |status: Option<LeadStatus>| {
            indicators(&epic, &[], status.as_ref(), 0)
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
    fn a_start_interrupted_ticket_says_which_step_failed() {
        let mut t = ticket("SHOP-28", "Cart badge count", Status::StartInterrupted);
        let p = start_problem(&t).unwrap();
        assert_eq!(p.heading, "Start failed");
        assert!(p.message.starts_with("SHOP-28 stopped before"));
        t.start_problem = Some(otaq_core::board::tickets::StartProblem {
            step: otaq_core::domain::StartStep::SyncBase,
            message: "main has diverged".into(),
        });
        let p = start_problem(&t).unwrap();
        assert!(p.heading.starts_with("Start failed: "));
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
