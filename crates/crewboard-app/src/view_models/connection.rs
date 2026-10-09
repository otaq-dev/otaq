//! What the window shows about its link to the background service (ARC-03, SVC-04).

use crewboard_core::client::{Compatibility, ConnectionState, ServiceInfo};

/// The app's view of the link, including the phases before a client exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkState {
    /// Connecting, starting the service first if it isn't running.
    Connecting,
    Connected(ServiceInfo),
    /// The connection dropped; the client is retrying. Agents keep running.
    Reconnecting {
        attempt: u32,
    },
    /// The service couldn't be reached or started.
    Failed(String),
}

impl From<ConnectionState> for LinkState {
    fn from(state: ConnectionState) -> Self {
        match state {
            ConnectionState::Connected(info) => Self::Connected(info),
            ConnectionState::Reconnecting { attempt } => Self::Reconnecting { attempt },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Calm,
    Attention,
    Error,
}

/// The connection line the skeleton window shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionViewModel {
    pub label: String,
    pub detail: Option<String>,
    pub tone: Tone,
    /// Whether the window accepts input that needs the service (disabled while reconnecting, SVC-04).
    pub input_enabled: bool,
}

impl ConnectionViewModel {
    pub fn new(state: &LinkState) -> Self {
        match state {
            LinkState::Connecting => Self {
                label: "Connecting…".into(),
                detail: None,
                tone: Tone::Calm,
                input_enabled: false,
            },
            LinkState::Connected(info) => Self {
                label: "Connected".into(),
                detail: Some(format!(
                    "Background service · PID {} · build {}{}",
                    info.status.pid,
                    info.status.build_id,
                    match info.compatibility {
                        Compatibility::Same => "",
                        Compatibility::ServiceOlder => " · update ready",
                        Compatibility::Incompatible => " · restart required",
                    }
                )),
                tone: match info.compatibility {
                    Compatibility::Incompatible => Tone::Attention,
                    _ => Tone::Calm,
                },
                input_enabled: info.compatibility != Compatibility::Incompatible,
            },
            LinkState::Reconnecting { .. } => Self {
                label: "Reconnecting…".into(),
                detail: Some("agents keep running".into()),
                tone: Tone::Attention,
                input_enabled: false,
            },
            LinkState::Failed(reason) => Self {
                label: "Not connected".into(),
                detail: Some(reason.clone()),
                tone: Tone::Error,
                input_enabled: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crewboard_core::protocol::{PROTOCOL_VERSION, ServiceStatus};

    use super::*;

    fn info(compatibility: Compatibility) -> ServiceInfo {
        ServiceInfo {
            status: ServiceStatus {
                pid: 4120,
                build_id: "abc.1".into(),
                protocol_version: PROTOCOL_VERSION,
                started_at: 0,
                uptime_secs: 0,
                agents_working: 0,
                agents_waiting: 0,
            },
            compatibility,
        }
    }

    #[test]
    fn connected_shows_the_service() {
        let vm = ConnectionViewModel::new(&LinkState::Connected(info(Compatibility::Same)));
        assert_eq!(vm.label, "Connected");
        assert_eq!(
            vm.detail.as_deref(),
            Some("Background service · PID 4120 · build abc.1")
        );
        assert!(vm.input_enabled);
        assert_eq!(vm.tone, Tone::Calm);
    }

    #[test]
    fn reconnecting_disables_input() {
        let vm = ConnectionViewModel::new(&LinkState::from(ConnectionState::Reconnecting {
            attempt: 2,
        }));
        assert_eq!(vm.label, "Reconnecting…");
        assert_eq!(vm.detail.as_deref(), Some("agents keep running"));
        assert!(!vm.input_enabled);
    }

    #[test]
    fn incompatible_service_disables_input() {
        let vm = ConnectionViewModel::new(&LinkState::Connected(info(Compatibility::Incompatible)));
        assert!(!vm.input_enabled);
        assert_eq!(vm.tone, Tone::Attention);
        let vm = ConnectionViewModel::new(&LinkState::Connected(info(Compatibility::ServiceOlder)));
        assert!(
            vm.input_enabled,
            "an older service keeps working until the user restarts"
        );
    }

    #[test]
    fn failure_and_connecting() {
        let vm = ConnectionViewModel::new(&LinkState::Failed("no such file".into()));
        assert_eq!(vm.tone, Tone::Error);
        assert_eq!(vm.detail.as_deref(), Some("no such file"));
        assert!(!ConnectionViewModel::new(&LinkState::Connecting).input_enabled);
    }
}
