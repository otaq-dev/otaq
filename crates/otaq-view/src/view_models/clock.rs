//! Times and durations as the panel shows them: "11:02" in local time, "41 min", "2 h 5 min".

/// The local time of day of a Unix-milliseconds timestamp, as `HH:MM`.
pub fn local_hhmm(at_ms: i64) -> String {
    match otaq_core::local_time::local(at_ms) {
        Some(t) => format!("{:02}:{:02}", t.hour, t.minute),
        None => "--:--".to_owned(),
    }
}

/// A duration in whole minutes: "0 min", "41 min", "72 min" (as the History frame), then "2 h 5 min", "3 h"
/// from two hours on.
pub fn duration(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    if minutes < 120 {
        return format!("{minutes} min");
    }
    match (minutes / 60, minutes % 60) {
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(duration(0), "0 min");
        assert_eq!(duration(59_999), "0 min");
        assert_eq!(duration(41 * 60_000 + 5_000), "41 min");
        assert_eq!(duration(72 * 60_000), "72 min");
        assert_eq!(duration(125 * 60_000), "2 h 5 min");
        assert_eq!(duration(180 * 60_000), "3 h");
        assert_eq!(duration(-5), "0 min");
    }

    #[test]
    fn time_of_day_has_two_digit_fields() {
        let t = local_hhmm(1_791_582_000_000);
        assert_eq!(t.len(), 5);
        assert_eq!(&t[2..3], ":");
    }
}
