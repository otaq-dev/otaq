//! The text and selection behind a text input, as plain Rust so editing is unit-tested. Offsets are UTF-8 byte
//! offsets on grapheme boundaries. A single-line buffer never holds a line break: pasted text is joined with
//! spaces.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditBuffer {
    text: String,
    selection: Range<usize>,
    /// The cursor sits at the start of the selection (it was made leftwards).
    reversed: bool,
    /// Text being composed by an input method (dead keys, CJK).
    marked: Option<Range<usize>>,
    multiline: bool,
}

impl EditBuffer {
    pub fn new(multiline: bool) -> Self {
        Self {
            multiline,
            ..Self::default()
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    /// Replaces the whole text and puts the cursor at its end.
    pub fn set_text(&mut self, text: &str) {
        self.text = self.clean(text);
        self.selection = self.text.len()..self.text.len();
        self.reversed = false;
        self.marked = None;
    }

    pub fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }

    pub fn is_reversed(&self) -> bool {
        self.reversed
    }

    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    pub fn cursor(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.selection.clone()]
    }

    /// Moves the cursor, extending the selection when `select`.
    pub fn move_to(&mut self, offset: usize, select: bool) {
        let offset = self.clamp(offset);
        if select {
            if self.reversed {
                self.selection.start = offset;
            } else {
                self.selection.end = offset;
            }
            if self.selection.end < self.selection.start {
                self.reversed = !self.reversed;
                self.selection = self.selection.end..self.selection.start;
            }
        } else {
            self.selection = offset..offset;
            self.reversed = false;
        }
    }

    pub fn left(&mut self, select: bool) {
        if !select && !self.selection.is_empty() {
            self.move_to(self.selection.start, false);
        } else {
            self.move_to(self.previous_boundary(self.cursor()), select);
        }
    }

    pub fn right(&mut self, select: bool) {
        if !select && !self.selection.is_empty() {
            self.move_to(self.selection.end, false);
        } else {
            self.move_to(self.next_boundary(self.cursor()), select);
        }
    }

    /// The start of the current line (of the whole text when single-line).
    pub fn home(&mut self, select: bool) {
        let cursor = self.cursor();
        let start = self.text[..cursor].rfind('\n').map_or(0, |i| i + 1);
        self.move_to(start, select);
    }

    /// The end of the current line.
    pub fn end(&mut self, select: bool) {
        let cursor = self.cursor();
        let end = self.text[cursor..]
            .find('\n')
            .map_or(self.text.len(), |i| cursor + i);
        self.move_to(end, select);
    }

    pub fn select_all(&mut self) {
        self.selection = 0..self.text.len();
        self.reversed = false;
    }

    /// Types or pastes `text` over the selection (or the marked text).
    pub fn insert(&mut self, text: &str) {
        let range = self.marked.take().unwrap_or(self.selection.clone());
        self.replace(range, text);
    }

    /// Replaces `range` with `text` and puts the cursor after it.
    pub fn replace(&mut self, range: Range<usize>, text: &str) {
        let text = self.clean(text);
        let range = self.clamp(range.start)..self.clamp(range.end);
        self.text.replace_range(range.clone(), &text);
        let cursor = range.start + text.len();
        self.selection = cursor..cursor;
        self.reversed = false;
        self.marked = None;
    }

    /// Replaces `range` with composing text, marks it, and selects `selected` (relative to the new text).
    pub fn replace_and_mark(
        &mut self,
        range: Range<usize>,
        text: &str,
        selected: Option<Range<usize>>,
    ) {
        let text = self.clean(text);
        let range = self.clamp(range.start)..self.clamp(range.end);
        self.text.replace_range(range.clone(), &text);
        self.marked = (!text.is_empty()).then(|| range.start..range.start + text.len());
        self.selection = match selected {
            Some(s) => self.clamp(range.start + s.start)..self.clamp(range.start + s.end),
            None => {
                let end = range.start + text.len();
                end..end
            }
        };
        self.reversed = false;
    }

    pub fn unmark(&mut self) {
        self.marked = None;
    }

    /// Deletes the selection, or the grapheme before the cursor. Returns false when there was nothing to delete.
    pub fn backspace(&mut self) -> bool {
        if self.selection.is_empty() {
            let previous = self.previous_boundary(self.cursor());
            if previous == self.cursor() {
                return false;
            }
            self.move_to(previous, true);
        }
        self.insert("");
        true
    }

    /// Deletes the selection, or the grapheme after the cursor.
    pub fn delete(&mut self) -> bool {
        if self.selection.is_empty() {
            let next = self.next_boundary(self.cursor());
            if next == self.cursor() {
                return false;
            }
            self.move_to(next, true);
        }
        self.insert("");
        true
    }

    /// A line break, in a multi-line buffer.
    pub fn newline(&mut self) {
        if self.multiline {
            self.insert("\n");
        }
    }

    fn clean(&self, text: &str) -> String {
        if self.multiline {
            text.replace("\r\n", "\n")
        } else {
            text.replace("\r\n", " ").replace(['\n', '\r'], " ")
        }
    }

    fn clamp(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .rev()
            .find_map(|(i, _)| (i < offset).then_some(i))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .find_map(|(i, _)| (i > offset).then_some(i))
            .unwrap_or(self.text.len())
    }

    /// UTF-16 offsets, as the platform's input method speaks them.
    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text[..self.clamp(offset)].encode_utf16().count()
    }

    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        for (i, ch) in self.text.char_indices() {
            if utf16 >= offset {
                return i;
            }
            utf16 += ch.len_utf16();
        }
        self.text.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(text: &str) -> EditBuffer {
        let mut b = EditBuffer::new(false);
        b.set_text(text);
        b
    }

    #[test]
    fn typing_and_deleting() {
        let mut b = buffer("Checkout");
        b.insert(" API");
        assert_eq!(b.text(), "Checkout API");
        assert!(b.backspace());
        assert_eq!(b.text(), "Checkout AP");
        b.home(false);
        assert!(!b.backspace(), "nothing before the start");
        assert!(b.delete());
        assert_eq!(b.text(), "heckout AP");
    }

    #[test]
    fn selection_replaces_and_moves() {
        let mut b = buffer("abcdef");
        b.move_to(2, false);
        b.right(true);
        b.right(true);
        assert_eq!(b.selected_text(), "cd");
        b.insert("X");
        assert_eq!(b.text(), "abXef");
        assert_eq!(b.cursor(), 3);
        b.left(true);
        b.left(true);
        assert!(b.is_reversed());
        assert_eq!(b.selected_text(), "bX");
        b.right(false);
        assert_eq!((b.cursor(), b.selection().is_empty()), (3, true));
        b.select_all();
        b.backspace();
        assert_eq!(b.text(), "");
    }

    #[test]
    fn graphemes_move_as_one() {
        let mut b = buffer("né👍🏽");
        b.left(false);
        assert_eq!(b.cursor(), "né".len());
        b.backspace();
        assert_eq!(b.text(), "n👍🏽");
        assert_eq!(b.offset_to_utf16(b.text().len()), 1 + 4);
        assert_eq!(b.offset_from_utf16(1), 1);
    }

    #[test]
    fn single_line_joins_line_breaks() {
        let mut b = buffer("");
        b.insert("one\ntwo\r\nthree");
        assert_eq!(b.text(), "one two three");
        b.newline();
        assert_eq!(b.text(), "one two three");
    }

    #[test]
    fn multi_line_home_end_and_newline() {
        let mut b = EditBuffer::new(true);
        b.set_text("first\nsecond line");
        b.home(false);
        assert_eq!(b.cursor(), 6);
        b.end(true);
        assert_eq!(b.selected_text(), "second line");
        b.move_to(5, false);
        b.newline();
        assert_eq!(b.text(), "first\n\nsecond line");
    }

    #[test]
    fn composing_text_is_marked_then_committed() {
        let mut b = buffer("a");
        b.replace_and_mark(1..1, "´", None);
        assert_eq!(b.marked(), Some(1..3));
        b.insert("é");
        assert_eq!(b.text(), "aé");
        assert_eq!(b.marked(), None);
    }
}
