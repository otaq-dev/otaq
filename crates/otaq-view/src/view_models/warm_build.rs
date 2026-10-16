//! What Cleanup (T-0302) and the Settings switch (T-0301) both say about a warm build (WRM-05…WRM-07, d0082):
//! "up to <size>" in the confirmations (clones share space), the shared-space line, "Clearing… (waiting for
//! SHOP-21’s copy)" while a Clear waits for a seed, and the toast with no number. One place, so both read the same.

use crate::view_models::cleanup::size;
use crate::view_models::toasts::{NewToast, ToastLevel};

/// The confirmations' line under the text (*Warm build · Clear · confirm (0082)*).
pub const SHARED_SPACE: &str =
    "Tickets copied from it share part of this space, so it may free less.";

/// "up to 36.1 GB": what a Clear may free, in the confirmations only (rows and totals keep the full size).
pub fn up_to(bytes: u64) -> String {
    format!("up to {}", size(bytes))
}

/// "Clear otaq’s warm build (up to 36.1 GB)?", the title of both confirmations.
pub fn confirm_title(name: &str, bytes: u64) -> String {
    format!("Clear {name}’s warm build ({})?", up_to(bytes))
}

/// "Clearing… (waiting for SHOP-21’s copy)" while a Clear waits for the seeds running from it (WRM-06); "Clearing…"
/// once none is left.
pub fn waiting(copying: &[String]) -> String {
    match copying {
        [] => CLEARING.to_owned(),
        [one] => format!("{CLEARING} (waiting for {one}’s copy)"),
        [first @ .., last] => format!(
            "{CLEARING} (waiting for {} and {last}’s copies)",
            first.join(", ")
        ),
    }
}

/// The disabled button while a Clear waits.
pub const CLEARING: &str = "Clearing…";

/// The toast once a warm build is cleared: "Cleared otaq’s warm build" (no number: clones share space).
pub fn cleared_toast(name: &str) -> NewToast {
    NewToast::new(
        ToastLevel::Success,
        format!("{CLEARED}{name}{WARM_BUILD}"),
        Some("The next Done ticket keeps a new one".to_owned()),
    )
}

/// The toast once turning the switch off cleared the warm build (*Warm build · off (0082)*): "Cleared shop-web’s warm
/// build", "New tickets build from nothing".
pub fn switched_off_toast(name: &str) -> NewToast {
    NewToast::new(
        ToastLevel::Success,
        format!("{CLEARED}{name}{WARM_BUILD}"),
        Some("New tickets build from nothing".to_owned()),
    )
}

/// The switch's confirmation button: "Turn off and clear up to 4.2 GB".
pub fn turn_off_label(bytes: u64) -> String {
    format!("Turn off and clear {}", up_to(bytes))
}

const CLEARED: &str = "Cleared ";
const WARM_BUILD: &str = "’s warm build";

/// Whether a toast is [`cleared_toast`]'s (the page gives it the broom, as Cleanup's own toast).
pub fn is_cleared_toast(title: &str) -> bool {
    title.starts_with(CLEARED) && title.ends_with(WARM_BUILD)
}

/// The toast when a Clear that waited didn't go through.
pub fn not_cleared_toast(name: &str) -> NewToast {
    NewToast::new(
        ToastLevel::Error,
        format!("Couldn’t clear {name}{WARM_BUILD}"),
        None,
    )
}

/// "3 days ago", "2 hours ago", "1 minute ago", "just now": the age in a sentence.
pub fn ago(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    let (n, unit) = match minutes {
        0 => return "just now".to_owned(),
        m if m < 60 => (m, "minute"),
        m if m < 24 * 60 => (m / 60, "hour"),
        m => (m / (24 * 60), "day"),
    };
    let s = if n == 1 { "" } else { "s" };
    format!("{n} {unit}{s} ago")
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn the_shared_wordings() {
        assert_eq!(
            confirm_title("shop-web", 42 * GB / 10),
            "Clear shop-web’s warm build (up to 4.2 GB)?"
        );
        assert_eq!(up_to(GB), "up to 1.0 GB");
        assert_eq!(waiting(&[]), "Clearing…");
        assert_eq!(
            waiting(&["SHOP-21".into()]),
            "Clearing… (waiting for SHOP-21’s copy)"
        );
        assert_eq!(
            waiting(&["SHOP-21".into(), "SHOP-22".into(), "SHOP-23".into()]),
            "Clearing… (waiting for SHOP-21, SHOP-22 and SHOP-23’s copies)"
        );
        let toast = cleared_toast("shop-web");
        assert_eq!(toast.title, "Cleared shop-web’s warm build");
        assert!(is_cleared_toast(&toast.title));
        assert!(!is_cleared_toast("Cleared 2 items"));
        assert!(!toast.text.unwrap().chars().any(|c| c.is_ascii_digit()));
        let off = switched_off_toast("shop-web");
        assert_eq!(off.title, "Cleared shop-web’s warm build");
        assert_eq!(off.text.as_deref(), Some("New tickets build from nothing"));
        assert!(is_cleared_toast(&off.title));
        assert_eq!(
            turn_off_label(42 * GB / 10),
            "Turn off and clear up to 4.2 GB"
        );
        assert_eq!(ago(3 * 24 * 3_600_000), "3 days ago");
        assert_eq!(ago(3_600_000), "1 hour ago");
        assert_eq!(ago(5 * 60_000), "5 minutes ago");
        assert_eq!(ago(10_000), "just now");
    }
}
