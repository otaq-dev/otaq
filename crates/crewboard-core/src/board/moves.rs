//! Board moves (SDD §7.3 transitions, §7.4 drag rules; TKT-04; d0004, d0034, d0036, d0037, d0041, d0044, d0052).
//!
//! [`check_move`] is one pure function: given a ticket and where it is dropped, it says whether the move is
//! allowed, needs a confirmation or a choice, comes with a warning, or is refused (with a reason for people).
//! [`Board::move_ticket`] carries out the moves whose effects exist so far: Backlog → Dev (Queued) and reopening
//! a never-started Closed ticket. The others (Park, Approve, PR rework, reopening started work) are allowed by
//! the rules but carried out by their own tickets (T-0104, T-0089/T-0095, T-0144/T-0150, T-0101); until then
//! they are refused as "not available yet".

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::tickets::{EventKind, Ticket, record_event};
use super::{Board, BoardError, Refusal};
use crate::domain::{Actor, Column, Kind, Status, stored_enum};
use crate::store::now_ms;
use crate::store::tickets::set_state_since;

stored_enum! {
    /// Where a card is dropped. Dev has two parts: Agent Working above Queued.
    pub enum DropTarget ("drop target") {
        Backlog = "backlog",
        /// The Dev column's header or its Queued section.
        DevQueued = "dev_queued",
        /// The Agent Working section of Dev.
        DevWorking = "dev_working",
        ReadyForReview = "ready_for_review",
        Approved = "approved",
        Done = "done",
    }
}

impl DropTarget {
    pub fn column(self) -> Column {
        match self {
            Self::Backlog => Column::Backlog,
            Self::DevQueued | Self::DevWorking => Column::Dev,
            Self::ReadyForReview => Column::ReadyForReview,
            Self::Approved => Column::Approved,
            Self::Done => Column::Done,
        }
    }
}

stored_enum! {
    /// What an allowed move does.
    pub enum MoveAction ("move action") {
        /// Backlog → Dev: Queued, starts when possible. Resumes the conversation for a Parked ticket (d0052).
        Start = "start",
        /// Dev → Backlog: stop the agent and keep the work (d0052).
        Park = "park",
        /// Ready for Review → Approved: asks Local merge or Open PR (d0015).
        Approve = "approve",
        /// Approved → Ready for Review: PR rework (d0041).
        PrRework = "pr_rework",
        /// Done (Closed) → Backlog (d0044).
        Reopen = "reopen",
    }
}

stored_enum! {
    /// A warning that never blocks: the user may go on ("Start anyway") or fix it first.
    pub enum MoveWarning ("move warning") {
        /// "Done when" is empty (TKT-04, d0019): "Start anyway" / "Add criteria".
        EmptyDoneWhen = "empty_done_when",
    }
}

/// The verdict of [`check_move`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum MoveCheck {
    /// Dropped where it already is: nothing happens.
    Unchanged,
    Allowed {
        action: MoveAction,
    },
    /// Allowed after the user confirms (Park).
    Confirm {
        action: MoveAction,
    },
    /// Allowed after the user picks how (Approve: Local merge or Open PR).
    Choose {
        action: MoveAction,
    },
    /// Allowed; shows a warning first, never blocks.
    Warning {
        action: MoveAction,
        warning: MoveWarning,
    },
    Refused {
        reason: String,
    },
}

impl MoveCheck {
    pub fn action(&self) -> Option<MoveAction> {
        match self {
            Self::Allowed { action }
            | Self::Confirm { action }
            | Self::Choose { action }
            | Self::Warning { action, .. } => Some(*action),
            Self::Unchanged | Self::Refused { .. } => None,
        }
    }

    pub fn is_refused(&self) -> bool {
        matches!(self, Self::Refused { .. })
    }
}

/// The reason given when a ticket is dragged into Ready for Review.
pub const ONLY_AGENT_SUBMITS: &str =
    "Only the agent or Submit for review moves a ticket to Ready for Review";
pub const MERGED_LOCKED: &str = "Merged tickets are locked";
pub const EPICS_NOT_TO_DEV: &str = "Epics don't go to Dev";

/// Decides a drag or a "Start" (⇧⌘D, which is a drop on [`DropTarget::DevQueued`]).
pub fn check_move(ticket: &Ticket, target: DropTarget) -> MoveCheck {
    use DropTarget as T;
    let refused = |reason: &str| MoveCheck::Refused {
        reason: reason.to_owned(),
    };
    let from = ticket.column;
    let to = target.column();

    if ticket.status == Status::Merged && to != Column::Done {
        return refused(MERGED_LOCKED);
    }
    if ticket.kind == Kind::Epic && to == Column::Dev {
        return refused(EPICS_NOT_TO_DEV);
    }
    if target == T::DevWorking && from != Column::Dev {
        return refused("Tickets enter Agent Working when their agent starts");
    }
    if from == to {
        return MoveCheck::Unchanged;
    }
    match (from, target) {
        (Column::Backlog, T::DevQueued) => {
            if ticket.done_when.is_empty() {
                MoveCheck::Warning {
                    action: MoveAction::Start,
                    warning: MoveWarning::EmptyDoneWhen,
                }
            } else {
                MoveCheck::Allowed {
                    action: MoveAction::Start,
                }
            }
        }
        (Column::Dev, T::Backlog) => MoveCheck::Confirm {
            action: MoveAction::Park,
        },
        (Column::ReadyForReview, T::Approved) => MoveCheck::Choose {
            action: MoveAction::Approve,
        },
        (Column::Approved, T::ReadyForReview) => MoveCheck::Allowed {
            action: MoveAction::PrRework,
        },
        (Column::Done, T::Backlog) => MoveCheck::Allowed {
            action: MoveAction::Reopen,
        },
        (_, T::ReadyForReview) => refused(ONLY_AGENT_SUBMITS),
        (Column::Done, _) => refused("Drag it to Backlog to reopen it first"),
        (Column::ReadyForReview, T::DevQueued | T::DevWorking) => {
            refused("Use Send back to return it to Dev with your comments")
        }
        (_, T::DevQueued | T::DevWorking) => {
            refused("Drag it back through Ready for Review: Approved tickets return with PR rework")
        }
        (_, T::Approved) => refused("Approve a ticket from Ready for Review"),
        (_, T::Done) => refused("Use Close ticket, Approve or Mark merged to finish a ticket"),
        (_, T::Backlog) => refused("Only Dev and Closed tickets go back to Backlog"),
    }
}

impl Board {
    /// Carries out a move. A warning needs `confirmed` (the user chose "Start anyway"); without it the move is
    /// refused with [`Refusal::EmptyDoneWhen`] so the client can show the dialog.
    pub async fn move_ticket(
        &self,
        key: &str,
        target: DropTarget,
        confirmed: bool,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let ticket = self.ticket(key).await?;
        let check = check_move(&ticket, target);
        match &check {
            MoveCheck::Unchanged => return Ok(ticket),
            MoveCheck::Refused { reason } => {
                return Err(BoardError::refused(Refusal::MoveRefused, reason.clone()));
            }
            MoveCheck::Warning {
                warning: MoveWarning::EmptyDoneWhen,
                ..
            } if !confirmed => {
                return Err(BoardError::refused(
                    Refusal::EmptyDoneWhen,
                    format!("{} has no \"Done when\" criteria", ticket.key),
                ));
            }
            MoveCheck::Confirm { .. } | MoveCheck::Choose { .. } if !confirmed => {
                return Err(BoardError::refused(
                    Refusal::NeedsConfirmation,
                    "This move needs your confirmation",
                ));
            }
            _ => {}
        }
        let action = check.action().expect("allowed moves have an action");
        let not_yet = |what: &str| {
            Err(BoardError::refused(
                Refusal::NotAvailableYet,
                format!("{what} isn't available yet"),
            ))
        };
        match action {
            MoveAction::Start if ticket.status == Status::Parked => {
                not_yet("Resuming a parked ticket")
            }
            MoveAction::Start => self.start(&ticket, actor).await,
            MoveAction::Reopen if self.has_been_to_dev(&ticket).await? => {
                not_yet("Reopening a ticket that has been to Dev")
            }
            MoveAction::Reopen => {
                self.set_status(&ticket, Status::Backlog, actor, EventKind::Moved)
                    .await
            }
            MoveAction::Park => not_yet("Park"),
            MoveAction::Approve => not_yet("Approve"),
            MoveAction::PrRework => not_yet("PR rework"),
        }
    }

    /// Backlog → Dev · Queued. Records the first start and locks the base (d0051); an epic child also locks
    /// its epic's base. A Queued ticket stays Queued until sessions (M3) and the scheduler (M5) exist.
    async fn start(&self, ticket: &Ticket, actor: &Actor) -> Result<Ticket, BoardError> {
        let now = now_ms();
        let mut tx = self.store.pool().begin().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, started_at = COALESCE(started_at, ?),
                                branch_locked = TRUE, updated_by = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(Column::Dev.as_str())
        .bind(Status::Queued.as_str())
        .bind(now)
        .bind(actor.to_string())
        .bind(now)
        .bind(Status::Queued.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        if let Some(epic) = &ticket.epic {
            let epic_id = self.ticket_id(&epic.to_string()).await?;
            sqlx::query("UPDATE tickets SET branch_locked = TRUE WHERE id = ?")
                .bind(epic_id)
                .execute(&mut *tx)
                .await?;
        }
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::Moved,
            actor,
            json!({"from": ticket.status, "to": Status::Queued}),
        )
        .await?;
        tx.commit().await?;
        self.ticket_by_id(&ticket.id).await
    }

    async fn set_status(
        &self,
        ticket: &Ticket,
        to: Status,
        actor: &Actor,
        kind: EventKind,
    ) -> Result<Ticket, BoardError> {
        let now = now_ms();
        let mut tx = self.store.pool().begin().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, outcome = NULL, updated_by = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(to.column().as_str())
        .bind(to.as_str())
        .bind(actor.to_string())
        .bind(now)
        .bind(to.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            &ticket.id,
            kind,
            actor,
            json!({"from": ticket.status, "to": to}),
        )
        .await?;
        tx.commit().await?;
        self.ticket_by_id(&ticket.id).await
    }
}
