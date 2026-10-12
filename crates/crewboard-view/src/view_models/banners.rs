//! The banner area's rules (P6, d0016; *11 · Banners → Stacking · usage limit + reconnecting*): one banner per
//! situation, never one per agent; at most two rows; ordered error › restart required › usage limit › permission
//! warning › service update ready › reconnecting; reconnecting folds into another banner as a suffix instead of
//! taking a row of its own.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;

/// Where the permission-mode warning's Read the docs goes (allowlists and auto mode).
pub const PERMISSION_DOCS_URL: &str = "https://code.claude.com/docs/en/permission-modes";

/// The situations a banner can stand for. The order of the variants is the stacking order (first = top).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "BannerKind")]
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
    /// The service couldn't be reached or started at all: the banner says why (Q40), with Start service to try
    /// again. The same situation as [`Situation::ServiceStopped`], so never a second banner.
    ServiceCouldNotStart {
        reason: String,
    },
    /// After a reboot, a crash (`unclean`) or a service restart: `count` sessions were interrupted; `at` is when
    /// the service started again ("02:14"); `agents` Project agents ended with it (d0068, T-0244).
    SessionsInterrupted {
        count: u32,
        unclean: bool,
        at: String,
        agents: u32,
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
            Self::ServiceStopped | Self::ServiceCouldNotStart { .. } => {
                SituationKind::ServiceStopped
            }
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
    /// A second button: "Review & resume" (opens the after-restart summary sheet).
    pub secondary: Option<&'static str>,
    pub dismissable: bool,
    /// The usage limit's Resume all: its label, whether it can be pressed, and whether it is highlighted.
    pub resume: Option<crate::view_models::limit_banner::ResumeAll>,
}

/// The banner's colour (*01 · Components → Banners*): orange for the usage limit, amber for warnings, the accent for
/// calm news, red when something broke and waits, grey while reconnecting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum BannerTone {
    Attention,
    Warning,
    Info,
    Error,
    Neutral,
}

/// Which of a banner's buttons was pressed, as the page sends it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum BannerButtonId {
    /// Restart now, Start service, Resume all (after interrupted sessions) or Read the docs.
    Action,
    /// Review & resume.
    Secondary,
    /// The usage limit's Resume all.
    Resume,
}

/// One button of a banner as drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct BannerButton {
    pub id: BannerButtonId,
    pub label: String,
    pub icon: IconName,
    pub enabled: bool,
    /// Highlighted (the usage limit's Resume all after the reset).
    pub primary: bool,
}

/// One banner row as the page draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "Banner")]
pub struct BannerView {
    pub kind: SituationKind,
    pub tone: BannerTone,
    pub icon: IconName,
    pub title: String,
    pub text: String,
    pub buttons: Vec<BannerButton>,
    pub dismissable: bool,
}

impl BannerRow {
    /// The row as drawn: tone and icon by situation; while the service starts or restarts (`busy`) its button says
    /// so and nothing can be pressed twice.
    pub fn view(&self, busy: bool) -> BannerView {
        let (tone, icon) = match self.kind {
            SituationKind::ServiceStopped => (BannerTone::Error, IconName::ServerOff),
            SituationKind::SessionsInterrupted => (BannerTone::Error, IconName::ZapOff),
            SituationKind::RestartRequired => (BannerTone::Warning, IconName::ServerCog),
            SituationKind::UsageLimit => (BannerTone::Attention, IconName::Gauge),
            SituationKind::PermissionWarning => (BannerTone::Warning, IconName::ShieldAlert),
            SituationKind::ServiceUpdateReady => (BannerTone::Info, IconName::RefreshCw),
            SituationKind::Reconnecting => (BannerTone::Neutral, IconName::RefreshCw),
        };
        let mut buttons = Vec::new();
        if let Some(label) = self.action {
            let restarts = matches!(
                self.kind,
                SituationKind::ServiceStopped
                    | SituationKind::RestartRequired
                    | SituationKind::ServiceUpdateReady
            );
            let (label, icon) = match (self.kind, busy && restarts) {
                (SituationKind::ServiceStopped, true) => ("Starting…", IconName::LoaderCircle),
                (_, true) => ("Restarting…", IconName::LoaderCircle),
                (SituationKind::ServiceStopped | SituationKind::SessionsInterrupted, false) => {
                    (label, IconName::Play)
                }
                (SituationKind::PermissionWarning, false) => (label, IconName::ExternalLink),
                (_, false) => (label, IconName::RotateCw),
            };
            buttons.push(BannerButton {
                id: BannerButtonId::Action,
                label: label.to_owned(),
                icon,
                enabled: !(busy && restarts),
                primary: false,
            });
        }
        if let Some(label) = self.secondary {
            buttons.push(BannerButton {
                id: BannerButtonId::Secondary,
                label: label.to_owned(),
                icon: IconName::ArrowRight,
                enabled: true,
                primary: false,
            });
        }
        if let Some(resume) = &self.resume {
            buttons.push(BannerButton {
                id: BannerButtonId::Resume,
                label: resume.label.clone(),
                icon: IconName::Play,
                enabled: resume.enabled && !busy,
                primary: resume.primary,
            });
        }
        BannerView {
            kind: self.kind,
            tone,
            icon,
            title: self.title.clone(),
            text: self.text.clone(),
            buttons,
            dismissable: self.dismissable,
        }
    }
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
        Situation::ServiceCouldNotStart { reason } => (
            "Background service couldn’t start".to_owned(),
            reason.clone(),
            Some("Start service"),
        ),
        // *16 → Opening after a reboot*; *11 → After the service restart*.
        Situation::SessionsInterrupted {
            count,
            unclean,
            at,
            agents,
        } => {
            let sessions = if *count == 1 { "session was" } else { "sessions were" };
            // "8 sessions were interrupted · 2 Project agents ended" (*16 → Opening after a reboot*).
            let ended = match agents {
                0 => String::new(),
                1 => " \u{b7} 1 Project agent ended".to_owned(),
                n => format!(" \u{b7} {n} Project agents ended"),
            };
            if *unclean {
                secondary = Some("Review & resume");
                (
                    format!("{count} {sessions} interrupted{ended}"),
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
                        "{count} {sessions} interrupted by the restart{ended} · waiting tickets keep their questions"
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
            agents: 0,
        });
        let row = &m.rows()[0];
        assert_eq!(row.title, "4 sessions were interrupted");
        m.raise(Situation::SessionsInterrupted {
            count: 8,
            unclean: true,
            at: "02:14".into(),
            agents: 2,
        });
        assert_eq!(
            m.rows()[0].title,
            "8 sessions were interrupted \u{b7} 2 Project agents ended"
        );
        let row = &m.rows()[0];
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
            agents: 0,
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
    fn a_start_error_is_the_stopped_banner_with_its_reason() {
        let mut m = BannerModel::default();
        m.raise(Situation::ServiceStopped);
        m.raise(Situation::ServiceCouldNotStart {
            reason: "the data folder's path is too long".into(),
        });
        let rows = m.rows();
        assert_eq!(rows.len(), 1, "one situation, one banner");
        assert_eq!(rows[0].title, "Background service couldn’t start");
        assert_eq!(rows[0].text, "the data folder's path is too long");
        let view = rows[0].view(false);
        assert_eq!(
            (view.tone, view.icon),
            (BannerTone::Error, IconName::ServerOff)
        );
        assert_eq!(view.buttons[0].label, "Start service");
        assert!(!view.dismissable);
    }

    #[test]
    fn the_drawn_banner_names_its_buttons() {
        let mut m = BannerModel::default();
        m.raise(Situation::ServiceUpdateReady {
            working: 3,
            waiting: 2,
        });
        let view = m.rows()[0].view(false);
        assert_eq!(
            (view.tone, view.icon),
            (BannerTone::Info, IconName::RefreshCw)
        );
        assert_eq!(
            view.buttons,
            [BannerButton {
                id: BannerButtonId::Action,
                label: "Restart now".into(),
                icon: IconName::RotateCw,
                enabled: true,
                primary: false,
            }]
        );
        assert!(view.dismissable);
        let busy = m.rows()[0].view(true);
        assert_eq!(busy.buttons[0].label, "Restarting…");
        assert_eq!(busy.buttons[0].icon, IconName::LoaderCircle);
        assert!(!busy.buttons[0].enabled, "never pressed twice");

        let stopped = row(&Situation::ServiceStopped).view(true);
        assert_eq!(stopped.buttons[0].label, "Starting…");

        let interrupted = row(&Situation::SessionsInterrupted {
            count: 4,
            unclean: true,
            at: "02:14".into(),
            agents: 0,
        })
        .view(false);
        let labels: Vec<(&str, IconName)> = interrupted
            .buttons
            .iter()
            .map(|b| (b.label.as_str(), b.icon))
            .collect();
        assert_eq!(
            labels,
            [
                ("Resume all", IconName::Play),
                ("Review & resume", IconName::ArrowRight)
            ]
        );
        let warning = row(&Situation::PermissionWarning).view(true);
        assert_eq!(
            (
                warning.tone,
                warning.buttons[0].icon,
                warning.buttons[0].enabled
            ),
            (BannerTone::Warning, IconName::ExternalLink, true),
            "a restart doesn't hold Read the docs"
        );
    }

    #[test]
    fn the_usage_limit_draws_its_resume_all() {
        let limit = |enabled, primary| Situation::UsageLimit {
            title: "Usage limit reset".into(),
            text: "Your plan limit reset".into(),
            resume: Some(crate::view_models::limit_banner::ResumeAll {
                label: "Resume all (8)".into(),
                enabled,
                primary,
            }),
        };
        let view = row(&limit(true, true)).view(false);
        assert_eq!(view.tone, BannerTone::Attention);
        assert_eq!(
            view.buttons,
            [BannerButton {
                id: BannerButtonId::Resume,
                label: "Resume all (8)".into(),
                icon: IconName::Play,
                enabled: true,
                primary: true,
            }]
        );
        assert!(!row(&limit(false, false)).view(false).buttons[0].enabled);
        assert!(!row(&limit(true, true)).view(true).buttons[0].enabled);
        assert!(!view.dismissable);
    }

    #[test]
    fn counts_read_naturally() {
        assert_eq!(agent_counts(1, 0), "1 agent working, 0 waiting");
        assert_eq!(agent_counts(3, 2), "3 agents working, 2 waiting");
    }
}
