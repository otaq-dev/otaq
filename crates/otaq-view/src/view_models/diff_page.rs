//! A diff's hunks as the Files tab draws them (T-0225, T-0468; REV-01, REV-03, FTB-04, FTB-05; d0017, d0075): each
//! file's lines highlighted, the side-by-side rows, and the anchor a line comment takes. All changes and a file's
//! Diff draw them (`files.diff`), and so does the Approve dialog's "changed since you reviewed it" (REV-06).
//!
//! **Highlighting** is Rust's: each line arrives cut into spans by `highlight.rs` (`syntect`), each span named by its
//! [`Token`]; the page colours tokens with the theme's CSS variables, so a theme switch recolours the diff live
//! without asking again.

use otaq_core::board::comments::Side;
use otaq_core::git::diff::{DiffLine, FileDiff, FileStatus, LineKind};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::highlight::{self, Token};
use crate::view_models::comments::Anchor;
use crate::view_models::diff as vm;

/// Which side of the diff a line number (and a comment) is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "DiffSide")]
pub enum DiffSide {
    Old,
    New,
}

impl From<Side> for DiffSide {
    fn from(side: Side) -> Self {
        match side {
            Side::Old => Self::Old,
            Side::New => Self::New,
        }
    }
}

impl From<DiffSide> for Side {
    fn from(side: DiffSide) -> Self {
        match side {
            DiffSide::Old => Self::Old,
            DiffSide::New => Self::New,
        }
    }
}

/// Where a comment sits; `key` names the thread on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "DiffAnchor")]
pub struct AnchorView {
    pub file: String,
    pub line: u32,
    pub side: DiffSide,
    pub key: String,
}

impl AnchorView {
    pub fn of(anchor: &Anchor) -> Self {
        Self::new(&anchor.file, anchor.line, anchor.side.into())
    }

    pub fn new(file: &str, line: u32, side: DiffSide) -> Self {
        let tag = match side {
            DiffSide::Old => "old",
            DiffSide::New => "new",
        };
        Self {
            file: file.to_owned(),
            line,
            side,
            key: format!("{file}\u{0}{tag}\u{0}{line}"),
        }
    }
}

/// How a file changed, for its letter's colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "DiffFileStatus")]
pub enum StatusView {
    Added,
    Modified,
    Deleted,
    Renamed,
}

impl From<FileStatus> for StatusView {
    fn from(status: FileStatus) -> Self {
        match status {
            FileStatus::Added => Self::Added,
            FileStatus::Modified => Self::Modified,
            FileStatus::Deleted => Self::Deleted,
            FileStatus::Renamed => Self::Renamed,
        }
    }
}

/// What a span of code is, for its colour (`highlight::Token`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "CodeToken")]
pub enum TokenView {
    Plain,
    Keyword,
    String,
    Number,
    Comment,
    Function,
    Type,
}

impl From<Token> for TokenView {
    fn from(token: Token) -> Self {
        match token {
            Token::Plain => Self::Plain,
            Token::Keyword => Self::Keyword,
            Token::String => Self::String,
            Token::Number => Self::Number,
            Token::Comment => Self::Comment,
            Token::Function => Self::Function,
            Token::Type => Self::Type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CodeSpan")]
pub struct SpanView {
    pub text: String,
    pub token: TokenView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "DiffLineKind")]
pub enum LineKindView {
    Context,
    Added,
    Removed,
}

/// One line of a hunk, highlighted, with the anchor a comment on it takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DiffLineView")]
pub struct LineView {
    pub kind: LineKindView,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub sign: &'static str,
    pub anchor: Option<AnchorView>,
    pub spans: Vec<SpanView>,
}

/// A hunk: its header, its lines, and the side-by-side rows as indexes into the lines (`null`: a blank side).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DiffHunkView")]
pub struct HunkView {
    pub header: String,
    pub lines: Vec<LineView>,
    pub rows: Vec<(Option<u32>, Option<u32>)>,
}

/// One file's lines (`diff.file`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DiffFileView")]
pub struct FileView {
    pub path: String,
    pub hunks: Vec<HunkView>,
    /// "This file's diff is too long to show in full."
    pub truncated: Option<&'static str>,
}

fn line_view(path: &str, line: &DiffLine, spans: Option<&highlight::Spans>) -> LineView {
    let (kind, sign) = match line.kind {
        LineKind::Added => (LineKindView::Added, "+"),
        LineKind::Removed => (LineKindView::Removed, "-"),
        LineKind::Context => (LineKindView::Context, " "),
    };
    let text = &line.text;
    let spans = match spans {
        Some(spans) => spans
            .iter()
            .filter_map(|(range, token)| {
                Some(SpanView {
                    text: text.get(range.clone())?.to_owned(),
                    token: (*token).into(),
                })
            })
            .collect(),
        None => vec![SpanView {
            text: text.clone(),
            token: TokenView::Plain,
        }],
    };
    LineView {
        kind,
        old: line.old,
        new: line.new,
        sign,
        anchor: Anchor::of(path, line).map(|a| AnchorView::of(&a)),
        spans,
    }
}

/// A loaded file: every hunk highlighted as one run of lines.
pub fn file_view(diff: &FileDiff) -> FileView {
    let path = &diff.file.path;
    let hunks = diff
        .hunks
        .iter()
        .map(|hunk| {
            let texts: Vec<&str> = hunk.lines.iter().map(|l| l.text.as_str()).collect();
            let spans = highlight::highlight(path, &texts);
            HunkView {
                header: format!(
                    "@@ -{},{} +{},{} @@ {}",
                    hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines, hunk.header
                )
                .trim_end()
                .to_owned(),
                lines: hunk
                    .lines
                    .iter()
                    .enumerate()
                    .map(|(i, l)| line_view(path, l, spans.get(i)))
                    .collect(),
                rows: vm::side_by_side(hunk)
                    .into_iter()
                    .map(|r| {
                        (
                            r.left.map(|(i, _)| i as u32),
                            r.right.map(|(i, _)| i as u32),
                        )
                    })
                    .collect(),
            }
        })
        .collect();
    FileView {
        path: path.clone(),
        hunks,
        truncated: diff
            .truncated
            .then_some("This file\u{2019}s diff is too long to show in full."),
    }
}

#[cfg(test)]
mod tests {
    use otaq_core::git::diff::{ChangedFile, Hunk};

    use super::*;

    fn file(path: &str, status: FileStatus, added: u32, removed: u32) -> ChangedFile {
        ChangedFile {
            status,
            path: path.into(),
            old_path: None,
            added,
            removed,
            binary: false,
            size: None,
            old_size: None,
            similarity: None,
        }
    }

    fn line(kind: LineKind, old: Option<u32>, new: Option<u32>, text: &str) -> DiffLine {
        DiffLine {
            kind,
            old,
            new,
            text: text.into(),
            no_newline: false,
        }
    }

    #[test]
    fn lines_arrive_highlighted_with_anchors_and_side_rows() {
        use LineKind::*;
        let diff = FileDiff {
            file: file("src/lib/cart/validate.ts", FileStatus::Modified, 2, 1),
            hunks: vec![Hunk {
                old_start: 12,
                old_lines: 2,
                new_start: 12,
                new_lines: 3,
                header: "export function validateCart".into(),
                lines: vec![
                    line(
                        Context,
                        Some(12),
                        Some(12),
                        "export function validateCart(cart) {",
                    ),
                    line(Removed, Some(13), None, "  if (empty) throw new Error()"),
                    line(Added, None, Some(13), "  if (empty) {"),
                    line(Added, None, Some(14), "    return { ok: false }"),
                ],
            }],
            truncated: false,
        };
        let view = file_view(&diff);
        let hunk = &view.hunks[0];
        assert_eq!(
            hunk.header,
            "@@ -12,2 +12,3 @@ export function validateCart"
        );
        assert_eq!(
            hunk.rows,
            [(Some(0), Some(0)), (Some(1), Some(2)), (None, Some(3))]
        );
        let removed = &hunk.lines[1];
        assert_eq!((removed.sign, removed.kind), ("-", LineKindView::Removed));
        let anchor = removed.anchor.as_ref().unwrap();
        assert_eq!((anchor.line, anchor.side), (13, DiffSide::Old));
        let first = &hunk.lines[0];
        let text: String = first.spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, "export function validateCart(cart) {");
        assert!(
            first
                .spans
                .iter()
                .any(|s| s.text == "export" && s.token == TokenView::Keyword),
            "{:?}",
            first.spans
        );
        assert_eq!(view.truncated, None);
    }
}
