//! The dependency chain of a hovered or selected card (DEP-05, d0006; *03 · Board & columns → Chain highlight ·
//! hover or select SHOP-20*): its parents and children, transitively, keep full opacity with an accent
//! outline; everything else dims; arrows are drawn only for this chain, parent → child. No graph view.

use std::collections::HashSet;

use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::TicketKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chain {
    pub members: HashSet<TicketKey>,
    /// (parent, child) for every link between members.
    pub edges: Vec<(TicketKey, TicketKey)>,
}

/// The chain through `key`, or `None` when the ticket has no links (nothing to highlight).
pub fn chain(key: &TicketKey, tickets: &[Ticket]) -> Option<Chain> {
    let parents_of = |k: &TicketKey| -> Vec<TicketKey> {
        tickets
            .iter()
            .find(|t| &t.key == k)
            .map(|t| t.depends_on.clone())
            .unwrap_or_default()
    };
    let children_of = |k: &TicketKey| -> Vec<TicketKey> {
        tickets
            .iter()
            .filter(|t| t.depends_on.contains(k))
            .map(|t| t.key.clone())
            .collect()
    };
    let mut members: HashSet<TicketKey> = HashSet::from([key.clone()]);
    for walk in [
        &parents_of as &dyn Fn(&TicketKey) -> Vec<TicketKey>,
        &children_of,
    ] {
        let mut stack = vec![key.clone()];
        let mut seen = HashSet::from([key.clone()]);
        while let Some(at) = stack.pop() {
            for next in walk(&at) {
                if seen.insert(next.clone()) {
                    members.insert(next.clone());
                    stack.push(next);
                }
            }
        }
    }
    if members.len() < 2 {
        return None;
    }
    let mut edges: Vec<(TicketKey, TicketKey)> = tickets
        .iter()
        .filter(|t| members.contains(&t.key))
        .flat_map(|t| {
            t.depends_on
                .iter()
                .filter(|p| members.contains(p))
                .map(|p| (p.clone(), t.key.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    edges.sort();
    Some(Chain { members, edges })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use crewboard_core::domain::Status;

    fn sample() -> Vec<Ticket> {
        let with = |key: &str, deps: &[&str]| {
            let mut t = ticket(key, key, Status::Backlog);
            t.depends_on = deps.iter().map(|d| d.parse().unwrap()).collect();
            t
        };
        vec![
            with("SHOP-17", &[]),
            with("SHOP-18", &["SHOP-17"]),
            with("SHOP-19", &[]),
            with("SHOP-20", &["SHOP-18", "SHOP-19"]),
            with("SHOP-21", &["SHOP-20"]),
            with("SHOP-22", &[]),
            with("SHOP-23", &["SHOP-19"]),
        ]
    }

    fn key(k: &str) -> TicketKey {
        k.parse().unwrap()
    }

    #[test]
    fn hovering_shop_20_lights_its_whole_chain() {
        let tickets = sample();
        let chain = chain(&key("SHOP-20"), &tickets).unwrap();
        let mut members: Vec<String> = chain.members.iter().map(ToString::to_string).collect();
        members.sort();
        assert_eq!(
            members,
            ["SHOP-17", "SHOP-18", "SHOP-19", "SHOP-20", "SHOP-21"]
        );
        assert!(!chain.members.contains(&key("SHOP-22")), "dimmed");
        assert!(
            !chain.members.contains(&key("SHOP-23")),
            "a sibling through SHOP-19 isn't in SHOP-20's chain"
        );
        let edges: Vec<String> = chain
            .edges
            .iter()
            .map(|(p, c)| format!("{p}→{c}"))
            .collect();
        assert_eq!(
            edges,
            [
                "SHOP-17→SHOP-18",
                "SHOP-18→SHOP-20",
                "SHOP-19→SHOP-20",
                "SHOP-20→SHOP-21"
            ]
        );
    }

    #[test]
    fn a_ticket_without_links_highlights_nothing() {
        assert_eq!(chain(&key("SHOP-22"), &sample()), None);
        let from_root = chain(&key("SHOP-17"), &sample()).unwrap();
        assert!(
            from_root.members.contains(&key("SHOP-21")),
            "children down the whole way"
        );
        assert!(
            !from_root.members.contains(&key("SHOP-19")),
            "not a co-parent"
        );
    }
}
