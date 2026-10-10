//! The reconcile table of REC-01 (d0018, d0045), as a pure function: what a ticket becomes when the service finds
//! its agent session alive or gone.
//!
//! | State before | Session alive | Session gone (reboot, crash, service restart, `claude` exited) |
//! |---|---|---|
//! | Dev · working, in background, waiting for limit reset | reattach silently | **Interrupted** (inbox item, Resume / Resume all) |
//! | Dev · orange (Asks you, Needs you, Idle) | reattach silently | **Interrupted, keeping the label and question** |
//! | Ready for Review, Approved | reattach silently | **not** Interrupted; History notes "session ended (restart)" |
//! | anything else | — | unchanged |
//!
//! Run and Shell are sessions, not ticket states: a restart ends them as "stopped by restart"
//! ([`crate::session::END_STOPPED_BY_RESTART`]). **Nothing resumes automatically.**

use crate::domain::Status;

/// What reconcile does with a ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing to do: the session is alive (reattach silently), or the state doesn't depend on a session.
    Unchanged,
    /// Interrupted, with an inbox item and Resume. `keep_label`: an orange ticket keeps its label and question.
    Interrupt { keep_label: bool },
    /// Ready for Review or Approved: not Interrupted, no inbox item, not in Resume all; History notes that the
    /// session ended. Only Send back resumes it.
    NoteSessionEnded,
}

/// The statuses whose agent is at work: their session ending interrupts them.
pub const AT_WORK: &[Status] = &[
    Status::Working,
    Status::WorkingInBackground,
    Status::WaitingForLimitReset,
    Status::AsksYou,
    Status::NeedsYou,
    Status::Idle,
];

/// REC-01's table.
pub fn reconcile(before: Status, session_alive: bool) -> Outcome {
    if session_alive {
        return Outcome::Unchanged;
    }
    match before {
        s if s.is_orange() => Outcome::Interrupt { keep_label: true },
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
    fn orange_becomes_interrupted_and_keeps_its_label() {
        for status in Status::ORANGE {
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
}
