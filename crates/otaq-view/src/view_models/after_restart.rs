//! What the app says when it connects (REC-06; d0045, d0022; *16 → Opening after a reboot*, *11 → After the service
//! restart*): after a reboot, a crash or a service restart the service started again and its startup reconcile
//! interrupted sessions, so the app opens with "4 sessions were interrupted" (Resume all, Review & resume). After a
//! normal reopen, with the service still running, only a quiet "Reconnected".
//!
//! [`summary`] is the summary sheet beside the banner (*16 → Opening after a reboot*, T-0235): the interrupted
//! tickets with a checkbox and Resume each, Resume selected, what each Resume did (resumed, or Couldn't resume with
//! Retry and Start from scratch), and the tickets the restart left alone. Nothing resumes without a click (d0045).
//! A ticket that was validating says Resume starts a fresh validation; the Project agents the restart ended have their
//! own rows with Resume (same conversation), outside Resume selected (T-0244; d0063, d0068).

use std::collections::{BTreeMap, BTreeSet};

use otaq_core::board::tickets::Ticket;
use otaq_core::domain::Status;
use otaq_core::project_agents::{AgentState, ProjectAgent};
use otaq_core::protocol::ServiceStatus;
use otaq_core::recovery::StartupReport;
use serde::Serialize;
use ts_rs::TS;

use crate::shell::tilde;
use crate::view_models::banners::Situation;
use crate::view_models::card::tokens_label;
use crate::view_models::clock::local_hhmm;
use crate::view_models::status::pill;

/// A service that started this long before the app opened was already running: a normal reopen.
pub const ALREADY_RUNNING_MS: i64 = 15_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnConnect {
    /// Raise the summary banner (once per service start).
    Summary(Situation),
    /// "Reconnected": the app reopened while the service kept running.
    Reconnected,
    Nothing,
}

/// What a connection to the service shows. `app_started_ms`: when this app opened; `first_connect`: the app's
/// first connection; `shown_for`: the service start a summary was already shown for.
pub fn on_connect(
    status: &ServiceStatus,
    app_started_ms: i64,
    first_connect: bool,
    shown_for: Option<i64>,
) -> OnConnect {
    if shown_for == Some(status.started_at) {
        return OnConnect::Nothing;
    }
    let already_running = status.started_at + ALREADY_RUNNING_MS < app_started_ms;
    if already_running {
        return if first_connect {
            OnConnect::Reconnected
        } else {
            OnConnect::Nothing
        };
    }
    if status.interrupted_at_start == 0 {
        return OnConnect::Nothing;
    }
    OnConnect::Summary(Situation::SessionsInterrupted {
        count: status.interrupted_at_start,
        unclean: status.unclean_start,
        at: local_hhmm(status.started_at),
        // The startup report adds them once it is read (`with_agents`).
        agents: 0,
    })
}

/// Rows the sheet lists one by one; the rest fold into one "+3" row (*Opening after a reboot*).
pub const LISTED: usize = 5;

/// What one row of the sheet shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "AfterRestartRowState")]
pub enum RowState {
    /// Still Interrupted: the checkbox and Resume.
    Interrupted,
    /// Resumed (or queued to resume): no button.
    Resumed,
    /// Resume failed: Retry and Start from scratch (REC-03).
    Failed,
}

/// One interrupted ticket. `AfterRestartRow` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AfterRestartRow")]
pub struct SummaryRow {
    pub key: String,
    /// The project's name ("shop-web").
    pub project: String,
    pub title: String,
    pub state: RowState,
    /// "SHOP-19 · Resumed", "BILL-4 · Couldn’t resume"; `None` while Interrupted.
    pub headline: Option<String>,
    /// "was Agent Working · round 2 · 88k", "Agent Working again · same conversation", or why Resume failed.
    pub detail: String,
    pub checked: bool,
    /// While the page waits for its Resume: "SHOP-25 · Resuming…" and what it does.
    pub resuming: String,
    pub resuming_detail: String,
}

/// The rows after the first [`LISTED`], folded into one. `AfterRestartMore` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AfterRestartMore")]
pub struct SummaryMore {
    /// "+3".
    pub count: String,
    /// "SHOP-30, SHOP-40.4, SHOP-40.2".
    pub keys: String,
    /// "was working · expand to resume one by one": a click unfolds the row.
    pub detail: &'static str,
    /// Every folded ticket still to resume is selected.
    pub checked: bool,
    /// The folded tickets still to resume, for its checkbox and its Resume.
    pub resumable: Vec<String>,
}

/// One Project agent the restart ended (*16 → Opening after a reboot*, PROJECT AGENTS). `AfterRestartAgent` on the
/// page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AfterRestartAgent")]
pub struct AgentRow {
    pub id: String,
    pub project: String,
    /// Where a ticket key sits: "agent".
    pub tag: &'static str,
    pub name: String,
    /// "Ended by restart · same conversation", "Ended by restart · was asking you"; "Resumed" once it runs again.
    pub detail: String,
    /// Still ended: Resume continues the same conversation.
    pub resumable: bool,
}

/// The Project agents part of the sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AfterRestartAgents")]
pub struct AgentsPart {
    /// "PROJECT AGENTS · NOT IN RESUME SELECTED · TERMINALS ARE GONE".
    pub title: &'static str,
    pub rows: Vec<AgentRow>,
    /// "A “New agent” that had no request yet is discarded, not listed."
    pub discarded: Option<String>,
}

/// The summary sheet (*16 → Opening after a reboot*). `AfterRestartSummary` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AfterRestartSummary")]
pub struct SummaryPage {
    pub title: String,
    pub text: &'static str,
    pub rows: Vec<SummaryRow>,
    pub more: Option<SummaryMore>,
    /// The Project agents the restart ended; `None` when it ended none and discarded none.
    pub agents: Option<AgentsPart>,
    pub notes_title: &'static str,
    /// The tickets the restart left alone or that Resume selected leaves to the inbox.
    pub notes: Vec<String>,
    pub resume: &'static str,
    pub retry: &'static str,
    pub scratch: &'static str,
    pub foot: &'static str,
    pub later: &'static str,
    /// "Resume selected (4)".
    pub resume_selected: String,
    /// What Resume selected resumes, in order.
    pub selected: Vec<String>,
}

fn joined(keys: &[&Ticket]) -> String {
    keys.iter()
        .map(|t| t.key.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn was(ticket: &Ticket) -> String {
    let mut parts = vec![format!("was {}", pill(Status::Working).1)];
    if let Some(back) = &ticket.flags.sent_back {
        parts.push(format!("round {}", back.round));
    }
    if let Some(tokens) = ticket.flags.tokens {
        parts.push(tokens_label(tokens));
    }
    parts.join(" · ")
}

fn row(
    ticket: &Ticket,
    projects: &BTreeMap<String, String>,
    unchecked: &BTreeSet<String>,
    validating: bool,
) -> SummaryRow {
    let key = ticket.key.to_string();
    let (state, headline, detail) = match ticket.status {
        // d0063: no round is counted and the old validator conversation isn't resumed.
        Status::Interrupted if validating => (
            RowState::Interrupted,
            None,
            "was Validating \u{b7} Resume = fresh validation".to_owned(),
        ),
        Status::Interrupted => (RowState::Interrupted, None, was(ticket)),
        Status::ResumeFailed => (
            RowState::Failed,
            Some(format!("{key} · Couldn’t resume")),
            "Claude has no saved conversation for this session, so it can’t continue where it left off. Retry \
             tries again; Start from scratch begins a new session in the same worktree."
                .to_owned(),
        ),
        other => (
            RowState::Resumed,
            Some(format!("{key} · Resumed")),
            format!("{} again · same conversation", pill(other).1),
        ),
    };
    let worktree = ticket
        .work
        .as_ref()
        .and_then(|w| w.worktree.as_deref())
        .map(tilde);
    SummaryRow {
        project: projects
            .get(ticket.key.project.as_str())
            .cloned()
            .unwrap_or_else(|| ticket.key.project.to_string()),
        title: ticket.title.clone(),
        state,
        headline,
        detail,
        checked: state == RowState::Interrupted && !unchecked.contains(&key),
        resuming: format!("{key} · Resuming…"),
        resuming_detail: match worktree {
            Some(w) => format!("Reattaching claude --resume in {w}"),
            None => "Reattaching claude --resume".to_owned(),
        },
        key,
    }
}

fn agents_part(
    report: &StartupReport,
    agents: &[ProjectAgent],
    projects: &BTreeMap<String, String>,
) -> Option<AgentsPart> {
    let rows: Vec<AgentRow> = report
        .project_agents_ended
        .iter()
        .filter_map(|id| agents.iter().find(|a| &a.id == id))
        .map(|a| {
            let ended = a.state == AgentState::EndedByRestart;
            AgentRow {
                id: a.id.clone(),
                project: projects
                    .get(&a.project)
                    .cloned()
                    .unwrap_or_else(|| a.project.clone()),
                tag: "agent",
                name: a.name.clone(),
                detail: match (ended, a.question.is_some() || a.needs_you.is_some()) {
                    (false, _) => "Resumed \u{b7} same conversation".to_owned(),
                    (true, true) => "Ended by restart \u{b7} was asking you".to_owned(),
                    (true, false) => "Ended by restart \u{b7} same conversation".to_owned(),
                },
                resumable: ended,
            }
        })
        .collect();
    let discarded = match report.new_agents_discarded {
        0 => None,
        1 => Some(
            "A \u{201c}New agent\u{201d} that had no request yet is discarded, not listed."
                .to_owned(),
        ),
        n => Some(format!(
            "{n} \u{201c}New agent\u{201d}s that had no request yet are discarded, not listed."
        )),
    };
    (!rows.is_empty() || discarded.is_some()).then_some(AgentsPart {
        title: "PROJECT AGENTS \u{b7} NOT IN RESUME SELECTED \u{b7} TERMINALS ARE GONE",
        rows,
        discarded,
    })
}

/// "8 sessions were interrupted · 2 Project agents ended".
pub fn title(sessions: usize, agents: usize) -> String {
    let sessions = match sessions {
        0 => None,
        1 => Some("1 session was interrupted".to_owned()),
        n => Some(format!("{n} sessions were interrupted")),
    };
    let agents = match agents {
        0 => None,
        1 => Some("1 Project agent ended".to_owned()),
        n => Some(format!("{n} Project agents ended")),
    };
    [sessions, agents]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// The sheet for this service start, or `None` once it interrupted nothing. `tickets`: the ones `report` names,
/// as they are now; `agents`: the Project agents it names; `projects`: key → name; `unchecked`: the rows the user
/// unticked (every row starts ticked); `unfolded`: the "+3" row was opened.
pub fn summary(
    report: &StartupReport,
    tickets: &[Ticket],
    agents: &[ProjectAgent],
    projects: &BTreeMap<String, String>,
    unchecked: &BTreeSet<String>,
    unfolded: bool,
) -> Option<SummaryPage> {
    let find = |key: &otaq_core::domain::TicketKey| tickets.iter().find(|t| &t.key == key);
    let interrupted: Vec<&Ticket> = report.interrupted.iter().filter_map(find).collect();
    let agents_part = agents_part(report, agents, projects);
    if interrupted.is_empty() && agents_part.is_none() {
        return None;
    }
    // Needs-you tickets kept their question: answering from the inbox resumes and delivers it (d0045).
    let (kept, listed): (Vec<&Ticket>, Vec<&Ticket>) = interrupted
        .iter()
        .partition(|t| t.status == Status::Interrupted && t.interrupted_from.is_some());
    let mut rows: Vec<SummaryRow> = listed
        .iter()
        .map(|t| {
            row(
                t,
                projects,
                unchecked,
                report.was_validating.contains(&t.key),
            )
        })
        .collect();
    let more = (!unfolded && rows.len() > LISTED + 1).then(|| {
        let folded = rows.split_off(LISTED);
        let resumable: Vec<String> = folded
            .iter()
            .filter(|r| r.state == RowState::Interrupted)
            .map(|r| r.key.clone())
            .collect();
        SummaryMore {
            count: format!("+{}", folded.len()),
            keys: folded
                .iter()
                .map(|r| r.key.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            detail: "was working · expand to resume one by one",
            checked: !resumable.is_empty() && resumable.iter().all(|k| !unchecked.contains(k)),
            resumable,
        }
    });
    let mut selected: Vec<String> = rows
        .iter()
        .filter(|r| r.checked)
        .map(|r| r.key.clone())
        .collect();
    if let Some(m) = &more {
        selected.extend(
            m.resumable
                .iter()
                .filter(|k| !unchecked.contains(*k))
                .cloned(),
        );
    }
    let mut notes = Vec::new();
    if !kept.is_empty() {
        notes.push(format!(
            "Turquoise tickets keep their question — {}. Answering from the inbox resumes and delivers it.",
            joined(&kept)
        ));
    }
    let untouched: Vec<&Ticket> = report.session_ended.iter().filter_map(find).collect();
    if !untouched.is_empty() {
        notes.push(format!(
            "Review and Approved are untouched — {}. Run and Shell show “stopped by restart”.",
            joined(&untouched)
        ));
    }
    Some(SummaryPage {
        title: title(report.interrupted.len(), report.project_agents_ended.len()),
        text: "Only Dev tickets that were working become Interrupted. Each one can resume its Claude session in the \
               same worktree; resuming uses tokens, so it needs your click.",
        rows,
        more,
        agents: agents_part,
        // Q44: the frame says "NOT IN RESUME ALL", but Resume all includes the
        // needs-you tickets (d0045); the sheet's own button leaves them out.
        notes_title: "NOT IN RESUME SELECTED",
        notes,
        resume: "Resume",
        retry: "Retry",
        scratch: "Start from scratch",
        foot: "They stay in your inbox if you close this.",
        later: "Later",
        resume_selected: format!("Resume selected ({})", selected.len()),
        selected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures;

    const APP: i64 = 1_791_600_000_000;

    fn status(started_at: i64, interrupted: u32, unclean: bool) -> ServiceStatus {
        ServiceStatus {
            started_at,
            interrupted_at_start: interrupted,
            unclean_start: unclean,
            ..Default::default()
        }
    }

    #[test]
    fn after_a_reboot_the_app_opens_with_the_summary_once() {
        let s = status(APP + 2_000, 4, true);
        let OnConnect::Summary(Situation::SessionsInterrupted {
            count, unclean, at, ..
        }) = on_connect(&s, APP, true, None)
        else {
            panic!("summary")
        };
        assert_eq!((count, unclean, at.len()), (4, true, 5));
        assert_eq!(
            on_connect(&s, APP, false, Some(s.started_at)),
            OnConnect::Nothing
        );
    }

    #[test]
    fn a_restart_while_the_app_runs_shows_it_too() {
        let s = status(APP + 600_000, 3, false);
        assert!(matches!(
            on_connect(&s, APP, false, Some(APP - 1)),
            OnConnect::Summary(Situation::SessionsInterrupted { unclean: false, .. })
        ));
        assert_eq!(
            on_connect(&status(APP + 600_000, 0, false), APP, false, None),
            OnConnect::Nothing
        );
    }

    #[test]
    fn a_normal_reopen_is_a_quiet_reconnected() {
        let s = status(APP - 3_600_000, 4, true);
        assert_eq!(on_connect(&s, APP, true, None), OnConnect::Reconnected);
        assert_eq!(on_connect(&s, APP, false, None), OnConnect::Nothing);
    }

    fn report(interrupted: &[&str], session_ended: &[&str]) -> StartupReport {
        StartupReport {
            unclean: true,
            interrupted: interrupted.iter().map(|k| k.parse().unwrap()).collect(),
            session_ended: session_ended.iter().map(|k| k.parse().unwrap()).collect(),
            ..Default::default()
        }
    }

    fn projects() -> BTreeMap<String, String> {
        BTreeMap::from([("SHOP".to_owned(), "shop-web".to_owned())])
    }

    #[test]
    fn the_sheet_lists_the_interrupted_tickets_all_ticked() {
        let mut working =
            card_fixtures::ticket("SHOP-27", "Search results paging", Status::Interrupted);
        working.flags.tokens = Some(88_000);
        working.flags.sent_back = Some(otaq_core::domain::flags::SentBack {
            comments: 1,
            round: 2,
            decisions: 0,
        });
        let mut needs_you = card_fixtures::ticket("SHOP-22", "Login", Status::Interrupted);
        needs_you.interrupted_from = Some(Status::AsksYou);
        let review = card_fixtures::ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
        let tickets = vec![working, needs_you, review];
        let page = summary(
            &report(&["SHOP-27", "SHOP-22"], &["SHOP-18"]),
            &tickets,
            &[],
            &projects(),
            &BTreeSet::new(),
            false,
        )
        .unwrap();
        assert_eq!(page.title, "2 sessions were interrupted");
        assert_eq!(
            page.rows.len(),
            1,
            "the needs-you ticket is a note, not a row"
        );
        let r = &page.rows[0];
        assert_eq!(
            (r.project.as_str(), r.state, r.checked),
            ("shop-web", RowState::Interrupted, true)
        );
        assert_eq!(r.detail, "was Agent Working · round 2 · 88k");
        assert_eq!(page.resume_selected, "Resume selected (1)");
        assert_eq!(page.selected, ["SHOP-27"]);
        assert_eq!(
            page.notes,
            [
                "Turquoise tickets keep their question — SHOP-22. Answering from the inbox resumes and delivers it.",
                "Review and Approved are untouched — SHOP-18. Run and Shell show “stopped by restart”."
            ]
        );
        // Unticked: Resume selected leaves it out; nothing resumed it.
        let page = summary(
            &report(&["SHOP-27"], &[]),
            &tickets,
            &[],
            &projects(),
            &BTreeSet::from(["SHOP-27".to_owned()]),
            false,
        )
        .unwrap();
        assert!(!page.rows[0].checked);
        assert_eq!(page.resume_selected, "Resume selected (0)");
    }

    #[test]
    fn rows_say_what_resume_did() {
        let resumed = card_fixtures::ticket("SHOP-19", "Price table", Status::Working);
        let failed = card_fixtures::ticket("BILL-4", "Invoice PDF export", Status::ResumeFailed);
        let page = summary(
            &report(&["SHOP-19", "BILL-4"], &[]),
            &[resumed, failed],
            &[],
            &projects(),
            &BTreeSet::new(),
            false,
        )
        .unwrap();
        assert_eq!(page.rows[0].headline.as_deref(), Some("SHOP-19 · Resumed"));
        assert_eq!(
            page.rows[0].detail,
            "Agent Working again · same conversation"
        );
        assert_eq!(page.rows[1].state, RowState::Failed);
        assert_eq!(
            page.rows[1].headline.as_deref(),
            Some("BILL-4 · Couldn’t resume")
        );
        assert_eq!(
            page.rows[1].project, "BILL",
            "an unknown project shows its key"
        );
        assert!(page.selected.is_empty());
        assert_eq!(page.rows[0].resuming, "SHOP-19 · Resuming…");
    }

    #[test]
    fn a_long_list_folds_into_one_row_and_unfolds() {
        let keys: Vec<String> = (1..=8).map(|i| format!("SHOP-{i}")).collect();
        let tickets: Vec<Ticket> = keys
            .iter()
            .map(|k| card_fixtures::ticket(k, "t", Status::Interrupted))
            .collect();
        let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let r = report(&refs, &[]);
        let page = summary(
            &r,
            &tickets,
            &[],
            &projects(),
            &BTreeSet::from(["SHOP-8".to_owned()]),
            false,
        )
        .unwrap();
        assert_eq!(page.rows.len(), LISTED);
        let more = page.more.unwrap();
        assert_eq!(
            (more.count.as_str(), more.keys.as_str()),
            ("+3", "SHOP-6, SHOP-7, SHOP-8")
        );
        assert!(!more.checked);
        assert_eq!(page.selected.len(), 7);
        assert_eq!(page.title, "8 sessions were interrupted");
        let open = summary(&r, &tickets, &[], &projects(), &BTreeSet::new(), true).unwrap();
        assert_eq!((open.rows.len(), open.more), (8, None));
        // Nothing interrupted: no sheet.
        assert_eq!(
            summary(
                &report(&[], &[]),
                &tickets,
                &[],
                &projects(),
                &BTreeSet::new(),
                false
            ),
            None
        );
    }

    fn ended_agent(id: &str, name: &str, asked: bool) -> ProjectAgent {
        ProjectAgent {
            id: id.into(),
            project: "SHOP".into(),
            name: name.into(),
            name_source: None,
            state: AgentState::EndedByRestart,
            claude_session_id: "s".into(),
            got_request: true,
            mid_turn: false,
            limit_stopped: false,
            created_at: 0,
            ended_at: Some(0),
            closed: false,
            question: None,
            needs_you: asked.then(|| "Allow gh issue list?".to_owned()),
            focused: false,
            tokens: Default::default(),
            session: None,
            epic: None,
        }
    }

    #[test]
    fn validating_tickets_and_project_agents_are_counted_and_listed() {
        let tickets = vec![card_fixtures::ticket(
            "SHOP-29",
            "Coupon field",
            Status::Interrupted,
        )];
        let agents = vec![
            ended_agent("a1", "Checkout slowness", false),
            ended_agent("a2", "Import GitHub issues", true),
        ];
        let mut r = report(&["SHOP-29"], &[]);
        r.was_validating = vec!["SHOP-29".parse().unwrap()];
        r.project_agents_ended = vec!["a1".into(), "a2".into()];
        r.new_agents_discarded = 1;
        let page = summary(&r, &tickets, &agents, &projects(), &BTreeSet::new(), false).unwrap();
        assert_eq!(
            page.title,
            "1 session was interrupted \u{b7} 2 Project agents ended"
        );
        assert_eq!(
            page.rows[0].detail,
            "was Validating \u{b7} Resume = fresh validation"
        );
        assert_eq!(
            page.selected,
            ["SHOP-29"],
            "agents aren't in Resume selected"
        );
        let part = page.agents.unwrap();
        assert_eq!(
            part.title,
            "PROJECT AGENTS \u{b7} NOT IN RESUME SELECTED \u{b7} TERMINALS ARE GONE"
        );
        let rows: Vec<(&str, &str, &str, bool)> = part
            .rows
            .iter()
            .map(|a| {
                (
                    a.project.as_str(),
                    a.name.as_str(),
                    a.detail.as_str(),
                    a.resumable,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                (
                    "shop-web",
                    "Checkout slowness",
                    "Ended by restart \u{b7} same conversation",
                    true
                ),
                (
                    "shop-web",
                    "Import GitHub issues",
                    "Ended by restart \u{b7} was asking you",
                    true
                ),
            ]
        );
        assert_eq!(
            part.discarded.as_deref(),
            Some("A \u{201c}New agent\u{201d} that had no request yet is discarded, not listed.")
        );

        // Only agents ended: the sheet still opens; a resumed one says so.
        let mut only = report(&[], &[]);
        only.project_agents_ended = vec!["a1".into()];
        let mut running = ended_agent("a1", "Checkout slowness", false);
        running.state = AgentState::Running;
        let page = summary(&only, &[], &[running], &projects(), &BTreeSet::new(), false).unwrap();
        assert_eq!(page.title, "1 Project agent ended");
        let row = &page.agents.unwrap().rows[0];
        assert!(!row.resumable);
        assert_eq!(row.detail, "Resumed \u{b7} same conversation");
    }
}
