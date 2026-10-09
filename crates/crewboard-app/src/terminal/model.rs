//! The terminal's state without any drawing: an `alacritty_terminal` grid fed with a session's stored scrollback
//! and then its live output, and where the link to the session stands (SES-09, SVC-04, SVC-05).
//!
//! - **Attach** starts a fresh grid and replays the stored scrollback. Answers the program asked for during the
//!   replay (cursor position, colours) belong to the past and are dropped.
//! - **Live output** carries offsets in the session's whole output: bytes the grid already has are skipped, so a
//!   reconnect never shows output twice. A gap (output the client never got) asks for a re-attach.
//! - **Disconnected**: the scrollback stays visible and input is off until the next attach (SVC-04).
//! - **Ended**: read-only scrollback with an end marker.

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event as TermEvent, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Processor, Rgb, StdSyncHandler};
use crewboard_core::session::{SessionInfo, SessionState, TerminalSize};

/// Lines kept above the screen for scrolling back (the service keeps its own, bounded scrollback too).
pub const HISTORY_LINES: usize = 10_000;

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

/// Answers the emulator wants to send to the program.
#[derive(Clone, Default)]
pub struct Replies(Arc<Mutex<Vec<u8>>>);

impl EventListener for Replies {
    fn send_event(&self, event: TermEvent) {
        if let TermEvent::PtyWrite(text) = event
            && let Ok(mut out) = self.0.lock()
        {
            out.extend_from_slice(text.as_bytes());
        }
    }
}

impl Replies {
    fn take(&self) -> Vec<u8> {
        self.0
            .lock()
            .map(|mut v| std::mem::take(&mut *v))
            .unwrap_or_default()
    }
}

pub struct TerminalModel {
    pub term: Term<Replies>,
    parser: Processor<StdSyncHandler>,
    replies: Replies,
    pub phase: Phase,
    /// The attached session, as last reported.
    pub session: Option<SessionInfo>,
    /// The offset of the next byte expected from the session.
    next_offset: u64,
    size: TerminalSize,
    /// Set when output was missed; the view re-attaches.
    pub needs_reattach: bool,
}

/// What live output did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Fed {
    /// New bytes reached the grid.
    pub changed: bool,
    /// Answers for the program (send them as replies, not as typing).
    pub replies: Vec<u8>,
}

fn new_term(size: TerminalSize, replies: Replies) -> Term<Replies> {
    let config = Config {
        scrolling_history: HISTORY_LINES,
        ..Config::default()
    };
    Term::new(config, &term_size(size), replies)
}

fn term_size(size: TerminalSize) -> TermSize {
    TermSize::new(usize::from(size.cols.max(2)), usize::from(size.rows.max(1)))
}

impl TerminalModel {
    pub fn new(size: TerminalSize) -> Self {
        let replies = Replies::default();
        Self {
            term: new_term(size, replies.clone()),
            parser: Processor::new(),
            replies,
            phase: Phase::NoSession,
            session: None,
            next_offset: 0,
            size,
            needs_reattach: false,
        }
    }

    /// The stored output arrived: a fresh grid with the scrollback replayed. `end` is the offset after it.
    pub fn attach(&mut self, session: SessionInfo, scrollback: &[u8], end: u64) {
        self.replies = Replies::default();
        self.term = new_term(self.size, self.replies.clone());
        self.parser = Processor::new();
        self.parser.advance(&mut self.term, scrollback);
        // Questions in the replayed output were answered long ago.
        self.replies.take();
        self.next_offset = end;
        self.needs_reattach = false;
        self.phase = if session.state == SessionState::Running {
            Phase::Live
        } else {
            Phase::Ended
        };
        self.session = Some(session);
    }

    /// Live output that starts at `offset` in the session's output.
    pub fn output(&mut self, offset: u64, bytes: &[u8]) -> Fed {
        if !matches!(self.phase, Phase::Live | Phase::Ended) {
            return Fed::default();
        }
        if offset > self.next_offset {
            // Some output never reached us: redraw from the service.
            self.needs_reattach = true;
            return Fed::default();
        }
        let skip = usize::try_from(self.next_offset - offset).unwrap_or(usize::MAX);
        if skip >= bytes.len() {
            return Fed::default();
        }
        self.parser.advance(&mut self.term, &bytes[skip..]);
        self.next_offset = offset + bytes.len() as u64;
        let replies = self.replies.take();
        Fed {
            changed: true,
            replies: if self.phase == Phase::Live {
                replies
            } else {
                Vec::new()
            },
        }
    }

    /// The connection to the service is lost.
    pub fn disconnected(&mut self) {
        if matches!(self.phase, Phase::Live | Phase::Attaching) {
            self.phase = Phase::Reconnecting;
        }
    }

    /// Waiting for an attach answer (after connecting or reconnecting).
    pub fn attaching(&mut self) {
        if self.phase != Phase::Reconnecting {
            self.phase = Phase::Attaching;
        }
    }

    /// The ticket has no session to show.
    pub fn no_session(&mut self) {
        self.phase = Phase::NoSession;
        self.session = None;
    }

    /// The session ended (its `session_state` event).
    pub fn ended(&mut self, session: SessionInfo) {
        if self.session.as_ref().is_some_and(|s| s.id == session.id) {
            self.session = Some(session);
            self.phase = Phase::Ended;
        }
    }

    /// Keystrokes go to the session only while it is live and connected.
    pub fn input_enabled(&self) -> bool {
        self.phase == Phase::Live
    }

    pub fn size(&self) -> TerminalSize {
        self.size
    }

    /// Fits the grid to the view; `true` when the size changed (the PTY should follow).
    pub fn resize(&mut self, size: TerminalSize) -> bool {
        if size == self.size || size.cols < 2 || size.rows < 1 {
            return false;
        }
        self.size = size;
        self.term.resize(term_size(size));
        true
    }

    pub fn mode(&self) -> TermMode {
        *self.term.mode()
    }

    /// The visible screen as text, one string per line (trailing spaces trimmed). For tests and find.
    pub fn screen_text(&self) -> Vec<String> {
        let grid = self.term.grid();
        let offset = grid.display_offset() as i32;
        (0..grid.screen_lines())
            .map(|line| {
                let row = &grid[Line(line as i32 - offset)];
                let text: String = (0..grid.columns()).map(|c| row[Column(c)].c).collect();
                text.trim_end().to_owned()
            })
            .collect()
    }

    /// The colour of the cell at `line`, `column` of the screen, for tests.
    pub fn cell_fg(&self, line: usize, column: usize) -> alacritty_terminal::vte::ansi::Color {
        self.term.grid()[Point::new(Line(line as i32), Column(column))].fg
    }

    /// A colour the program set (OSC 4/10/11), if any.
    pub fn color_override(&self, index: usize) -> Option<Rgb> {
        self.term.colors()[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::vte::ansi::{Color, NamedColor};

    fn session(state: SessionState) -> SessionInfo {
        SessionInfo {
            id: "s1".into(),
            kind: crewboard_core::session::SessionKind::Agent,
            project: "SHOP".into(),
            ticket: Some("SHOP-19".parse().unwrap()),
            state,
            started_at: 0,
            ended_at: None,
            end_reason: None,
            claude_session_id: None,
            size: None,
        }
    }

    fn model() -> TerminalModel {
        TerminalModel::new(TerminalSize { cols: 40, rows: 5 })
    }

    #[test]
    fn attach_replays_then_follows_live_output() {
        let mut m = model();
        assert_eq!(m.phase, Phase::NoSession);
        m.attaching();
        assert_eq!(m.phase, Phase::Attaching);
        assert!(!m.input_enabled());
        m.attach(session(SessionState::Running), b"one\r\n", 5);
        assert_eq!(m.phase, Phase::Live);
        assert!(m.input_enabled());
        assert_eq!(m.screen_text()[0], "one");
        let fed = m.output(5, b"two\r\n");
        assert!(fed.changed);
        assert_eq!(&m.screen_text()[..2], ["one", "two"]);
    }

    #[test]
    fn output_already_shown_is_skipped() {
        let mut m = model();
        m.attach(session(SessionState::Running), b"abc", 3);
        // A chunk that overlaps the scrollback: only "de" is new.
        m.output(1, b"bcde");
        assert_eq!(m.screen_text()[0], "abcde");
        // A chunk entirely in the past changes nothing.
        assert!(!m.output(0, b"abc").changed);
        assert_eq!(m.screen_text()[0], "abcde");
    }

    #[test]
    fn a_gap_asks_for_a_reattach() {
        let mut m = model();
        m.attach(session(SessionState::Running), b"abc", 3);
        assert!(!m.output(10, b"zzz").changed);
        assert!(m.needs_reattach);
        assert_eq!(m.screen_text()[0], "abc");
    }

    #[test]
    fn disconnect_keeps_the_screen_and_disables_input_then_reattach_redraws_once() {
        let mut m = model();
        m.attach(session(SessionState::Running), b"kept\r\n", 6);
        m.disconnected();
        assert_eq!(m.phase, Phase::Reconnecting);
        assert!(!m.input_enabled());
        assert_eq!(m.screen_text()[0], "kept");
        // Output while disconnected isn't applied.
        assert!(!m.output(6, b"lost").changed);
        m.attaching();
        assert_eq!(m.phase, Phase::Reconnecting, "stays until the answer");
        // The service's scrollback now holds what happened meanwhile; it is drawn once, not appended.
        m.attach(session(SessionState::Running), b"kept\r\nmore\r\n", 12);
        assert_eq!(m.phase, Phase::Live);
        assert_eq!(&m.screen_text()[..3], ["kept", "more", ""]);
    }

    #[test]
    fn ended_sessions_are_read_only() {
        let mut m = model();
        m.attach(session(SessionState::Running), b"x", 1);
        let mut ended = session(SessionState::Ended);
        ended.end_reason = Some("exit 0".into());
        m.ended(ended);
        assert_eq!(m.phase, Phase::Ended);
        assert!(!m.input_enabled());
        // Another session's end is ignored.
        let mut m2 = model();
        m2.attach(session(SessionState::Running), b"x", 1);
        let mut other = session(SessionState::Ended);
        other.id = "s2".into();
        m2.ended(other);
        assert_eq!(m2.phase, Phase::Live);
        // Attaching to an ended session shows it ended.
        let mut m3 = model();
        m3.attach(session(SessionState::Ended), b"done", 4);
        assert_eq!(m3.phase, Phase::Ended);
    }

    #[test]
    fn replies_are_sent_live_but_not_for_replayed_questions() {
        let mut m = model();
        // A cursor-position question inside the stored output: not answered.
        m.attach(session(SessionState::Running), b"\x1b[6n", 4);
        let fed = m.output(4, b"\x1b[6n");
        assert_eq!(fed.replies, b"\x1b[1;1R");
    }

    #[test]
    fn colours_and_attributes_reach_the_grid() {
        let mut m = model();
        m.attach(
            session(SessionState::Running),
            b"\x1b[31mR\x1b[38;5;208mO\x1b[38;2;217;119;87mT\x1b[0m",
            0,
        );
        assert_eq!(m.cell_fg(0, 0), Color::Named(NamedColor::Red));
        assert_eq!(m.cell_fg(0, 1), Color::Indexed(208));
        assert!(matches!(m.cell_fg(0, 2), Color::Spec(c) if (c.r, c.g, c.b) == (217, 119, 87)));
    }

    #[test]
    fn resize_changes_the_grid() {
        let mut m = model();
        assert!(!m.resize(TerminalSize { cols: 40, rows: 5 }));
        assert!(m.resize(TerminalSize { cols: 80, rows: 24 }));
        assert_eq!(m.term.grid().columns(), 80);
        assert_eq!(m.screen_text().len(), 24);
        assert!(!m.resize(TerminalSize { cols: 1, rows: 24 }), "too small");
    }

    #[test]
    fn wide_characters_take_two_cells() {
        let mut m = model();
        m.attach(session(SessionState::Running), "a界b".as_bytes(), 0);
        let row = &m.term.grid()[Line(0)];
        assert_eq!(row[Column(1)].c, '界');
        assert_eq!(row[Column(3)].c, 'b');
    }
}
