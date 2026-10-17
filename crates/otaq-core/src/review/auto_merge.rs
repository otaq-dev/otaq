//! **Auto-merge** (AMG-01 … AMG-05; d0064, d0038, d0018): a validated ticket that the project's Auto-merge applies
//! to is Local-merged without the confirmation, with the squash message the confirmation would propose.
//!
//! - **Merged** → Done · "Merged automatically", a History line, no inbox item and no notification; the Done
//!   cleanup and the waiting tickets follow as after any Local merge (the setting is the user's standing go-ahead).
//! - **Conflict** → nothing merged; the ticket goes back to its agent **without a click** with the `restack` skill
//!   (typed once its input line is empty, never over the user's text), "Merge conflict · fixing". The third
//!   conflict in a row stops: turquoise "Keeps conflicting with `<target>`". A merge in between resets the count.
//!   An epic child's first such stop goes to its live lead instead, once (LEAD-07, [`crate::epic_lead::stuck`]).
//! - **Base diverged** / **the user's checkout dirty** → turquoise with the Local merge's message; Retry merge and
//!   Move to Review are the user's.
//!
//! A manual Local merge keeps its confirmation and "Send to agent to fix" (AMG-05).

use super::{LocalMergeOutcome, Return, Review, other};
use crate::board::BoardError;
use crate::board::validation::AUTO_MERGE_CONFLICT_LIMIT;
use crate::domain::validation::{keeps_conflicting, merged_toast};
use crate::domain::{Actor, AutoMergeStop, Status};
use crate::epic_lead::stuck::{FirstStop, hold_in_dev};
use crate::naming;
use crate::protocol::Event;
use crate::session::SessionKind;
use crate::session::deliveries::DeliveryKind;
use crate::start::StartOutcome;

/// What one Auto-merge did.
#[derive(Debug, Clone, PartialEq)]
pub enum AutoMerged {
    /// Done · "Merged automatically"; `toast` is the event clients show (UI-07).
    Merged { toast: Event },
    /// Back to the agent with restack.
    Conflict,
    /// Turquoise: kept conflicting, base diverged or checkout dirty.
    Stopped(AutoMergeStop),
    /// Kept conflicting for the first time with the epic's lead live: held at work for the lead (LEAD-07).
    WithLead,
    /// Left in Ready for Review (uncommitted files in the worktree: never auto-fixed).
    Left,
}

impl Review {
    /// Runs Auto-merge for a validated ticket in Ready for Review.
    pub async fn auto_merge(&self, key: &str) -> Result<AutoMerged, BoardError> {
        let board = &self.inner.board;
        let proposal = board.merge_proposal(key).await?;
        let actor = Actor::current_user();
        let ticket = board.ticket(key).await?;
        let outcome = self
            .local_merge(key, &proposal.message, true, &actor)
            .await?;
        Ok(match outcome {
            LocalMergeOutcome::Merged {
                ticket: merged,
                started,
                ..
            } => {
                board
                    .record_auto_merged(&merged.id, &proposal.target)
                    .await?;
                tracing::info!(ticket = %merged.key, target = %proposal.target, "merged automatically");
                // "into the Wishlist branch" for an epic child, else the base branch.
                let into = match (&ticket.epic, proposal.into_epic) {
                    (Some(epic), true) => match board.ticket(&epic.to_string()).await {
                        Ok(epic) => format!("the {} branch", epic.title),
                        Err(_) => proposal.target.clone(),
                    },
                    _ => proposal.target.clone(),
                };
                let round = ticket
                    .flags
                    .review_entry
                    .map_or(ticket.validation_round, |e| e.round);
                let started: Vec<String> = started.iter().map(ToString::to_string).collect();
                let (title, message) =
                    merged_toast(&ticket.key.to_string(), &into, round, &started);
                AutoMerged::Merged {
                    toast: Event::MergedAutomatically {
                        project: ticket.key.project.to_string(),
                        key: ticket.key.clone(),
                        title,
                        message,
                    },
                }
            }
            LocalMergeOutcome::Conflict { target, .. } => {
                if ticket.auto_merge_conflicts + 1 >= AUTO_MERGE_CONFLICT_LIMIT {
                    // An epic child's first "Keeps conflicting" goes to its live lead once (LEAD-07): held at
                    // work, the conflict counted.
                    let held = board.leads().takes_stop(&ticket).await;
                    if held {
                        hold_in_dev(board, &ticket.id, true).await?;
                        let first = FirstStop::keeps_conflicting(&target);
                        if board.leads().child_stuck(&ticket, &first).await {
                            return Ok(AutoMerged::WithLead);
                        }
                    }
                    board
                        .auto_merge_stopped(
                            &ticket.id,
                            AutoMergeStop::KeepsConflicting,
                            &keeps_conflicting(&target),
                            !held,
                        )
                        .await?;
                    AutoMerged::Stopped(AutoMergeStop::KeepsConflicting)
                } else {
                    self.hand_conflict_to_agent(&ticket, &target, &actor)
                        .await?;
                    AutoMerged::Conflict
                }
            }
            LocalMergeOutcome::RefusedDiverged { message } => {
                board
                    .auto_merge_stopped(&ticket.id, AutoMergeStop::BaseDiverged, &message, false)
                    .await?;
                AutoMerged::Stopped(AutoMergeStop::BaseDiverged)
            }
            LocalMergeOutcome::RefusedDirtyCheckout { message } => {
                board
                    .auto_merge_stopped(&ticket.id, AutoMergeStop::CheckoutDirty, &message, false)
                    .await?;
                AutoMerged::Stopped(AutoMergeStop::CheckoutDirty)
            }
            LocalMergeOutcome::RefusedUncommitted { message, .. } => {
                tracing::warn!(ticket = %ticket.key, %message, "not merged automatically");
                AutoMerged::Left
            }
            LocalMergeOutcome::Changed { .. } => AutoMerged::Left,
        })
    }

    /// AMG-03: back to the agent with restack, no click: queued for its input line like any automatic message.
    async fn hand_conflict_to_agent(
        &self,
        ticket: &crate::board::tickets::Ticket,
        target: &str,
        actor: &Actor,
    ) -> Result<(), BoardError> {
        let inner = &self.inner;
        let action = Return::Fix { target };
        let skill = action.skill();
        let full = self.limit_full().await?;
        let live = inner.sessions.running_of(&ticket.id, SessionKind::Agent);
        match (&live, full) {
            (Some(session), false) => {
                // A build freed at Review comes back first (M19-c); a queued or resumed ticket gets it at its start.
                let _held = inner
                    .ticket_env
                    .fetch_for_agent(ticket, action.fetch_why(), actor)
                    .await;
                self.record(ticket, &action, Status::Working, None, actor)
                    .await?;
                inner
                    .deliveries
                    .queue_delivery(&session.id, DeliveryKind::Skill, &naming::skill(skill))
                    .await
                    .map_err(other)?;
            }
            (_, true) => {
                self.record(ticket, &action, Status::Queued, Some(skill), actor)
                    .await?;
                inner.scheduler.wake();
            }
            (None, false) => {
                self.record(ticket, &action, Status::Queued, Some(skill), actor)
                    .await?;
                match inner.starter.start(&ticket.key.to_string(), actor).await? {
                    StartOutcome::Started(_) | StartOutcome::Skipped => {}
                    StartOutcome::Held { message } | StartOutcome::Interrupted { message, .. } => {
                        tracing::warn!(ticket = %ticket.key, %message, "resuming the agent stopped");
                    }
                }
            }
        }
        inner.board.auto_merge_conflict(&ticket.id).await?;
        tracing::info!(ticket = %ticket.key, %target, "auto-merge conflict handed to the agent");
        Ok(())
    }
}
