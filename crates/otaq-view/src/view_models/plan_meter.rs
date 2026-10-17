//! The plan meter in the header (USG-05; d0020; *02 · App shell → Header states → Header · plan meter (Pro /
//! Max)*): "5h · 82% · week · 40%" from Claude Code's current windows (USG-09: 5h is its 300-minute window, week its
//! 10,080-minute one), so the limit is visible before agents stop. A value turns amber from 80 %, always next to its
//! number. Hover shows both reset times. Hidden when Claude Code has no windows (API-key users, who get Claude
//! Code's automatic retries instead); a missing window, or one whose reset time has passed, shows "—" until new data
//! arrives. Activity's windows per tool replace the meter in M21.

use otaq_core::agent_tools::{LimitWindow, Tool};
use otaq_core::usage::plan::PlanLimits;
use serde::Serialize;
use ts_rs::TS;

/// A value turns amber from here.
pub const AMBER_FROM: f64 = 80.0;

/// The 5-hour window's length, in minutes.
const FIVE_HOURS: u32 = 5 * 60;
/// The weekly window's length, in minutes.
const WEEK: u32 = 7 * 24 * 60;

/// A window with a reset time ahead.
#[derive(Clone, Copy)]
struct Current {
    used: f64,
    resets_at: i64,
}

/// One window in the meter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PlanMeterCell")]
pub struct Cell {
    /// "5h" or "week".
    pub label: &'static str,
    /// "82%", or "—" when unknown.
    pub value: String,
    pub amber: bool,
}

/// What the header draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
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

/// The meter at `now`; `None` (hidden) when Claude Code has no windows.
pub fn meter(state: Option<&PlanLimits>, now: i64) -> Option<PlanMeter> {
    let windows: Vec<&LimitWindow> = state?.windows_of(Tool::Claude).collect();
    if windows.is_empty() {
        return None;
    }
    let current = |minutes: u32| {
        let w = windows.iter().find(|w| w.length_minutes == Some(minutes))?;
        let resets_at = w.resets_at.filter(|&at| at > now)?;
        Some(Current {
            used: w.used_percent,
            resets_at,
        })
    };
    let (five, week) = (current(FIVE_HOURS), current(WEEK));
    let cell = |label, w: Option<Current>| Cell {
        label,
        value: w.map_or_else(|| "—".to_owned(), |w| percent(w.used)),
        amber: w.is_some_and(|w| w.used >= AMBER_FROM),
    };
    let line = |name: &str, w: Option<Current>| match w {
        Some(w) => format!(
            "{name}  {} · resets {}",
            percent(w.used),
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
    let t = otaq_core::local_time::local(at_ms)?;
    Some(LocalClock {
        hour: t.hour,
        minute: t.minute,
        weekday: t.weekday,
    })
}

#[cfg(test)]
mod tests {
    use otaq_core::usage::plan::ToolWindow;

    use super::*;

    const NOW: i64 = 1_791_547_200_000;

    /// The state as the store gives it: Claude Code's windows, keyed as its status line does.
    fn state(five: Option<(f64, i64)>, week: Option<(f64, i64)>) -> PlanLimits {
        let w = |key: &str, minutes: u32, x: Option<(f64, i64)>| {
            x.map(|(used_percent, resets_at)| ToolWindow {
                tool: Tool::Claude,
                window: LimitWindow {
                    key: key.into(),
                    length_minutes: Some(minutes),
                    used_percent,
                    resets_at: Some(resets_at),
                    credits: None,
                },
                observed_at: NOW,
            })
        };
        PlanLimits {
            windows: [w("5h", FIVE_HOURS, five), w("7d", WEEK, week)]
                .into_iter()
                .flatten()
                .collect(),
            limit: None,
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
        // A window the tool dropped: "—", with its hover line.
        let one = meter(Some(&state(None, Some((40.0, NOW + 1000)))), NOW).unwrap();
        assert_eq!(one.five_hour.value, "—");
        assert_eq!(one.lines[0], "5-hour session  unknown · no recent data");
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
