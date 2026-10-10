//! The Diff tab's logic (REV-01, REV-02; d0017): the file list with its status letters and counts, "viewed"
//! ticks kept per ticket and round, the agent's summary open or folded per ticket, the base indicator, files
//! loaded one at a time as they come into view, and the tab's "+412 −38".
//!
//! Crewboard runs no tests, so the tab shows no test result (P5).

use std::collections::{HashMap, HashSet};

use crewboard_core::board::diff::DiffSummary;
use crewboard_core::domain::TicketKey;
use crewboard_core::git::diff::{ChangedFile, FileDiff, FileStatus};

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
}

impl DiffModel {
    /// A new file list: hunks loaded for an earlier one are dropped, the selection kept if the file is still there.
    pub fn set_summary(&mut self, summary: DiffSummary) {
        self.files.clear();
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

    /// Marks the file at `index` as loading if nothing was asked for it yet; returns its path to load. Binary
    /// files need no hunks.
    pub fn needs_load(&mut self, index: usize) -> Option<String> {
        let file = self.files().get(index)?.clone();
        if file.binary || self.files.contains_key(&file.path) {
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
        return Some(match file.size {
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
        }
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
