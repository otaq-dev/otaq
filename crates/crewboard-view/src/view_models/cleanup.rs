//! The Cleanup view (REC-04; d0018; *16 → Cleanup · 3 items, Cleanup · 20 items, Cleanup · empty, Delete · confirm,
//! Success, Where Cleanup lives*): the service's leftovers grouped by kind with checkboxes and their size, path and
//! age. **Nothing is selected by default** (the user selects, d0018; Q24); Delete is disabled until something is;
//! the confirmation names every item and only Confirm calls `cleanup.delete`. [`CleanupModel::page`] is the overlay
//! as the Tauri page draws it (T-0235).

use std::collections::BTreeSet;

use crewboard_core::recovery::cleanup::{CleanupGroup, CleanupItem, CleanupOutcome, CleanupTarget};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::shell::tilde;

/// A group's heading and icon, for `n` rows (*Cleanup · 3 items*, *Cleanup · 20 items*).
pub fn group_title(group: CleanupGroup, n: usize) -> (&'static str, IconName) {
    match group {
        CleanupGroup::Sessions => ("Orphaned sessions", IconName::SquareTerminal),
        CleanupGroup::Worktrees => ("Worktrees", IconName::FolderGit2),
        CleanupGroup::Branches => ("Branches", IconName::GitBranch),
        CleanupGroup::Ports => ("Ports", IconName::Plug),
        CleanupGroup::KeptWorktrees if n == 1 => (
            "Done ticket with uncommitted changes",
            IconName::TriangleAlert,
        ),
        CleanupGroup::KeptWorktrees => (
            "Done tickets with uncommitted changes",
            IconName::TriangleAlert,
        ),
    }
}

/// A group's name on the page and in its requests (`kept_worktrees`), as the service spells it.
pub fn group_id(group: CleanupGroup) -> &'static str {
    match group {
        CleanupGroup::Sessions => "sessions",
        CleanupGroup::Worktrees => "worktrees",
        CleanupGroup::Branches => "branches",
        CleanupGroup::Ports => "ports",
        CleanupGroup::KeptWorktrees => "kept_worktrees",
    }
}

/// Longer lists start with their groups folded (*Cleanup · 20 items*: "Large lists collapse per group").
pub const FOLD_OVER: usize = 12;

/// What deleting an item does, for the confirmation ("(kill)", "(remove worktree)").
pub fn deletion(group: CleanupGroup) -> &'static str {
    match group {
        CleanupGroup::Sessions => "kill",
        CleanupGroup::Worktrees => "remove worktree",
        CleanupGroup::Branches => "delete branch",
        CleanupGroup::Ports => "free ports",
        CleanupGroup::KeptWorktrees => "remove worktree · its uncommitted changes are lost",
    }
}

/// "412 MB", "1.2 GB", "880 KB".
pub fn size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{} MB", bytes / MB)
    } else {
        format!("{} KB", bytes.div_ceil(KB))
    }
}

/// "9 d", "3 h", "5 min".
pub fn age(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    match minutes {
        m if m < 60 => format!("{m} min"),
        m if m < 24 * 60 => format!("{} h", m / 60),
        m => format!("{} d", m / (24 * 60)),
    }
}

/// One row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupRow {
    pub target: CleanupTarget,
    /// The path with `~`, or the label.
    pub title: String,
    /// Why it is listed (and the uncommitted files of a kept worktree).
    pub detail: String,
    pub size: String,
    pub age: String,
    pub checked: bool,
    /// Deleting it loses uncommitted work.
    pub warning: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupGroupView {
    pub group: CleanupGroup,
    pub title: &'static str,
    pub icon: IconName,
    pub rows: Vec<CleanupRow>,
    /// Every row of the group is checked.
    pub all_checked: bool,
}

/// The confirmation (*Delete · confirm*).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteConfirm {
    pub title: String,
    pub text: String,
    /// One line per item: "~/.crewboard/wt/shop-web/SHOP-14 · 412 MB (remove worktree)".
    pub lines: Vec<String>,
    /// Each line's icon: its group's.
    pub icons: Vec<IconName>,
    pub confirm: String,
}

#[derive(Debug, Clone, Default)]
pub struct CleanupModel {
    items: Vec<CleanupItem>,
    selected: BTreeSet<(CleanupGroup, String)>,
}

fn key(t: &CleanupTarget) -> (CleanupGroup, String) {
    (t.group, t.id.clone())
}

impl CleanupModel {
    /// A new scan; the selection keeps only items still listed.
    pub fn set_items(&mut self, items: Vec<CleanupItem>) {
        self.selected
            .retain(|(g, id)| items.iter().any(|i| i.group == *g && &i.id == id));
        self.items = items;
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn toggle(&mut self, target: &CleanupTarget) {
        let k = key(target);
        if !self.selected.remove(&k) {
            self.selected.insert(k);
        }
    }

    /// The group's checkbox: all on, or all off when every row was on.
    pub fn toggle_group(&mut self, group: CleanupGroup) {
        let rows: Vec<(CleanupGroup, String)> = self
            .items
            .iter()
            .filter(|i| i.group == group)
            .map(|i| key(&i.target()))
            .collect();
        if rows.iter().all(|k| self.selected.contains(k)) {
            for k in &rows {
                self.selected.remove(k);
            }
        } else {
            self.selected.extend(rows);
        }
    }

    pub fn selected(&self) -> Vec<CleanupTarget> {
        self.items
            .iter()
            .filter(|i| self.selected.contains(&key(&i.target())))
            .map(CleanupItem::target)
            .collect()
    }

    fn selected_items(&self) -> Vec<&CleanupItem> {
        self.items
            .iter()
            .filter(|i| self.selected.contains(&key(&i.target())))
            .collect()
    }

    /// The size of what is selected.
    pub fn selected_bytes(&self) -> u64 {
        self.selected_items()
            .iter()
            .filter_map(|i| i.size_bytes)
            .sum()
    }

    /// "2 selected · 412 MB"; `None` while nothing is selected (Delete is disabled then).
    pub fn footer(&self) -> Option<String> {
        let chosen = self.selected_items();
        if chosen.is_empty() {
            return None;
        }
        let bytes = self.selected_bytes();
        Some(if bytes > 0 {
            format!("{} selected · {}", chosen.len(), size(bytes))
        } else {
            format!("{} selected", chosen.len())
        })
    }

    pub fn groups(&self) -> Vec<CleanupGroupView> {
        let mut out: Vec<CleanupGroupView> = Vec::new();
        for item in &self.items {
            let checked = self.selected.contains(&key(&item.target()));
            let title = match (&item.path, item.group) {
                (Some(path), CleanupGroup::Worktrees | CleanupGroup::KeptWorktrees) => tilde(path),
                _ => item.label.clone(),
            };
            let detail = if item.uncommitted.is_empty() {
                item.why.clone()
            } else {
                let n = item.uncommitted.len();
                let files = if n == 1 { "file" } else { "files" };
                format!(
                    "{} · {n} uncommitted {files}: {}",
                    item.why,
                    item.uncommitted.join(", ")
                )
            };
            let row = CleanupRow {
                target: item.target(),
                title,
                detail,
                size: item.size_bytes.map(size).unwrap_or_else(|| "—".into()),
                age: item.age_ms.map(age).unwrap_or_else(|| "—".into()),
                checked,
                warning: item.group == CleanupGroup::KeptWorktrees,
            };
            match out.iter_mut().find(|g| g.group == item.group) {
                Some(g) => g.rows.push(row),
                None => {
                    let n = self.items.iter().filter(|i| i.group == item.group).count();
                    let (title, icon) = group_title(item.group, n);
                    out.push(CleanupGroupView {
                        group: item.group,
                        title,
                        icon,
                        rows: vec![row],
                        all_checked: false,
                    });
                }
            }
        }
        for g in &mut out {
            g.all_checked = g.rows.iter().all(|r| r.checked);
        }
        out
    }

    /// The confirmation, naming every selected item; `None` while nothing is selected.
    pub fn confirm(&self) -> Option<DeleteConfirm> {
        let chosen = self.selected_items();
        if chosen.is_empty() {
            return None;
        }
        let n = chosen.len();
        let items = if n == 1 { "item" } else { "items" };
        Some(DeleteConfirm {
            title: format!("Delete {n} {items}?"),
            text: format!(
                "This can’t be undone. Nothing outside {} and {}’s own branches is touched.",
                tilde(&crate::view_models::cleanup::data_folder()),
                crewboard_core::naming::PRODUCT_NAME
            ),
            lines: chosen
                .iter()
                .map(|i| {
                    let what = match (&i.path, i.group) {
                        (Some(p), CleanupGroup::Worktrees | CleanupGroup::KeptWorktrees) => {
                            tilde(p)
                        }
                        _ => i.label.clone(),
                    };
                    match i.size_bytes {
                        Some(b) => format!("{what} · {} ({})", size(b), deletion(i.group)),
                        None => format!("{what} ({})", deletion(i.group)),
                    }
                })
                .collect(),
            icons: chosen.iter().map(|i| group_title(i.group, 1).1).collect(),
            confirm: format!("Delete {n} {items}"),
        })
    }
}

/// The data folder, as the confirmation names it.
pub fn data_folder() -> String {
    crewboard_core::data_dir::DataDir::from_env()
        .map(|d| d.root().display().to_string())
        .unwrap_or_else(|_| format!("~/{}", crewboard_core::naming::data_dir_name()))
}

/// The success toast (*Success*): "Cleaned up 2 items" · "Freed 412 MB · 1 item left in Cleanup".
pub fn success(outcome: &CleanupOutcome, freed: u64, left: usize) -> (String, Option<String>) {
    let n = outcome.deleted.len();
    let items = if n == 1 { "item" } else { "items" };
    let title = format!("{CLEANED_UP}{n} {items}");
    let mut parts = Vec::new();
    if freed > 0 {
        parts.push(format!("Freed {}", size(freed)));
    }
    if !outcome.failed.is_empty() {
        parts.push(format!("{} couldn’t be deleted", outcome.failed.len()));
    }
    if left > 0 {
        let things = if left == 1 { "item" } else { "items" };
        parts.push(format!("{left} {things} left in Cleanup"));
    }
    (title, (!parts.is_empty()).then(|| parts.join(" · ")))
}

/// The success toast's title starts so (the page gives it the broom icon).
pub const CLEANED_UP: &str = "Cleaned up ";

/// An item as the page names it in `recovery.cleanup_toggle`: `{"group": "worktrees", "id": "/…/SHOP-14"}`, the
/// service's own [`CleanupTarget`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "CleanupTarget")]
pub struct TargetPage {
    pub group: String,
    pub id: String,
}

impl TargetPage {
    fn of(target: &CleanupTarget) -> Self {
        Self {
            group: group_id(target.group).to_owned(),
            id: target.id.clone(),
        }
    }

    /// The service's target, or `None` for a group it doesn't know.
    pub fn target(&self) -> Option<CleanupTarget> {
        serde_json::from_value(serde_json::json!({ "group": self.group, "id": self.id })).ok()
    }
}

/// One row of the overlay. `CleanupRowPage` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CleanupRowPage")]
pub struct RowPage {
    pub target: TargetPage,
    pub title: String,
    pub detail: String,
    pub size: String,
    pub age: String,
    pub checked: bool,
    /// Deleting it loses uncommitted work: the detail in the paused colour.
    pub warning: bool,
}

/// One group: its heading (fold arrow, checkbox, icon, title, count, "select all" while folded) and rows.
/// `CleanupGroupPage` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CleanupGroupPage")]
pub struct GroupPage {
    pub group: String,
    pub title: &'static str,
    pub icon: IconName,
    pub count: String,
    pub all_checked: bool,
    /// The page folds it when the overlay opens; the arrow unfolds it.
    pub starts_folded: bool,
    pub select_all: &'static str,
    pub rows: Vec<RowPage>,
}

/// One line of the confirmation, with its group's icon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CleanupConfirmLine")]
pub struct ConfirmLine {
    pub icon: IconName,
    pub text: String,
}

/// *Delete · confirm*. `CleanupConfirm` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CleanupConfirm")]
pub struct ConfirmPage {
    pub title: String,
    pub text: String,
    pub lines: Vec<ConfirmLine>,
    pub confirm: String,
}

/// *Cleanup · empty*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CleanupEmpty")]
pub struct EmptyPage {
    pub title: &'static str,
    pub text: &'static str,
}

/// The Cleanup overlay (*16 → Cleanup · 3 items, Cleanup · 20 items, Cleanup · empty, Delete · confirm*).
/// `CleanupPage` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CleanupPage")]
pub struct Page {
    pub title: &'static str,
    /// "3 items".
    pub count: String,
    pub text: &'static str,
    pub groups: Vec<GroupPage>,
    pub empty: Option<EmptyPage>,
    /// "2 selected · 412 MB"; `None` while nothing is selected.
    pub footer: Option<String>,
    /// "Delete selected…", disabled while nothing is selected.
    pub delete: &'static str,
    /// The confirmation for what is selected; the page shows it after Delete selected….
    pub confirm: Option<ConfirmPage>,
}

fn items_text(n: usize) -> String {
    if n == 1 {
        "1 item".to_owned()
    } else {
        format!("{n} items")
    }
}

impl CleanupModel {
    /// The overlay as the page draws it.
    pub fn page(&self) -> Page {
        let folded = self.len() > FOLD_OVER;
        Page {
            title: "Cleanup",
            count: items_text(self.len()),
            text: "Leftovers that no ticket owns. Review and select — nothing is deleted automatically.",
            groups: self
                .groups()
                .into_iter()
                .map(|g| GroupPage {
                    group: group_id(g.group).to_owned(),
                    title: g.title,
                    icon: g.icon,
                    count: g.rows.len().to_string(),
                    all_checked: g.all_checked,
                    starts_folded: folded,
                    select_all: "select all",
                    rows: g
                        .rows
                        .into_iter()
                        .map(|r| RowPage {
                            target: TargetPage::of(&r.target),
                            title: r.title,
                            detail: r.detail,
                            size: r.size,
                            age: r.age,
                            checked: r.checked,
                            warning: r.warning,
                        })
                        .collect(),
                })
                .collect(),
            empty: self.is_empty().then_some(EmptyPage {
                title: "Nothing to clean up",
                text: "Every session, worktree, branch and port belongs to a ticket.",
            }),
            footer: self.footer(),
            delete: "Delete selected…",
            confirm: self.confirm().map(|c| ConfirmPage {
                title: c.title,
                text: c.text,
                lines: c
                    .icons
                    .into_iter()
                    .zip(c.lines)
                    .map(|(icon, text)| ConfirmLine { icon, text })
                    .collect(),
                confirm: c.confirm,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    fn item(group: CleanupGroup, id: &str, label: &str, bytes: Option<u64>) -> CleanupItem {
        CleanupItem {
            group,
            id: id.into(),
            label: label.into(),
            project: None,
            ticket: None,
            path: matches!(group, CleanupGroup::Worktrees | CleanupGroup::KeptWorktrees)
                .then(|| format!("/data/wt/shop-web/{label}")),
            size_bytes: bytes,
            age_ms: Some(9 * 24 * 3_600_000),
            why: "no ticket uses this worktree".into(),
            uncommitted: if group == CleanupGroup::KeptWorktrees {
                vec!["src/lib/stripe/client.ts".into(), ".env.local".into()]
            } else {
                Vec::new()
            },
        }
    }

    fn sample() -> CleanupModel {
        let mut m = CleanupModel::default();
        m.set_items(vec![
            item(CleanupGroup::Sessions, "s1", "SHOP-9 · agent session", None),
            item(
                CleanupGroup::Worktrees,
                "/data/wt/shop-web/SHOP-14",
                "SHOP-14",
                Some(412 * MB),
            ),
            item(
                CleanupGroup::KeptWorktrees,
                "t17",
                "SHOP-17",
                Some(220 * MB),
            ),
        ]);
        m
    }

    #[test]
    fn nothing_is_selected_by_default_and_delete_waits_for_a_choice() {
        let m = sample();
        assert!(m.groups().iter().flat_map(|g| &g.rows).all(|r| !r.checked));
        assert_eq!(m.footer(), None);
        assert_eq!(m.confirm(), None);
        let groups = m.groups();
        let titles: Vec<&str> = groups.iter().map(|g| g.title).collect();
        assert_eq!(
            titles,
            [
                "Orphaned sessions",
                "Worktrees",
                "Done ticket with uncommitted changes"
            ]
        );
        let kept = &groups[2].rows[0];
        assert!(kept.warning);
        assert_eq!(
            kept.detail,
            "no ticket uses this worktree · 2 uncommitted files: src/lib/stripe/client.ts, .env.local"
        );
        assert_eq!((kept.size.as_str(), kept.age.as_str()), ("220 MB", "9 d"));
    }

    #[test]
    fn the_confirmation_names_every_selected_item() {
        let mut m = sample();
        let groups = m.groups();
        m.toggle(&groups[0].rows[0].target);
        m.toggle(&groups[1].rows[0].target);
        assert_eq!(m.footer().as_deref(), Some("2 selected · 412 MB"));
        let c = m.confirm().unwrap();
        assert_eq!(c.title, "Delete 2 items?");
        assert_eq!(c.confirm, "Delete 2 items");
        assert_eq!(c.lines[0], "SHOP-9 · agent session (kill)");
        assert!(
            c.lines[1].ends_with("SHOP-14 · 412 MB (remove worktree)"),
            "{}",
            c.lines[1]
        );
        assert_eq!(m.selected().len(), 2);
        // Unticking takes it out again.
        m.toggle(&groups[0].rows[0].target);
        assert_eq!(m.selected().len(), 1);
    }

    #[test]
    fn group_checkbox_and_a_new_scan() {
        let mut m = sample();
        m.toggle_group(CleanupGroup::Worktrees);
        assert!(m.groups()[1].all_checked);
        m.toggle_group(CleanupGroup::Worktrees);
        assert!(!m.groups()[1].all_checked);
        m.toggle_group(CleanupGroup::Sessions);
        m.set_items(vec![item(
            CleanupGroup::Worktrees,
            "/data/wt/x",
            "x",
            Some(MB),
        )]);
        assert!(m.selected().is_empty(), "gone items leave the selection");
    }

    #[test]
    fn the_page_draws_groups_the_confirmation_and_the_empty_state() {
        let mut m = sample();
        let page = m.page();
        assert_eq!((page.title, page.count.as_str()), ("Cleanup", "3 items"));
        assert_eq!(page.empty, None);
        assert_eq!(page.footer, None);
        assert_eq!(page.confirm, None);
        assert_eq!(page.groups[2].group, "kept_worktrees");
        assert_eq!(page.groups[2].title, "Done ticket with uncommitted changes");
        assert!(page.groups.iter().all(|g| !g.starts_folded));
        let target = page.groups[1].rows[0].target.target().unwrap();
        assert_eq!(target.group, CleanupGroup::Worktrees);
        m.toggle(&target);
        let confirm = m.page().confirm.unwrap();
        assert_eq!(confirm.lines[0].icon, IconName::FolderGit2);
        assert!(confirm.lines[0].text.ends_with("(remove worktree)"));
        assert_eq!(
            TargetPage {
                group: "nonsense".into(),
                id: "x".into()
            }
            .target(),
            None
        );

        let empty = CleanupModel::default().page();
        assert_eq!(empty.count, "0 items");
        assert_eq!(empty.empty.unwrap().title, "Nothing to clean up");
    }

    #[test]
    fn a_long_list_folds_its_groups_and_plural_titles_follow_the_count() {
        let mut m = CleanupModel::default();
        let mut items: Vec<CleanupItem> = (0..FOLD_OVER)
            .map(|i| item(CleanupGroup::Ports, &format!("p{i}"), "port", None))
            .collect();
        items.push(item(CleanupGroup::KeptWorktrees, "a", "SHOP-1", None));
        items.push(item(CleanupGroup::KeptWorktrees, "b", "SHOP-2", None));
        m.set_items(items);
        let page = m.page();
        assert!(page.groups.iter().all(|g| g.starts_folded));
        assert_eq!(page.groups[0].title, "Ports");
        assert_eq!(
            page.groups[1].title,
            "Done tickets with uncommitted changes"
        );
        assert_eq!(page.groups[1].count, "2");
    }

    #[test]
    fn sizes_ages_and_the_toast() {
        assert_eq!(size(412 * MB), "412 MB");
        assert_eq!(size(3 * 1024 * MB / 2), "1.5 GB");
        assert_eq!(size(10), "1 KB");
        assert_eq!(age(5 * 60_000), "5 min");
        assert_eq!(age(3 * 3_600_000), "3 h");
        let outcome = CleanupOutcome {
            deleted: vec![
                CleanupTarget {
                    group: CleanupGroup::Sessions,
                    id: "a".into(),
                },
                CleanupTarget {
                    group: CleanupGroup::Worktrees,
                    id: "b".into(),
                },
            ],
            failed: Vec::new(),
        };
        assert_eq!(
            success(&outcome, 412 * MB, 1),
            (
                "Cleaned up 2 items".to_owned(),
                Some("Freed 412 MB · 1 item left in Cleanup".to_owned())
            )
        );
    }
}
