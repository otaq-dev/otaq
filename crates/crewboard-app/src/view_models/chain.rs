//! The dependency chain of a hovered or selected card (DEP-05, d0006; *03 · Board & columns → Chain highlight ·
//! hover or select SHOP-20*): its parents and children, transitively, keep full opacity with an accent
//! outline; everything else dims; arrows are drawn only for this chain, parent → child. No graph view.

pub use crewboard_core::board::stack::{Chain, chain};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use crewboard_core::board::tickets::Ticket;
    use crewboard_core::domain::Status;
    use crewboard_core::domain::TicketKey;

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
