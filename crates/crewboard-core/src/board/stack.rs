//! A ticket's dependency chain and its Stack list (DEP-05; d0006, d0049): shared by the board's chain highlight,
//! the panel's Stack section and `crewboard dep tree`, so they always agree.
//!
//! The chain is the ticket's parents and children, transitively. The Stack list shows it indented, from the roots
//! (members with no parent in the chain) down to the dependents, each with its status; a ticket with several
//! parents appears under each one, and the current ticket is marked.

use std::collections::HashSet;

use super::readiness::readiness;
use super::tickets::Ticket;
use crate::domain::{Kind, Status, TicketKey};

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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StackRow {
    pub key: TicketKey,
    pub title: String,
    pub status: Status,
    pub kind: Kind,
    /// 0 for a root; one more per level down.
    pub depth: usize,
    pub current: bool,
    /// A Queued ticket's waiting text, the card's ("waits for SHOP-18, SHOP-19 to merge (SHOP-18 in Review)",
    /// "parent SHOP-26 closed"; T-0083).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waits: Option<String>,
}

/// What a ticket in Dev · Queued waits for, as its card says it: the readiness reason while it waits for its
/// parents or a closed parent blocks it, "waiting for a slot" while the agent limit is full (T-0083). `None`
/// for tickets that aren't Queued, or are ready to start.
pub fn queue_reason(ticket: &Ticket, tickets: &[Ticket]) -> Option<String> {
    match ticket.status {
        Status::Queued | Status::ParentClosed => readiness(ticket, tickets).reason(),
        Status::WaitingForSlot => Some("waiting for a slot".into()),
        _ => None,
    }
}

/// The rows, or an empty list when the ticket has no links.
pub fn stack_rows(current: &TicketKey, tickets: &[Ticket]) -> Vec<StackRow> {
    let Some(chain) = chain(current, tickets) else {
        return Vec::new();
    };
    let find = |k: &TicketKey| tickets.iter().find(|t| &t.key == k);
    let mut roots: Vec<&TicketKey> = chain
        .members
        .iter()
        .filter(|k| {
            find(k).is_some_and(|t| t.depends_on.iter().all(|p| !chain.members.contains(p)))
        })
        .collect();
    roots.sort();
    let mut rows = Vec::new();
    fn walk(
        key: &TicketKey,
        depth: usize,
        current: &TicketKey,
        edges: &[(TicketKey, TicketKey)],
        tickets: &[Ticket],
        path: &mut Vec<TicketKey>,
        rows: &mut Vec<StackRow>,
    ) {
        let Some(t) = tickets.iter().find(|t| &t.key == key) else {
            return;
        };
        rows.push(StackRow {
            key: t.key.clone(),
            title: t.title.clone(),
            status: t.status,
            kind: t.kind,
            depth,
            current: &t.key == current,
            waits: queue_reason(t, tickets),
        });
        path.push(key.clone());
        let mut children: Vec<&TicketKey> = edges
            .iter()
            .filter(|(p, _)| p == key)
            .map(|(_, c)| c)
            .collect();
        children.sort();
        for child in children {
            if !path.contains(child) {
                walk(child, depth + 1, current, edges, tickets, path, rows);
            }
        }
        path.pop();
    }
    for root in roots {
        walk(
            root,
            0,
            current,
            &chain.edges,
            tickets,
            &mut Vec::new(),
            &mut rows,
        );
    }
    rows
}

/// "No dependencies · nothing waits on SHOP-12": the Stack list of a ticket without links.
pub fn empty_text(key: &TicketKey) -> String {
    format!("No dependencies · nothing waits on {key}")
}
