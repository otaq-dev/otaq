//! The plan meter in the header (USG-05; d0020; *02 · App shell → Header states → Header · plan meter (Pro /
//! Max)*): "5h · 82% · week · 40%" from the newest status-line data, so the limit is visible before agents stop. A
//! value turns amber from 80 %, always next to its number. Hover shows both reset times. Hidden when no data was
//! ever recorded (API-key users, who get Claude Code's automatic retries instead); a window whose reset time has
//! passed shows "—" until new data arrives.

use crewboard_core::usage::plan::{PlanLimits, Window};

/// A value turns amber from here.
pub const AMBER_FROM: f64 = 80.0;

/// One window in the meter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// "5h" or "week".
    pub label: &'static str,
    /// "82%", or "—" when unknown.
    pub value: String,
    pub amber: bool,
}

/// What the header draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanMeter {
    pub five_hour: Cell,
    pub week: Cell,
    /// "Plan usage".
    pub title: String,
    /// "5-hour session  82% · resets 3:45 pm (in 1 h 20 min)", "Week  40% · resets Mon 12:00 am".
    pub lines: Vec<String>,
    /// "New tickets hold at 100% · nothing resumes by itself".
    pub note: String,
}

/// The meter at `now`; `None` (hidden) when no window was ever recorded.
pub fn meter(state: Option<&PlanLimits>, now: i64) -> Option<PlanMeter> {
    let windows = state?.windows;
    if windows.five_hour.is_none() && windows.seven_day.is_none() {
        return None;
    }
    let current = |w: Option<Window>| w.filter(|w| w.resets_at > now);
    let (five, week) = (current(windows.five_hour), current(windows.seven_day));
    let cell = |label, w: Option<Window>| Cell {
        label,
        value: w.map_or_else(|| "—".to_owned(), |w| percent(w.used_percentage)),
        amber: w.is_some_and(|w| w.used_percentage >= AMBER_FROM),
    };
    let line = |name: &str, w: Option<Window>| match w {
        Some(w) => format!(
            "{name}  {} · resets {}",
            percent(w.used_percentage),
            reset_label(w.resets_at, now)
        ),
        None => format!("{name}  unknown · no recent data"),
    };
    Some(PlanMeter {
        five_hour: cell("5h", five),
        week: cell("week", week),
        title: "Plan usage".to_owned(),
        lines: vec![line("5-hour session", five), line("Week", week)],
        note: "New tickets hold at 100% · nothing resumes by itself".to_owned(),
    })
}

/// "82%".
pub fn percent(used: f64) -> String {
    format!("{}%", used.clamp(0., 999.).round() as u32)
}

/// "3:45 pm (in 1 h 20 min)" within a day, else "Mon 12:00 am".
pub fn reset_label(at: i64, now: i64) -> String {
    let Some(t) = local(at) else {
        return "at an unknown time".to_owned();
    };
    if at - now < 24 * 3_600_000 {
        format!(
            "{} (in {})",
            clock_12h(t.hour, t.minute),
            in_label(at - now)
        )
    } else {
        format!(
            "{} {}",
            WEEKDAYS[t.weekday as usize],
            clock_12h(t.hour, t.minute)
        )
    }
}

const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// "3:45 pm", "12:00 am".
pub fn clock_12h(hour: u32, minute: u32) -> String {
    let (h, suffix) = match hour {
        0 => (12, "am"),
        1..=11 => (hour, "am"),
        12 => (12, "pm"),
        _ => (hour - 12, "pm"),
    };
    format!("{h}:{minute:02} {suffix}")
}

/// "42 min", "1 h 20 min", "3 h".
pub fn in_label(ms: i64) -> String {
    let minutes = (ms.max(0) + 59_999) / 60_000;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// Local time fields of a moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalClock {
    pub hour: u32,
    pub minute: u32,
    /// 0 = Sunday.
    pub weekday: u32,
}

pub fn local(at_ms: i64) -> Option<LocalClock> {
    let t = crewboard_core::local_time::local(at_ms)?;
    Some(LocalClock {
        hour: t.hour,
        minute: t.minute,
        weekday: t.weekday,
    })
}

#[cfg(test)]
mod tests {
    use crewboard_core::usage::plan::RateLimits;

    use super::*;

    const NOW: i64 = 1_791_547_200_000;

    fn state(five: Option<(f64, i64)>, week: Option<(f64, i64)>) -> PlanLimits {
        let w = |x: Option<(f64, i64)>| {
            x.map(|(used_percentage, resets_at)| Window {
                used_percentage,
                resets_at,
            })
        };
        PlanLimits {
            windows: RateLimits {
                five_hour: w(five),
                seven_day: w(week),
            },
            limit: None,
            observed_at: NOW,
        }
    }

    #[test]
    fn both_windows_with_amber_from_80() {
        let s = state(
            Some((82.4, NOW + 80 * 60_000)),
            Some((40.0, NOW + 3 * 86_400_000)),
        );
        let m = meter(Some(&s), NOW).unwrap();
        assert_eq!(
            m.five_hour,
            Cell {
                label: "5h",
                value: "82%".into(),
                amber: true
            }
        );
        assert_eq!(
            m.week,
            Cell {
                label: "week",
                value: "40%".into(),
                amber: false
            }
        );
        assert!(m.lines[0].starts_with("5-hour session  82% · resets "));
        assert!(m.lines[0].ends_with("(in 1 h 20 min)"), "{}", m.lines[0]);
        assert!(m.lines[1].starts_with("Week  40% · resets "));
        assert!(!m.lines[1].contains("(in"), "days away: day and time");
        assert_eq!(
            m.note,
            "New tickets hold at 100% · nothing resumes by itself"
        );
        // The threshold itself is amber; just below isn't.
        let at = |p| {
            meter(Some(&state(Some((p, NOW + 1000)), None)), NOW)
                .unwrap()
                .five_hour
                .amber
        };
        assert!(at(80.0));
        assert!(!at(79.9));
    }

    #[test]
    fn hidden_without_data_and_unknown_once_stale() {
        assert_eq!(meter(None, NOW), None);
        assert_eq!(meter(Some(&state(None, None)), NOW), None);
        let stale = state(Some((100.0, NOW - 1)), Some((55.0, NOW + 86_400_000 * 2)));
        let m = meter(Some(&stale), NOW).unwrap();
        assert_eq!(m.five_hour.value, "—");
        assert!(!m.five_hour.amber);
        assert_eq!(m.week.value, "55%");
    }

    #[test]
    fn times_read_like_the_frame() {
        assert_eq!(clock_12h(15, 45), "3:45 pm");
        assert_eq!(clock_12h(0, 0), "12:00 am");
        assert_eq!(clock_12h(12, 5), "12:05 pm");
        assert_eq!(in_label(42 * 60_000), "42 min");
        assert_eq!(in_label(80 * 60_000), "1 h 20 min");
        assert_eq!(in_label(3 * 3_600_000), "3 h");
        assert_eq!(percent(82.4), "82%");
        assert_eq!(percent(99.6), "100%");
    }
}
