//! What the app says when it connects (REC-06; d0045, d0022; *16 → Opening after a reboot*, *11 → After the service
//! restart*): after a reboot, a crash or a service restart the service started again and its startup reconcile
//! interrupted sessions, so the app opens with "4 sessions were interrupted" (Resume all, Review & resume). After a
//! normal reopen, with the service still running, only a quiet "Reconnected".

use crewboard_core::protocol::ServiceStatus;

use crate::view_models::banners::Situation;
use crate::view_models::clock::local_hhmm;

/// A service that started this long before the app opened was already running: a normal reopen.
pub const ALREADY_RUNNING_MS: i64 = 15_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnConnect {
    /// Raise the summary banner (once per service start).
    Summary(Situation),
    /// "Reconnected": the app reopened while the service kept running.
    Reconnected,
    Nothing,
}

/// What a connection to the service shows. `app_started_ms`: when this app opened; `first_connect`: the app's
/// first connection; `shown_for`: the service start a summary was already shown for.
pub fn on_connect(
    status: &ServiceStatus,
    app_started_ms: i64,
    first_connect: bool,
    shown_for: Option<i64>,
) -> OnConnect {
    if shown_for == Some(status.started_at) {
        return OnConnect::Nothing;
    }
    let already_running = status.started_at + ALREADY_RUNNING_MS < app_started_ms;
    if already_running {
        return if first_connect {
            OnConnect::Reconnected
        } else {
            OnConnect::Nothing
        };
    }
    if status.interrupted_at_start == 0 {
        return OnConnect::Nothing;
    }
    OnConnect::Summary(Situation::SessionsInterrupted {
        count: status.interrupted_at_start,
        unclean: status.unclean_start,
        at: local_hhmm(status.started_at),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: i64 = 1_791_600_000_000;

    fn status(started_at: i64, interrupted: u32, unclean: bool) -> ServiceStatus {
        ServiceStatus {
            started_at,
            interrupted_at_start: interrupted,
            unclean_start: unclean,
            ..Default::default()
        }
    }

    #[test]
    fn after_a_reboot_the_app_opens_with_the_summary_once() {
        let s = status(APP + 2_000, 4, true);
        let OnConnect::Summary(Situation::SessionsInterrupted { count, unclean, at }) =
            on_connect(&s, APP, true, None)
        else {
            panic!("summary")
        };
        assert_eq!((count, unclean, at.len()), (4, true, 5));
        assert_eq!(
            on_connect(&s, APP, false, Some(s.started_at)),
            OnConnect::Nothing
        );
    }

    #[test]
    fn a_restart_while_the_app_runs_shows_it_too() {
        let s = status(APP + 600_000, 3, false);
        assert!(matches!(
            on_connect(&s, APP, false, Some(APP - 1)),
            OnConnect::Summary(Situation::SessionsInterrupted { unclean: false, .. })
        ));
        assert_eq!(
            on_connect(&status(APP + 600_000, 0, false), APP, false, None),
            OnConnect::Nothing
        );
    }

    #[test]
    fn a_normal_reopen_is_a_quiet_reconnected() {
        let s = status(APP - 3_600_000, 4, true);
        assert_eq!(on_connect(&s, APP, true, None), OnConnect::Reconnected);
        assert_eq!(on_connect(&s, APP, false, None), OnConnect::Nothing);
    }
}
