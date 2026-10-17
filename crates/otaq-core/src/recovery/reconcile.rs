//! The reconcile table of REC-01 (d0018, d0045), as a pure function: what a ticket becomes when the service finds
//! its agent session alive or gone.
//!
//! | State before | Session alive | Session gone (reboot, crash, service restart, the agent tool exited) |
//! |---|---|---|
//! | Dev · working, in background, waiting for limit reset | reattach silently | **Interrupted** (inbox item, Resume / Resume all) |
//! | Dev · turquoise (Asks you, Needs you, Idle) | reattach silently | **Interrupted, keeping the label and question** |
//! | Ready for Review, Approved | reattach silently | **not** Interrupted; History notes "session ended (restart)" |
//! | Dev · Validating…, Needs you · validator (**the validator's** session) | reattach silently | **Interrupted** from Validating: Resume starts a fresh validation (d0063) |
//! | anything else | — | unchanged |
//!
//! The validation and Auto-merge stops wait for the user's click and need no session: a restart leaves them as
//! they are. Merge conflict · fixing is the agent at work.
//!
//! Run and Shell are sessions, not ticket states: a restart ends them as "stopped by restart"
//! ([`crate::session::END_STOPPED_BY_RESTART`]). **Nothing resumes automatically.**

use crate::domain::Status;

/// What reconcile does with a ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing to do: the session is alive (reattach silently), or the state doesn't depend on a session.
    Unchanged,
    /// Interrupted, with an inbox item and Resume. `keep_label`: a needs-you ticket keeps its label and question.
    Interrupt { keep_label: bool },
    /// Ready for Review or Approved: not Interrupted, no inbox item, not in Resume all; History notes that the
    /// session ended. Only Send back resumes it.
    NoteSessionEnded,
    /// The validator is gone: Interrupted from Validating; Resume starts a fresh validation, the round doesn't
    /// count, the old validator conversation is never resumed (REC-01, VAL-07).
    InterruptValidation,
}

/// The statuses whose agent is at work: their session ending interrupts them.
pub const AT_WORK: &[Status] = &[
    Status::Working,
    Status::WorkingInBackground,
    Status::WaitingForLimitReset,
    Status::MergeConflictFixing,
    Status::AsksYou,
    Status::NeedsYou,
    Status::Idle,
];

/// REC-01's M12 row: a validating ticket and its validator's session.
pub fn reconcile_validation(before: Status, validator_alive: bool) -> Outcome {
    match before {
        Status::Validating | Status::ValidatorNeedsYou if !validator_alive => {
            Outcome::InterruptValidation
        }
        _ => Outcome::Unchanged,
    }
}

/// REC-01's table.
pub fn reconcile(before: Status, session_alive: bool) -> Outcome {
    if session_alive {
        return Outcome::Unchanged;
    }
    match before {
        s if s.is_agent_waiting() => Outcome::Interrupt { keep_label: true },
        s if AT_WORK.contains(&s) => Outcome::Interrupt { keep_label: false },
        Status::ReadyForReview | Status::PrOpen => Outcome::NoteSessionEnded,
        _ => Outcome::Unchanged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_background_and_limit_wait_become_interrupted() {
        for status in [
            Status::Working,
            Status::WorkingInBackground,
            Status::WaitingForLimitReset,
        ] {
            assert_eq!(
                reconcile(status, false),
                Outcome::Interrupt { keep_label: false },
                "{status}"
            );
        }
    }

    #[test]
    fn needs_you_becomes_interrupted_and_keeps_its_label() {
        for status in Status::AGENT_WAITS {
            assert_eq!(
                reconcile(*status, false),
                Outcome::Interrupt { keep_label: true },
                "{status}"
            );
        }
    }

    #[test]
    fn review_and_approved_only_note_the_end() {
        assert_eq!(
            reconcile(Status::ReadyForReview, false),
            Outcome::NoteSessionEnded
        );
        assert_eq!(reconcile(Status::PrOpen, false), Outcome::NoteSessionEnded);
    }

    #[test]
    fn a_live_session_is_reattached_silently() {
        for status in Status::ALL {
            assert_eq!(reconcile(*status, true), Outcome::Unchanged, "{status}");
        }
    }

    #[test]
    fn everything_else_is_left_alone() {
        for status in [
            Status::Backlog,
            Status::Parked,
            Status::EpicInProgress,
            Status::Queued,
            Status::WaitingForSlot,
            Status::Held,
            Status::ParentClosed,
            Status::StartInterrupted,
            Status::Interrupted,
            Status::ResumeFailed,
            Status::Merged,
            Status::ClosedNotMerged,
        ] {
            assert_eq!(reconcile(status, false), Outcome::Unchanged, "{status}");
        }
    }

    /// T-0188 (REC-01, d0063): the validator gone → Interrupted from Validating; alive → nothing. The agent's
    /// session doesn't decide a validating ticket, and the M12 stops wait for the user with or without one.
    #[test]
    fn the_m12_rows() {
        for status in [Status::Validating, Status::ValidatorNeedsYou] {
            assert_eq!(
                reconcile_validation(status, false),
                Outcome::InterruptValidation
            );
            assert_eq!(reconcile_validation(status, true), Outcome::Unchanged);
            assert_eq!(reconcile(status, false), Outcome::Unchanged, "{status}");
        }
        for status in [
            Status::ValidationFailed,
            Status::ValidationNoResult,
            Status::AutoMergeStopped,
        ] {
            assert_eq!(reconcile(status, false), Outcome::Unchanged, "{status}");
        }
        assert_eq!(
            reconcile(Status::MergeConflictFixing, false),
            Outcome::Interrupt { keep_label: false }
        );
    }
}
