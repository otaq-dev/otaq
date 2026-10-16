//! The epic lead's log for the Messages section (MSG-01, LEAD-06, LEAD-07, LEAD-10; d0083; T-0290): the questions
//! the epic's children asked its lead ([`super::questions::log_of`]) and the lead's notes to them
//! ([`super::notes`]), on the epic (every child's) and on a child (its own). A ticket outside an epic has none.

use serde::{Deserialize, Serialize};

use super::notes::{self, LeadNote};
use super::questions::{self, LeadQuestion};
use crate::board::{Board, BoardError};

/// What `ticket.lead_log` answers, oldest first in each list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeadLog {
    pub questions: Vec<LeadQuestion>,
    pub notes: Vec<LeadNote>,
}

/// The lead's log on the ticket `key`: an epic's covers every child, a child's only its own.
pub async fn of(board: &Board, key: &str) -> Result<LeadLog, BoardError> {
    let ticket = board.ticket(key).await?;
    let epic = if ticket.is_epic() {
        Some(ticket.key.to_string())
    } else {
        ticket.epic.as_ref().map(ToString::to_string)
    };
    let Some(epic) = epic else {
        return Ok(LeadLog::default());
    };
    let questions = questions::log_of(board, &ticket.id).await?;
    let notes = notes::list_for_epic(board, &epic)
        .await?
        .into_iter()
        .filter(|n| ticket.is_epic() || n.child_id == ticket.id)
        .collect();
    Ok(LeadLog { questions, notes })
}
