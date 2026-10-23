//! The dependency chain of a hovered or selected card (DEP-05, DPV-02; d0006; *24 · Dependencies on the board*
//! `s24-09`): its parents and children, transitively, keep full opacity with an accent outline; everything else dims;
//! arrows are drawn for this chain, parent → child, and a tooltip says why ("SHOP-20 is blocked" · "by SHOP-18
//! (Review) and SHOP-29 (Dev)"). With Dependencies: Always every open link stays drawn as well
//! ([`crate::view_models::dependencies::links`]). No graph view.

use otaq_core::board::tickets::Ticket;
use otaq_core::domain::TicketKey;
use serde::Serialize;
use ts_rs::TS;

use crate::view_models::dependencies::{self, Tip};
use crate::view_models::flow_columns::Columns;

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
    /// The tooltip under the hovered card (DPV-02); `null` when its links are all merged.
    pub tip: Option<Tip>,
}

/// The chain of every linked ticket in `tickets` (unlinked ones have none: nothing to highlight), its tooltip
/// naming where each linked ticket is in `columns`.
pub fn views(columns: &Columns, tickets: &[Ticket]) -> Vec<View> {
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
                tip: dependencies::tip(columns, t, tickets),
            })
        })
        .collect()
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
    fn every_linked_card_gets_its_chain_up_front() {
        let views = views(
            &Columns::of(&otaq_core::flow::default_file().flow),
            &sample(),
        );
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
        // Every ticket of the sample is in Backlog: SHOP-20 waits for both parents (DPV-02's tooltip).
        assert_eq!(
            shop_20
                .tip
                .as_ref()
                .map(|t| (t.title.as_str(), t.detail.as_str())),
            Some((
                "SHOP-20 is blocked",
                "by SHOP-18 (Backlog) and SHOP-19 (Backlog)"
            ))
        );
        let json = serde_json::to_value(shop_20).unwrap();
        assert_eq!(json["edges"][0]["parent"], "SHOP-17");
    }
}
