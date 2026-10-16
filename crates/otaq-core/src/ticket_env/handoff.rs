//! How a build moves between the stock and a worktree (M19-a/b, d0085): cloned where the data folder's disk can
//! clone, renamed where it can't. Never a copy or a hardlink.

use serde::{Deserialize, Serialize};

use crate::warm::clone::CloneSupport;

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
