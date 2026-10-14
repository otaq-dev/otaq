//! macOS and Linux: raw mode with termios, the window size from the terminal, resizes from SIGWINCH.

use std::io;
use std::os::fd::AsFd;

use otaq_core::session::TerminalSize;

/// The local terminal in raw mode for as long as this lives; restored on drop (also on errors).
pub struct RawTerminal {
    saved: rustix::termios::Termios,
}

impl RawTerminal {
    /// `None` when stdin isn't a terminal (keys are then relayed as they come).
    pub fn enter() -> Option<Self> {
        let stdin = std::io::stdin();
        if !rustix::termios::isatty(stdin.as_fd()) {
            return None;
        }
        let saved = rustix::termios::tcgetattr(stdin.as_fd()).ok()?;
        let mut raw = saved.clone();
        raw.make_raw();
        rustix::termios::tcsetattr(stdin.as_fd(), rustix::termios::OptionalActions::Now, &raw)
            .ok()?;
        Some(Self { saved })
    }

    pub fn size(&self) -> Option<TerminalSize> {
        let size = rustix::termios::tcgetwinsize(std::io::stdout().as_fd()).ok()?;
        (size.ws_col > 0 && size.ws_row > 0).then_some(TerminalSize {
            cols: size.ws_col,
            rows: size.ws_row,
        })
    }
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        let _ = rustix::termios::tcsetattr(
            std::io::stdin().as_fd(),
            rustix::termios::OptionalActions::Now,
            &self.saved,
        );
    }
}

/// Resizes of the local terminal window.
pub struct WindowChanges(tokio::signal::unix::Signal);

impl WindowChanges {
    pub fn new() -> io::Result<Self> {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::window_change()).map(Self)
    }

    /// Resolves at the next resize.
    pub async fn next(&mut self) {
        if self.0.recv().await.is_none() {
            std::future::pending::<()>().await;
        }
    }
}
