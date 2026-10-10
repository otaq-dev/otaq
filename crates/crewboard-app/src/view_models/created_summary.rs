//! The Project terminal's summary of created tickets (TKT-08, RUN-06; *17 · Project terminal → The review moment,
//! Undo a created ticket*): "4 tickets, 3 dependencies · all in Backlog", one row per ticket with its dependency
//! badges and number of "Done when" items, a trash button while it never went to Dev (d0060), and "Nothing runs
//! until you drag a ticket to Dev." It sits beside the terminal, never over its output.

use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Column, TicketKey};

/// The line under the rows.
pub const FOOTER: &str =
    "Edit them on the board before starting. Nothing runs until you drag a ticket to Dev.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryRow {
    pub key: TicketKey,
    pub title: String,
    /// What it depends on ("SHOP-17").
    pub deps: Vec<String>,
    /// "3 criteria", "1 criterion", "no criteria".
    pub criteria: String,
    /// Never went to Dev: it can be deleted (else only closed, d0060).
    pub deletable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// "4 tickets, 3 dependencies".
    pub headline: String,
    /// "all in Backlog", "3 in Backlog".
    pub place: String,
    pub rows: Vec<SummaryRow>,
}

/// The summary of `created` (in creation order); `None` when nothing was created.
pub fn summary(created: &[Ticket]) -> Option<Summary> {
    if created.is_empty() {
        return None;
    }
    let keys: Vec<&TicketKey> = created.iter().map(|t| &t.key).collect();
    // Links between the created tickets (a link to an older ticket isn't one of "theirs").
    let links: usize = created
        .iter()
        .map(|t| t.depends_on.iter().filter(|d| keys.contains(d)).count())
        .sum();
    let n = created.len();
    let in_backlog = created
        .iter()
        .filter(|t| t.column == Column::Backlog)
        .count();
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    Some(Summary {
        headline: format!(
            "{}, {}",
            plural(n, "ticket", "tickets"),
            plural(links, "dependency", "dependencies")
        ),
        place: if in_backlog == n {
            "all in Backlog".to_owned()
        } else {
            format!("{in_backlog} in Backlog")
        },
        rows: created
            .iter()
            .map(|t| SummaryRow {
                key: t.key.clone(),
                title: t.title.clone(),
                deps: t.depends_on.iter().map(ToString::to_string).collect(),
                criteria: match t.done_when.len() {
                    0 => "no criteria".to_owned(),
                    n => plural(n, "criterion", "criteria"),
                },
                deletable: t.column == Column::Backlog && !t.has_started(),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use crewboard_core::board::tickets::DoneWhenItem;
    use crewboard_core::domain::{Actor, Status};

    fn key(s: &str) -> TicketKey {
        s.parse().unwrap()
    }

    fn criteria(t: &mut Ticket, n: usize) {
        t.done_when = (0..n)
            .map(|i| DoneWhenItem {
                text: format!("item {i}"),
                added_by: Actor::User("me".into()),
            })
            .collect();
    }

    #[test]
    fn the_review_moment() {
        let mut t18 = ticket("SHOP-18", "Checkout API", Status::Backlog);
        t18.depends_on = vec![key("SHOP-17")];
        criteria(&mut t18, 3);
        let mut t19 = ticket("SHOP-19", "Price table", Status::Backlog);
        criteria(&mut t19, 2);
        let mut t20 = ticket("SHOP-20", "Checkout page", Status::Backlog);
        t20.depends_on = vec![key("SHOP-18"), key("SHOP-19")];
        criteria(&mut t20, 3);
        let mut t21 = ticket("SHOP-21", "Order confirmation email", Status::Backlog);
        t21.depends_on = vec![key("SHOP-20")];
        criteria(&mut t21, 0);
        let s = summary(&[t18, t19, t20, t21]).unwrap();
        assert_eq!(s.headline, "4 tickets, 3 dependencies");
        assert_eq!(s.place, "all in Backlog");
        assert_eq!(s.rows[0].deps, ["SHOP-17"]);
        assert_eq!(s.rows[0].criteria, "3 criteria");
        assert_eq!(s.rows[3].criteria, "no criteria");
        assert!(s.rows.iter().all(|r| r.deletable));
        assert_eq!(summary(&[]), None);
    }

    #[test]
    fn a_ticket_dragged_to_dev_has_no_trash() {
        let mut t = ticket("SHOP-19", "Price table", Status::Queued);
        criteria(&mut t, 1);
        let s = summary(&[t, ticket("SHOP-21", "Email", Status::Backlog)]).unwrap();
        assert_eq!(s.headline, "2 tickets, 0 dependencies");
        assert_eq!(s.place, "1 in Backlog");
        assert_eq!(s.rows[0].criteria, "1 criterion");
        assert!(!s.rows[0].deletable, "it went to Dev: only Close");
        assert!(s.rows[1].deletable);
        // A parked ticket is back in Backlog but has started.
        let mut parked = ticket("SHOP-22", "Parked", Status::Parked);
        parked.started_at = Some(1);
        assert!(!summary(&[parked]).unwrap().rows[0].deletable);
    }
}
