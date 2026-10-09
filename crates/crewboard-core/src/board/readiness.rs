//! Dependency state per ticket, for `crewboard ready` and `crewboard blocked` (d0032, d0049).
//!
//! **Stub until T-0077 (M5):** a ticket is ready when every parent is Merged. T-0077 replaces [`readiness`] with
//! the full rule (merged into the child's base, closed parents never count, epic dependencies) and the waiting
//! reasons the cards show.

use serde::{Deserialize, Serialize};

use super::tickets::Ticket;
use crate::domain::{Column, Status, TicketKey};

/// A parent a ticket still waits for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitingParent {
    pub key: TicketKey,
    pub status: Status,
}

/// One ticket's dependency state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Readiness {
    pub key: TicketKey,
    pub title: String,
    pub status: Status,
    /// Every parent is merged (or there are none).
    pub ready: bool,
    /// The parents not merged yet, in key order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waiting_for: Vec<WaitingParent>,
}

/// The dependency state of every ticket of a project that isn't done yet (Done tickets are left out), in key
/// order.
pub fn readiness(tickets: &[Ticket]) -> Vec<Readiness> {
    let mut states: Vec<Readiness> = tickets
        .iter()
        .filter(|t| t.column != Column::Done)
        .map(|t| {
            let waiting_for: Vec<WaitingParent> = t
                .depends_on
                .iter()
                .filter_map(|parent| {
                    let status = tickets.iter().find(|p| &p.key == parent)?.status;
                    (status != Status::Merged).then(|| WaitingParent {
                        key: parent.clone(),
                        status,
                    })
                })
                .collect();
            Readiness {
                key: t.key.clone(),
                title: t.title.clone(),
                status: t.status,
                ready: waiting_for.is_empty(),
                waiting_for,
            }
        })
        .collect();
    states.sort_by(|a, b| a.key.cmp(&b.key));
    states
}
