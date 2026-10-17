//! "Working in background" and what it waits for (ATT-04, d0042): "background: test run, 1 helper agent · 6 min",
//! or "wakes up at 4:10pm" when only scheduled wakeups are pending.
//!
//! Otaq's own type: each tool's adapter fills it from what its turn end reports (Claude Code's hook payload,
//! `agent_tools/claude/hooks.rs`). A wakeup comes as a cron schedule without its next time, so the next fire time
//! is computed here, in local time (minute, hour, day of month, month, day of week; `*`, lists, ranges and steps).

use serde::{Deserialize, Serialize};

/// One background task the turn left running.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundTask {
    /// The tool's task type (Claude Code: `shell`, a subagent type, `monitor`, …).
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// A helper agent (subagent or teammate): counted, not described.
    #[serde(default)]
    pub helper_agent: bool,
}

/// One scheduled wakeup the turn left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wakeup {
    /// A five-field cron schedule in local time.
    pub schedule: String,
    #[serde(default)]
    pub recurring: bool,
}

/// What a turn left running.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundWork {
    #[serde(default)]
    pub tasks: Vec<BackgroundTask>,
    #[serde(default)]
    pub wakeups: Vec<Wakeup>,
}

impl BackgroundWork {
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty() && self.wakeups.is_empty()
    }

    /// "background: test run, 1 helper agent" for running tasks; `None` when only wakeups are pending.
    pub fn tasks_text(&self) -> Option<String> {
        if self.tasks.is_empty() {
            return None;
        }
        let mut parts: Vec<String> = Vec::new();
        for task in self.tasks.iter().filter(|t| !t.helper_agent) {
            let what = task
                .description
                .clone()
                .unwrap_or_else(|| task.kind.replace('_', " "));
            if !parts.contains(&what) {
                parts.push(what);
            }
        }
        match self.tasks.iter().filter(|t| t.helper_agent).count() {
            0 => {}
            1 => parts.push("1 helper agent".to_owned()),
            n => parts.push(format!("{n} helper agents")),
        }
        Some(format!("background: {}", parts.join(", ")))
    }

    /// The earliest next wakeup after `now_ms`, in Unix milliseconds.
    pub fn next_wakeup(&self, now_ms: i64) -> Option<i64> {
        self.wakeups
            .iter()
            .filter_map(|w| next_fire(&w.schedule, now_ms))
            .min()
    }

    /// The card's line: "background: test run, 1 helper agent · 6 min" (since the turn ended at `since_ms`), or
    /// "wakes up at 4:10pm".
    pub fn summary(&self, since_ms: i64, now_ms: i64) -> String {
        match self.tasks_text() {
            Some(tasks) => format!("{tasks} · {}", minutes(now_ms - since_ms)),
            None => match self.next_wakeup(now_ms) {
                Some(at) => wakeup_text(at),
                None => "waiting for a scheduled wakeup".to_owned(),
            },
        }
    }
}

/// "6 min", "< 1 min", "2 h 5 min".
pub fn minutes(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    match minutes {
        0 => "< 1 min".to_owned(),
        m if m < 120 => format!("{m} min"),
        m if m % 60 == 0 => format!("{} h", m / 60),
        m => format!("{} h {} min", m / 60, m % 60),
    }
}

/// "wakes up at 4:10pm" (local time).
pub fn wakeup_text(at_ms: i64) -> String {
    match local(at_ms) {
        Some(t) => format!("wakes up at {}", clock_12h(t.hour, t.minute)),
        None => "wakes up later".to_owned(),
    }
}

/// "4:10pm", "12:05am".
pub fn clock_12h(hour: u32, minute: u32) -> String {
    let suffix = if hour < 12 { "am" } else { "pm" };
    let h = match hour % 12 {
        0 => 12,
        h => h,
    };
    format!("{h}:{minute:02}{suffix}")
}

/// Local calendar fields of a moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub minute: u32,
    pub hour: u32,
    /// 1–31.
    pub day: u32,
    /// 1–12.
    pub month: u32,
    /// 0 = Sunday.
    pub weekday: u32,
}

/// The local time of a Unix-milliseconds moment.
pub fn local(at_ms: i64) -> Option<LocalTime> {
    let t = crate::local_time::local(at_ms)?;
    Some(LocalTime {
        minute: t.minute,
        hour: t.hour,
        day: t.day,
        month: t.month,
        weekday: t.weekday,
    })
}

/// The first whole minute after `after_ms` that matches the five-field cron `schedule`, within eight days.
pub fn next_fire(schedule: &str, after_ms: i64) -> Option<i64> {
    let cron = Cron::parse(schedule)?;
    let start = (after_ms.div_euclid(60_000) + 1) * 60_000;
    (0..8 * 24 * 60)
        .map(|n| start + n * 60_000)
        .find(|at| local(*at).is_some_and(|t| cron.matches(t)))
}

/// A parsed five-field cron schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cron {
    minute: Vec<u32>,
    hour: Vec<u32>,
    day: Vec<u32>,
    month: Vec<u32>,
    weekday: Vec<u32>,
    /// Both day fields restricted: either may match (cron's rule).
    day_or_weekday: bool,
}

impl Cron {
    pub fn parse(schedule: &str) -> Option<Self> {
        let fields: Vec<&str> = schedule.split_whitespace().collect();
        let [minute, hour, day, month, weekday] = fields.as_slice() else {
            return None;
        };
        let mut weekdays = field(weekday, 0, 7)?;
        for w in &mut weekdays {
            *w %= 7;
        }
        Some(Self {
            minute: field(minute, 0, 59)?,
            hour: field(hour, 0, 23)?,
            day: field(day, 1, 31)?,
            month: field(month, 1, 12)?,
            weekday: weekdays,
            day_or_weekday: *day != "*" && *weekday != "*",
        })
    }

    pub fn matches(&self, t: LocalTime) -> bool {
        let day = self.day.contains(&t.day);
        let weekday = self.weekday.contains(&t.weekday);
        let date = if self.day_or_weekday {
            day || weekday
        } else {
            day && weekday
        };
        self.minute.contains(&t.minute)
            && self.hour.contains(&t.hour)
            && self.month.contains(&t.month)
            && date
    }
}

/// One cron field: `*`, `5`, `1-5`, `*/15`, `0-30/10`, and lists of those.
fn field(text: &str, min: u32, max: u32) -> Option<Vec<u32>> {
    let mut values = Vec::new();
    for part in text.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => (range, step.parse::<u32>().ok().filter(|s| *s > 0)?),
            None => (part, 1),
        };
        let (from, to) = match range {
            "*" => (min, max),
            r => match r.split_once('-') {
                Some((a, b)) => (a.parse().ok()?, b.parse().ok()?),
                None => {
                    let v: u32 = r.parse().ok()?;
                    (v, if part.contains('/') { max } else { v })
                }
            },
        };
        if from < min || to > max || from > to {
            return None;
        }
        values.extend((from..=to).step_by(step as usize));
    }
    values.sort_unstable();
    values.dedup();
    Some(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(kind: &str, description: Option<&str>, helper: bool) -> BackgroundTask {
        BackgroundTask {
            kind: kind.into(),
            description: description.map(Into::into),
            helper_agent: helper,
        }
    }

    #[test]
    fn the_card_sample() {
        let work = BackgroundWork {
            tasks: vec![
                task("shell", Some("test run"), false),
                task("subagent", Some("explore"), true),
            ],
            wakeups: vec![],
        };
        assert_eq!(
            work.summary(0, 6 * 60_000 + 30_000),
            "background: test run, 1 helper agent · 6 min"
        );
        let helpers = BackgroundWork {
            tasks: vec![task("subagent", None, true), task("subagent", None, true)],
            wakeups: vec![],
        };
        assert_eq!(helpers.tasks_text().unwrap(), "background: 2 helper agents");
        assert_eq!(minutes(10_000), "< 1 min");
        assert_eq!(minutes(125 * 60_000), "2 h 5 min");
    }

    #[test]
    fn a_wakeup_reads_as_a_local_time() {
        assert_eq!(clock_12h(16, 10), "4:10pm");
        assert_eq!(clock_12h(0, 5), "12:05am");
        assert_eq!(clock_12h(12, 0), "12:00pm");
        let work = BackgroundWork {
            tasks: vec![],
            wakeups: vec![Wakeup {
                schedule: "10 16 * * *".into(),
                recurring: false,
            }],
        };
        let now = crate::store::now_ms();
        let at = work.next_wakeup(now).unwrap();
        let t = local(at).unwrap();
        assert_eq!((t.hour, t.minute), (16, 10));
        assert!(at > now && at - now <= 24 * 3600 * 1000 + 3600 * 1000);
        assert_eq!(work.summary(now, now), "wakes up at 4:10pm");
    }

    #[test]
    fn cron_fields() {
        assert_eq!(field("*/15", 0, 59).unwrap(), [0, 15, 30, 45]);
        assert_eq!(field("1-5", 0, 7).unwrap(), [1, 2, 3, 4, 5]);
        assert_eq!(field("5,1,3", 0, 59).unwrap(), [1, 3, 5]);
        assert_eq!(field("10/20", 0, 59).unwrap(), [10, 30, 50]);
        assert!(field("61", 0, 59).is_none());
        assert!(Cron::parse("* * *").is_none());
        let weekdays = Cron::parse("0 9 * * 1-5").unwrap();
        let monday_9 = LocalTime {
            minute: 0,
            hour: 9,
            day: 12,
            month: 10,
            weekday: 1,
        };
        assert!(weekdays.matches(monday_9));
        assert!(!weekdays.matches(LocalTime {
            weekday: 0,
            ..monday_9
        }));
        // Sunday may be 7.
        assert!(Cron::parse("0 9 * * 7").unwrap().matches(LocalTime {
            weekday: 0,
            ..monday_9
        }));
    }
}
