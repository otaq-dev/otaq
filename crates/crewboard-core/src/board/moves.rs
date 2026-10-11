//! Board moves (SDD §7.3 transitions, §7.4 drag rules; TKT-04; d0004, d0034, d0036, d0037, d0041, d0044, d0052).
//!
//! [`check_move`] is one pure function: given a ticket and where it is dropped, it says whether the move is
//! allowed, needs a confirmation or a choice, comes with a warning, or is refused (with a reason for people).
//! [`Board::move_ticket`] carries out the moves whose effects exist so far: Backlog → Dev (Queued) and Reopen
//! (Done · Closed → Backlog, T-0101). The others (Park, Approve, PR rework) are allowed by the rules but carried
//! out by their own requests (T-0104, T-0089/T-0095, T-0144/T-0150); until then they are refused here as
//! "not available yet".

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
    Refused {
        reason: String,
    },
}

impl MoveCheck {
    pub fn action(&self) -> Option<MoveAction> {
        match self {
            Self::Allowed { action } | Self::Confirm { action } | Self::Choose { action } => {
                Some(*action)
            }
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
/// The refusal of Reopen on a merged ticket (CLS-03).
pub const MERGED_LOCKED_REOPEN: &str = "Merged tickets are locked. Create a new ticket.";
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
        // Never a warning: Crewboard doesn't judge how a ticket is written (d0067).
        (Column::Backlog, T::DevQueued) => MoveCheck::Allowed {
            action: MoveAction::Start,
        },
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
    /// Carries out a move. A confirmation or a choice needs `confirmed`; without it the move is refused with
    /// [`Refusal::NeedsConfirmation`] so the client can ask.
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
            // A parked ticket resumes its conversation when the start sequence sees it (d0052).
            MoveAction::Start => self.start(&ticket, actor).await,
            MoveAction::Reopen => self.reopen(key, actor).await,
            MoveAction::Park => not_yet("Park"),
            MoveAction::Approve => not_yet("Approve"),
            MoveAction::PrRework => self.pr_rework(&ticket, actor).await,
        }
    }

    /// Backlog → Dev · Queued. Records the first start and the queue time, and locks the base (d0051); an epic
    /// child also locks its epic's base. The scheduler starts it once its dependencies are merged (SCH-01).
    async fn start(&self, ticket: &Ticket, actor: &Actor) -> Result<Ticket, BoardError> {
        let now = now_ms();
        let mut tx = self.store.pool().begin().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, started_at = COALESCE(started_at, ?),
                                queued_at = ?, branch_locked = TRUE, updated_by = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(Column::Dev.as_str())
        .bind(Status::Queued.as_str())
        .bind(now)
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

    /// **Reopen** (CLS-03; d0044): Done · Closed → Backlog. Merged tickets are locked. A ticket that has been to
    /// Dev is marked reopened: its next start rebases its old branch onto the current base and starts a new
    /// agent session (GIT-06). Reopening an epic child returns its epic to Backlog.
    pub async fn reopen(&self, key: &str, actor: &Actor) -> Result<Ticket, BoardError> {
        let ticket = self.ticket(key).await?;
        match ticket.status {
            Status::Merged => {
                return Err(BoardError::refused(
                    Refusal::MoveRefused,
                    MERGED_LOCKED_REOPEN,
                ));
            }
            Status::ClosedNotMerged => {}
            other => {
                return Err(BoardError::refused(
                    Refusal::MoveRefused,
                    format!(
                        "{} is {}; only Closed tickets can be reopened",
                        ticket.key,
                        other.label()
                    ),
                ));
            }
        }
        let epic = match &ticket.epic {
            Some(epic) => Some(self.ticket(&epic.to_string()).await?),
            None => None,
        };
        if let Some(epic) = &epic
            && epic.status == Status::Merged
        {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!(
                    "{}'s epic {} is merged and locked. Create a new ticket.",
                    ticket.key, epic.key
                ),
            ));
        }
        let started = self.has_been_to_dev(&ticket).await?;
        let reopened = self
            .set_status(&ticket, Status::Backlog, actor, EventKind::Reopened)
            .await?;
        if started {
            sqlx::query("UPDATE tickets SET reopened = TRUE WHERE id = ?")
                .bind(&ticket.id)
                .execute(self.store.pool())
                .await?;
        }
        if let Some(epic) = epic
            && epic.column != Column::Backlog
        {
            let to = if epic.status == Status::EpicInProgress {
                Status::EpicInProgress
            } else {
                Status::Backlog
            };
            self.set_status(&epic, to, actor, EventKind::Reopened)
                .await?;
        }
        tracing::info!(key = %reopened.key, started, "reopened");
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
