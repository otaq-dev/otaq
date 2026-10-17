//! How a build moves between the stock and a worktree (M19-a/b, d0085): cloned where the data folder's disk can
//! clone, renamed where it can't. Never a copy or a hardlink.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::warm::clone::{CloneError, CloneSupport, move_tree_or_back};

/// The handoff of a stock entry to a worktree on this data folder's disk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffMode {
    /// Copy-on-write clone: the entry stays in stock; one forward-only entry per repo.
    #[default]
    Clone,
    /// Rename: the entry leaves the stock; several entries, up to the stock limit.
    Move,
}

impl HandoffMode {
    /// `Clone` where the disk can clone, otherwise `Move`.
    pub fn of(support: &CloneSupport) -> Self {
        if support.available {
            Self::Clone
        } else {
            Self::Move
        }
    }
}

/// What renames a build between the stock and a worktree, so the tests can stand in for it (fail it, record it).
pub trait Mover: fmt::Debug + Send + Sync {
    /// Renames `only` (paths relative to `from`), or all of `from`, into the same places under `to`. On any failure
    /// every path already moved is moved back, so `from` is as it was.
    fn move_tree(&self, from: &Path, to: &Path, only: Option<&[PathBuf]>)
    -> Result<(), CloneError>;
}

/// The file system's renames: [`move_tree_or_back`].
#[derive(Debug, Clone, Copy, Default)]
pub struct FsMover;

impl Mover for FsMover {
    fn move_tree(
        &self,
        from: &Path,
        to: &Path,
        only: Option<&[PathBuf]>,
    ) -> Result<(), CloneError> {
        move_tree_or_back(from, to, only)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn support(available: bool) -> CloneSupport {
        CloneSupport {
            available,
            file_system: String::new(),
            volume: String::new(),
        }
    }

    #[test]
    fn the_disk_picks_the_mode() {
        assert_eq!(HandoffMode::of(&support(true)), HandoffMode::Clone);
        assert_eq!(HandoffMode::of(&support(false)), HandoffMode::Move);
        assert_eq!(
            serde_json::to_string(&HandoffMode::Move).unwrap(),
            "\"move\""
        );
        assert_eq!(HandoffMode::default(), HandoffMode::Clone);
    }
}
