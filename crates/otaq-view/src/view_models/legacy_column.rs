//! The board's six state columns as the default flow draws them (d0004, d0066, d0087): the view models' own copy
//! since the core stores and matches stages instead (T-0400, STG-04). A ticket's column is computed from its place
//! (a stage by its kind on the default flow); a status's from its set. T-0404 replaces this with the columns of the
//! project's flow (STG-08).

use serde::{Deserialize, Serialize};

use otaq_core::board::moves::DropTarget;
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Place, Status, StatusSet};
use otaq_core::flow::engine::status_set;

/// The six state columns, left to right (the board draws Ready for Review and Approved as one Review column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Column {
    Backlog,
    /// Waiting before the first Agent stage (d0066).
    ReadyForDev,
    /// An Agent stage and its agent check: working, validating, turquoise, background, Interrupted…
    Dev,
    /// The human check.
    ReadyForReview,
    /// The Delivery stage: PR open.
    Approved,
    Done,
}

impl Column {
    pub const ALL: &'static [Self] = &[
        Self::Backlog,
        Self::ReadyForDev,
        Self::Dev,
        Self::ReadyForReview,
        Self::Approved,
        Self::Done,
    ];

    /// The stored form today's ids use (`ready_for_dev`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::ReadyForDev => "ready_for_dev",
            Self::Dev => "dev",
            Self::ReadyForReview => "ready_for_review",
            Self::Approved => "approved",
            Self::Done => "done",
        }
    }

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

    /// The column of a status set's cards.
    pub fn of_set(set: StatusSet) -> Self {
        match set {
            StatusSet::Backlog => Self::Backlog,
            StatusSet::Waiting => Self::ReadyForDev,
            StatusSet::AgentStage | StatusSet::AgentCheck => Self::Dev,
            StatusSet::HumanCheck => Self::ReadyForReview,
            StatusSet::Delivery => Self::Approved,
            StatusSet::Done => Self::Done,
        }
    }

    /// The core's board group of this column ([`StatusSet::BOARD`]).
    pub fn set(self) -> StatusSet {
        match self {
            Self::Backlog => StatusSet::Backlog,
            Self::ReadyForDev => StatusSet::Waiting,
            Self::Dev => StatusSet::AgentStage,
            Self::ReadyForReview => StatusSet::HumanCheck,
            Self::Approved => StatusSet::Delivery,
            Self::Done => StatusSet::Done,
        }
    }

    /// The drop target of the column.
    pub fn target(self) -> DropTarget {
        match self {
            Self::Backlog => DropTarget::Backlog,
            Self::ReadyForDev => DropTarget::ReadyForDev,
            Self::Dev => DropTarget::Dev,
            Self::ReadyForReview => DropTarget::ReadyForReview,
            Self::Approved => DropTarget::Approved,
            Self::Done => DropTarget::Done,
        }
    }

    /// The column a drop target is.
    pub fn of_target(target: DropTarget) -> Self {
        match target {
            DropTarget::Backlog => Self::Backlog,
            DropTarget::ReadyForDev => Self::ReadyForDev,
            DropTarget::Dev => Self::Dev,
            DropTarget::ReadyForReview => Self::ReadyForReview,
            DropTarget::Approved => Self::Approved,
            DropTarget::Done => Self::Done,
        }
    }
}

impl std::fmt::Display for Column {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The column a status or a ticket shows in.
pub trait ColumnOf {
    fn column(&self) -> Column;
}

impl ColumnOf for Status {
    fn column(&self) -> Column {
        Column::of_set(self.set())
    }
}

impl ColumnOf for Ticket {
    fn column(&self) -> Column {
        match &self.place {
            Place::Backlog => Column::Backlog,
            Place::Waiting(_) => Column::ReadyForDev,
            Place::Done => Column::Done,
            Place::InStage(id) => otaq_core::flow::default_file().flow.stage(id).map_or_else(
                || self.status.column(),
                |s| Column::of_set(status_set(s.kind)),
            ),
        }
    }
}
