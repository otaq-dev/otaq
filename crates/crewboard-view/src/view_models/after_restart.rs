//! What the app says when it connects (REC-06; d0045, d0022; *16 → Opening after a reboot*, *11 → After the service
//! restart*): after a reboot, a crash or a service restart the service started again and its startup reconcile
//! interrupted sessions, so the app opens with "4 sessions were interrupted" (Resume all, Review & resume). After a
//! normal reopen, with the service still running, only a quiet "Reconnected".
//!
//! [`summary`] is the summary sheet beside the banner (*16 → Opening after a reboot*, T-0235): the interrupted
//! tickets with a checkbox and Resume each, Resume selected, what each Resume did (resumed, or Couldn't resume with
//! Retry and Start from scratch), and the tickets the restart left alone. Nothing resumes without a click (d0045).
//! T-0244 adds its Project-agent and validating rows.

use std::collections::{BTreeMap, BTreeSet};

use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::Status;
use crewboard_core::protocol::ServiceStatus;
use crewboard_core::recovery::StartupReport;
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

/// The summary sheet (*16 → Opening after a reboot*). `AfterRestartSummary` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AfterRestartSummary")]
pub struct SummaryPage {
    pub title: String,
    pub text: &'static str,
    pub rows: Vec<SummaryRow>,
    pub more: Option<SummaryMore>,
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
) -> SummaryRow {
    let key = ticket.key.to_string();
    let (state, headline, detail) = match ticket.status {
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

/// The sheet for this service start, or `None` once it interrupted nothing. `tickets`: the ones `report` names,
/// as they are now; `projects`: key → name; `unchecked`: the rows the user unticked (every row starts ticked);
/// `unfolded`: the "+3" row was opened.
pub fn summary(
    report: &StartupReport,
    tickets: &[Ticket],
    projects: &BTreeMap<String, String>,
    unchecked: &BTreeSet<String>,
    unfolded: bool,
) -> Option<SummaryPage> {
    let find = |key: &crewboard_core::domain::TicketKey| tickets.iter().find(|t| &t.key == key);
    let interrupted: Vec<&Ticket> = report.interrupted.iter().filter_map(find).collect();
    if interrupted.is_empty() {
        return None;
    }
    // Orange tickets kept their question: answering from the inbox resumes and delivers it (d0045).
    let (kept, listed): (Vec<&Ticket>, Vec<&Ticket>) = interrupted
        .iter()
        .partition(|t| t.status == Status::Interrupted && t.interrupted_from.is_some());
    let mut rows: Vec<SummaryRow> = listed.iter().map(|t| row(t, projects, unchecked)).collect();
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
            "Orange tickets keep their question — {}. Answering from the inbox resumes and delivers it.",
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
    let n = report.interrupted.len();
    let sessions = if n == 1 {
        "session was"
    } else {
        "sessions were"
    };
    Some(SummaryPage {
        title: format!("{n} {sessions} interrupted"),
        text: "Only Dev tickets that were working become Interrupted. Each one can resume its Claude session in the \
               same worktree; resuming uses tokens, so it needs your click.",
        rows,
        more,
        // Q44: the frame says "NOT IN RESUME ALL", but Resume all includes the
        // orange tickets (d0045); the sheet's own button leaves them out.
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
        let OnConnect::Summary(Situation::SessionsInterrupted { count, unclean, at }) =
            on_connect(&s, APP, true, None)
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
        working.flags.sent_back = Some(crewboard_core::domain::flags::SentBack {
            comments: 1,
            round: 2,
        });
        let mut orange = card_fixtures::ticket("SHOP-22", "Login", Status::Interrupted);
        orange.interrupted_from = Some(Status::AsksYou);
        let review = card_fixtures::ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
        let tickets = vec![working, orange, review];
        let page = summary(
            &report(&["SHOP-27", "SHOP-22"], &["SHOP-18"]),
            &tickets,
            &projects(),
            &BTreeSet::new(),
            false,
        )
        .unwrap();
        assert_eq!(page.title, "2 sessions were interrupted");
        assert_eq!(page.rows.len(), 1, "the orange ticket is a note, not a row");
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
                "Orange tickets keep their question — SHOP-22. Answering from the inbox resumes and delivers it.",
                "Review and Approved are untouched — SHOP-18. Run and Shell show “stopped by restart”."
            ]
        );
        // Unticked: Resume selected leaves it out; nothing resumed it.
        let page = summary(
            &report(&["SHOP-27"], &[]),
            &tickets,
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
        let open = summary(&r, &tickets, &projects(), &BTreeSet::new(), true).unwrap();
        assert_eq!((open.rows.len(), open.more), (8, None));
        // Nothing interrupted: no sheet.
        assert_eq!(
            summary(
                &report(&[], &[]),
                &tickets,
                &projects(),
                &BTreeSet::new(),
                false
            ),
            None
        );
    }
}
