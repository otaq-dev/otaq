//! The usage-limit banner (USG-06, USG-08; d0020, d0046; *11 · Banners → Usage limit · just hit, waiting for
//! reset, after reset, weekly, reset time unknown*) and its row at the top of the inbox (*10 → Usage limit active ·
//! no duplicates*). One banner for the situation, never one per agent (P6): it says which limit, when it resets
//! (a countdown in the last hour), how many agents it stopped and that nothing new starts. Resume all is the
//! user's click and never happens by itself: disabled until a known reset time, highlighted after the reset,
//! always available when the reset time is unknown (an enterprise plan's limit too, USG-08: no Request more, no
//! Retry). After the reset, agents Claude Code already continued by itself don't count (Q27); with none left
//! waiting the banner goes.

use crewboard_core::usage::limit::LimitKind;
use crewboard_core::usage::plan::PlanLimits;

use crate::view_models::plan_meter::{clock_12h, in_label, local};

/// The error toast when Resume all failed.
pub const RESUME_FAILED: &str = "Resume all failed";

/// The toast after Resume all: "8 agents told to continue" and what happens next.
pub fn resumed(agents: usize) -> (String, String) {
    let noun = if agents == 1 { "agent" } else { "agents" };
    (
        format!("{agents} {noun} told to continue"),
        "Each continues once it is idle and its input line is empty.".to_owned(),
    )
}

/// The Resume all button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeAll {
    /// "Resume all" or "Resume all (8)".
    pub label: String,
    pub enabled: bool,
    /// Highlighted (primary) after the reset.
    pub primary: bool,
}

/// What the banner shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitBanner {
    /// "Session limit reached", "Weekly limit reached", "Usage limit reached", "Usage limit reset".
    pub title: String,
    /// "Resets at 3:45 pm · 8 agents stopped · no new tickets will start".
    pub text: String,
    pub resume: ResumeAll,
    /// The reset has happened.
    pub after_reset: bool,
    /// The inbox's row: "Usage limit reached · 8 agents waiting for limit reset · resets 3:00 pm. Resume all appears
    /// here after the reset."
    pub inbox_row: String,
}

/// The banner at `now`, from the plan state and the number of agents still waiting for the reset (all projects).
pub fn banner(state: Option<&PlanLimits>, waiting: u32, now: i64) -> Option<LimitBanner> {
    let limit = state?.limit.as_ref()?;
    let agents = |verb: &str| {
        let noun = if waiting == 1 { "agent" } else { "agents" };
        format!("{waiting} {noun} {verb}")
    };
    let reset_passed = limit.reset_seen_at.is_some() || limit.resets_at.is_some_and(|at| at <= now);
    if reset_passed {
        if waiting == 0 {
            return None;
        }
        let at = limit
            .reset_seen_at
            .or(limit.resets_at)
            .and_then(local)
            .map(|t| format!(" at {}", clock_12h(t.hour, t.minute)))
            .unwrap_or_default();
        let are = if waiting == 1 { "is" } else { "are" };
        return Some(LimitBanner {
            title: "Usage limit reset".to_owned(),
            text: format!(
                "Your plan limit reset{at} · {waiting} {} {are} waiting · nothing resumes until you click",
                if waiting == 1 { "agent" } else { "agents" }
            ),
            resume: ResumeAll {
                label: format!("Resume all ({waiting})"),
                enabled: true,
                primary: true,
            },
            after_reset: true,
            inbox_row: format!(
                "Usage limit reset · {} · nothing resumes until you click Resume all.",
                agents("waiting for limit reset")
            ),
        });
    }
    let title = match limit.kind {
        LimitKind::Session => "Session limit reached",
        LimitKind::Weekly => "Weekly limit reached",
        LimitKind::Other => "Usage limit reached",
    };
    let Some(at) = limit.resets_at else {
        return Some(LimitBanner {
            title: "Usage limit reached".to_owned(),
            text: format!(
                "Reset time unknown · {} · no new tickets will start",
                agents("waiting for limit reset")
            ),
            resume: ResumeAll {
                label: "Resume all".to_owned(),
                enabled: true,
                primary: false,
            },
            after_reset: false,
            inbox_row: format!(
                "Usage limit reached · {} · reset time unknown. Resume all when you know it has reset.",
                agents("waiting for limit reset")
            ),
        });
    };
    let left = at - now;
    let clock = local(at)
        .map(|t| clock_12h(t.hour, t.minute))
        .unwrap_or_default();
    let (when, tail) = if left <= 3_600_000 {
        (
            format!("Resets in {} ({clock})", in_label(left)),
            "queued tickets wait",
        )
    } else if left < 24 * 3_600_000 {
        (format!("Resets at {clock}"), "no new tickets will start")
    } else {
        let day = local(at)
            .map(|t| ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][t.weekday as usize])
            .unwrap_or_default();
        (format!("Resets {day} {clock}"), "no new tickets will start")
    };
    Some(LimitBanner {
        title: title.to_owned(),
        text: format!("{when} · {} · {tail}", agents("stopped")),
        resume: ResumeAll {
            label: "Resume all".to_owned(),
            enabled: false,
            primary: false,
        },
        after_reset: false,
        inbox_row: format!(
            "Usage limit reached · {} · {}. Resume all appears here after the reset.",
            agents("waiting for limit reset"),
            when.replacen("Resets", "resets", 1)
        ),
    })
}

#[cfg(test)]
mod tests {
    use crewboard_core::usage::limit::ResetSource;
    use crewboard_core::usage::plan::{ActiveLimit, RateLimits};

    use super::*;

    const NOW: i64 = 1_791_547_200_000;

    fn limited(kind: LimitKind, resets_at: Option<i64>, reset_seen_at: Option<i64>) -> PlanLimits {
        PlanLimits {
            windows: RateLimits::default(),
            limit: Some(ActiveLimit {
                kind,
                text: String::new(),
                hit_at: NOW - 60_000,
                resets_at,
                reset_source: resets_at.map(|_| ResetSource::StatusLine),
                reset_seen_at,
            }),
            observed_at: NOW,
        }
    }

    fn clock(at: i64) -> String {
        let t = local(at).unwrap();
        clock_12h(t.hour, t.minute)
    }

    #[test]
    fn no_limit_no_banner() {
        assert_eq!(banner(None, 3, NOW), None);
        let mut free = limited(LimitKind::Session, None, None);
        free.limit = None;
        assert_eq!(banner(Some(&free), 3, NOW), None);
    }

    #[test]
    fn just_hit_says_when_and_holds_resume_all() {
        let at = NOW + 3 * 3_600_000;
        let b = banner(Some(&limited(LimitKind::Session, Some(at), None)), 8, NOW).unwrap();
        assert_eq!(b.title, "Session limit reached");
        assert_eq!(
            b.text,
            format!(
                "Resets at {} · 8 agents stopped · no new tickets will start",
                clock(at)
            )
        );
        assert_eq!(
            b.resume,
            ResumeAll {
                label: "Resume all".into(),
                enabled: false,
                primary: false
            }
        );
        assert!(
            b.inbox_row
                .starts_with("Usage limit reached · 8 agents waiting for limit reset · resets at")
        );
        assert!(
            b.inbox_row
                .ends_with("Resume all appears here after the reset.")
        );
    }

    #[test]
    fn the_last_hour_counts_down() {
        let at = NOW + 42 * 60_000;
        let b = banner(Some(&limited(LimitKind::Session, Some(at), None)), 1, NOW).unwrap();
        assert_eq!(
            b.text,
            format!(
                "Resets in 42 min ({}) · 1 agent stopped · queued tickets wait",
                clock(at)
            )
        );
        let later = banner(
            Some(&limited(LimitKind::Session, Some(at), None)),
            1,
            NOW + 41 * 60_000,
        )
        .unwrap();
        assert!(later.text.starts_with("Resets in 1 min"));
    }

    #[test]
    fn weekly_shows_the_day() {
        let at = NOW + 3 * 86_400_000;
        let b = banner(Some(&limited(LimitKind::Weekly, Some(at), None)), 8, NOW).unwrap();
        assert_eq!(b.title, "Weekly limit reached");
        assert!(b.text.starts_with("Resets "));
        assert!(
            b.text
                .ends_with(" · 8 agents stopped · no new tickets will start")
        );
        assert!(!b.text.contains("Resets at"));
        assert!(!b.text.contains("Resets in"));
    }

    #[test]
    fn unknown_reset_keeps_resume_all_available() {
        let b = banner(Some(&limited(LimitKind::Other, None, None)), 2, NOW).unwrap();
        assert_eq!(b.title, "Usage limit reached");
        assert_eq!(
            b.text,
            "Reset time unknown · 2 agents waiting for limit reset · no new tickets will start"
        );
        assert!(b.resume.enabled && !b.resume.primary);
        assert_eq!(
            b.inbox_row,
            "Usage limit reached · 2 agents waiting for limit reset · reset time unknown. Resume all when you know it has reset."
        );
    }

    #[test]
    fn after_the_reset_resume_all_is_the_call_to_action() {
        let at = NOW - 60_000;
        let b = banner(
            Some(&limited(LimitKind::Session, Some(at), Some(at))),
            8,
            NOW,
        )
        .unwrap();
        assert!(b.after_reset);
        assert_eq!(b.title, "Usage limit reset");
        assert_eq!(
            b.text,
            format!(
                "Your plan limit reset at {} · 8 agents are waiting · nothing resumes until you click",
                clock(at)
            )
        );
        assert_eq!(
            b.resume,
            ResumeAll {
                label: "Resume all (8)".into(),
                enabled: true,
                primary: true
            }
        );
        // The reset time passed but the service hasn't marked it yet: the same.
        assert!(
            banner(Some(&limited(LimitKind::Session, Some(at), None)), 8, NOW)
                .unwrap()
                .after_reset
        );
        // Claude Code continued every agent by itself (Q27): nothing left to resume, no banner.
        assert_eq!(
            banner(
                Some(&limited(LimitKind::Session, Some(at), Some(at))),
                0,
                NOW
            ),
            None
        );
    }

    #[test]
    fn resume_all_says_how_many_were_told() {
        assert_eq!(resumed(1).0, "1 agent told to continue");
        assert_eq!(resumed(8).0, "8 agents told to continue");
        assert_eq!(
            resumed(8).1,
            "Each continues once it is idle and its input line is empty."
        );
    }
}
