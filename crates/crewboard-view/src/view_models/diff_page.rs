//! The Diff tab as the Tauri page draws it (T-0225; REV-01, REV-02, REV-03; d0017, d0075): the [`diff`] and
//! [`comments`] view models turned into plain, serialized shapes with every text already worded. The page keeps only
//! the moment of interaction (the filter being typed, folded folders, "Load diff", the open comment box).
//!
//! **Highlighting** is Rust's: each line arrives cut into spans by `highlight.rs` (`syntect`), each span named by its
//! [`Token`]; the page colours tokens with the theme's CSS variables, so a theme switch recolours the diff live
//! without asking again.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crewboard_core::board::comments::{ReviewComment, Side};
use crewboard_core::board::diff::DiffSummary;
use crewboard_core::git::diff::{ChangedFile, DiffLine, FileDiff, FileStatus, LineKind};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::highlight::{self, Token};
use crate::keymap::Platform;
use crate::view_models::card::age_label;
use crate::view_models::comments::{self as cm, Anchor};
use crate::view_models::diff::{self as vm, ReviewMemory};

/// The two views of the diff, in the toolbar's order.
pub const MODES: [&str; 2] = ["Unified", "Side-by-side"];
/// Above a long file list.
pub fn filter_placeholder(files: usize) -> String {
    format!("Filter {files} files")
}

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

/// One file: its row in the list and its section's header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DiffFileRow")]
pub struct FileRow {
    pub path: String,
    /// "lib/money.ts → lib/format/money.ts" for a rename.
    pub label: String,
    pub letter: &'static str,
    pub status: StatusView,
    /// "+184" when lines were added.
    pub added: Option<String>,
    /// "−12" when lines were removed.
    pub removed: Option<String>,
    pub binary: bool,
    /// "new file", "deleted", "renamed", "binary file · 12 KB".
    pub note: Option<String>,
    pub viewed: bool,
    /// Large and generated files wait for "Load diff".
    pub collapsed: Option<String>,
    /// "Binary file changed · 184 KB → 96 KB" in place of lines.
    pub binary_label: Option<String>,
    /// "Renamed with 3 changed lines (98 % similar). Showing only the changes."
    pub rename_label: Option<String>,
    /// About how many rows its lines take, for the room kept while they load.
    pub rows: u32,
    /// In a long list's folder: the label without the folder ("page.tsx", "old/Summary.tsx → Summary.tsx").
    pub in_group: Option<String>,
}

/// A folder of a long file list (*Large diff · 54 files*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DiffGroup")]
pub struct GroupRow {
    pub name: String,
    /// What the folded generated group says: "generated · collapsed".
    pub folded_label: String,
    /// Indexes into the files.
    pub files: Vec<u32>,
    pub starts_folded: bool,
    /// An epic's group of one merged child (EPC-05, *05 → Epic in Review · Diff by child*) instead of a folder.
    pub child: Option<ChildHead>,
}

/// The head of an epic's child group: "SHOP-40.1 Wishlist API · +210 −4 · 6 files".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DiffChildHead")]
pub struct ChildHead {
    pub key: String,
    pub title: String,
    pub counts: String,
}

/// The agent's summary above the diff, foldable per ticket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DiffAgentSummary")]
pub struct AgentSummary {
    pub title: &'static str,
    pub text: String,
    pub note: String,
    pub open: bool,
}

/// *Binary, renamed, empty → No changes*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DiffEmpty")]
pub struct Empty {
    pub title: &'static str,
    pub base: String,
    pub text: &'static str,
}

/// The Diff tab's view: the file list and everything around it; the lines come per file ([`FileView`]).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "DiffTabView")]
pub struct TabView {
    pub key: String,
    pub round: u32,
    /// Changes when the file list changes, so the page drops lines it loaded for an older list.
    pub revision: String,
    /// "main at a1b2c3d"
    pub base: String,
    /// "· 9 files · +412 −38"
    pub rest: String,
    pub base_prefix: &'static str,
    /// "4 / 9 viewed"
    pub viewed: String,
    pub modes: [&'static str; 2],
    pub side_by_side: bool,
    /// Below this panel width Side-by-side shows as Unified.
    pub side_by_side_from: f32,
    pub summary: Option<AgentSummary>,
    pub files: Vec<FileRow>,
    /// Long lists only (from [`vm::GROUP_FROM`] files): the folders, generated files last.
    pub groups: Option<Vec<GroupRow>>,
    pub filter_placeholder: String,
    pub empty: Option<Empty>,
    pub viewed_label: &'static str,
    pub load_diff: &'static str,
}

fn revision(s: &DiffSummary) -> String {
    let mut h = DefaultHasher::new();
    s.base_sha.hash(&mut h);
    s.round.hash(&mut h);
    for f in &s.files {
        (&f.path, f.added, f.removed, f.binary, &f.size).hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

fn count(sign: &str, n: u32) -> Option<String> {
    (n > 0).then(|| format!("{sign}{n}"))
}

pub fn file_row(file: &ChangedFile, viewed: bool) -> FileRow {
    FileRow {
        path: file.path.clone(),
        label: vm::file_label(file),
        letter: vm::status_letter(file.status),
        status: file.status.into(),
        added: count("+", file.added),
        removed: count("\u{2212}", file.removed),
        binary: file.binary,
        note: vm::file_note(file),
        viewed,
        collapsed: vm::starts_collapsed(file).then(|| {
            if vm::is_generated(&file.path) {
                "Generated file: collapsed.".to_owned()
            } else {
                "Large change: collapsed.".to_owned()
            }
        }),
        binary_label: file.binary.then(|| vm::binary_label(file)),
        rename_label: vm::rename_label(file),
        rows: (file.added + file.removed).clamp(1, vm::LARGE_FILE_LINES) + 4,
        in_group: None,
    }
}

/// A file's label inside its folder: each side loses the folder it starts with.
pub fn label_in(file: &ChangedFile, folder: &str) -> String {
    let short = |path: &str| path.strip_prefix(folder).unwrap_or(path).to_owned();
    match &file.old_path {
        Some(old) => format!("{} \u{2192} {}", short(old), short(&file.path)),
        None => short(&file.path),
    }
}

/// The tab for a loaded file list.
pub fn tab_view(summary: &DiffSummary, memory: &ReviewMemory) -> TabView {
    let key = &summary.key;
    let round = summary.round;
    let (base, rest) = vm::base_label(summary);
    let mut files: Vec<FileRow> = summary
        .files
        .iter()
        .map(|f| file_row(f, memory.is_viewed(key, round, &f.path)))
        .collect();
    let groups = (summary.files.len() >= vm::GROUP_FROM).then(|| {
        vm::file_groups(&summary.files, "")
            .into_iter()
            .inspect(|g| {
                if !g.generated {
                    for &i in &g.files {
                        files[i].in_group = Some(label_in(&summary.files[i], &g.name));
                    }
                }
            })
            .map(|g| GroupRow {
                folded_label: if g.generated {
                    format!("{} \u{b7} collapsed", g.name)
                } else {
                    g.name.clone()
                },
                name: g.name,
                files: g.files.into_iter().map(|i| i as u32).collect(),
                starts_folded: g.generated,
                child: None,
            })
            .collect()
    });
    // An epic's diff groups its files by the child that brought them (EPC-05), whatever the list's length.
    let groups = if summary.children.is_empty() {
        groups
    } else {
        for f in &mut files {
            f.in_group = None;
        }
        Some(child_groups(summary))
    };
    let rest = if summary.children.is_empty() {
        rest
    } else {
        epic_rest(summary)
    };
    TabView {
        key: key.to_string(),
        round,
        revision: revision(summary),
        base,
        rest,
        base_prefix: "diff vs",
        viewed: vm::viewed_label(
            memory.viewed_count(key, round, &summary.files),
            summary.files.len(),
        ),
        modes: MODES,
        side_by_side: memory.side_by_side,
        side_by_side_from: vm::SIDE_BY_SIDE_FROM,
        summary: summary.review_summary.as_ref().map(|text| AgentSummary {
            title: "Agent summary",
            text: text.clone(),
            note: format!(
                "no tests are run by {}",
                crewboard_core::naming::PRODUCT_NAME
            ),
            open: memory.summary_open(key),
        }),
        filter_placeholder: filter_placeholder(summary.files.len()),
        empty: summary.files.is_empty().then(|| Empty {
            title: vm::EMPTY_TITLE,
            base: format!("vs {} at {}", summary.base_branch, summary.base_short),
            text: vm::EMPTY_TEXT,
        }),
        files,
        groups,
        viewed_label: "Viewed",
        load_diff: "Load diff",
    }
}

/// One group per merged child, in the order they merged; a file two children touched shows under both.
fn child_groups(summary: &DiffSummary) -> Vec<GroupRow> {
    summary
        .children
        .iter()
        .map(|child| GroupRow {
            name: child.key.to_string(),
            folded_label: child.key.to_string(),
            files: child
                .files
                .iter()
                .filter_map(|f| summary.files.iter().position(|s| s.path == f.path))
                .map(|i| i as u32)
                .collect(),
            starts_folded: false,
            child: Some(ChildHead {
                key: child.key.to_string(),
                title: child.title.clone(),
                counts: crate::view_models::epic_review::child_label(child),
            }),
        })
        .collect()
}

/// "· 3 children · 21 files" after "main at 7e4f9a0" (*Epic in Review · Diff by child*).
fn epic_rest(summary: &DiffSummary) -> String {
    let label = crate::view_models::epic_review::diff_label(summary);
    let at = format!(" at {} ", summary.base_short);
    label
        .split_once(&at)
        .map_or(label.clone(), |(_, rest)| rest.to_owned())
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

/// One comment as its card shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DiffComment")]
pub struct CommentView {
    pub id: String,
    pub author: &'static str,
    pub initial: &'static str,
    /// "2m ago"
    pub age: String,
    /// "pending", or "sent · round 1".
    pub badge: String,
    pub pending: bool,
    /// Pending comments on a ticket in review can be edited and deleted.
    pub editable: bool,
    pub body: String,
}

/// The comments on one anchor, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DiffThread")]
pub struct ThreadView {
    pub anchor: AnchorView,
    pub comments: Vec<CommentView>,
}

/// The comment box's wording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DiffComposerTexts")]
pub struct ComposerTexts {
    pub placeholder: &'static str,
    pub hint: &'static str,
    pub add: &'static str,
    pub save: &'static str,
    pub cancel: &'static str,
    pub reply: &'static str,
    pub edit: &'static str,
    pub delete: &'static str,
    pub add_on_line: &'static str,
}

pub const COMPOSER: ComposerTexts = ComposerTexts {
    placeholder: "Leave a comment",
    hint: "Markdown \u{b7} comments are sent to the agent only on Send back",
    add: "Add comment",
    save: "Save",
    cancel: "Cancel",
    reply: "Reply",
    edit: "Edit comment",
    delete: "Delete comment",
    add_on_line: "Comment on this line",
};

/// A ticket's line comments (`diff.comments`), with the pending counter Send back uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DiffComments")]
pub struct CommentsView {
    pub key: String,
    /// Comments can be added (Ready for Review, not an epic).
    pub can_comment: bool,
    pub pending: u32,
    /// "2 pending comments", when there are some.
    pub pending_label: Option<String>,
    /// "Send back (2)".
    pub send_back_label: String,
    pub threads: Vec<ThreadView>,
    pub texts: ComposerTexts,
    /// What saves the comment box: "⌘↵", "Ctrl+↵".
    pub save_keys: &'static str,
}

/// The comments, grouped into threads in the order their first comment was written.
pub fn comments_view(
    key: &str,
    comments: &[ReviewComment],
    can_comment: bool,
    now: i64,
) -> CommentsView {
    let mut threads: Vec<ThreadView> = Vec::new();
    for c in comments {
        let view = CommentView {
            id: c.id.clone(),
            author: "You",
            initial: "Y",
            age: format!("{} ago", age_label(now - c.created_at)),
            badge: cm::badge(c),
            pending: c.is_pending(),
            editable: can_comment && cm::editable(c),
            body: c.body.clone(),
        };
        let anchor = AnchorView::new(&c.file, c.line, c.side.into());
        match threads.iter_mut().find(|t| t.anchor == anchor) {
            Some(thread) => thread.comments.push(view),
            None => threads.push(ThreadView {
                anchor,
                comments: vec![view],
            }),
        }
    }
    let pending = cm::pending(comments);
    CommentsView {
        key: key.to_owned(),
        can_comment,
        pending: pending as u32,
        pending_label: (pending > 0).then(|| cm::pending_label(pending)),
        send_back_label: cm::send_back_label(pending),
        threads,
        texts: COMPOSER,
        save_keys: match Platform::current() {
            Platform::Mac => "\u{2318}\u{21b5}",
            _ => "Ctrl+\u{21b5}",
        },
    }
}

#[cfg(test)]
mod tests {
    use crewboard_core::domain::TicketKey;
    use crewboard_core::git::diff::Hunk;

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

    fn summary(files: Vec<ChangedFile>) -> DiffSummary {
        DiffSummary {
            key: "SHOP-18".parse().unwrap(),
            base_branch: "main".into(),
            base_sha: "a1b2c3d4e5".into(),
            base_short: "a1b2c3d".into(),
            branch: crewboard_core::naming::branch("SHOP-18"),
            added: files.iter().map(|f| f.added).sum(),
            removed: files.iter().map(|f| f.removed).sum(),
            files,
            round: 2,
            review_summary: Some("Added POST /api/checkout.".into()),
            children: vec![],
        }
    }

    #[test]
    fn an_epic_groups_its_files_by_child() {
        use crewboard_core::board::diff::ChildDiff;
        let api = file("src/app/api/wishlist/route.ts", FileStatus::Added, 88, 0);
        let store = file("src/lib/wishlist/store.ts", FileStatus::Added, 64, 0);
        let header = file("src/components/Header.tsx", FileStatus::Modified, 6, 2);
        let mut s = summary(vec![api.clone(), store.clone(), header.clone()]);
        s.key = "SHOP-40".parse().unwrap();
        s.base_short = "7e4f9a0".into();
        s.children = vec![
            ChildDiff {
                key: "SHOP-40.1".parse().unwrap(),
                title: "Wishlist API".into(),
                commit: "c1".into(),
                files: vec![api, store],
                added: 152,
                removed: 0,
            },
            ChildDiff {
                key: "SHOP-40.3".parse().unwrap(),
                title: "Wishlist page".into(),
                commit: "c3".into(),
                files: vec![header],
                added: 6,
                removed: 2,
            },
        ];
        let tab = tab_view(&s, &ReviewMemory::default());
        assert_eq!(tab.base, "main at 7e4f9a0");
        assert_eq!(tab.rest, "\u{b7} 2 children \u{b7} 3 files");
        let groups = tab.groups.unwrap();
        assert_eq!(
            groups.len(),
            2,
            "a short epic list is still grouped by child"
        );
        assert_eq!(groups[0].files, [0, 1]);
        assert_eq!(groups[1].files, [2]);
        assert!(!groups[1].starts_folded);
        assert_eq!(
            groups[0].child,
            Some(ChildHead {
                key: "SHOP-40.1".into(),
                title: "Wishlist API".into(),
                counts: "+152 \u{2212}0 \u{b7} 2 files".into(),
            })
        );
        assert!(
            tab.files.iter().all(|f| f.in_group.is_none()),
            "full paths, as the frame"
        );
    }

    #[test]
    fn the_tab_reads_like_the_frame() {
        let key: TicketKey = "SHOP-18".parse().unwrap();
        let mut memory = ReviewMemory::default();
        memory.toggle_viewed(&key, 2, "api/checkout/route.ts");
        let mut png = file("public/hero.png", FileStatus::Modified, 0, 0);
        png.binary = true;
        png.old_size = Some(184 * 1024);
        png.size = Some(96 * 1024);
        let s = summary(vec![
            file("api/checkout/route.ts", FileStatus::Added, 184, 0),
            file("lib/stripe/session.ts", FileStatus::Modified, 96, 12),
            file("package-lock.json", FileStatus::Modified, 40, 2),
            png,
        ]);
        let tab = tab_view(&s, &memory);
        assert_eq!(tab.base, "main at a1b2c3d");
        assert_eq!(tab.rest, "· 4 files · +320 \u{2212}14");
        assert_eq!(tab.viewed, "1 / 4 viewed");
        assert_eq!(tab.modes, ["Unified", "Side-by-side"]);
        assert!(tab.groups.is_none(), "short lists aren't grouped");
        let summary = tab.summary.unwrap();
        assert!(summary.open);
        assert!(summary.note.starts_with("no tests are run by "));
        let route = &tab.files[0];
        assert_eq!(
            (
                route.letter,
                route.added.as_deref(),
                route.removed.as_deref()
            ),
            ("A", Some("+184"), None)
        );
        assert_eq!(route.note.as_deref(), Some("new file"));
        assert!(route.viewed);
        assert_eq!(tab.files[1].removed.as_deref(), Some("\u{2212}12"));
        assert_eq!(
            tab.files[2].collapsed.as_deref(),
            Some("Generated file: collapsed.")
        );
        assert_eq!(
            tab.files[3].binary_label.as_deref(),
            Some("Binary file changed \u{b7} 184 KB \u{2192} 96 KB")
        );
        assert!(tab.empty.is_none());
    }

    #[test]
    fn a_long_list_groups_and_an_empty_one_explains_itself() {
        let mut files: Vec<ChangedFile> = (0..20)
            .map(|i| file(&format!("src/lib/f{i}.ts"), FileStatus::Modified, 1, 0))
            .collect();
        files.push(file("yarn.lock", FileStatus::Modified, 900, 0));
        let tab = tab_view(&summary(files), &ReviewMemory::default());
        assert_eq!(tab.filter_placeholder, "Filter 21 files");
        let groups = tab.groups.unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(
            (groups[0].name.as_str(), groups[0].files.len()),
            ("src/lib/", 20)
        );
        assert!(!groups[0].starts_folded);
        assert_eq!(tab.files[0].in_group.as_deref(), Some("f0.ts"));
        assert_eq!(
            tab.files[20].in_group, None,
            "generated files keep their path"
        );
        let mut moved = file("src/app/checkout/Summary.tsx", FileStatus::Renamed, 2, 2);
        moved.old_path = Some("src/app/checkout/old/Summary.tsx".into());
        assert_eq!(
            label_in(&moved, "src/app/checkout/"),
            "old/Summary.tsx \u{2192} Summary.tsx"
        );
        assert_eq!(groups[1].folded_label, "generated \u{b7} collapsed");
        assert!(groups[1].starts_folded);

        let empty = tab_view(&summary(vec![]), &ReviewMemory::default());
        let e = empty.empty.unwrap();
        assert_eq!(
            (e.title, e.base.as_str()),
            ("No changes", "vs main at a1b2c3d")
        );
    }

    #[test]
    fn the_revision_follows_the_file_list() {
        let a = summary(vec![file("a.ts", FileStatus::Added, 1, 0)]);
        let mut b = a.clone();
        assert_eq!(revision(&a), revision(&b));
        b.files[0].added = 2;
        assert_ne!(revision(&a), revision(&b));
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

    fn comment(id: &str, line: u32, side: Side, sent: bool) -> ReviewComment {
        ReviewComment {
            id: id.into(),
            round: 1,
            file: "lib/a.ts".into(),
            line,
            side,
            body: format!("body {id}"),
            created_at: 0,
            sent_at: sent.then_some(5),
        }
    }

    #[test]
    fn comments_make_threads_and_the_pending_counter() {
        let comments = vec![
            comment("1", 12, Side::New, true),
            comment("2", 9, Side::Old, false),
            comment("3", 12, Side::New, false),
        ];
        let view = comments_view("SHOP-18", &comments, true, 120_000);
        assert_eq!(view.pending, 2);
        assert_eq!(view.pending_label.as_deref(), Some("2 pending comments"));
        assert_eq!(view.send_back_label, "Send back (2)");
        let ids: Vec<Vec<&str>> = view
            .threads
            .iter()
            .map(|t| t.comments.iter().map(|c| c.id.as_str()).collect())
            .collect();
        assert_eq!(ids, [vec!["1", "3"], vec!["2"]]);
        let sent = &view.threads[0].comments[0];
        assert_eq!(
            (sent.badge.as_str(), sent.editable),
            ("sent · round 1", false)
        );
        assert_eq!(sent.age, "2m ago");
        assert!(view.threads[0].comments[1].editable);
        assert_eq!(view.threads[1].anchor.side, DiffSide::Old);
        assert_eq!(
            view.threads[0].anchor,
            AnchorView::new("lib/a.ts", 12, DiffSide::New)
        );

        let read_only = comments_view("SHOP-18", &comments, false, 0);
        assert!(
            read_only
                .threads
                .iter()
                .flat_map(|t| &t.comments)
                .all(|c| !c.editable)
        );
        let none = comments_view("SHOP-18", &[], true, 0);
        assert_eq!(
            (none.pending_label, none.send_back_label.as_str()),
            (None, "Send back")
        );
    }
}
