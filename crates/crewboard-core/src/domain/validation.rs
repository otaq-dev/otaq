//! Validation and Auto-merge (SDD §7.2 M12 rows, §14.0, §14.8; d0063, d0064): how a ticket reached Review, the
//! outcome of a validation round, why Auto-merge stopped, and the exact texts the board shows for them.
//!
//! Texts with a number or a branch ("Validation failed 3 times", "Keeps conflicting with `main`") are built
//! here, so the card, the panel, the inbox and the CLI read the same.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::stored_enum;

/// "Validating…": the agent submitted and the validator is checking (neutral, d0063).
pub const VALIDATING: &str = "Validating…";
/// The validator waits on a permission prompt (orange).
pub const VALIDATOR_NEEDS_YOU: &str = "Needs you · validator";
/// The validator ended its turn without a verdict (orange).
pub const NO_RESULT: &str = "Validation ended without a result";
/// The validator changed tracked files or committed: its verdict is thrown away (VAL-02).
pub const DISCARDED: &str = "Validation discarded: the validator changed files";
/// Auto-merge's free rebase conflicted; the agent is rebasing with `restack` (AMG-03).
pub const MERGE_CONFLICT_FIXING: &str = "Merge conflict · fixing";
/// Done through Auto-merge (AMG-02).
pub const MERGED_AUTOMATICALLY: &str = "Merged automatically";

/// "Validation failed 3 times": the last round failed (VAL-06).
pub fn failed_times(times: u32) -> String {
    format!("Validation failed {times} times")
}

/// "Keeps conflicting with `main`": the third Auto-merge conflict in a row (AMG-03).
pub fn keeps_conflicting(target: &str) -> String {
    format!("Keeps conflicting with `{target}`")
}

stored_enum! {
    /// How a ticket reached Ready for Review (d0063). `None` on tickets that got there before M12 or with
    /// validation off.
    pub enum ReviewEntry ("review entry") {
        /// Passed validation: "Validated · round N". Only these may auto-merge (d0064).
        Validated = "validated",
        /// No verdict, and the user moved it on: "Validation skipped by you".
        Skipped = "skipped",
        /// The user's Submit for review, which skips validation: "Submitted by you" (d0052).
        SubmittedByUser = "submitted_by_user",
        /// Failed every round, moved on by the user: "Failed M times · moved by you" (reads as "did not pass").
        FailedMoved = "failed_moved",
    }
}

impl ReviewEntry {
    /// The card's text; `round` is the ticket's validation round (the round that passed, or the rounds that
    /// failed).
    pub fn label(self, round: u32) -> String {
        match self {
            Self::Validated => format!("Validated · round {round}"),
            Self::Skipped => "Validation skipped by you".to_owned(),
            Self::SubmittedByUser => "Submitted by you".to_owned(),
            Self::FailedMoved => format!("Failed {round} times · moved by you"),
        }
    }

    /// Auto-merge applies only to validated tickets (AMG-01).
    pub fn may_auto_merge(self) -> bool {
        self == Self::Validated
    }
}

/// The Review entry as a card flag: the entry and its round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewEntryFlag {
    pub entry: ReviewEntry,
    pub round: u32,
}

impl fmt::Display for ReviewEntryFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.entry.label(self.round))
    }
}

/// "Validation failed · round 1 of 3": a working card whose last round failed and went back to the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationRound {
    pub round: u32,
    pub of: u32,
}

impl fmt::Display for ValidationRound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Validation failed · round {} of {}", self.round, self.of)
    }
}

stored_enum! {
    /// Who started a validation round (`validations.trigger`).
    pub enum ValidationTrigger ("validation trigger") {
        /// The agent's `review submit`.
        Agent = "agent",
        /// The user's Validate and submit.
        User = "user",
    }
}

stored_enum! {
    /// How a validation round ended (`validations.outcome`, SDD §8). Only `pass` and `fail` count as rounds.
    pub enum ValidationOutcome ("validation outcome") {
        Pass = "pass",
        Fail = "fail",
        /// The code or the description changed through the agent or the user (VAL-08).
        Cancelled = "cancelled",
        /// The validator's turn ended without a verdict (VAL-07).
        NoResult = "no_result",
        /// The validator changed files; its verdict was thrown away (VAL-02).
        Discarded = "discarded",
        /// The validator's session died with the service or the machine (REC-01).
        Interrupted = "interrupted",
        /// The user's Skip to Review.
        Skipped = "skipped",
        /// The user's Stop validation, Park or Close.
        Stopped = "stopped",
    }
}

impl ValidationOutcome {
    /// Whether the round counts toward the rounds setting.
    pub fn counts(self) -> bool {
        matches!(self, Self::Pass | Self::Fail)
    }
}

stored_enum! {
    /// Why Auto-merge stopped and turned the card orange (AMG-03, AMG-04).
    pub enum AutoMergeStop ("auto-merge stop") {
        /// Three conflicts in a row: "Keeps conflicting with `<target>`".
        KeepsConflicting = "keeps_conflicting",
        /// The base diverged from its remote (GIT-01).
        BaseDiverged = "base_diverged",
        /// The user's checkout of the target branch has uncommitted changes (GIT-03).
        CheckoutDirty = "checkout_dirty",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact texts of SDD §7.2.
    #[test]
    fn labels() {
        assert_eq!(VALIDATING, "Validating…");
        assert_eq!(
            ValidationRound { round: 1, of: 3 }.to_string(),
            "Validation failed · round 1 of 3"
        );
        assert_eq!(failed_times(3), "Validation failed 3 times");
        assert_eq!(keeps_conflicting("main"), "Keeps conflicting with `main`");
        let entries: Vec<String> = ReviewEntry::ALL.iter().map(|e| e.label(2)).collect();
        assert_eq!(
            entries,
            [
                "Validated · round 2",
                "Validation skipped by you",
                "Submitted by you",
                "Failed 2 times · moved by you"
            ]
        );
        assert!(ReviewEntry::Validated.may_auto_merge());
        assert!(
            ReviewEntry::ALL
                .iter()
                .filter(|e| e.may_auto_merge())
                .count()
                == 1
        );
    }

    #[test]
    fn only_verdicts_count_as_rounds() {
        let counted: Vec<_> = ValidationOutcome::ALL
            .iter()
            .filter(|o| o.counts())
            .collect();
        assert_eq!(
            counted,
            [&ValidationOutcome::Pass, &ValidationOutcome::Fail]
        );
    }
}
