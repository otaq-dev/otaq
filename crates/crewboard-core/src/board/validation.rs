//! The M12 rows of SDD §7.3 as one pure function (d0063, d0064): validation before Review and Auto-merge.
//!
//! [`transition`] takes where a ticket stands ([`ValidationState`]) and what happened ([`ValidationEvent`]) and
//! says where it goes, how it entered Review, what happens to the round counter, and whether the validator's
//! findings go to the agent. It does no I/O: the service carries the result out (T-0183 … T-0189), and every row
//! is tested on its own.

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::tickets::{EventKind, Ticket, record_event};
use super::{Board, BoardError};
use crate::domain::{
    Actor, AutoMergeStop, ReviewEntry, Status, ValidationOutcome, ValidationTrigger,
};
use crate::store::tickets::set_state_since;
use crate::store::{new_id, now_ms};

/// Conflicts in a row before Auto-merge stops trying (AMG-03).
pub const AUTO_MERGE_CONFLICT_LIMIT: u32 = 3;

/// What the rules need to know about a ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationState {
    pub status: Status,
    /// Rounds counted so far (`tickets.validation_round`).
    pub round: u32,
    /// The project's rounds setting plus the extra rounds the user allowed.
    pub rounds: u32,
    /// Validation is on for the project.
    pub validation: bool,
    /// The project's Auto-merge applies to this ticket (its kind and the setting; never an epic).
    pub auto_merge: bool,
    /// Auto-merge conflicts in a row so far.
    pub conflicts: u32,
}

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationEvent {
    /// The agent's `review submit`.
    AgentSubmit,
    /// The user's Validate and submit.
    ValidateAndSubmit,
    /// The user's Submit for review: skips validation (d0052, d0063).
    SubmitByUser,
    Pass,
    Fail,
    /// The validator's turn ended without `validate pass|fail`.
    NoVerdict,
    /// The validator changed tracked files or committed (VAL-02).
    ValidatorChangedFiles,
    /// The validator waits on a permission prompt.
    ValidatorNeedsYou,
    /// The user typed into the waiting validator.
    ValidatorContinues,
    /// Tracked files or the head changed through the agent, or the user edited the description (VAL-08).
    CodeChanged,
    StopValidation,
    SkipToReview,
    MoveToReviewAnyway,
    AnotherRound,
    AnswerAgent,
    RetryValidation,
    /// Auto-merge's Local merge landed.
    AutoMerged,
    /// Auto-merge's rebase conflicted.
    AutoMergeConflict,
    /// Auto-merge couldn't run: the base diverged or the user's checkout is dirty.
    AutoMergeBlocked(AutoMergeStop),
    RetryMerge,
    /// From an Auto-merge stop: into Ready for Review, still "Validated · round N".
    MoveToReview,
}

/// What a ticket's round counter does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Round {
    Keep,
    /// The round that just ended counts.
    Count,
    /// Back to 0 (Answer the agent, an Auto-merge conflict).
    Reset,
}

/// The result of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    pub to: Status,
    /// Set when the ticket enters Ready for Review.
    pub entry: Option<ReviewEntry>,
    pub round: Round,
    /// How the open validation round ends, if one does.
    pub outcome: Option<ValidationOutcome>,
    /// A fresh validator starts (submit, Retry validation).
    pub start_validator: bool,
    /// The findings go to the agent as an automatic message (VAL-04, Give it another round, Answer the agent).
    pub deliver_findings: bool,
    /// The agent is told the validation was cancelled because the code changed (VAL-08).
    pub tell_cancelled: bool,
    /// Run the Local merge without the confirmation (AMG-02).
    pub auto_merge: bool,
    /// Hand the conflict to the agent with the restack skill, no click (AMG-03).
    pub restack: bool,
    /// One more round is allowed (Give it another round).
    pub extra_round: bool,
    /// Why Auto-merge stopped, when it did.
    pub stop: Option<AutoMergeStop>,
}

impl Transition {
    fn to(to: Status) -> Self {
        Self {
            to,
            entry: None,
            round: Round::Keep,
            outcome: None,
            start_validator: false,
            deliver_findings: false,
            tell_cancelled: false,
            auto_merge: false,
            restack: false,
            extra_round: false,
            stop: None,
        }
    }

    fn review(entry: ReviewEntry) -> Self {
        Self {
            entry: Some(entry),
            ..Self::to(Status::ReadyForReview)
        }
    }

    fn ended(mut self, outcome: ValidationOutcome) -> Self {
        self.outcome = Some(outcome);
        self
    }
}

/// Why an event doesn't apply where the ticket stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotNow(pub String);

/// Agent Working, with or without the orange family: where a submit is accepted.
fn agent_works(status: Status) -> bool {
    matches!(
        status,
        Status::Working
            | Status::WorkingInBackground
            | Status::AsksYou
            | Status::NeedsYou
            | Status::Idle
            | Status::MergeConflictFixing
    )
}

fn validating(status: Status) -> bool {
    matches!(status, Status::Validating | Status::ValidatorNeedsYou)
}

/// One M12 row of SDD §7.3.
pub fn transition(s: ValidationState, event: ValidationEvent) -> Result<Transition, NotNow> {
    use ValidationEvent as E;
    let not_now = || {
        Err(NotNow(format!(
            "{event:?} doesn't apply while the ticket is {}",
            s.status.label()
        )))
    };
    let submit = |s: ValidationState| {
        if !s.validation {
            // Validation off for the project: straight to Review, as before M12.
            return Transition::to(Status::ReadyForReview);
        }
        let mut t = Transition::to(Status::Validating);
        t.start_validator = true;
        // A fix after an Auto-merge conflict validates again with the counter at 0 (AMG-03).
        if s.status == Status::MergeConflictFixing {
            t.round = Round::Reset;
        }
        t
    };
    Ok(match event {
        E::AgentSubmit | E::ValidateAndSubmit if agent_works(s.status) => submit(s),
        E::SubmitByUser if agent_works(s.status) => {
            Transition::review(ReviewEntry::SubmittedByUser)
        }
        E::Pass if validating(s.status) => {
            let mut t = if s.auto_merge {
                Transition {
                    auto_merge: true,
                    entry: Some(ReviewEntry::Validated),
                    ..Transition::to(Status::ReadyForReview)
                }
            } else {
                Transition::review(ReviewEntry::Validated)
            };
            t.round = Round::Count;
            t.ended(ValidationOutcome::Pass)
        }
        E::Fail if validating(s.status) => {
            let last = s.round + 1 >= s.rounds;
            let mut t = if last {
                Transition::to(Status::ValidationFailed)
            } else {
                Transition {
                    deliver_findings: true,
                    ..Transition::to(Status::Working)
                }
            };
            t.round = Round::Count;
            t.ended(ValidationOutcome::Fail)
        }
        E::NoVerdict if validating(s.status) => {
            Transition::to(Status::ValidationNoResult).ended(ValidationOutcome::NoResult)
        }
        E::ValidatorChangedFiles if validating(s.status) => {
            Transition::to(Status::ValidationNoResult).ended(ValidationOutcome::Discarded)
        }
        E::ValidatorNeedsYou if s.status == Status::Validating => {
            Transition::to(Status::ValidatorNeedsYou)
        }
        E::ValidatorContinues if s.status == Status::ValidatorNeedsYou => {
            Transition::to(Status::Validating)
        }
        E::CodeChanged if validating(s.status) => Transition {
            tell_cancelled: true,
            ..Transition::to(Status::Working)
        }
        .ended(ValidationOutcome::Cancelled),
        E::StopValidation if validating(s.status) => {
            Transition::to(Status::Working).ended(ValidationOutcome::Stopped)
        }
        E::SkipToReview if validating(s.status) => {
            Transition::review(ReviewEntry::Skipped).ended(ValidationOutcome::Skipped)
        }
        E::MoveToReviewAnyway if s.status == Status::ValidationFailed => {
            Transition::review(ReviewEntry::FailedMoved)
        }
        // No verdict: "Validation skipped by you" (d0063 changelog).
        E::MoveToReviewAnyway if s.status == Status::ValidationNoResult => {
            Transition::review(ReviewEntry::Skipped)
        }
        E::AnotherRound if s.status == Status::ValidationFailed => Transition {
            deliver_findings: true,
            extra_round: true,
            ..Transition::to(Status::Working)
        },
        E::AnswerAgent if s.status == Status::ValidationFailed => Transition {
            deliver_findings: true,
            round: Round::Reset,
            ..Transition::to(Status::Working)
        },
        E::RetryValidation if s.status == Status::ValidationNoResult => Transition {
            start_validator: true,
            ..Transition::to(Status::Validating)
        },
        E::AutoMerged if s.status == Status::ReadyForReview => Transition::to(Status::Merged),
        E::AutoMergeConflict if s.status == Status::ReadyForReview => {
            if s.conflicts + 1 >= AUTO_MERGE_CONFLICT_LIMIT {
                Transition {
                    stop: Some(AutoMergeStop::KeepsConflicting),
                    ..Transition::to(Status::AutoMergeStopped)
                }
            } else {
                Transition {
                    restack: true,
                    round: Round::Reset,
                    ..Transition::to(Status::MergeConflictFixing)
                }
            }
        }
        E::AutoMergeBlocked(stop) if s.status == Status::ReadyForReview => Transition {
            stop: Some(stop),
            ..Transition::to(Status::AutoMergeStopped)
        },
        E::RetryMerge if s.status == Status::AutoMergeStopped => Transition {
            auto_merge: true,
            ..Transition::to(Status::ReadyForReview)
        },
        E::MoveToReview if s.status == Status::AutoMergeStopped => {
            Transition::to(Status::ReadyForReview)
        }
        _ => return not_now(),
    })
}

impl Board {
    /// Opens a validation round for a submit (VAL-01): records the summary and head like any submit, opens the
    /// round in `validations` and sets Validating… (or "Waiting for limit reset" while the usage limit holds,
    /// marked as a validation that waits).
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn enter_validation(
        &self,
        ticket: &Ticket,
        step: &Transition,
        head: &str,
        summary: &str,
        trigger: ValidationTrigger,
        held: bool,
        actor: &Actor,
    ) -> Result<(), BoardError> {
        let now = now_ms();
        let counted = if step.round == Round::Reset {
            0
        } else {
            ticket.validation_round
        };
        let status = if held {
            Status::WaitingForLimitReset
        } else {
            step.to
        };
        let mut tx = self.store.pool().begin().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, review_entry = NULL, validation_round = ?,
                                interrupted_from = ?, signal_text = NULL, signal_since = NULL, wakeup_at = NULL,
                                sent_back_at = NULL, resume_skill = NULL, conflicts_with = NULL,
                                conflict_files = NULL, conflict_reason = NULL, auto_merge_stop = NULL,
                                updated_by = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(status.column().as_str())
        .bind(status.as_str())
        .bind(i64::from(counted))
        .bind(held.then_some(Status::Validating.as_str()))
        .bind(actor.to_string())
        .bind(now)
        .bind(status.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO validations (id, ticket_id, round, trigger, started_at, head_sha) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(new_id())
        .bind(&ticket.id)
        .bind(i64::from(counted + 1))
        .bind(trigger.as_str())
        .bind(now)
        .bind(head)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::ReviewSubmitted,
            actor,
            json!({ "summary": summary.trim(), "head": head, "validation": true }),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(key = %ticket.key, %actor, round = counted + 1, held, "validation opened");
        Ok(())
    }

    /// The validator of the ticket's open round started: its session is stored on the round, and a validation
    /// that waited for the usage limit shows Validating… again.
    pub(crate) async fn validator_started(
        &self,
        ticket_id: &str,
        session_id: &str,
    ) -> Result<(), BoardError> {
        let now = now_ms();
        let mut tx = self.store.pool().begin().await?;
        sqlx::query(
            "UPDATE validations SET session_id = ? WHERE id = (
                 SELECT id FROM validations WHERE ticket_id = ? AND ended_at IS NULL
                 ORDER BY started_at DESC, id DESC LIMIT 1)",
        )
        .bind(session_id)
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
        let validating = Status::Validating;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, interrupted_from = NULL, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ? AND state = ? AND interrupted_from = ?"
        ))
        .bind(validating.column().as_str())
        .bind(validating.as_str())
        .bind(now)
        .bind(validating.as_str())
        .bind(now)
        .bind(ticket_id)
        .bind(Status::WaitingForLimitReset.as_str())
        .bind(validating.as_str())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The validator couldn't start: "Validation ended without a result" with the reason, the round ends without
    /// one (it doesn't count) and Retry validation or Move to Review anyway are the user's (VAL-07).
    pub(crate) async fn validator_did_not_start(
        &self,
        ticket_id: &str,
        reason: &str,
    ) -> Result<(), BoardError> {
        let now = now_ms();
        let to = Status::ValidationNoResult;
        let mut tx = self.store.pool().begin().await?;
        sqlx::query(
            "UPDATE validations SET ended_at = ?, outcome = ?, report = ? WHERE ticket_id = ? AND ended_at IS NULL",
        )
        .bind(now)
        .bind(ValidationOutcome::NoResult.as_str())
        .bind(format!("The validator didn't start: {reason}"))
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, signal_text = ?, signal_since = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(to.column().as_str())
        .bind(to.as_str())
        .bind(format!("The validator didn't start: {reason}"))
        .bind(now)
        .bind(now)
        .bind(to.as_str())
        .bind(now)
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Tickets whose validation waits for the usage limit to reset (SCH-04).
    pub(crate) async fn validations_waiting_for_limit(&self) -> Result<Vec<String>, BoardError> {
        let ids: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM tickets WHERE state = ? AND interrupted_from = ?")
                .bind(Status::WaitingForLimitReset.as_str())
                .bind(Status::Validating.as_str())
                .fetch_all(self.store.pool())
                .await?;
        Ok(ids.into_iter().map(|(id,)| id).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ValidationEvent as E;

    fn state(status: Status) -> ValidationState {
        ValidationState {
            status,
            round: 0,
            rounds: 3,
            validation: true,
            auto_merge: false,
            conflicts: 0,
        }
    }

    fn go(s: ValidationState, event: ValidationEvent) -> Transition {
        transition(s, event).unwrap_or_else(|e| panic!("{e:?}"))
    }

    /// Working → Validating on the agent's submit and on Validate and submit; with validation off, straight to
    /// Ready for Review.
    #[test]
    fn submit_starts_a_validator() {
        for event in [E::AgentSubmit, E::ValidateAndSubmit] {
            let t = go(state(Status::Working), event);
            assert_eq!(t.to, Status::Validating);
            assert!(t.start_validator && t.round == Round::Keep && t.outcome.is_none());
        }
        let off = ValidationState {
            validation: false,
            ..state(Status::Working)
        };
        let t = go(off, E::AgentSubmit);
        assert_eq!((t.to, t.entry), (Status::ReadyForReview, None));
        assert!(!t.start_validator);
        assert!(transition(state(Status::Validating), E::AgentSubmit).is_err());
    }

    /// Submit for review skips validation: "Submitted by you", never auto-merged.
    #[test]
    fn submit_by_user_skips_validation() {
        let t = go(
            ValidationState {
                auto_merge: true,
                ..state(Status::Working)
            },
            E::SubmitByUser,
        );
        assert_eq!(
            (t.to, t.entry, t.auto_merge),
            (
                Status::ReadyForReview,
                Some(ReviewEntry::SubmittedByUser),
                false
            )
        );
        assert!(transition(state(Status::Validating), E::SubmitByUser).is_err());
    }

    /// Validating → Ready for Review "Validated · round N" on pass; with Auto-merge, the merge runs.
    #[test]
    fn pass() {
        let t = go(state(Status::Validating), E::Pass);
        assert_eq!(
            (t.to, t.entry, t.round, t.outcome, t.auto_merge),
            (
                Status::ReadyForReview,
                Some(ReviewEntry::Validated),
                Round::Count,
                Some(ValidationOutcome::Pass),
                false
            )
        );
        let auto = ValidationState {
            auto_merge: true,
            ..state(Status::Validating)
        };
        assert!(go(auto, E::Pass).auto_merge);
    }

    /// Fail with rounds left → back to the agent with the findings; the last round → orange, nothing delivered.
    #[test]
    fn fail_and_the_last_fail() {
        let t = go(state(Status::Validating), E::Fail);
        assert_eq!(
            (t.to, t.deliver_findings, t.round),
            (Status::Working, true, Round::Count)
        );
        let last = ValidationState {
            round: 2,
            ..state(Status::Validating)
        };
        let t = go(last, E::Fail);
        assert_eq!(
            (t.to, t.deliver_findings, t.round, t.outcome),
            (
                Status::ValidationFailed,
                false,
                Round::Count,
                Some(ValidationOutcome::Fail)
            )
        );
        // One round: the first failure is the last; five rounds: the fourth isn't.
        let one = ValidationState {
            rounds: 1,
            ..state(Status::Validating)
        };
        assert_eq!(go(one, E::Fail).to, Status::ValidationFailed);
        let five = ValidationState {
            rounds: 5,
            round: 3,
            ..state(Status::Validating)
        };
        assert_eq!(go(five, E::Fail).to, Status::Working);
    }

    /// No verdict and a discarded verdict: orange, not a round.
    #[test]
    fn no_verdict() {
        for (event, outcome) in [
            (E::NoVerdict, ValidationOutcome::NoResult),
            (E::ValidatorChangedFiles, ValidationOutcome::Discarded),
        ] {
            let t = go(state(Status::Validating), event);
            assert_eq!(
                (t.to, t.round, t.outcome),
                (Status::ValidationNoResult, Round::Keep, Some(outcome))
            );
            assert!(!outcome.counts());
        }
    }

    #[test]
    fn validator_permission_prompt_and_back() {
        assert_eq!(
            go(state(Status::Validating), E::ValidatorNeedsYou).to,
            Status::ValidatorNeedsYou
        );
        assert_eq!(
            go(state(Status::ValidatorNeedsYou), E::ValidatorContinues).to,
            Status::Validating
        );
        // A verdict also lands while the prompt shows.
        assert_eq!(
            go(state(Status::ValidatorNeedsYou), E::Pass).to,
            Status::ReadyForReview
        );
    }

    /// Code changed, Stop validation and Skip to Review: none counts as a round.
    #[test]
    fn changes_stop_and_skip() {
        let t = go(state(Status::Validating), E::CodeChanged);
        assert_eq!(
            (t.to, t.tell_cancelled, t.round, t.outcome),
            (
                Status::Working,
                true,
                Round::Keep,
                Some(ValidationOutcome::Cancelled)
            )
        );
        let t = go(state(Status::Validating), E::StopValidation);
        assert_eq!(
            (t.to, t.tell_cancelled, t.outcome),
            (Status::Working, false, Some(ValidationOutcome::Stopped))
        );
        let t = go(
            ValidationState {
                auto_merge: true,
                ..state(Status::Validating)
            },
            E::SkipToReview,
        );
        assert_eq!(
            (t.to, t.entry, t.auto_merge),
            (Status::ReadyForReview, Some(ReviewEntry::Skipped), false)
        );
        for event in [E::StopValidation, E::SkipToReview, E::CodeChanged] {
            assert!(
                transition(state(Status::Working), event).is_err(),
                "{event:?}"
            );
        }
    }

    /// After the last failed round: Move to Review anyway, Give it another round, Answer the agent.
    #[test]
    fn after_the_last_failed_round() {
        let failed = state(Status::ValidationFailed);
        let t = go(failed, E::MoveToReviewAnyway);
        assert_eq!(
            (t.to, t.entry, t.auto_merge),
            (
                Status::ReadyForReview,
                Some(ReviewEntry::FailedMoved),
                false
            )
        );
        let t = go(failed, E::AnotherRound);
        assert_eq!(
            (t.to, t.deliver_findings, t.extra_round, t.round),
            (Status::Working, true, true, Round::Keep)
        );
        let t = go(failed, E::AnswerAgent);
        assert_eq!(
            (t.to, t.deliver_findings, t.round),
            (Status::Working, true, Round::Reset)
        );
    }

    /// Ended without a result: Retry validation (a fresh validator, not a round) or Move to Review anyway
    /// ("Validation skipped by you").
    #[test]
    fn after_no_result() {
        let t = go(state(Status::ValidationNoResult), E::RetryValidation);
        assert_eq!(
            (t.to, t.start_validator, t.round),
            (Status::Validating, true, Round::Keep)
        );
        let t = go(state(Status::ValidationNoResult), E::MoveToReviewAnyway);
        assert_eq!(t.entry, Some(ReviewEntry::Skipped));
        assert!(transition(state(Status::ValidationFailed), E::RetryValidation).is_err());
    }

    /// Auto-merge: success → Merged; a conflict → back to the agent with restack, the counter at 0; the third
    /// conflict in a row → stopped; base diverged / checkout dirty → stopped; Retry merge and Move to Review.
    #[test]
    fn auto_merge() {
        let review = state(Status::ReadyForReview);
        assert_eq!(go(review, E::AutoMerged).to, Status::Merged);
        let t = go(review, E::AutoMergeConflict);
        assert_eq!(
            (t.to, t.restack, t.round),
            (Status::MergeConflictFixing, true, Round::Reset)
        );
        let third = ValidationState {
            conflicts: 2,
            ..review
        };
        let t = go(third, E::AutoMergeConflict);
        assert_eq!(
            (t.to, t.restack, t.stop),
            (
                Status::AutoMergeStopped,
                false,
                Some(AutoMergeStop::KeepsConflicting)
            )
        );
        for stop in [AutoMergeStop::BaseDiverged, AutoMergeStop::CheckoutDirty] {
            let t = go(review, E::AutoMergeBlocked(stop));
            assert_eq!((t.to, t.stop), (Status::AutoMergeStopped, Some(stop)));
        }
        let stopped = state(Status::AutoMergeStopped);
        let t = go(stopped, E::RetryMerge);
        assert_eq!((t.to, t.auto_merge), (Status::ReadyForReview, true));
        let t = go(stopped, E::MoveToReview);
        assert_eq!(
            (t.to, t.auto_merge, t.entry),
            (Status::ReadyForReview, false, None),
            "still Validated · round N"
        );
        // The fix after a conflict validates again with the counter at 0.
        let t = go(state(Status::MergeConflictFixing), E::AgentSubmit);
        assert_eq!((t.to, t.round), (Status::Validating, Round::Reset));
    }
}
