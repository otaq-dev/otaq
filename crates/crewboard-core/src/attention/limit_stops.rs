//! Agents stopped by the plan limit raise no inbox items while the plan-limit banner is up (ATT-09, d0046): one
//! banner for the situation, not one item per agent (P6).
//!
//! The switch and the marks live in the service's memory. The plan-limit work of M8 (T-0129) turns the switch on
//! while its banner is active and marks the agents the limit stopped; a ticket in **Waiting for limit reset**
//! always counts as stopped by the limit. The needs-you items (and so the inbox, its counts and back-pressure)
//! leave those tickets out while the switch is on.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::board::tickets::Ticket;
use crate::domain::Status;

/// The switch and the marked tickets. Cheap to clone; clones share the same state.
#[derive(Debug, Clone, Default)]
pub struct LimitStops {
    inner: Arc<Mutex<Suppression>>,
}

/// A snapshot of [`LimitStops`], applied when deriving needs-you items.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Suppression {
    /// `suppress_limit_stopped`: the plan-limit banner is up.
    pub suppress_limit_stopped: bool,
    /// Tickets whose agent the limit stopped (by id).
    pub marked: HashSet<String>,
}

impl Suppression {
    /// Whether the ticket's item is left out.
    pub fn hides(&self, ticket: &Ticket) -> bool {
        self.suppress_limit_stopped
            && (ticket.status == Status::WaitingForLimitReset || self.marked.contains(&ticket.id))
    }
}

impl LimitStops {
    /// Turns `suppress_limit_stopped` on or off; returns whether it changed.
    pub fn set_suppress(&self, on: bool) -> bool {
        let mut state = self.inner.lock().expect("limit stops");
        std::mem::replace(&mut state.suppress_limit_stopped, on) != on
    }

    /// Marks a ticket's agent as stopped by the limit (or not any more).
    pub fn mark(&self, ticket_id: &str, stopped: bool) {
        let mut state = self.inner.lock().expect("limit stops");
        if stopped {
            state.marked.insert(ticket_id.to_owned());
        } else {
            state.marked.remove(ticket_id);
        }
    }

    pub fn snapshot(&self) -> Suppression {
        self.inner.lock().expect("limit stops").clone()
    }
}
