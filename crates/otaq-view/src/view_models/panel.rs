//! What the ticket sheet (`sheet.rs`, SHT-01) keeps of the old side panel's logic (UI-04; d0058): previous/next ticket
//! in the column's order (j / k, ⌘[ / ⌘]); the Start interrupted callout; the branch the header shows. The panel's
//! tabs are gone: the sheet's tabs and Session's switch replaced them (`sheet::Content`, `session_tab.rs`, T-0466).

use otaq_core::board::order::sort_column;
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::TicketKey;

/// The ticket before (`-1`) or after (`+1`) `current` in its column's order (both Dev sections, top to bottom).
pub fn neighbour(tickets: &[Ticket], current: &TicketKey, step: i32) -> Option<TicketKey> {
    let ticket = tickets.iter().find(|t| &t.key == current)?;
    let order = sort_column(ticket.status.set(), tickets);
    let keys: Vec<&TicketKey> = order.cards.iter().collect();
    let at = keys.iter().position(|k| *k == current)? as i32;
    let next = at + step;
    (next >= 0 && (next as usize) < keys.len()).then(|| keys[next as usize].clone())
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
