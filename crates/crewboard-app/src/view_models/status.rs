//! How a ticket's status (the core's `Status`, SDD §7.2) is drawn: which entry of the *Status Legend* it uses
//! (colour + icon) and its label. Colour is never the only signal: every pill has an icon and a label.

use crewboard_core::domain::Status as Core;

use crate::theme::Status as Legend;

/// The status a ticket shows: while its terminal has focus a waiting agent stays calm, "Agent Working" with a
/// quiet "your turn" (ATT-03, d0042); otherwise its own status.
pub fn shown(ticket: &crewboard_core::board::tickets::Ticket) -> Core {
    if ticket.flags.your_turn {
        Core::Working
    } else {
        ticket.status
    }
}

/// The legend entry and the label for a status.
pub fn pill(status: Core) -> (Legend, &'static str) {
    let legend = match status {
        Core::Backlog => Legend::Backlog,
        Core::Parked => Legend::Parked,
        Core::EpicInProgress => Legend::Epic,
        Core::Queued | Core::WaitingForSlot | Core::Held => Legend::Queued,
        Core::ParentClosed => Legend::ParentClosed,
        Core::StartInterrupted => Legend::StartInterrupted,
        Core::Working => Legend::AgentWorking,
        Core::WorkingInBackground => Legend::WorkingInBackground,
        Core::AsksYou => Legend::AsksYou,
        Core::NeedsYou => Legend::NeedsYou,
        Core::Idle => Legend::Idle,
        Core::WaitingForLimitReset => Legend::WaitingForLimitReset,
        Core::Interrupted | Core::ResumeFailed => Legend::Interrupted,
        Core::ReadyForReview => Legend::ReadyForReview,
        Core::PrOpen => Legend::PrOpen,
        Core::Merged => Legend::Merged,
        Core::ClosedNotMerged => Legend::ClosedNotMerged,
        // The M12 states draw with their nearest legend entry until their own pills arrive (T-0192).
        Core::Validating => Legend::AgentWorking,
        Core::MergeConflictFixing => Legend::Conflicts,
        Core::ValidatorNeedsYou
        | Core::ValidationFailed
        | Core::ValidationNoResult
        | Core::AutoMergeStopped => Legend::NeedsYou,
    };
    let label = match status {
        // The legend's name for a working agent.
        Core::Working => legend.label(),
        other => other.label(),
    };
    (legend, label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Tier;

    #[test]
    fn every_status_has_an_icon_and_a_label() {
        for status in Core::ALL {
            let (legend, label) = pill(*status);
            assert!(!label.is_empty(), "{status}");
            let tier = legend.spec().0;
            if status.is_orange() || *status == Core::Interrupted {
                assert_eq!(tier, Tier::NeedsYou, "{status} must stand out");
            }
        }
        assert_eq!(pill(Core::Working), (Legend::AgentWorking, "Agent Working"));
        assert_eq!(pill(Core::Held).1, "Held");
        assert_eq!(pill(Core::ResumeFailed).1, "Resume failed");
        assert_eq!(
            pill(Core::ClosedNotMerged),
            (Legend::ClosedNotMerged, "Closed · not merged")
        );
    }
}
