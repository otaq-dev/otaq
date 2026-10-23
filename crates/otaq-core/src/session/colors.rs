//! A program's colour questions (d0117 point 4): "what is the text colour?" (OSC 10) and "what is the background?"
//! (OSC 11), which tools ask at start to pick light or dark colours, often before any terminal shows the session.
//!
//! **Who answers.** A terminal that shows the session and answers (the app's xterm.js, or the terminal running
//! `otaq attach`) answers from its theme, as it always did. While none does, the service answers itself with the
//! theme's `term-fg` / `term-bg` taken when the session started, in xterm's form (`ESC ] 11 ; rgb:RRRR/GGGG/BBBB`)
//! ended as the question was (BEL or `ESC \`). The reader thread decides under the output lock that attaches take
//! too ([`super::Sessions::attach_answering`]), so exactly one answer goes back: a terminal that attaches after a
//! question sees it only in its replay, whose answers it drops. Before this the questions of a session no
//! terminal showed went unanswered, and the tool fell back to its default (dark) after its own timeout.

use crate::board::settings::TerminalColors;

/// One question found in a program's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorQuery {
    /// 10: text colour, 11: background.
    pub which: u8,
    /// The question's terminator, which the answer ends with: BEL, or `ESC \` (ST).
    pub bel: bool,
}

impl ColorQuery {
    /// The answer in xterm's form, each channel's byte written twice as 16 bits (`fa` → `fafa`), as xterm.js does.
    pub fn answer(self, colors: TerminalColors) -> Vec<u8> {
        let color = if self.which == 10 {
            colors.fg
        } else {
            colors.bg
        };
        let [_, r, g, b] = color.to_be_bytes();
        let end = if self.bel { "\x07" } else { "\x1b\\" };
        format!(
            "\x1b]{};rgb:{r:02x}{r:02x}/{g:02x}{g:02x}/{b:02x}{b:02x}{end}",
            self.which
        )
        .into_bytes()
    }
}

/// The questions it looks for, with either terminator.
const QUERIES: [(&[u8], ColorQuery); 4] = [
    (
        b"\x1b]10;?\x07",
        ColorQuery {
            which: 10,
            bel: true,
        },
    ),
    (
        b"\x1b]10;?\x1b\\",
        ColorQuery {
            which: 10,
            bel: false,
        },
    ),
    (
        b"\x1b]11;?\x07",
        ColorQuery {
            which: 11,
            bel: true,
        },
    ),
    (
        b"\x1b]11;?\x1b\\",
        ColorQuery {
            which: 11,
            bel: false,
        },
    ),
];

/// Finds the questions in a session's output, read chunk by chunk: a question split between two reads is found
/// once the second arrives.
#[derive(Debug, Default)]
pub struct QueryScanner {
    /// The end of the previous chunk that may be the start of a question.
    carry: Vec<u8>,
}

impl QueryScanner {
    /// The questions that end in `chunk`, in order.
    pub fn scan(&mut self, chunk: &[u8]) -> Vec<ColorQuery> {
        let mut data = std::mem::take(&mut self.carry);
        data.extend_from_slice(chunk);
        let mut found = Vec::new();
        let mut at = 0;
        // Where the text not part of a found question starts.
        let mut free = 0;
        while let Some(start) = data[at..].iter().position(|b| *b == 0x1b).map(|i| at + i) {
            match QUERIES
                .iter()
                .find(|(bytes, _)| data[start..].starts_with(bytes))
            {
                Some((bytes, query)) => {
                    found.push(*query);
                    at = start + bytes.len();
                    free = at;
                }
                None => at = start + 1,
            }
        }
        // The longest end that a question could still continue.
        let tail = &data[free..];
        let longest = QUERIES
            .iter()
            .map(|(bytes, _)| bytes.len())
            .max()
            .unwrap_or(0);
        self.carry = (1..longest.min(tail.len() + 1))
            .rev()
            .map(|n| &tail[tail.len() - n..])
            .find(|end| QUERIES.iter().any(|(bytes, _)| bytes.starts_with(end)))
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLORS: TerminalColors = TerminalColors {
        fg: 0x2A2C31,
        bg: 0xFAFAF8,
    };

    #[test]
    fn answers_take_xterms_form_and_the_questions_terminator() {
        let bg = ColorQuery {
            which: 11,
            bel: true,
        };
        assert_eq!(bg.answer(COLORS), b"\x1b]11;rgb:fafa/fafa/f8f8\x07");
        let fg = ColorQuery {
            which: 10,
            bel: false,
        };
        assert_eq!(fg.answer(COLORS), b"\x1b]10;rgb:2a2a/2c2c/3131\x1b\\");
    }

    #[test]
    fn finds_both_questions_with_either_terminator() {
        let mut scan = QueryScanner::default();
        let found = scan.scan(b"hi\x1b]10;?\x1b\\\x1b[0m\x1b]11;?\x07there");
        assert_eq!(
            found,
            [
                ColorQuery {
                    which: 10,
                    bel: false
                },
                ColorQuery {
                    which: 11,
                    bel: true
                },
            ]
        );
        // Setting a colour, another OSC, or a question about a palette colour: not answered.
        assert!(
            scan.scan(b"\x1b]11;#ffffff\x07\x1b]0;title\x07\x1b]4;1;?\x07")
                .is_empty()
        );
    }

    #[test]
    fn a_question_split_between_reads_is_found_once() {
        let mut scan = QueryScanner::default();
        assert!(scan.scan(b"text \x1b]1").is_empty());
        assert!(scan.scan(b"1;?\x1b").is_empty());
        assert_eq!(
            scan.scan(b"\\ more"),
            [ColorQuery {
                which: 11,
                bel: false
            }]
        );
        // Nothing carried over after it.
        assert!(scan.scan(b"\x07").is_empty());
        // Split right after ESC.
        assert!(scan.scan(b"\x1b").is_empty());
        assert_eq!(
            scan.scan(b"]10;?\x07"),
            [ColorQuery {
                which: 10,
                bel: true
            }]
        );
    }
}
