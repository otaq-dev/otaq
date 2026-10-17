//! Kinds, columns and statuses (SDD §7.1, §7.2; d0004, d0039).
//!
//! A ticket is always in one [`Column`] and has one [`Status`]; the status determines the column. Priorities are
//! their own type ([`super::Priority`], d0065). There are no stacking states such as Paused or "needs restack"
//! (d0049).

use super::stored_enum;

stored_enum! {
    /// The two ticket types (TKT-02). An epic is a container and a delivery unit (d0039, d0059).
    #[derive(Default)]
    pub enum Kind ("ticket kind") {
        #[default]
        Ticket = "ticket",
        Epic = "epic",
    }
}

stored_enum! {
    /// The six board columns, left to right (d0004, d0034, d0036, d0037, d0066).
    pub enum Column ("column") {
        Backlog = "backlog",
        /// The Queued state shown as its own column: tickets the user let go, waiting to start (d0066).
        ReadyForDev = "ready_for_dev",
        /// Tickets an agent is on: working, validating, turquoise, background, Interrupted…
        Dev = "dev",
        ReadyForReview = "ready_for_review",
        Approved = "approved",
        Done = "done",
    }
}

impl Column {
    /// The column's name as shown on the board.
    pub fn label(self) -> &'static str {
        match self {
            Self::Backlog => "Backlog",
            Self::ReadyForDev => "Ready for Dev",
            Self::Dev => "Dev",
            Self::ReadyForReview => "Ready for Review",
            Self::Approved => "Approved",
            Self::Done => "Done",
        }
    }
}

stored_enum! {
    /// Every row of SDD §7.2: where a ticket stands. Flags (pinned, sent back, …) are separate ([`super::Flags`]).
    pub enum Status ("status") {
        // Backlog
        /// Not started.
        Backlog = "backlog",
        /// Parked · has work: taken out of Dev; branch, worktree and the agent's conversation kept (d0052).
        Parked = "parked",
        /// An epic whose children are working (d0059).
        EpicInProgress = "epic_in_progress",

        // Ready for Dev (Queued)
        /// Queued, waiting to start: for its dependencies to merge, or for the scheduler (d0049).
        Queued = "queued",
        /// Queued while the optional agent limit is full (d0016).
        WaitingForSlot = "waiting_for_slot",
        /// Held: the plan limit is active, or the base has diverged (d0020, d0059).
        Held = "held",
        /// A parent was closed without merging (d0033, d0044).
        ParentClosed = "parent_closed",
        /// The start sequence stopped halfway; Retry start (d0018).
        StartInterrupted = "start_interrupted",

        // Dev · Agent Working
        /// The agent is working.
        Working = "working",
        /// The turn ended but background tasks or wakeups are pending (neutral, d0042).
        WorkingInBackground = "working_in_background",
        /// Needs-you family: the agent asked the user a question (d0042).
        AsksYou = "asks_you",
        /// Needs-you family: the agent waits on a permission prompt (d0042).
        NeedsYou = "needs_you",
        /// Needs-you family: the agent stopped and waits for input (d0042).
        Idle = "idle",
        /// Stopped by the plan limit (neutral, no inbox item, d0046).
        WaitingForLimitReset = "waiting_for_limit_reset",
        /// The session died; Resume (d0018, d0045).
        Interrupted = "interrupted",
        /// Resuming failed; Retry or Start from scratch (d0018).
        ResumeFailed = "resume_failed",
        /// "Validating…": submitted; a separate validator checks the work (neutral, one slot, d0063).
        Validating = "validating",
        /// Turquoise: the validator waits on a permission prompt (d0063).
        ValidatorNeedsYou = "validator_needs_you",
        /// Turquoise: the last validation round failed; Move to Review anyway / another round / answer (d0063).
        ValidationFailed = "validation_failed",
        /// Turquoise: the validator ended without a verdict, or its verdict was discarded (d0063).
        ValidationNoResult = "validation_no_result",
        /// Auto-merge's rebase conflicted; the agent is fixing it with `restack` (working, d0064).
        MergeConflictFixing = "merge_conflict_fixing",
        /// Turquoise: Auto-merge stopped (keeps conflicting, base diverged, checkout dirty; d0064).
        AutoMergeStopped = "auto_merge_stopped",

        // Ready for Review
        /// Submitted by the agent or the user (d0004, d0052).
        ReadyForReview = "ready_for_review",

        // Approved
        /// A PR is open; waits for Mark merged / Mark closed (d0015, d0017).
        PrOpen = "pr_open",

        // Done
        /// Merged locally or marked merged.
        Merged = "merged",
        /// Closed without merging; reopenable (d0033).
        ClosedNotMerged = "closed_not_merged",
    }
}

impl Status {
    /// The needs-you family: the agent waits for the user (d0042), and from M12 the validation and Auto-merge stops
    /// (d0063, d0064).
    pub const NEEDS_YOU_FAMILY: &'static [Status] = &[
        Self::AsksYou,
        Self::NeedsYou,
        Self::Idle,
        Self::ValidatorNeedsYou,
        Self::ValidationFailed,
        Self::ValidationNoResult,
        Self::AutoMergeStopped,
    ];

    /// The column this status belongs to.
    pub fn column(self) -> Column {
        match self {
            Self::Backlog | Self::Parked | Self::EpicInProgress => Column::Backlog,
            Self::Queued
            | Self::WaitingForSlot
            | Self::Held
            | Self::ParentClosed
            | Self::StartInterrupted => Column::ReadyForDev,
            Self::Working
            | Self::WorkingInBackground
            | Self::AsksYou
            | Self::NeedsYou
            | Self::Idle
            | Self::WaitingForLimitReset
            | Self::Interrupted
            | Self::ResumeFailed
            | Self::Validating
            | Self::ValidatorNeedsYou
            | Self::ValidationFailed
            | Self::ValidationNoResult
            | Self::MergeConflictFixing
            | Self::AutoMergeStopped => Column::Dev,
            Self::ReadyForReview => Column::ReadyForReview,
            Self::PrOpen => Column::Approved,
            Self::Merged | Self::ClosedNotMerged => Column::Done,
        }
    }

    /// Whether this is one of the Queued sub-states: the Ready for Dev column (d0066).
    pub fn is_queued(self) -> bool {
        matches!(
            self,
            Self::Queued
                | Self::WaitingForSlot
                | Self::Held
                | Self::ParentClosed
                | Self::StartInterrupted
        )
    }

    pub fn is_needs_you_family(self) -> bool {
        Self::NEEDS_YOU_FAMILY.contains(&self)
    }

    /// The agent's own wait (Asks you, Needs you, Idle): its input, a command or an answer clears it (d0042).
    /// The validation and Auto-merge stops are turquoise too, but only their own actions clear them.
    pub const AGENT_WAITS: &'static [Status] = &[Self::AsksYou, Self::NeedsYou, Self::Idle];

    pub fn is_agent_waiting(self) -> bool {
        Self::AGENT_WAITS.contains(&self)
    }

    /// The label shown on the card's status pill.
    pub fn label(self) -> &'static str {
        match self {
            Self::Backlog => "Backlog",
            Self::Parked => "Parked · has work",
            Self::EpicInProgress => "In progress",
            Self::Queued => "Queued",
            Self::WaitingForSlot => "Waiting for a slot",
            Self::Held => "Held",
            Self::ParentClosed => "Parent closed",
            Self::StartInterrupted => "Start interrupted",
            Self::Working => "Working",
            Self::WorkingInBackground => "Working in background",
            Self::AsksYou => "Asks you",
            Self::NeedsYou => "Needs you",
            Self::Idle => "Idle",
            Self::WaitingForLimitReset => "Waiting for limit reset",
            Self::Interrupted => "Interrupted",
            Self::ResumeFailed => "Resume failed",
            Self::Validating => super::validation::VALIDATING,
            Self::ValidatorNeedsYou => super::validation::VALIDATOR_NEEDS_YOU,
            // The card adds the count: "Validation failed 3 times" (validation::failed_times).
            Self::ValidationFailed => "Validation failed",
            Self::ValidationNoResult => super::validation::NO_RESULT,
            Self::MergeConflictFixing => super::validation::MERGE_CONFLICT_FIXING,
            // The card names the reason ("Keeps conflicting with `main`", base diverged, checkout dirty).
            Self::AutoMergeStopped => "Auto-merge stopped",
            Self::ReadyForReview => "Ready for Review",
            Self::PrOpen => "PR open",
            Self::Merged => "Merged",
            Self::ClosedNotMerged => "Closed · not merged",
        }
    }
}

stored_enum! {
    /// The steps of the start sequence, in order (SES-01). A start that stops names the step it stopped at.
    pub enum StartStep ("start step") {
        /// Resolve the base branch (the epic's branch for an epic child).
        Base = "base",
        /// Fetch and fast-forward the base; hold if it diverged (d0059).
        SyncBase = "sync_base",
        /// Branch, worktree and base_sha.
        Worktree = "worktree",
        EnvFiles = "env_files",
        /// The project's Create command makes the ticket's own resources (M19-d, d0087).
        Resources = "resources",
        Setup = "setup",
        Ports = "ports",
        /// Start the agent session.
        Launch = "launch",
        /// Not a step: the plan's usage limit holds every start (SCH-04; d0020). Set on Held tickets by the
        /// scheduler, which also lifts it.
        PlanLimit = "plan_limit",
    }
}

impl StartStep {
    /// How the step reads on a card: "setup command failed".
    pub fn failure_label(self) -> &'static str {
        match self {
            Self::Base => "base branch not found",
            Self::SyncBase => "updating the base failed",
            Self::Worktree => "creating the worktree failed",
            Self::EnvFiles => "copying env files failed",
            Self::Resources => "creating ticket resources failed",
            Self::Setup => "setup command failed",
            Self::Ports => "no free ports",
            Self::Launch => "starting the agent failed",
            Self::PlanLimit => "usage limit reached",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(all: &[T])
    where
        T: Copy + PartialEq + std::fmt::Debug + std::str::FromStr + serde::Serialize,
        T: serde::de::DeserializeOwned,
        T: std::fmt::Display,
        <T as std::str::FromStr>::Err: std::fmt::Debug,
    {
        let mut seen = std::collections::HashSet::new();
        for value in all {
            let stored = value.to_string();
            assert!(seen.insert(stored.clone()), "{stored} used twice");
            assert_eq!(stored.parse::<T>().unwrap(), *value);
            let json = serde_json::to_string(value).unwrap();
            assert_eq!(json, format!("\"{stored}\""));
            assert_eq!(serde_json::from_str::<T>(&json).unwrap(), *value);
        }
    }

    #[test]
    fn every_enum_round_trips_through_its_stored_form() {
        round_trip(Kind::ALL);
        round_trip(Column::ALL);
        round_trip(Status::ALL);
        assert!("paused".parse::<Status>().is_err());
        assert!(serde_json::from_str::<Column>("\"investigate\"").is_err());
    }

    #[test]
    fn six_columns_in_board_order() {
        let labels: Vec<_> = Column::ALL.iter().map(|c| c.label()).collect();
        assert_eq!(
            labels,
            [
                "Backlog",
                "Ready for Dev",
                "Dev",
                "Ready for Review",
                "Approved",
                "Done"
            ]
        );
    }

    #[test]
    fn two_kinds() {
        assert_eq!(Kind::ALL, &[Kind::Ticket, Kind::Epic]);
    }

    #[test]
    fn every_status_has_a_column_and_every_column_a_status() {
        for column in Column::ALL {
            assert!(
                Status::ALL.iter().any(|s| s.column() == *column),
                "{column}"
            );
        }
        let dev: Vec<_> = Status::ALL
            .iter()
            .filter(|s| s.column() == Column::Dev)
            .collect();
        assert_eq!(dev.len(), 14);
        for status in Status::ALL {
            assert_eq!(
                status.is_queued(),
                status.column() == Column::ReadyForDev,
                "{status}: every queued status sits in Ready for Dev"
            );
        }
    }

    #[test]
    fn needs_you_family_names_match_the_stored_states() {
        let names: Vec<_> = Status::NEEDS_YOU_FAMILY
            .iter()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "asks_you",
                "needs_you",
                "idle",
                "validator_needs_you",
                "validation_failed",
                "validation_no_result",
                "auto_merge_stopped"
            ]
        );
        for status in [Status::Validating, Status::MergeConflictFixing] {
            assert!(!status.is_needs_you_family(), "{status} is neutral");
        }
    }

    #[test]
    fn no_stacking_states() {
        for status in Status::ALL {
            let stored = status.as_str();
            assert!(
                !stored.contains("pause") && !stored.contains("restack"),
                "{stored} is a stacking state (d0049)"
            );
        }
    }
}
