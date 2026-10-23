//! Diffs as the Files tab shows them (REV-01, REV-02, FTB-04; d0017): status letters and counts, large and generated
//! files collapsed, removed and added lines side by side, binary and renamed files in words, and Unified or Split kept
//! while the app runs ([`ReviewMemory`]).
//!
//! Otaq runs no tests, so the tab shows no test result (P5).

use otaq_core::git::diff::{ChangedFile, DiffLine, FileStatus, Hunk, LineKind};

/// What the user chose in the Files tab, kept while the app runs: Split instead of Unified, for every ticket
/// (T-0092, FTB-04).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewMemory {
    pub side_by_side: bool,
}

/// Files with more changed lines than this start collapsed with "Load diff" (*Large diff · 54 files*).
pub const LARGE_FILE_LINES: u32 = 400;
/// Below this width Split reads as Unified (FTB-04).
pub const SIDE_BY_SIDE_FROM: f32 = 900.;

/// Lock files and generated output: collapsed by default, and grouped at the end of a long file list.
pub fn is_generated(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    const LOCKS: &[&str] = &[
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "Cargo.lock",
        "Gemfile.lock",
        "poetry.lock",
        "composer.lock",
        "go.sum",
    ];
    LOCKS.contains(&name)
        || name.ends_with(".lock")
        || name.ends_with(".min.js")
        || name.ends_with(".min.css")
        || name.ends_with(".map")
        || name.ends_with(".snap")
        || path.split('/').any(|part| {
            matches!(
                part,
                "dist" | "build" | "generated" | "__generated__" | "vendor" | "node_modules"
            )
        })
}

/// Large and generated files start collapsed.
pub fn starts_collapsed(file: &ChangedFile) -> bool {
    !file.binary && (file.added + file.removed > LARGE_FILE_LINES || is_generated(&file.path))
}

/// One row of the side-by-side view: the old line on the left, the new on the right; a missing side is blank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideRow {
    pub left: Option<(usize, DiffLine)>,
    pub right: Option<(usize, DiffLine)>,
}

/// Lines up a hunk's old and new lines: context lines face each other, a run of removed lines faces the added
/// lines that follow it, and the longer side gets blanks opposite. The `usize` is the line's index in the hunk.
pub fn side_by_side(hunk: &Hunk) -> Vec<SideRow> {
    let mut rows = Vec::new();
    let mut removed: Vec<(usize, DiffLine)> = Vec::new();
    let mut added: Vec<(usize, DiffLine)> = Vec::new();
    let flush = |removed: &mut Vec<(usize, DiffLine)>,
                 added: &mut Vec<(usize, DiffLine)>,
                 rows: &mut Vec<SideRow>| {
        let n = removed.len().max(added.len());
        let mut r = removed.drain(..);
        let mut a = added.drain(..);
        for _ in 0..n {
            rows.push(SideRow {
                left: r.next(),
                right: a.next(),
            });
        }
    };
    for (i, line) in hunk.lines.iter().enumerate() {
        match line.kind {
            LineKind::Removed => {
                if !added.is_empty() {
                    flush(&mut removed, &mut added, &mut rows);
                }
                removed.push((i, line.clone()));
            }
            LineKind::Added => added.push((i, line.clone())),
            LineKind::Context => {
                flush(&mut removed, &mut added, &mut rows);
                rows.push(SideRow {
                    left: Some((i, line.clone())),
                    right: Some((i, line.clone())),
                });
            }
        }
    }
    flush(&mut removed, &mut added, &mut rows);
    rows
}

/// What a binary file's section says: "Binary file changed · 184 KB → 96 KB".
pub fn binary_label(file: &ChangedFile) -> String {
    match (file.old_size, file.size) {
        (Some(old), Some(new)) => format!(
            "Binary file changed \u{b7} {} \u{2192} {}",
            size_label(old),
            size_label(new)
        ),
        (None, Some(new)) => format!("Binary file added \u{b7} {}", size_label(new)),
        (Some(old), None) => format!("Binary file deleted \u{b7} {}", size_label(old)),
        (None, None) => "Binary file changed".to_owned(),
    }
}

/// What a renamed file's section says above its changes.
pub fn rename_label(file: &ChangedFile) -> Option<String> {
    if file.status != FileStatus::Renamed {
        return None;
    }
    let changed = file.added.max(file.removed);
    let similar = file
        .similarity
        .map(|s| format!(" ({s} % similar)"))
        .unwrap_or_default();
    Some(if changed == 0 {
        format!("Renamed without changes{similar}.")
    } else {
        let lines = if changed == 1 { "line" } else { "lines" };
        format!("Renamed with {changed} changed {lines}{similar}. Showing only the changes.")
    })
}

/// The empty diff's explanation (*Binary, renamed, empty*).
pub const EMPTY_TITLE: &str = "No changes";

/// "+412 −38" (a real minus sign); "+0 −0" for an empty diff.
pub fn counts(added: u32, removed: u32) -> String {
    format!("+{added} \u{2212}{removed}")
}

/// The file list's status letter: A, M, D, R.
pub fn status_letter(status: FileStatus) -> &'static str {
    match status {
        FileStatus::Added => "A",
        FileStatus::Modified => "M",
        FileStatus::Deleted => "D",
        FileStatus::Renamed => "R",
    }
}

/// How a file is named in the list: "lib/money.ts → lib/format/money.ts" for a rename.
pub fn file_label(file: &ChangedFile) -> String {
    match &file.old_path {
        Some(old) => format!("{old} \u{2192} {}", file.path),
        None => file.path.clone(),
    }
}

/// The note after a file's name in its header: "new file", "deleted", "renamed", "binary file · 12 KB".
pub fn file_note(file: &ChangedFile) -> Option<String> {
    if file.binary {
        return Some(match file.size.or(file.old_size) {
            Some(bytes) => format!("binary file \u{b7} {}", size_label(bytes)),
            None => "binary file".to_owned(),
        });
    }
    match file.status {
        FileStatus::Added => Some("new file".to_owned()),
        FileStatus::Deleted => Some("deleted".to_owned()),
        FileStatus::Renamed => Some("renamed".to_owned()),
        FileStatus::Modified => None,
    }
}

/// "812 B", "12 KB", "3.4 MB".
pub fn size_label(bytes: u64) -> String {
    const KB: u64 = 1024;
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < KB * KB {
        format!("{} KB", bytes.div_ceil(KB))
    } else {
        format!("{:.1} MB", bytes as f64 / (KB * KB) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, status: FileStatus) -> ChangedFile {
        ChangedFile {
            status,
            path: path.into(),
            old_path: None,
            added: 1,
            removed: 0,
            binary: false,
            size: None,
            old_size: None,
            similarity: None,
        }
    }

    fn line(kind: LineKind, text: &str) -> DiffLine {
        DiffLine {
            kind,
            old: None,
            new: None,
            text: text.into(),
            no_newline: false,
        }
    }

    #[test]
    fn side_by_side_faces_removed_and_added_lines() {
        use LineKind::*;
        let hunk = Hunk {
            old_start: 12,
            old_lines: 4,
            new_start: 12,
            new_lines: 6,
            header: String::new(),
            lines: vec![
                line(Context, "fn"),
                line(Removed, "a"),
                line(Added, "A1"),
                line(Added, "A2"),
                line(Added, "A3"),
                line(Context, "for"),
                line(Removed, "b"),
                line(Removed, "c"),
                line(Added, "B"),
            ],
        };
        let text = |l: Option<(usize, DiffLine)>| l.map(|(_, l)| l.text);
        let rows: Vec<(Option<String>, Option<String>)> = side_by_side(&hunk)
            .into_iter()
            .map(|r| (text(r.left), text(r.right)))
            .collect();
        let s = |t: &str| Some(t.to_owned());
        assert_eq!(
            rows,
            [
                (s("fn"), s("fn")),
                (s("a"), s("A1")),
                (None, s("A2")),
                (None, s("A3")),
                (s("for"), s("for")),
                (s("b"), s("B")),
                (s("c"), None),
            ]
        );
    }

    #[test]
    fn large_and_generated_files_start_collapsed() {
        let mut big = file("src/big.ts", FileStatus::Modified);
        big.added = 380;
        big.removed = 21;
        assert!(starts_collapsed(&big));
        assert!(starts_collapsed(&file(
            "package-lock.json",
            FileStatus::Modified
        )));
        assert!(starts_collapsed(&file(
            "web/dist/app.js",
            FileStatus::Added
        )));
        assert!(!starts_collapsed(&file("src/app.ts", FileStatus::Modified)));
    }

    #[test]
    fn binary_renamed_and_empty_read_like_the_frame() {
        let mut png = file("public/hero.png", FileStatus::Modified);
        png.binary = true;
        png.old_size = Some(184 * 1024);
        png.size = Some(96 * 1024);
        assert_eq!(
            binary_label(&png),
            "Binary file changed \u{b7} 184 KB \u{2192} 96 KB"
        );
        png.old_size = None;
        assert_eq!(binary_label(&png), "Binary file added \u{b7} 96 KB");
        let mut moved = file("lib/format/money.ts", FileStatus::Renamed);
        moved.added = 3;
        moved.removed = 3;
        moved.similarity = Some(98);
        assert_eq!(
            rename_label(&moved).as_deref(),
            Some("Renamed with 3 changed lines (98 % similar). Showing only the changes.")
        );
        moved.added = 0;
        moved.removed = 0;
        moved.similarity = Some(100);
        assert_eq!(
            rename_label(&moved).as_deref(),
            Some("Renamed without changes (100 % similar).")
        );
        assert_eq!(rename_label(&file("a", FileStatus::Added)), None);
    }

    #[test]
    fn the_file_list_reads_like_the_frame() {
        let mut renamed = file("lib/format/money.ts", FileStatus::Renamed);
        renamed.old_path = Some("lib/money.ts".into());
        assert_eq!(
            file_label(&renamed),
            "lib/money.ts \u{2192} lib/format/money.ts"
        );
        assert_eq!(status_letter(FileStatus::Deleted), "D");
        assert_eq!(
            file_note(&file("x", FileStatus::Added)).as_deref(),
            Some("new file")
        );
        let mut png = file("logo.png", FileStatus::Modified);
        png.binary = true;
        png.size = Some(12_000);
        assert_eq!(file_note(&png).as_deref(), Some("binary file \u{b7} 12 KB"));
        assert_eq!(counts(412, 38), "+412 \u{2212}38");
    }
}
