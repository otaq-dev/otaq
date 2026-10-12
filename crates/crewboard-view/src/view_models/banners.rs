//! The banner area's rules (P6, d0016; *11 · Banners → Stacking · usage limit + reconnecting*): one banner per
//! situation, never one per agent; at most two rows; ordered error › restart required › usage limit › permission
//! warning › service update ready › reconnecting; reconnecting folds into another banner as a suffix instead of
//! taking a row of its own.

use std::collections::{BTreeMap, BTreeSet};

/// The situations a banner can stand for. The order of the variants is the stacking order (first = top).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SituationKind {
    /// Red: the background service has ended (reboot or crash).
    ServiceStopped,
    /// Red: the service came back and its sessions were interrupted (REC-06).
    SessionsInterrupted,
    RestartRequired,
    UsageLimit,
    PermissionWarning,
    ServiceUpdateReady,
    Reconnecting,
}

/// A situation with what its banner shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Situation {
    ServiceStopped,
    /// After a reboot, a crash (`unclean`) or a service restart: `count` sessions were interrupted; `at` is when
    /// the service started again ("02:14").
    SessionsInterrupted {
        count: u32,
        unclean: bool,
        at: String,
    },
    RestartRequired {
        working: u32,
        waiting: u32,
    },
    /// The plan limit (USG-06; [`crate::view_models::limit_banner`]): its title, the line after it, and Resume all.
    UsageLimit {
        title: String,
        text: String,
        resume: Option<crate::view_models::limit_banner::ResumeAll>,
    },
    PermissionWarning,
    ServiceUpdateReady {
        working: u32,
        waiting: u32,
    },
    Reconnecting,
}

impl Situation {
    pub fn kind(&self) -> SituationKind {
        match self {
            Self::ServiceStopped => SituationKind::ServiceStopped,
            Self::SessionsInterrupted { .. } => SituationKind::SessionsInterrupted,
            Self::RestartRequired { .. } => SituationKind::RestartRequired,
            Self::UsageLimit { .. } => SituationKind::UsageLimit,
            Self::PermissionWarning => SituationKind::PermissionWarning,
            Self::ServiceUpdateReady { .. } => SituationKind::ServiceUpdateReady,
            Self::Reconnecting => SituationKind::Reconnecting,
        }
    }
}

impl SituationKind {
    /// Whether the user can close it. Reconnecting goes away by itself; stopped, restart required and the
    /// usage limit stay until the situation ends.
    pub fn dismissable(self) -> bool {
        matches!(
            self,
            Self::PermissionWarning | Self::ServiceUpdateReady | Self::SessionsInterrupted
        )
    }
}

/// "3 agents working, 2 waiting".
pub fn agent_counts(working: u32, waiting: u32) -> String {
    let agents = if working == 1 { "agent" } else { "agents" };
    format!("{working} {agents} working, {waiting} waiting")
}

/// What one banner row shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BannerRow {
    pub kind: SituationKind,
    pub title: String,
    pub text: String,
    /// The Restart now button (update ready, restart required), Start service (stopped) or Resume all.
    pub action: Option<&'static str>,
    /// A second button: "Review & resume" (opens the inbox).
    pub secondary: Option<&'static str>,
    pub dismissable: bool,
    /// The usage limit's Resume all: its label, whether it can be pressed, and whether it is highlighted.
    pub resume: Option<crate::view_models::limit_banner::ResumeAll>,
}

const RECONNECTING_SUFFIX: &str = "  ·  ↻ reconnecting to the background service…";

/// Up to this many banner rows are shown at once.
pub const MAX_ROWS: usize = 2;

#[derive(Debug, Default, Clone)]
pub struct BannerModel {
    active: BTreeMap<SituationKind, Situation>,
    dismissed: BTreeSet<SituationKind>,
}

impl BannerModel {
    /// Raises (or updates) a situation. Raising the same situation again replaces it, never adds a banner.
    pub fn raise(&mut self, situation: Situation) {
        self.active.insert(situation.kind(), situation);
    }

    /// The situation has ended; a later raise shows it again (also after a dismissal).
    pub fn clear(&mut self, kind: SituationKind) {
        self.active.remove(&kind);
        self.dismissed.remove(&kind);
    }

    /// The user closed the banner. Ignored for banners that can't be closed.
    pub fn dismiss(&mut self, kind: SituationKind) {
        if kind.dismissable() {
            self.dismissed.insert(kind);
        }
    }

    pub fn is_active(&self, kind: SituationKind) -> bool {
        self.active.contains_key(&kind)
    }

    /// With "Restart required", only Restart works: everything else that talks to the service is disabled.
    pub fn only_restart_allowed(&self) -> bool {
        self.is_active(SituationKind::RestartRequired)
    }

    /// The rows to draw, top first.
    pub fn rows(&self) -> Vec<BannerRow> {
        let shown: Vec<&Situation> = self
            .active
            .values()
            .filter(|s| !self.dismissed.contains(&s.kind()))
            .collect();
        let reconnecting = shown
            .iter()
            .any(|s| s.kind() == SituationKind::Reconnecting);
        let others: Vec<&Situation> = shown
            .iter()
            .copied()
            .filter(|s| s.kind() != SituationKind::Reconnecting)
            .collect();
        if others.is_empty() {
            return if reconnecting {
                vec![row(&Situation::Reconnecting)]
            } else {
                Vec::new()
            };
        }
        let mut rows: Vec<BannerRow> = others.into_iter().take(MAX_ROWS).map(row).collect();
        if reconnecting {
            rows[0].text.push_str(RECONNECTING_SUFFIX);
        }
        rows
    }
}

fn row(situation: &Situation) -> BannerRow {
    let kind = situation.kind();
    let mut secondary = None;
    let mut resume = None;
    let (title, text, action) = match situation {
        Situation::ServiceStopped => (
            "Background service stopped".to_owned(),
            "Its agent sessions ended with it. Nothing restarts on its own.".to_owned(),
            Some("Start service"),
        ),
        // *16 → Opening after a reboot*; *11 → After the service restart*.
        Situation::SessionsInterrupted { count, unclean, at } => {
            let sessions = if *count == 1 { "session was" } else { "sessions were" };
            if *unclean {
                secondary = Some("Review & resume");
                (
                    format!("{count} {sessions} interrupted"),
                    format!(
                        "The background service started again at {at} after a reboot or crash. Nothing restarts on its own."
                    ),
                    Some("Resume all"),
                )
            } else {
                secondary = Some("Review & resume");
                (
                    "Service restarted".to_owned(),
                    format!(
                        "{count} {sessions} interrupted by the restart · waiting tickets keep their questions"
                    ),
                    Some("Resume all"),
                )
            }
        }
        Situation::RestartRequired { working, waiting } => (
            "Restart required".to_owned(),
            format!(
                "This app can’t talk to the running service (older version) · {}",
                agent_counts(*working, *waiting)
            ),
            Some("Restart now"),
        ),
        Situation::UsageLimit {
            title,
            text,
            resume: button,
        } => {
            resume = button.clone();
            (title.clone(), text.clone(), None)
        }
        Situation::PermissionWarning => (
            "Permission prompts are on".to_owned(),
            "With 5–10 agents, permission prompts will stop agents often. See allowlists · auto mode in the Claude Code docs".to_owned(),
            Some("Read the docs"),
        ),
        Situation::ServiceUpdateReady { working, waiting } => (
            "Service update ready".to_owned(),
            format!(
                "Restart now ({}) · or later, when it suits you",
                agent_counts(*working, *waiting)
            ),
            Some("Restart now"),
        ),
        Situation::Reconnecting => (
            "Reconnecting…".to_owned(),
            "Lost contact with the background service. Agents keep running; the board will catch up."
                .to_owned(),
            None,
        ),
    };
    BannerRow {
        kind,
        title,
        text,
        action,
        secondary,
        dismissable: kind.dismissable(),
        resume,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(model: &BannerModel) -> Vec<SituationKind> {
        model.rows().iter().map(|r| r.kind).collect()
    }

    #[test]
    fn the_same_situation_twice_is_one_banner() {
        let mut m = BannerModel::default();
        m.raise(Situation::ServiceUpdateReady {
            working: 1,
            waiting: 0,
        });
        m.raise(Situation::ServiceUpdateReady {
            working: 3,
            waiting: 2,
        });
        let rows = m.rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].text,
            "Restart now (3 agents working, 2 waiting) · or later, when it suits you"
        );
    }

    #[test]
    fn stacking_order_and_at_most_two_rows() {
        let mut m = BannerModel::default();
        m.raise(Situation::ServiceUpdateReady {
            working: 0,
            waiting: 0,
        });
        m.raise(Situation::PermissionWarning);
        m.raise(Situation::UsageLimit {
            title: "Session limit reached".into(),
            text: "Resets at 3:00 pm".into(),
            resume: None,
        });
        assert_eq!(
            kinds(&m),
            [SituationKind::UsageLimit, SituationKind::PermissionWarning]
        );
        m.raise(Situation::RestartRequired {
            working: 0,
            waiting: 0,
        });
        m.raise(Situation::ServiceStopped);
        assert_eq!(
            kinds(&m),
            [
                SituationKind::ServiceStopped,
                SituationKind::RestartRequired
            ]
        );
    }

    #[test]
    fn reconnecting_alone_has_its_own_row() {
        let mut m = BannerModel::default();
        m.raise(Situation::Reconnecting);
        let rows = m.rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Reconnecting…");
        assert!(!rows[0].dismissable);
        m.clear(SituationKind::Reconnecting);
        assert!(m.rows().is_empty());
    }

    #[test]
    fn reconnecting_folds_into_another_banner() {
        let mut m = BannerModel::default();
        m.raise(Situation::UsageLimit {
            title: "Session limit reached".into(),
            text: "Resets at 3:00 pm · 8 agents stopped".into(),
            resume: None,
        });
        m.raise(Situation::Reconnecting);
        let rows = m.rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].text,
            "Resets at 3:00 pm · 8 agents stopped  ·  ↻ reconnecting to the background service…"
        );
    }

    #[test]
    fn restart_required_disables_other_actions() {
        let mut m = BannerModel::default();
        assert!(!m.only_restart_allowed());
        m.raise(Situation::RestartRequired {
            working: 3,
            waiting: 2,
        });
        assert!(m.only_restart_allowed());
        let rows = m.rows();
        assert_eq!(rows[0].action, Some("Restart now"));
        assert!(
            !rows[0].dismissable,
            "only Restart works; it can't be closed"
        );
        m.dismiss(SituationKind::RestartRequired);
        assert_eq!(m.rows().len(), 1, "dismiss is ignored");
    }

    #[test]
    fn dismissed_banners_stay_away_until_raised_after_clearing() {
        let mut m = BannerModel::default();
        m.raise(Situation::ServiceUpdateReady {
            working: 0,
            waiting: 0,
        });
        m.dismiss(SituationKind::ServiceUpdateReady);
        assert!(m.rows().is_empty());
        m.raise(Situation::ServiceUpdateReady {
            working: 1,
            waiting: 0,
        });
        assert!(m.rows().is_empty(), "same situation, still dismissed");
        m.clear(SituationKind::ServiceUpdateReady);
        m.raise(Situation::ServiceUpdateReady {
            working: 1,
            waiting: 0,
        });
        assert_eq!(m.rows().len(), 1);
    }

    #[test]
    fn interrupted_sessions_after_a_reboot_or_a_restart() {
        let mut m = BannerModel::default();
        m.raise(Situation::SessionsInterrupted {
            count: 4,
            unclean: true,
            at: "02:14".into(),
        });
        let row = &m.rows()[0];
        assert_eq!(row.title, "4 sessions were interrupted");
        assert_eq!(
            row.text,
            "The background service started again at 02:14 after a reboot or crash. Nothing restarts on its own."
        );
        assert_eq!(
            (row.action, row.secondary, row.dismissable),
            (Some("Resume all"), Some("Review & resume"), true)
        );
        m.raise(Situation::SessionsInterrupted {
            count: 1,
            unclean: false,
            at: "09:00".into(),
        });
        let row = &m.rows()[0];
        assert_eq!(row.title, "Service restarted");
        assert_eq!(
            row.text,
            "1 session was interrupted by the restart · waiting tickets keep their questions"
        );
        m.dismiss(SituationKind::SessionsInterrupted);
        assert!(m.rows().is_empty());
    }

    #[test]
    fn counts_read_naturally() {
        assert_eq!(agent_counts(1, 0), "1 agent working, 0 waiting");
        assert_eq!(agent_counts(3, 2), "3 agents working, 2 waiting");
    }
}
