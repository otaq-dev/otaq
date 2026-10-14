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
/// The validator waits on a permission prompt (turquoise).
pub const VALIDATOR_NEEDS_YOU: &str = "Needs you · validator";
/// The validator ended its turn without a verdict (turquoise).
pub const NO_RESULT: &str = "Validation ended without a result";
/// The validator changed tracked files or committed: its verdict is thrown away (VAL-02).
pub const DISCARDED: &str = "Validation discarded: the validator changed files";
/// Auto-merge's free rebase conflicted; the agent is rebasing with `restack` (AMG-03).
pub const MERGE_CONFLICT_FIXING: &str = "Merge conflict · fixing";
/// Done through Auto-merge (AMG-02).
pub const MERGED_AUTOMATICALLY: &str = "Merged automatically";

/// "Validation failed 3 times": the last round failed (VAL-06).
pub fn failed_times(times: u32) -> String {
    format!("Validation failed {}", times_word(times))
}

/// "1 time", "3 times".
fn times_word(times: u32) -> String {
    if times == 1 {
        "1 time".to_owned()
    } else {
        format!("{times} times")
    }
}

/// What changed under a validation and cancelled it (VAL-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelledBy {
    /// The agent changed tracked files or committed.
    Code,
    /// The user edited the description.
    Description,
}

/// The cancel toast (frame *11 → Success toasts*): "Validation cancelled: SHOP-29’s code changed" · "Round not
/// counted · the agent was told to submit again".
pub fn cancelled_toast(key: &str, by: CancelledBy) -> (String, String) {
    let what = match by {
        CancelledBy::Code => "code",
        CancelledBy::Description => "description",
    };
    (
        format!("Validation cancelled: {key}’s {what} changed"),
        "Round not counted · the agent was told to submit again".to_owned(),
    )
}

/// The Auto-merge toast (frame *11 → Success toasts*): "SHOP-40.1 merged automatically into the Wishlist branch" ·
/// "Validated · round 1 · local, not pushed · SHOP-40.2 can start". `into` is "the Wishlist branch" for an epic
/// child, else the base branch; `started` are the waiting tickets that started after the merge.
pub fn merged_toast(key: &str, into: &str, round: u32, started: &[String]) -> (String, String) {
    let mut message = format!("Validated · round {round} · local, not pushed");
    match started {
        [] => {}
        [one] => message.push_str(&format!(" · {one} can start")),
        [rest @ .., last] => {
            message.push_str(&format!(" · {} and {last} can start", rest.join(", ")))
        }
    }
    (format!("{key} merged automatically into {into}"), message)
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
            Self::FailedMoved => format!("Failed {} · moved by you", times_word(round)),
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
    /// Why Auto-merge stopped and turned the card turquoise (AMG-03, AMG-04).
    pub enum AutoMergeStop ("auto-merge stop") {
        /// Three conflicts in a row: "Keeps conflicting with `<target>`".
        KeepsConflicting = "keeps_conflicting",
        /// The base diverged from its remote (GIT-01).
        BaseDiverged = "base_diverged",
        /// The user's checkout of the target branch has uncommitted changes (GIT-03).
        CheckoutDirty = "checkout_dirty",
    }
}

stored_enum! {
    /// The user's actions on a validation (VAL-06, VAL-07, VAL-09): the panel's buttons and "…" menu items.
    pub enum ValidationAction ("validation action") {
        /// The agent's flow, started by the user (agent not mid-turn): rounds and Auto-merge apply.
        ValidateAndSubmit = "validate_and_submit",
        /// While validating: the validator stops, the round doesn't count, back to Agent Working.
        Stop = "stop",
        /// While validating: Ready for Review, "Validation skipped by you", never auto-merged.
        Skip = "skip",
        /// After the last failed round, or no result: Ready for Review without a pass, never auto-merged.
        MoveToReviewAnyway = "move_to_review_anyway",
        /// After the last failed round: the findings go to the agent, one more round is allowed.
        AnotherRound = "another_round",
        /// After the last failed round: the user's note goes with the findings, the counter restarts at 0.
        AnswerAgent = "answer_agent",
        /// After no result: a fresh validator, not a round.
        Retry = "retry",
        /// After an Auto-merge stop: run the merge again (d0064).
        RetryMerge = "retry_merge",
        /// After an Auto-merge stop: Ready for Review, still "Validated · round N", not merged again.
        MoveToReview = "move_to_review",
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
        assert_eq!(failed_times(1), "Validation failed 1 time");
        assert_eq!(
            ReviewEntry::FailedMoved.label(1),
            "Failed 1 time · moved by you"
        );
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

    /// The toasts of frame *11 → Success toasts* (T-0196).
    #[test]
    fn toasts() {
        assert_eq!(
            merged_toast("SHOP-40.1", "the Wishlist branch", 1, &["SHOP-40.2".into()]),
            (
                "SHOP-40.1 merged automatically into the Wishlist branch".to_owned(),
                "Validated · round 1 · local, not pushed · SHOP-40.2 can start".to_owned()
            )
        );
        assert_eq!(
            merged_toast("SHOP-18", "main", 2, &[]).1,
            "Validated · round 2 · local, not pushed"
        );
        assert_eq!(
            merged_toast(
                "SHOP-1",
                "main",
                1,
                &["SHOP-2".into(), "SHOP-3".into(), "SHOP-4".into()]
            )
            .1,
            "Validated · round 1 · local, not pushed · SHOP-2, SHOP-3 and SHOP-4 can start"
        );
        assert_eq!(
            cancelled_toast("SHOP-29", CancelledBy::Code),
            (
                "Validation cancelled: SHOP-29’s code changed".to_owned(),
                "Round not counted · the agent was told to submit again".to_owned()
            )
        );
        assert_eq!(
            cancelled_toast("SHOP-29", CancelledBy::Description).0,
            "Validation cancelled: SHOP-29’s description changed"
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
