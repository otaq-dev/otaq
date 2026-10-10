//! The local clock: a moment's local date and time, and a local date and time back to a moment. The C library
//! does the work with the system's time zone (`localtime_r`/`mktime`; `localtime_s`/`mktime` on Windows).

/// A moment's fields in local time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    /// 1–12.
    pub month: u32,
    /// 1–31.
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    /// 0 = Sunday.
    pub weekday: u32,
}

/// The local time of a Unix-milliseconds moment.
pub fn local(at_ms: i64) -> Option<LocalTime> {
    let tm = local_tm(at_ms)?;
    let field = |v: i32| u32::try_from(v).ok();
    Some(LocalTime {
        year: tm.tm_year + 1900,
        month: field(tm.tm_mon + 1)?,
        day: field(tm.tm_mday)?,
        hour: field(tm.tm_hour)?,
        minute: field(tm.tm_min)?,
        second: field(tm.tm_sec)?,
        weekday: field(tm.tm_wday)?,
    })
}

/// The moment (Unix milliseconds) on `day_ms`'s local date at `hour:minute`.
pub fn on_same_day_at(day_ms: i64, hour: u32, minute: u32) -> Option<i64> {
    let mut tm = local_tm(day_ms)?;
    tm.tm_hour = i32::try_from(hour).ok()?;
    tm.tm_min = i32::try_from(minute).ok()?;
    tm.tm_sec = 0;
    tm.tm_isdst = -1;
    // SAFETY: mktime only reads and normalises the struct passed to it.
    #[cfg(unix)]
    let t = unsafe { libc::mktime(&mut tm) };
    #[cfg(windows)]
    let t = unsafe { _mktime64(&mut tm) };
    (t != -1).then_some(t * 1000)
}

// The `libc` crate has no `mktime` for Windows; the C runtime exports it as `_mktime64`.
#[cfg(windows)]
unsafe extern "C" {
    fn _mktime64(tm: *mut libc::tm) -> i64;
}

fn local_tm(at_ms: i64) -> Option<libc::tm> {
    let secs = libc::time_t::try_from(at_ms.div_euclid(1000)).ok()?;
    // SAFETY: the conversion writes only into the zeroed struct passed to it.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    #[cfg(unix)]
    let ok = unsafe { !libc::localtime_r(&secs, &mut tm).is_null() };
    #[cfg(windows)]
    let ok = unsafe { libc::localtime_s(&mut tm, &secs) } == 0;
    ok.then_some(tm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_time_of_day() {
        let now = 1_791_547_200_000;
        let t = local(now).unwrap();
        assert!((1..=12).contains(&t.month) && (1..=31).contains(&t.day) && t.weekday < 7);
        let back = on_same_day_at(now, t.hour, t.minute).unwrap();
        assert_eq!(
            back,
            now - i64::from(t.second) * 1000 - now.rem_euclid(1000)
        );
        let noon = local(on_same_day_at(now, 12, 30).unwrap()).unwrap();
        assert_eq!((noon.day, noon.hour, noon.minute), (t.day, 12, 30));
    }
}
