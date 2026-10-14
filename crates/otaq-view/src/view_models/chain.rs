//! The dependency chain of a hovered or selected card (DEP-05, d0006; *03 · Board & columns → Chain highlight ·
//! hover or select SHOP-20*): its parents and children, transitively, keep full opacity with an accent
//! outline; everything else dims; arrows are drawn only for this chain, parent → child. No graph view.

use std::collections::HashSet;

use otaq_core::board::tickets::Ticket;
use otaq_core::domain::TicketKey;
use serde::Serialize;
use ts_rs::TS;

pub use otaq_core::board::stack::{Chain, chain};

/// One arrow of a chain, parent → child.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ChainEdge")]
pub struct Edge {
    pub parent: String,
    pub child: String,
}

/// A card's chain as the Tauri board draws it while the card is hovered or selected (T-0229). Sent with the board
/// for every linked card up front, so a hover never waits on a round trip (UIT-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ChainView")]
pub struct View {
    /// The hovered or selected card.
    pub key: String,
    /// The card itself, its parents and its children, transitively, in board key order.
    pub members: Vec<String>,
    pub edges: Vec<Edge>,
    /// The note at the board's bottom right: "Chain of SHOP-20 · SHOP-17 → SHOP-18 → SHOP-20 ← SHOP-19 ·
    /// SHOP-20 → SHOP-21".
    pub legend: String,
}

/// The chain of every linked ticket in `tickets` (unlinked ones have none: nothing to highlight).
pub fn views(tickets: &[Ticket]) -> Vec<View> {
    tickets
        .iter()
        .filter_map(|t| {
            let chain = chain(&t.key, tickets)?;
            let mut members: Vec<&TicketKey> = chain.members.iter().collect();
            members.sort();
            Some(View {
                key: t.key.to_string(),
                members: members.iter().map(ToString::to_string).collect(),
                edges: chain
                    .edges
                    .iter()
                    .map(|(parent, child)| Edge {
                        parent: parent.to_string(),
                        child: child.to_string(),
                    })
                    .collect(),
                legend: legend(&t.key, tickets),
            })
        })
        .collect()
}

/// The line from `from` following each ticket's first link (`next`), as far as it goes, `from` first.
fn line(
    from: &TicketKey,
    next: &dyn Fn(&TicketKey) -> Option<TicketKey>,
    seen: &mut HashSet<TicketKey>,
) -> Vec<TicketKey> {
    let mut keys = vec![from.clone()];
    seen.insert(from.clone());
    while let Some(k) = next(keys.last().expect("not empty")) {
        if !seen.insert(k.clone()) {
            break;
        }
        keys.push(k);
    }
    keys
}

fn join(keys: &[TicketKey], arrow: &str) -> String {
    keys.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(arrow)
}

/// "Chain of SHOP-20 · SHOP-17 → SHOP-18 → SHOP-20 ← SHOP-19 · SHOP-20 → SHOP-21": the first parent's line up to the
/// card, its other parents after "←", then one line down from each child. A branching line follows the first link
/// only; the highlight itself always shows the whole chain.
pub fn legend(key: &TicketKey, tickets: &[Ticket]) -> String {
    let find = |k: &TicketKey| tickets.iter().find(|t| &t.key == k);
    let parents = |k: &TicketKey| find(k).map(|t| t.depends_on.clone()).unwrap_or_default();
    let children = |k: &TicketKey| -> Vec<TicketKey> {
        tickets
            .iter()
            .filter(|t| t.depends_on.contains(k))
            .map(|t| t.key.clone())
            .collect()
    };
    let first_parent = |k: &TicketKey| parents(k).into_iter().next();
    let first_child = |k: &TicketKey| children(k).into_iter().next();
    let mut parts = vec![format!("Chain of {key}")];
    let ups = parents(key);
    if let Some((first, rest)) = ups.split_first() {
        let mut seen = HashSet::from([key.clone()]);
        let mut up = line(first, &first_parent, &mut seen);
        up.reverse();
        up.push(key.clone());
        let mut text = join(&up, " → ");
        for other in rest {
            text.push_str(" ← ");
            text.push_str(&join(&line(other, &first_parent, &mut seen), " ← "));
        }
        parts.push(text);
    }
    for child in children(key) {
        let mut seen = HashSet::from([key.clone()]);
        let mut down = vec![key.clone()];
        down.extend(line(&child, &first_child, &mut seen));
        parts.push(join(&down, " → "));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use otaq_core::board::tickets::Ticket;
    use otaq_core::domain::Status;
    use otaq_core::domain::TicketKey;

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

    #[test]
    fn the_legend_reads_the_chain_as_the_frame_does() {
        let tickets = sample();
        assert_eq!(
            legend(&key("SHOP-20"), &tickets),
            "Chain of SHOP-20 · SHOP-17 → SHOP-18 → SHOP-20 ← SHOP-19 · SHOP-20 → SHOP-21",
            "*03 → Chain highlight · hover or select SHOP-20 → Chain legend*"
        );
        assert_eq!(
            legend(&key("SHOP-17"), &tickets),
            "Chain of SHOP-17 · SHOP-17 → SHOP-18 → SHOP-20 → SHOP-21"
        );
        assert_eq!(
            legend(&key("SHOP-19"), &tickets),
            "Chain of SHOP-19 · SHOP-19 → SHOP-20 → SHOP-21 · SHOP-19 → SHOP-23"
        );
    }

    #[test]
    fn every_linked_card_gets_its_chain_up_front() {
        let views = views(&sample());
        let keys: Vec<&str> = views.iter().map(|v| v.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "SHOP-17", "SHOP-18", "SHOP-19", "SHOP-20", "SHOP-21", "SHOP-23"
            ],
            "SHOP-22 has no links"
        );
        let shop_20 = &views[3];
        assert_eq!(
            shop_20.members,
            ["SHOP-17", "SHOP-18", "SHOP-19", "SHOP-20", "SHOP-21"]
        );
        assert_eq!(
            shop_20.edges[1],
            Edge {
                parent: "SHOP-18".into(),
                child: "SHOP-20".into()
            }
        );
        let json = serde_json::to_value(shop_20).unwrap();
        assert_eq!(json["edges"][0]["parent"], "SHOP-17");
    }
}
