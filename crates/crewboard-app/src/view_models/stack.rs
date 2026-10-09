//! The Stack list (DEP-05; d0006, d0049; *12 · Stack & dependencies → Stack list · detail*): a ticket's chain
//! as an indented list, from the roots (parents with no parents of their own) down to the dependents, each
//! with its status. A ticket with several parents appears under each one; the current ticket is marked.
//! Only the chain of the current ticket is listed (its other dependents' chains are not).

use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::TicketKey;

pub use crewboard_core::board::stack::{StackRow, empty_text, stack_rows};

/// The collapsed Stack row's text: "Depends on SHOP-18, SHOP-19 · SHOP-21 waits on it", or the empty text.
pub fn summary(current: &TicketKey, tickets: &[Ticket]) -> String {
    let parents: Vec<String> = tickets
        .iter()
        .find(|t| &t.key == current)
        .map(|t| t.depends_on.iter().map(|k| k.to_string()).collect())
        .unwrap_or_default();
    let children: Vec<String> = tickets
        .iter()
        .filter(|t| t.depends_on.contains(current))
        .map(|t| t.key.to_string())
        .collect();
    let mut parts = Vec::new();
    if !parents.is_empty() {
        parts.push(format!("Depends on {}", parents.join(", ")));
    }
    if !children.is_empty() {
        let verb = if children.len() == 1 { "waits" } else { "wait" };
        parts.push(format!("{} {verb} on it", children.join(", ")));
    }
    if parts.is_empty() {
        empty_text(current)
    } else {
        parts.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use crewboard_core::domain::Status;

    fn key(k: &str) -> TicketKey {
        k.parse().unwrap()
    }

    fn with(k: &str, title: &str, status: Status, deps: &[&str]) -> Ticket {
        let mut t = ticket(k, title, status);
        t.depends_on = deps.iter().map(|d| key(d)).collect();
        t
    }

    fn sample() -> Vec<Ticket> {
        vec![
            with("SHOP-17", "Add Stripe client", Status::Merged, &[]),
            with("SHOP-18", "Checkout API", Status::Backlog, &["SHOP-17"]),
            with("SHOP-19", "Price table", Status::Backlog, &[]),
            with(
                "SHOP-20",
                "Checkout page",
                Status::Backlog,
                &["SHOP-18", "SHOP-19"],
            ),
            with(
                "SHOP-21",
                "Order confirmation email",
                Status::Backlog,
                &["SHOP-20"],
            ),
            with(
                "SHOP-23",
                "Split user full name",
                Status::Backlog,
                &["SHOP-19"],
            ),
        ]
    }

    #[test]
    fn the_frames_example() {
        let rows = stack_rows(&key("SHOP-20"), &sample());
        let flat: Vec<(String, usize, bool)> = rows
            .iter()
            .map(|r| (r.key.to_string(), r.depth, r.current))
            .collect();
        assert_eq!(
            flat,
            [
                ("SHOP-17".into(), 0, false),
                ("SHOP-18".into(), 1, false),
                ("SHOP-20".into(), 2, true),
                ("SHOP-21".into(), 3, false),
                ("SHOP-19".into(), 0, false),
                ("SHOP-20".into(), 1, true),
                ("SHOP-21".into(), 2, false),
            ]
        );
        assert!(
            rows.iter().all(|r| r.key != key("SHOP-23")),
            "not in SHOP-20's chain"
        );
        assert_eq!(rows[0].status, Status::Merged);
    }

    #[test]
    fn deep_chains_and_no_links() {
        let tickets: Vec<Ticket> = (1..=6)
            .map(|i| {
                let deps: Vec<String> = if i > 1 {
                    vec![format!("SHOP-{}", i - 1)]
                } else {
                    vec![]
                };
                let refs: Vec<&str> = deps.iter().map(String::as_str).collect();
                with(&format!("SHOP-{i}"), "t", Status::Backlog, &refs)
            })
            .collect();
        let rows = stack_rows(&key("SHOP-6"), &tickets);
        assert_eq!(rows.len(), 6);
        assert_eq!(rows.last().unwrap().depth, 5);
        assert!(
            stack_rows(
                &key("SHOP-23"),
                &[with("SHOP-23", "x", Status::Backlog, &[])]
            )
            .is_empty()
        );
        assert_eq!(
            empty_text(&key("SHOP-12")),
            "No dependencies · nothing waits on SHOP-12"
        );
    }

    #[test]
    fn collapsed_summary() {
        let tickets = vec![
            with("SHOP-18", "Checkout API", Status::ReadyForReview, &[]),
            with("SHOP-19", "Payment form", Status::Working, &[]),
            with(
                "SHOP-20",
                "Checkout page",
                Status::Queued,
                &["SHOP-18", "SHOP-19"],
            ),
            with("SHOP-21", "Order email", Status::Backlog, &["SHOP-20"]),
            with("SHOP-30", "Alone", Status::Backlog, &[]),
        ];
        assert_eq!(
            summary(&key("SHOP-20"), &tickets),
            "Depends on SHOP-18, SHOP-19 · SHOP-21 waits on it"
        );
        assert_eq!(summary(&key("SHOP-18"), &tickets), "SHOP-20 waits on it");
        assert_eq!(
            summary(&key("SHOP-30"), &tickets),
            "No dependencies · nothing waits on SHOP-30"
        );
    }
}
