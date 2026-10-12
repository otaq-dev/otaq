//! The terminal's state as the panel shows it; the emulator itself stays in the app.

/// Where the terminal stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// The ticket has no session (yet).
    NoSession,
    /// Waiting for the stored output.
    Attaching,
    /// Following the live session; input goes to it.
    Live,
    /// The connection to the service is lost: the scrollback stays, input is paused (SVC-04).
    Reconnecting,
    /// The session ended: read-only scrollback with an end marker.
    Ended,
}
