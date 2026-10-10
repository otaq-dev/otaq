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
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};
use crewboard_core::session::{SessionInfo, SessionState, TerminalSize};

/// Lines kept above the screen for scrolling back (the service keeps its own, bounded scrollback too).
pub const HISTORY_LINES: usize = 10_000;

/// The most live output kept while an attach answer is on its way; older chunks go first (a gap then asks for a
/// reattach).
const EARLY_LIMIT: usize = 1 << 20;

/// Live output that arrived before the attach answer: the session, the chunk's offset and its bytes.
type Early = (String, u64, Vec<u8>);

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
    /// Lines of new output that arrived while the view was scrolled up ("142 new lines").
    pub new_lines: usize,
    pub find: Find,
    /// An attach answer is on its way: live output is kept until it comes, since it may start past the stored
    /// output the answer carries.
    awaiting: bool,
    early: Vec<Early>,
    early_bytes: usize,
}

/// One match of find in scrollback: from `start` to `end`, both cells included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub start: Point,
    pub end: Point,
}

impl Match {
    pub fn contains(&self, p: Point) -> bool {
        p >= self.start && p <= self.end
    }
}

/// Find in scrollback (⌘F): the query, its matches top to bottom, and the current one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Find {
    pub open: bool,
    pub query: String,
    pub matches: Vec<Match>,
    pub current: Option<usize>,
    /// Output arrived since the matches were computed.
    pub stale: bool,
}

impl Find {
    /// "3 of 12", or "No matches".
    pub fn label(&self) -> String {
        match (self.current, self.matches.len()) {
            (_, 0) if self.query.is_empty() => String::new(),
            (_, 0) => "No matches".to_owned(),
            (Some(i), n) => format!("{} of {n}", i + 1),
            (None, n) => format!("{n} matches"),
        }
    }
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
            new_lines: 0,
            find: Find::default(),
            awaiting: false,
            early: Vec::new(),
            early_bytes: 0,
        }
    }

    /// The stored output arrived: a fresh grid with the scrollback replayed, then the session's live output that
    /// came before this answer. `end` is the offset after the scrollback.
    pub fn attach(&mut self, session: SessionInfo, scrollback: &[u8], end: u64) -> Fed {
        self.replies = Replies::default();
        self.term = new_term(self.size, self.replies.clone());
        self.parser = Processor::new();
        self.parser.advance(&mut self.term, scrollback);
        // Questions in the replayed output were answered long ago.
        self.replies.take();
        self.next_offset = end;
        self.needs_reattach = false;
        self.new_lines = 0;
        if self.find.open {
            self.find.stale = true;
        }
        self.phase = if session.state == SessionState::Running {
            Phase::Live
        } else {
            Phase::Ended
        };
        self.session = Some(session);
        self.awaiting = false;
        self.early_bytes = 0;
        let id = self.session.as_ref().map(|s| s.id.clone());
        let mut fed = Fed::default();
        for (chunk_session, offset, bytes) in std::mem::take(&mut self.early) {
            if Some(&chunk_session) == id.as_ref() {
                let more = self.output(offset, &bytes);
                fed.changed |= more.changed;
                fed.replies.extend(more.replies);
            }
        }
        fed
    }

    /// A `session_output` event, for any session: kept while an attach answer is on its way, else applied when
    /// it is this terminal's session.
    pub fn receive(&mut self, session: &str, offset: u64, bytes: &[u8]) -> Fed {
        if self.awaiting {
            self.early_bytes += bytes.len();
            self.early
                .push((session.to_owned(), offset, bytes.to_vec()));
            while self.early_bytes > EARLY_LIMIT && !self.early.is_empty() {
                let (_, _, dropped) = self.early.remove(0);
                self.early_bytes -= dropped.len();
            }
            return Fed::default();
        }
        if self.session.as_ref().is_some_and(|s| s.id == session) {
            self.output(offset, bytes)
        } else {
            Fed::default()
        }
    }

    fn forget_early(&mut self) {
        self.awaiting = false;
        self.early.clear();
        self.early_bytes = 0;
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
        let scrolled = self.term.grid().display_offset();
        self.parser.advance(&mut self.term, &bytes[skip..]);
        self.next_offset = offset + bytes.len() as u64;
        // Scrolled up, the view stays where it is (the grid moves the offset with the output); count what
        // came in.
        let now = self.term.grid().display_offset();
        if scrolled > 0 {
            self.new_lines += now.saturating_sub(scrolled);
        }
        if self.find.open {
            self.find.stale = true;
        }
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
        self.forget_early();
        if matches!(self.phase, Phase::Live | Phase::Attaching) {
            self.phase = Phase::Reconnecting;
        }
    }

    /// Waiting for an attach answer (after connecting or reconnecting).
    pub fn attaching(&mut self) {
        self.awaiting = true;
        if self.phase != Phase::Reconnecting {
            self.phase = Phase::Attaching;
        }
    }

    /// The ticket has no session to show.
    pub fn no_session(&mut self) {
        self.forget_early();
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

    /// The text on the cursor's line (the program's input line), trimmed; `None` when empty.
    pub fn cursor_line_text(&self) -> Option<String> {
        let grid = self.term.grid();
        let line = grid.cursor.point.line;
        let row = &grid[line];
        let text: String = (0..grid.columns()).map(|c| row[Column(c)].c).collect();
        let text = text.trim().to_owned();
        (!text.is_empty()).then_some(text)
    }

    /// Scrolls the view through the scrollback by `lines` (positive = up, into the past).
    pub fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
        if self.scrolled_up() == 0 {
            self.new_lines = 0;
        }
    }

    /// Back to the newest output ("jump to latest").
    pub fn jump_to_latest(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
        self.new_lines = 0;
    }

    /// How many lines the view is scrolled up from the newest output.
    pub fn scrolled_up(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// The first visible line's number and the total, for "… line 818 of 2,140".
    pub fn position(&self) -> (usize, usize) {
        let grid = self.term.grid();
        let total = grid.total_lines();
        (grid.history_size() - grid.display_offset() + 1, total)
    }

    /// Starts a selection at a grid point: one click selects cells, two a word, three a line.
    pub fn select_start(&mut self, point: Point, side: Side, clicks: usize) {
        let ty = match clicks {
            0 | 1 => SelectionType::Simple,
            2 => SelectionType::Semantic,
            _ => SelectionType::Lines,
        };
        self.term.selection = Some(Selection::new(ty, point, side));
    }

    /// Extends the selection while dragging.
    pub fn select_to(&mut self, point: Point, side: Side) {
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, side);
        }
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    /// The selected text, if anything is selected.
    pub fn selected_text(&self) -> Option<String> {
        self.term.selection_to_string().filter(|s| !s.is_empty())
    }

    /// Opens find (⌘F), keeping the last query.
    pub fn open_find(&mut self) {
        self.find.open = true;
        let query = self.find.query.clone();
        self.set_query(&query);
    }

    pub fn close_find(&mut self) {
        self.find.open = false;
        self.find.matches.clear();
        self.find.current = None;
    }

    /// Searches the whole scrollback for `query` (case-insensitive, across wrapped lines) and selects the newest
    /// match.
    pub fn set_query(&mut self, query: &str) {
        self.find.query = query.to_owned();
        self.find.matches = self.search(query);
        self.find.stale = false;
        self.find.current = self.find.matches.len().checked_sub(1);
        self.reveal_current();
    }

    /// Recomputes the matches after new output, keeping the current one where possible.
    pub fn refresh_find(&mut self) {
        if !self.find.open || !self.find.stale {
            return;
        }
        let current = self
            .find
            .current
            .and_then(|i| self.find.matches.get(i).copied());
        self.find.matches = self.search(&self.find.query.clone());
        self.find.stale = false;
        self.find.current = match current {
            Some(m) => self
                .find
                .matches
                .iter()
                .position(|n| n.start >= m.start)
                .or(self.find.matches.len().checked_sub(1)),
            None => self.find.matches.len().checked_sub(1),
        };
    }

    /// ↵: the next match up (older output); ⇧↵: the next one down. Wraps around.
    pub fn step_find(&mut self, older: bool) {
        self.refresh_find();
        let n = self.find.matches.len();
        if n == 0 {
            return;
        }
        self.find.current = Some(match self.find.current {
            None => n - 1,
            Some(i) if older => (i + n - 1) % n,
            Some(i) => (i + 1) % n,
        });
        self.reveal_current();
    }

    /// Scrolls so the current match is on screen.
    fn reveal_current(&mut self) {
        let Some(m) = self
            .find
            .current
            .and_then(|i| self.find.matches.get(i).copied())
        else {
            return;
        };
        let grid = self.term.grid();
        let rows = grid.screen_lines() as i32;
        let offset = grid.display_offset() as i32;
        let (top, bottom) = (-offset, rows - 1 - offset);
        let line = m.start.line.0;
        if line < top || line > bottom {
            // Put it in the middle of the screen.
            let wanted = (rows / 2 - line).clamp(0, grid.history_size() as i32);
            self.term.scroll_display(Scroll::Delta(wanted - offset));
        }
    }

    fn search(&self, query: &str) -> Vec<Match> {
        let needle: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
        if needle.is_empty() {
            return Vec::new();
        }
        let grid = self.term.grid();
        let columns = grid.columns();
        let mut matches = Vec::new();
        // One logical line at a time: rows joined while a row wraps into the next.
        let mut chars: Vec<char> = Vec::new();
        let mut cells: Vec<Point> = Vec::new();
        let first = -(grid.history_size() as i32);
        let last = grid.screen_lines() as i32 - 1;
        for line in first..=last {
            let row = &grid[Line(line)];
            for column in 0..columns {
                let cell = &row[Column(column)];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                for c in cell.c.to_lowercase() {
                    chars.push(c);
                    cells.push(Point::new(Line(line), Column(column)));
                }
            }
            let wraps = row[Column(columns - 1)].flags.contains(Flags::WRAPLINE);
            if !wraps || line == last {
                let mut i = 0;
                while i + needle.len() <= chars.len() {
                    if chars[i..i + needle.len()] == needle[..] {
                        matches.push(Match {
                            start: cells[i],
                            end: cells[i + needle.len() - 1],
                        });
                        i += needle.len();
                    } else {
                        i += 1;
                    }
                }
                chars.clear();
                cells.clear();
            }
        }
        matches
    }

    /// The visible screen as text, one string per line (trailing spaces trimmed).
    #[cfg(test)]
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

    /// The colour of the cell at `line`, `column` of the screen.
    #[cfg(test)]
    pub fn cell_fg(&self, line: usize, column: usize) -> alacritty_terminal::vte::ansi::Color {
        self.term.grid()[Point::new(Line(line as i32), Column(column))].fg
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
    fn live_output_that_arrives_before_the_attach_answer_is_kept() {
        let mut m = model();
        m.attaching();
        // The first attach of a new shell: the prompt comes as live output before the (empty) stored output.
        assert!(!m.receive("s1", 0, b"$ ").changed);
        // Another terminal's session isn't drawn here.
        m.receive("other", 0, b"elsewhere");
        let fed = m.attach(session(SessionState::Running), b"", 0);
        assert!(fed.changed);
        assert_eq!(m.screen_text()[0], "$");
        assert!(!m.needs_reattach);
        // What the stored output already holds is skipped; output after it is applied.
        let mut m = model();
        m.attaching();
        m.receive("s1", 0, b"ab");
        m.receive("s1", 2, b"cd");
        m.attach(session(SessionState::Running), b"abc", 3);
        assert_eq!(m.screen_text()[0], "abcd");
        // Once live, output follows as before.
        assert!(m.receive("s1", 4, b"e").changed);
        assert_eq!(m.screen_text()[0], "abcde");
    }

    #[test]
    fn early_output_past_a_lost_chunk_asks_for_a_reattach() {
        let mut m = model();
        m.attaching();
        m.receive("s1", 5, b"late");
        m.attach(session(SessionState::Running), b"abc", 3);
        assert!(m.needs_reattach);
        assert_eq!(m.screen_text()[0], "abc");
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

    fn long(lines: usize) -> Vec<u8> {
        (0..lines)
            .flat_map(|i| format!("line {i}\r\n").into_bytes())
            .collect()
    }

    #[test]
    fn new_output_while_scrolled_up_doesnt_move_the_view() {
        let mut m = model();
        let history = long(50);
        m.attach(
            session(SessionState::Running),
            &history,
            history.len() as u64,
        );
        m.scroll(10);
        assert_eq!(m.scrolled_up(), 10);
        let top_before = m.screen_text()[0].clone();
        let more = long(3);
        m.output(history.len() as u64, &more);
        assert_eq!(m.screen_text()[0], top_before, "anchored");
        assert_eq!(m.new_lines, 3);
        assert_eq!(m.scrolled_up(), 13);
        m.jump_to_latest();
        assert_eq!(m.scrolled_up(), 0);
        assert_eq!(m.new_lines, 0);
        // At the bottom, output just scrolls; nothing to count.
        m.output(history.len() as u64 + more.len() as u64, b"x\r\n");
        assert_eq!(m.new_lines, 0);
        let (first, total) = m.position();
        assert_eq!(total, m.term.grid().total_lines());
        assert!(first > 1);
    }

    #[test]
    fn find_matches_across_wrapped_lines_and_steps() {
        let mut m = TerminalModel::new(TerminalSize { cols: 10, rows: 3 });
        // "plans.ts" split by the 10-column wrap: "Read(src/p" + "lans.ts)".
        let text = b"Read(src/plans.ts)\r\nother\r\nPLANS.TS again\r\nmore\r\nend";
        m.attach(session(SessionState::Running), text, text.len() as u64);
        m.open_find();
        m.set_query("plans.ts");
        assert_eq!(m.find.matches.len(), 2);
        let first = m.find.matches[0];
        assert_ne!(first.start.line, first.end.line, "the first match wraps");
        assert_eq!(m.find.current, Some(1), "starts at the newest");
        assert_eq!(m.find.label(), "2 of 2");
        m.step_find(true);
        assert_eq!(m.find.label(), "1 of 2");
        // The match is scrolled into view.
        let offset = m.scrolled_up() as i32;
        assert!(first.start.line.0 >= -offset && first.start.line.0 < 3 - offset);
        m.step_find(true);
        assert_eq!(m.find.label(), "2 of 2", "wraps around");
        m.step_find(false);
        assert_eq!(m.find.label(), "1 of 2");
        m.set_query("nothing here");
        assert_eq!(m.find.label(), "No matches");
        m.close_find();
        assert!(!m.find.open);
    }

    #[test]
    fn find_follows_new_output() {
        let mut m = model();
        m.attach(session(SessionState::Running), b"a needle\r\n", 10);
        m.open_find();
        m.set_query("needle");
        assert_eq!(m.find.matches.len(), 1);
        m.output(10, b"another needle\r\n");
        assert!(m.find.stale);
        m.refresh_find();
        assert_eq!(m.find.matches.len(), 2);
    }

    #[test]
    fn selection_copies_text() {
        let mut m = model();
        m.attach(session(SessionState::Running), b"hello world", 11);
        m.select_start(Point::new(Line(0), Column(0)), Side::Left, 1);
        m.select_to(Point::new(Line(0), Column(4)), Side::Right);
        assert_eq!(m.selected_text().as_deref(), Some("hello"));
        m.select_start(Point::new(Line(0), Column(7)), Side::Left, 2);
        assert_eq!(
            m.selected_text().as_deref(),
            Some("world"),
            "double click: a word"
        );
        m.select_start(Point::new(Line(0), Column(2)), Side::Left, 3);
        assert_eq!(
            m.selected_text().as_deref().map(str::trim_end),
            Some("hello world"),
            "triple click: the line"
        );
        m.clear_selection();
        assert_eq!(m.selected_text(), None);
    }

    #[test]
    fn the_input_line_is_readable() {
        let mut m = model();
        m.attach(session(SessionState::Running), b"output\r\n> half typed", 0);
        assert_eq!(m.cursor_line_text().as_deref(), Some("> half typed"));
        m.output(0, b"");
        let mut empty = model();
        empty.attach(session(SessionState::Running), b"x\r\n", 0);
        assert_eq!(empty.cursor_line_text(), None);
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
