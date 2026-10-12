//! The Diff tab's logic (REV-01, REV-02; d0017): the file list with its status letters and counts, "viewed"
//! ticks kept per ticket and round, the agent's summary open or folded per ticket, the base indicator, files
//! loaded one at a time as they come into view, and the tab's "+412 −38".
//!
//! Crewboard runs no tests, so the tab shows no test result (P5).

use std::collections::{HashMap, HashSet};

use crewboard_core::board::diff::DiffSummary;
use crewboard_core::domain::TicketKey;
use crewboard_core::git::diff::{ChangedFile, DiffLine, FileDiff, FileStatus, Hunk, LineKind};

/// What the user decided in Diff tabs, kept while the app runs: viewed files per ticket and round, and whether
/// the agent's summary is folded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewMemory {
    viewed: HashMap<(TicketKey, u32), HashSet<String>>,
    summary_folded: HashSet<TicketKey>,
    /// Side-by-side instead of Unified, for every ticket (T-0092).
    pub side_by_side: bool,
}

impl ReviewMemory {
    pub fn is_viewed(&self, key: &TicketKey, round: u32, path: &str) -> bool {
        self.viewed
            .get(&(key.clone(), round))
            .is_some_and(|v| v.contains(path))
    }

    /// Ticks or unticks a file; returns whether it is viewed now.
    pub fn toggle_viewed(&mut self, key: &TicketKey, round: u32, path: &str) -> bool {
        let set = self.viewed.entry((key.clone(), round)).or_default();
        if set.remove(path) {
            false
        } else {
            set.insert(path.to_owned());
            true
        }
    }

    pub fn viewed_count(&self, key: &TicketKey, round: u32, files: &[ChangedFile]) -> usize {
        files
            .iter()
            .filter(|f| self.is_viewed(key, round, &f.path))
            .count()
    }

    pub fn summary_open(&self, key: &TicketKey) -> bool {
        !self.summary_folded.contains(key)
    }

    pub fn toggle_summary(&mut self, key: &TicketKey) {
        if !self.summary_folded.remove(key) {
            self.summary_folded.insert(key.clone());
        }
    }
}

/// A file's hunks, loaded when it first comes into view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileLoad {
    Loading,
    Loaded(Box<FileDiff>),
    Failed(String),
}

/// The Diff tab of one ticket.
#[derive(Debug, Clone, Default)]
pub struct DiffModel {
    pub summary: Option<DiffSummary>,
    /// Why the file list couldn't load ("SHOP-12 has no branch yet: it hasn't started").
    pub problem: Option<String>,
    files: HashMap<String, FileLoad>,
    /// The file the list has selected (the diff scrolls to it).
    pub selected: Option<String>,
    /// Files that started collapsed and were opened with "Load diff".
    opened: HashSet<String>,
    /// File-list folders folded or unfolded by the user (the generated group starts folded).
    toggled_groups: HashSet<String>,
}

impl DiffModel {
    /// A new file list: hunks loaded for an earlier one are dropped, the selection kept if the file is still there.
    pub fn set_summary(&mut self, summary: DiffSummary) {
        self.files.clear();
        self.opened.clear();
        self.problem = None;
        if !self
            .selected
            .as_ref()
            .is_some_and(|s| summary.files.iter().any(|f| &f.path == s))
        {
            self.selected = None;
        }
        self.summary = Some(summary);
    }

    pub fn set_problem(&mut self, problem: String) {
        self.summary = None;
        self.files.clear();
        self.problem = Some(problem);
    }

    pub fn files(&self) -> &[ChangedFile] {
        self.summary.as_ref().map_or(&[], |s| &s.files)
    }

    pub fn file(&self, path: &str) -> Option<&FileLoad> {
        self.files.get(path)
    }

    /// Whether a file shows its lines: large and generated files wait for "Load diff".
    pub fn is_open(&self, file: &ChangedFile) -> bool {
        !starts_collapsed(file) || self.opened.contains(&file.path)
    }

    /// "Load diff" on a collapsed file.
    pub fn open(&mut self, path: &str) {
        self.opened.insert(path.to_owned());
    }

    /// Whether a file-list folder is folded.
    pub fn group_folded(&self, group: &FileGroup) -> bool {
        group.generated != self.toggled_groups.contains(&group.name)
    }

    pub fn toggle_group(&mut self, name: &str) {
        if !self.toggled_groups.remove(name) {
            self.toggled_groups.insert(name.to_owned());
        }
    }

    /// Marks the file at `index` as loading if nothing was asked for it yet; returns its path to load. Binary
    /// files need no hunks; collapsed ones wait for "Load diff".
    pub fn needs_load(&mut self, index: usize) -> Option<String> {
        let file = self.files().get(index)?.clone();
        if file.binary || self.files.contains_key(&file.path) || !self.is_open(&file) {
            return None;
        }
        self.files.insert(file.path.clone(), FileLoad::Loading);
        Some(file.path)
    }

    pub fn loaded(&mut self, path: &str, result: Result<FileDiff, String>) {
        self.files.insert(
            path.to_owned(),
            match result {
                Ok(diff) => FileLoad::Loaded(Box::new(diff)),
                Err(e) => FileLoad::Failed(e),
            },
        );
    }

    pub fn index_of(&self, path: &str) -> Option<usize> {
        self.files().iter().position(|f| f.path == path)
    }
}

/// Files with more changed lines than this start collapsed with "Load diff" (*Large diff · 54 files*).
pub const LARGE_FILE_LINES: u32 = 400;
/// From this many files on, the file list gets a filter and groups by folder.
pub const GROUP_FROM: usize = 20;
/// Below this panel width Side-by-side falls back to Unified (*Side-by-side*).
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

/// Side-by-side when chosen and the panel is wide enough.
pub fn shows_side_by_side(chosen: bool, panel_width: f32) -> bool {
    chosen && panel_width >= SIDE_BY_SIDE_FROM
}

/// A folder of a long file list: "src/lib/ · 19", or "generated · collapsed".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileGroup {
    pub name: String,
    /// Indexes into the diff's files.
    pub files: Vec<usize>,
    pub generated: bool,
}

/// A long file list grouped by folder (in the diff's order), generated files last; `filter` keeps the paths
/// that contain it (case-insensitive).
pub fn file_groups(files: &[ChangedFile], filter: &str) -> Vec<FileGroup> {
    let filter = filter.trim().to_lowercase();
    let mut groups: Vec<FileGroup> = Vec::new();
    let mut generated = FileGroup {
        name: "generated".to_owned(),
        files: Vec::new(),
        generated: true,
    };
    for (i, file) in files.iter().enumerate() {
        if !filter.is_empty() && !file_label(file).to_lowercase().contains(&filter) {
            continue;
        }
        if is_generated(&file.path) {
            generated.files.push(i);
            continue;
        }
        let folder = match file.path.rsplit_once('/') {
            Some((dir, _)) => format!("{dir}/"),
            None => "./".to_owned(),
        };
        match groups.iter_mut().find(|g| g.name == folder) {
            Some(group) => group.files.push(i),
            None => groups.push(FileGroup {
                name: folder,
                files: vec![i],
                generated: false,
            }),
        }
    }
    if !generated.files.is_empty() {
        groups.push(generated);
    }
    groups
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
pub const EMPTY_TEXT: &str = "This branch has no changes compared to its base. The agent may not have committed yet \u{2014} check the Terminal tab.";

/// "diff vs `main` at a1b2c3d · 9 files · +412 −38".
pub fn base_label(s: &DiffSummary) -> (String, String) {
    let files = if s.files.len() == 1 {
        "1 file".to_owned()
    } else {
        format!("{} files", s.files.len())
    };
    (
        format!("{} at {}", s.base_branch, s.base_short),
        format!("· {files} · {}", counts(s.added, s.removed)),
    )
}

/// "+412 −38" (a real minus sign); "+0 −0" for an empty diff.
pub fn counts(added: u32, removed: u32) -> String {
    format!("+{added} \u{2212}{removed}")
}

/// The Diff tab's indicator: "+412 −38", nothing before the diff is known or when it is empty.
pub fn tab_indicator(summary: Option<&DiffSummary>) -> Option<String> {
    let s = summary?;
    (!s.files.is_empty()).then(|| counts(s.added, s.removed))
}

/// "4 / 9 viewed".
pub fn viewed_label(viewed: usize, total: usize) -> String {
    format!("{viewed} / {total} viewed")
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
    fn side_by_side_needs_a_wide_panel() {
        assert!(shows_side_by_side(true, 1200.));
        assert!(!shows_side_by_side(true, 660.), "falls back to unified");
        assert!(!shows_side_by_side(false, 1200.));
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
        let mut m = DiffModel::default();
        m.set_summary(summary(vec![big.clone()]));
        assert_eq!(m.needs_load(0), None, "waits for Load diff");
        m.open("src/big.ts");
        assert_eq!(m.needs_load(0).as_deref(), Some("src/big.ts"));
    }

    #[test]
    fn long_file_lists_group_by_folder_with_generated_last() {
        let files = vec![
            file("src/app/checkout/page.tsx", FileStatus::Added),
            file("src/lib/money.ts", FileStatus::Modified),
            file("yarn.lock", FileStatus::Modified),
            file("src/app/checkout/form.tsx", FileStatus::Added),
            file("README.md", FileStatus::Modified),
        ];
        let groups = file_groups(&files, "");
        let shape: Vec<(&str, &[usize], bool)> = groups
            .iter()
            .map(|g| (g.name.as_str(), g.files.as_slice(), g.generated))
            .collect();
        assert_eq!(
            shape,
            [
                ("src/app/checkout/", &[0, 3][..], false),
                ("src/lib/", &[1][..], false),
                ("./", &[4][..], false),
                ("generated", &[2][..], true),
            ]
        );
        let filtered = file_groups(&files, "CHECKOUT");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].files, [0, 3]);
        let mut m = DiffModel::default();
        assert!(m.group_folded(&groups[3]), "generated starts folded");
        assert!(!m.group_folded(&groups[0]));
        m.toggle_group("generated");
        assert!(!m.group_folded(&groups[3]));
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
        assert!(EMPTY_TEXT.starts_with("This branch has no changes compared to its base."));
    }

    fn summary(files: Vec<ChangedFile>) -> DiffSummary {
        DiffSummary {
            key: "SHOP-18".parse().unwrap(),
            base_branch: "main".into(),
            base_sha: "a1b2c3d4e5".into(),
            base_short: "a1b2c3d".into(),
            branch: "x".into(),
            added: 412,
            removed: 38,
            files,
            round: 1,
            review_summary: None,
            children: vec![],
        }
    }

    #[test]
    fn viewed_ticks_are_kept_per_ticket_and_round() {
        let key: TicketKey = "SHOP-18".parse().unwrap();
        let other: TicketKey = "SHOP-19".parse().unwrap();
        let mut m = ReviewMemory::default();
        assert!(m.toggle_viewed(&key, 1, "a.ts"));
        assert!(m.is_viewed(&key, 1, "a.ts"));
        assert!(!m.is_viewed(&key, 2, "a.ts"), "a new round starts unticked");
        assert!(!m.is_viewed(&other, 1, "a.ts"));
        let files = vec![
            file("a.ts", FileStatus::Added),
            file("b.ts", FileStatus::Modified),
        ];
        assert_eq!(
            viewed_label(m.viewed_count(&key, 1, &files), files.len()),
            "1 / 2 viewed"
        );
        assert!(!m.toggle_viewed(&key, 1, "a.ts"));
        assert_eq!(m.viewed_count(&key, 1, &files), 0);
    }

    #[test]
    fn the_summary_folds_per_ticket() {
        let key: TicketKey = "SHOP-18".parse().unwrap();
        let mut m = ReviewMemory::default();
        assert!(m.summary_open(&key), "open by default");
        m.toggle_summary(&key);
        assert!(!m.summary_open(&key));
        assert!(m.summary_open(&"SHOP-19".parse().unwrap()));
        m.toggle_summary(&key);
        assert!(m.summary_open(&key));
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
        let s = summary(vec![file("a", FileStatus::Added); 9]);
        assert_eq!(
            base_label(&s),
            (
                "main at a1b2c3d".into(),
                "· 9 files · +412 \u{2212}38".into()
            )
        );
        assert_eq!(tab_indicator(Some(&s)).as_deref(), Some("+412 \u{2212}38"));
        assert_eq!(tab_indicator(Some(&summary(vec![]))), None);
    }

    #[test]
    fn files_load_once_as_they_come_into_view() {
        let mut png = file("logo.png", FileStatus::Added);
        png.binary = true;
        let mut m = DiffModel::default();
        m.set_summary(summary(vec![file("a.ts", FileStatus::Added), png]));
        assert_eq!(m.needs_load(0).as_deref(), Some("a.ts"));
        assert_eq!(m.file("a.ts"), Some(&FileLoad::Loading));
        assert_eq!(m.needs_load(0), None, "asked once");
        assert_eq!(m.needs_load(1), None, "binary files have no hunks");
        m.loaded("a.ts", Err("gone".into()));
        assert_eq!(m.file("a.ts"), Some(&FileLoad::Failed("gone".into())));
        // A new file list drops what was loaded.
        m.selected = Some("a.ts".into());
        m.set_summary(summary(vec![file("b.ts", FileStatus::Added)]));
        assert_eq!(m.file("a.ts"), None);
        assert_eq!(m.selected, None);
    }
}
