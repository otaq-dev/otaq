//! Claude Code's usage windows and limit text (USG-03, USG-04, USG-09; d0020, d0046, d0088).
//!
//! **Windows** come from the status line Otaq runs agents with (`otaq statusline`). Source: Claude Code docs,
//! "Customize your status line → Available data" (code.claude.com/docs/en/statusline, read 2026-10-10; Claude Code
//! 2.1.296): `rate_limits.five_hour` and `rate_limits.seven_day`, each with `used_percentage` (0–100) and
//! `resets_at` (Unix epoch **seconds**). `rate_limits` appears only for claude.ai Pro and Max subscribers (or behind
//! a gateway with a spend limit), only after the session's first API response; each window may be absent on its
//! own, and Claude Code drops a window once its `resets_at` passes. API-key users get no `rate_limits`, so nothing
//! is recorded for them. Saved samples: `tests/fixtures/claude/statusline/`.
//!
//! **"It happened"** comes from the plugin's `StopFailure` hook ([`super::hooks`]): error `rate_limit` with Claude
//! Code's limit text in `last_assistant_message` (or `error_details`). The wording follows the Claude Code docs,
//! "Error reference → usage limits" (code.claude.com/docs/en/errors, read 2026-10-10; Claude Code 2.1.296):
//!
//! ```text
//! You've hit your session limit · resets 3:45pm
//! You've hit your weekly limit · resets Mon 12:00am
//! You've hit your Opus limit · resets 3:45pm
//! ```
//!
//! Other `rate_limit` failures ("API Error: Request rejected (429) · this may be a temporary capacity issue…",
//! "Server is temporarily limiting requests (not your usage limit)") are not the plan limit: Claude Code retries
//! those itself, and they keep their existing handling.
//!
//! The reset time, in this order (`usage::limit`): the window the limit belongs to ([`window_key`]), else the time
//! in the limit text ([`reset_from_text`]: the next local occurrence of "3:45pm", or of "Mon 12:00am"), else
//! unknown.

use serde_json::Value;

use crate::agent_tools::events::LimitWindow;
use crate::usage::limit::{LimitHit, LimitKind};

/// The status line's 5-hour window, in minutes.
pub const FIVE_HOUR_MINUTES: u32 = 5 * 60;
/// The status line's weekly window, in minutes.
pub const SEVEN_DAY_MINUTES: u32 = 7 * 24 * 60;

/// The status line's windows: key and length.
const WINDOWS: [(&str, u32); 2] = [
    ("five_hour", FIVE_HOUR_MINUTES),
    ("seven_day", SEVEN_DAY_MINUTES),
];

/// The windows in a status-line JSON; `None` when it has none (API-key users, or before the first response).
pub fn windows_from_status_line(payload: &Value) -> Option<Vec<LimitWindow>> {
    let limits = payload.get("rate_limits")?;
    let windows: Vec<LimitWindow> = WINDOWS
        .iter()
        .filter_map(|&(key, minutes)| {
            let w = limits.get(key)?;
            Some(LimitWindow {
                key: key.to_owned(),
                length_minutes: Some(minutes),
                used_percent: w.get("used_percentage")?.as_f64()?,
                resets_at: Some(w.get("resets_at")?.as_f64()? as i64 * 1000),
                credits: None,
            })
        })
        .collect();
    (!windows.is_empty()).then_some(windows)
}

/// The window a limit of `kind` belongs to: session → `five_hour`, weekly → `seven_day`; another limit (a model's
/// own, a spend limit) has none.
pub fn window_key(kind: LimitKind) -> Option<&'static str> {
    match kind {
        LimitKind::Session => Some(WINDOWS[0].0),
        LimitKind::Weekly => Some(WINDOWS[1].0),
        LimitKind::Other => None,
    }
}

/// Whether a `StopFailure` with `error` and `message` is the plan limit, and which one.
pub fn recognise(error: &str, message: &str) -> Option<LimitHit> {
    if error != "rate_limit" {
        return None;
    }
    let lower = message.to_lowercase();
    // "You've hit your … limit"; the apostrophe may be typographic.
    let hit = lower.contains("hit your") && lower.contains("limit");
    if !hit {
        return None;
    }
    let kind = if lower.contains("weekly limit") {
        LimitKind::Weekly
    } else if lower.contains("session limit") {
        LimitKind::Session
    } else {
        LimitKind::Other
    };
    Some(LimitHit {
        kind,
        text: message.trim().to_owned(),
    })
}

/// The time named after "resets" in a limit text: an optional weekday (0 = Sunday) and a time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetText {
    pub weekday: Option<u32>,
    pub hour: u32,
    pub minute: u32,
}

/// When a limit text says the limit resets, after `now` (Unix milliseconds); `None` when it names no time.
pub fn reset_from_text(text: &str, now: i64) -> Option<i64> {
    next_local(parse_reset(text)?, now)
}

/// Reads "resets 3:45pm", "resets Mon 12:00am", "resets 3pm (Europe/Istanbul)".
pub fn parse_reset(text: &str) -> Option<ResetText> {
    let lower = text.to_lowercase();
    let after = &lower[lower.find("resets")? + "resets".len()..];
    let mut words = after.split_whitespace().peekable();
    let weekday = words.peek().and_then(|w| weekday(w));
    if weekday.is_some() {
        words.next();
    }
    let time = words.next()?;
    let (clock, pm) = match time.strip_suffix("pm") {
        Some(t) => (t, true),
        None => (time.strip_suffix("am")?, false),
    };
    let (h, m) = match clock.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (clock.parse::<u32>().ok()?, 0),
    };
    if !(1..=12).contains(&h) || m > 59 {
        return None;
    }
    let hour = match (h, pm) {
        (12, false) => 0,
        (12, true) => 12,
        (h, false) => h,
        (h, true) => h + 12,
    };
    Some(ResetText {
        weekday,
        hour,
        minute: m,
    })
}

fn weekday(word: &str) -> Option<u32> {
    let names = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
    let w = word.trim_end_matches(',');
    names
        .iter()
        .position(|n| {
            w.starts_with(n) && w.len() <= 9 && w.chars().all(|c| c.is_ascii_alphabetic())
        })
        .map(|i| i as u32)
}

/// The next local moment after `now` that matches `reset` (Unix milliseconds), within eight days.
pub fn next_local(reset: ResetText, now: i64) -> Option<i64> {
    let start_of_minute = (now.div_euclid(60_000) + 1) * 60_000;
    // Walk whole minutes is too slow over a week; walk days and set the time of day with the local clock.
    (0..=8).find_map(|day| {
        let at =
            crate::local_time::on_same_day_at(now + day * 86_400_000, reset.hour, reset.minute)?;
        let weekday_ok = reset
            .weekday
            .is_none_or(|w| crate::signals::background::local(at).is_some_and(|t| t.weekday == w));
        (at >= start_of_minute && weekday_ok).then_some(at)
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::signals::background::local;

    fn fixture(name: &str) -> Value {
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "tests",
            "fixtures",
            "claude",
            "statusline",
            name,
        ]
        .iter()
        .collect();
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    #[test]
    fn the_saved_status_line_gives_both_windows() {
        assert_eq!(
            windows_from_status_line(&fixture("with_rate_limits.json")).unwrap(),
            [
                LimitWindow {
                    key: "five_hour".into(),
                    length_minutes: Some(300),
                    used_percent: 23.5,
                    resets_at: Some(1_738_425_600_000),
                    credits: None
                },
                LimitWindow {
                    key: "seven_day".into(),
                    length_minutes: Some(10_080),
                    used_percent: 41.2,
                    resets_at: Some(1_738_857_600_000),
                    credits: None
                },
            ]
        );
        // API-key users, and before the first response.
        assert_eq!(
            windows_from_status_line(&fixture("without_rate_limits.json")),
            None
        );
    }

    #[test]
    fn a_window_dropped_after_its_reset_is_left_out() {
        let mut payload = fixture("with_rate_limits.json");
        payload["rate_limits"]
            .as_object_mut()
            .unwrap()
            .remove("five_hour");
        let windows = windows_from_status_line(&payload).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].key, "seven_day");
        payload["rate_limits"] = serde_json::json!({});
        assert_eq!(windows_from_status_line(&payload), None);
    }

    #[test]
    fn limits_belong_to_their_window() {
        assert_eq!(window_key(LimitKind::Session), Some("five_hour"));
        assert_eq!(window_key(LimitKind::Weekly), Some("seven_day"));
        assert_eq!(window_key(LimitKind::Other), None);
    }

    #[test]
    fn only_the_limit_text_with_rate_limit_is_the_plan_limit() {
        let session = recognise(
            "rate_limit",
            "You've hit your session limit · resets 3:45pm",
        )
        .unwrap();
        assert_eq!(session.kind, LimitKind::Session);
        assert_eq!(
            recognise(
                "rate_limit",
                "You’ve hit your weekly limit · resets Mon 12:00am"
            )
            .unwrap()
            .kind,
            LimitKind::Weekly
        );
        assert_eq!(
            recognise("rate_limit", "You've hit your Opus limit · resets 3:45pm")
                .unwrap()
                .kind,
            LimitKind::Other
        );
        assert_eq!(
            recognise(
                "rate_limit",
                "API Error: Request rejected (429) · this may be a temporary capacity issue."
            ),
            None
        );
        assert_eq!(
            recognise(
                "rate_limit",
                "API Error: Server is temporarily limiting requests (not your usage limit)"
            ),
            None
        );
        assert_eq!(
            recognise(
                "overloaded",
                "You've hit your session limit · resets 3:45pm"
            ),
            None
        );
    }

    #[test]
    fn reset_times_are_read_from_the_text() {
        let t = |s: &str| parse_reset(s);
        assert_eq!(
            t("You've hit your session limit · resets 3:45pm"),
            Some(ResetText {
                weekday: None,
                hour: 15,
                minute: 45
            })
        );
        assert_eq!(
            t("You've hit your weekly limit · resets Mon 12:00am"),
            Some(ResetText {
                weekday: Some(1),
                hour: 0,
                minute: 0
            })
        );
        assert_eq!(
            t("resets 12pm (Europe/Istanbul)"),
            Some(ResetText {
                weekday: None,
                hour: 12,
                minute: 0
            })
        );
        assert_eq!(
            t("resets 9am"),
            Some(ResetText {
                weekday: None,
                hour: 9,
                minute: 0
            })
        );
        assert_eq!(t("You've hit your session limit"), None);
        assert_eq!(t("resets soon"), None);
        assert_eq!(t("resets 13:00pm"), None);
    }

    #[test]
    fn the_next_local_occurrence_is_after_now() {
        let now = 1_791_547_200_000; // 2026-10-09 12:00 UTC
        let at = next_local(
            ResetText {
                weekday: None,
                hour: 15,
                minute: 45,
            },
            now,
        )
        .unwrap();
        assert!(at > now && at - now <= 86_400_000);
        let lt = local(at).unwrap();
        assert_eq!((lt.hour, lt.minute), (15, 45));
        let monday = next_local(
            ResetText {
                weekday: Some(1),
                hour: 0,
                minute: 0,
            },
            now,
        )
        .unwrap();
        assert!(monday > now && monday - now <= 7 * 86_400_000);
        let lt = local(monday).unwrap();
        assert_eq!((lt.weekday, lt.hour, lt.minute), (1, 0, 0));
        assert_eq!(
            reset_from_text("You've hit your session limit · resets 3:45pm", now),
            Some(at)
        );
        assert_eq!(reset_from_text("You've hit your weekly limit", now), None);
    }
}
