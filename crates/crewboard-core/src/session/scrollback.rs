//! A session's stored output, so a client can redraw its terminal after (re)connecting (SVC-05, d0022).
//!
//! It lives only in the service's memory: Crewboard never copies terminal output to disk (d0045).
//! Every byte has an offset in the session's whole output, so a client can tell replayed bytes from live
//! ones.

use std::collections::VecDeque;

/// How much output a session keeps (implementation choice, SDD §26): 2 MiB, about a long Claude Code
/// conversation's worth of screen updates.
pub const DEFAULT_LIMIT: usize = 2 * 1024 * 1024;

/// When trimming, the start moves on to the next line break if one comes within this many bytes, so a replay
/// starts at the beginning of a line rather than in the middle of an escape sequence.
const LINE_SEARCH: usize = 1024;

#[derive(Debug, Clone)]
pub struct Scrollback {
    buf: VecDeque<u8>,
    limit: usize,
    /// The offset of the byte after the last one ever written.
    end: u64,
}

impl Scrollback {
    pub fn new(limit: usize) -> Self {
        Self {
            buf: VecDeque::new(),
            limit: limit.max(1),
            end: 0,
        }
    }

    /// Appends output; returns the offset of its first byte.
    pub fn push(&mut self, data: &[u8]) -> u64 {
        let offset = self.end;
        self.buf.extend(data);
        self.end += data.len() as u64;
        if self.buf.len() > self.limit {
            let mut drop = self.buf.len() - self.limit;
            if let Some(newline) = self
                .buf
                .range(drop..)
                .take(LINE_SEARCH)
                .position(|b| *b == b'\n')
            {
                drop += newline + 1;
            }
            self.buf.drain(..drop.min(self.buf.len()));
        }
        offset
    }

    /// The kept bytes and the offset of the first one.
    pub fn contents(&self) -> (Vec<u8>, u64) {
        let start = self.end - self.buf.len() as u64;
        (self.buf.iter().copied().collect(), start)
    }

    /// The offset of the byte after the last one written.
    pub fn end(&self) -> u64 {
        self.end
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_everything_under_the_limit() {
        let mut s = Scrollback::new(100);
        assert_eq!(s.push(b"hello "), 0);
        assert_eq!(s.push(b"world"), 6);
        assert_eq!(s.contents(), (b"hello world".to_vec(), 0));
        assert_eq!(s.end(), 11);
    }

    #[test]
    fn trims_the_oldest_bytes_to_a_line_start() {
        let mut s = Scrollback::new(10);
        s.push(b"line one\nline two\n");
        let (bytes, start) = s.contents();
        assert!(bytes.len() <= 10);
        assert_eq!(bytes, b"line two\n");
        assert_eq!(start, 9);
        assert_eq!(s.end(), 18);
    }

    #[test]
    fn trims_mid_line_when_no_line_break_is_near() {
        let mut s = Scrollback::new(4);
        s.push(b"abcdefgh");
        assert_eq!(s.contents(), (b"efgh".to_vec(), 4));
    }

    #[test]
    fn stays_bounded_over_many_writes() {
        let mut s = Scrollback::new(1000);
        for i in 0..10_000 {
            s.push(format!("output {i}\n").as_bytes());
        }
        assert!(s.len() <= 1000);
        let (bytes, start) = s.contents();
        assert_eq!(start + bytes.len() as u64, s.end());
        assert!(bytes.ends_with(b"output 9999\n"));
    }
}
