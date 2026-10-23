//! The project **Files view** (FIL-01, FIL-03, FIL-04, FIL-07, FIL-10; d0087 and its 2026-10-23 lines; canvas
//! `s28-03`, `s28-04`, `s28-08`, `s28-09`): the toolbar's third view, Board · List · Files. On the left the source bar
//! ("main · every ticket’s changes" over "the base branch · 8 tickets, 11 files"), the search box (⌥⌘P, Go to file),
//! All · Changed 11 · Needs you 6 · Same file 1, and the tree of the project's base branch with every ticket's changes
//! on top: a changed file carries one chip per ticket, its mark and +/−, a folder counts the changed files below it, a
//! ticket on another base is tagged with that branch. The source bar opens the picker ("Find a ticket or epic"): BASE
//! BRANCH, EPICS · THE EPIC’S BRANCH, TICKETS · THEIR WORKTREE and the tickets with no worktree yet; a ticket or epic
//! picked there shows its own Files tab's tree, read-only, with "Open SHOP-29" and the back link.
//!
//! The rows are the tab's ([`TreeRow`], [`SearchView`]), widened with the chips; the data is `board/project_files.rs`'s
//! (`files.project_*`). The base branch is the project's setting (d0051): every label names it. The page keeps the
//! filter, the folders opened, the search being typed and the picked source; it asks again every 5 s while an agent
//! works ([`Page::working`], FIL-10) and on the tickets' events.
//!
//! Implementation choice: folders are listed one level at a time as the tab does; `s28-03`'s "api/checkout/coupon"
//! (single-child folders drawn as one row) isn't a drawn rule and isn't done.

use std::collections::{BTreeMap, HashSet};

use otaq_core::board::files::EntryKind;
use otaq_core::board::project_files::{
    EpicSource, FileOwner, NoWorktree, OwnerTicket, ProjectChanges, ProjectCounts, ProjectEntry,
    ProjectSearch, ProjectSource, ProjectSources,
};
use otaq_core::domain::{Status, StatusSet, TicketKey};
use otaq_core::git::diff::FileStatus;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::card::Tone;
use super::files_tab::{
    self as tab, ChipView, EmptyView, EntryKindView, MarkView, SearchBox, SearchRow, SearchView,
    TabView, TreeRow,
};

// ---- Words ----

pub const LOADING: &str = "Loading the tickets\u{2019} changes\u{2026}";
/// A filter's count while the tickets' changes load ("Changed …").
pub const COUNTING: &str = "\u{2026}";
pub const EMPTY_TITLE: &str = "No ticket is changing files right now.";
pub const EMPTY_TEXT: &str = "Start one from the board, or ask the Project agent.";
pub const FIND: &str = "Find a ticket or epic";
pub const BASE_SECTION: &str = "BASE BRANCH";
pub const THE_DEFAULT: &str = "the default";
pub const EPICS_SECTION: &str = "EPICS \u{b7} THE EPIC\u{2019}S BRANCH";
pub const TICKETS_SECTION: &str = "TICKETS \u{b7} THEIR WORKTREE";
pub const READ_ONLY: &str = "read-only";

/// "main · every ticket’s changes": the view's default source, and the picker's back link.
pub fn every_change(base: &str) -> String {
    format!("{base} \u{b7} every ticket\u{2019}s changes")
}

fn plural(n: u32, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

// ---- The source bar and the filters ----

/// What the left column shows (FIL-04). `FilesViewFilter` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "FilesViewFilter")]
pub enum Filter {
    /// The base's tree, folder by folder.
    All,
    /// Every changed file.
    Changed,
    /// The files changed by tickets that need you.
    NeedsYou,
    /// The files two tickets on the same base change (FIL-06).
    SameFile,
}

/// The bar on the tree that opens the picker: "main · every ticket’s changes" over "the base branch · 8 tickets, 11
/// files". `FilesViewSource` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FilesViewSource")]
pub struct SourceBar {
    pub title: String,
    pub subtitle: String,
}

/// `counts` is `None` while the tickets' changes load (the bare base shows at once, FIL-02).
pub fn source_bar(source: &ProjectSource, counts: Option<&ProjectCounts>) -> SourceBar {
    let subtitle = match counts {
        None => "the base branch".to_owned(),
        Some(c) if c.tickets == 0 => {
            "the base branch \u{b7} no ticket is changing files".to_owned()
        }
        Some(c) => format!(
            "the base branch \u{b7} {}, {}",
            plural(c.tickets, "ticket", "tickets"),
            plural(c.changed, "file", "files")
        ),
    };
    SourceBar {
        title: every_change(&source.base_branch),
        subtitle,
    }
}

/// One filter: "Changed 11"; Needs you's tooltip "Needs you · 6 files" over "changed by the 4 tickets that need
/// you". `FilesViewFilterOption` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FilesViewFilterOption")]
pub struct FilterOption {
    pub filter: Filter,
    pub label: String,
    pub tooltip: Option<String>,
    pub detail: Option<String>,
}

pub fn filters(counts: Option<&ProjectCounts>) -> Vec<FilterOption> {
    let count =
        |n: fn(&ProjectCounts) -> u32| counts.map_or(COUNTING.to_owned(), |c| n(c).to_string());
    let option = |filter, label: String| FilterOption {
        filter,
        label,
        tooltip: None,
        detail: None,
    };
    vec![
        option(Filter::All, "All".to_owned()),
        option(Filter::Changed, format!("Changed {}", count(|c| c.changed))),
        FilterOption {
            tooltip: counts
                .map(|c| format!("Needs you \u{b7} {}", plural(c.needs_you, "file", "files"))),
            detail: counts.map(|c| match c.needs_you_tickets {
                1 => "changed by the ticket that needs you".to_owned(),
                n => format!("changed by the {n} tickets that need you"),
            }),
            ..option(
                Filter::NeedsYou,
                format!("Needs you {}", count(|c| c.needs_you)),
            )
        },
        option(
            Filter::SameFile,
            format!("Same file {}", count(|c| c.same_file)),
        ),
    ]
}

// ---- The tree ----

/// A chip's colour: the ticket's status (FIL-03), turquoise while it needs you.
pub fn tone(status: Status, needs_you: bool) -> Tone {
    if needs_you {
        return Tone::Attention;
    }
    match status {
        Status::Interrupted
        | Status::ResumeFailed
        | Status::CheckFailed
        | Status::StartInterrupted => Tone::Error,
        _ => match status.set() {
            StatusSet::AgentStage | StatusSet::AgentCheck => Tone::Working,
            StatusSet::HumanCheck => Tone::Review,
            StatusSet::Backlog | StatusSet::Waiting | StatusSet::Delivery | StatusSet::Done => {
                Tone::Muted
            }
        },
    }
}

fn chips(owners: &[FileOwner]) -> Vec<ChipView> {
    owners
        .iter()
        .map(|o| ChipView {
            key: o.key.to_string(),
            tone: tone(o.status, o.needs_you),
        })
        .collect()
}

/// A file's mark from its owners: the first one's letter (the one that needs you, else flow order), +/− only while
/// one ticket changes it (`s28-03`: pricing.ts's two chips have none), uncommitted when any of it is.
fn mark(owners: &[FileOwner]) -> Option<MarkView> {
    let first = owners.first()?;
    let mut mark = tab::mark(&otaq_core::board::files::FileChange {
        status: first.file.status,
        added: first.file.added,
        removed: first.file.removed,
        binary: first.file.binary,
        uncommitted: owners.iter().any(|o| o.uncommitted),
    });
    if owners.len() > 1 {
        mark.counts = None;
    }
    Some(mark)
}

/// The other base branch a ticket changing the file is on ("release/2.3").
fn other_branch(owners: &[FileOwner]) -> Option<String> {
    owners
        .iter()
        .find(|o| o.other_base)
        .map(|o| o.base_branch.clone())
}

/// "3 changed".
fn changed_below(n: u32) -> Option<String> {
    (n > 0).then(|| format!("{n} changed"))
}

fn file_row(
    name: String,
    path: String,
    kind: EntryKindView,
    depth: u32,
    owners: &[FileOwner],
) -> TreeRow {
    TreeRow {
        name,
        path,
        kind,
        depth,
        mark: mark(owners),
        quiet: owners.is_empty(),
        opens: kind != EntryKindView::Submodule,
        tags: Vec::new(),
        child: None,
        chips: chips(owners),
        changed_below: None,
        branch: other_branch(owners),
        struck: owners
            .first()
            .is_some_and(|o| o.file.status == FileStatus::Deleted),
    }
}

fn folder_row(name: String, path: String, depth: u32, changed: u32) -> TreeRow {
    TreeRow {
        name,
        path,
        kind: EntryKindView::Folder,
        depth,
        mark: None,
        quiet: false,
        opens: true,
        tags: Vec::new(),
        child: None,
        chips: Vec::new(),
        changed_below: changed_below(changed),
        branch: None,
        struck: false,
    }
}

/// A folder's entries at `depth` (`files.project_tree`), as the tab's rows with the chips.
pub fn entry_rows(entries: &[ProjectEntry], depth: u32) -> Vec<TreeRow> {
    entries
        .iter()
        .map(|e| match e.kind {
            EntryKind::Folder => folder_row(e.name.clone(), e.path.clone(), depth, e.changed_below),
            kind => file_row(
                e.name.clone(),
                e.path.clone(),
                kind.into(),
                depth,
                &e.owners,
            ),
        })
        .collect()
}

/// Two owners of one file on the same base (FIL-06), as `files.project_*` counts Same file.
fn same_file(owners: &[FileOwner]) -> bool {
    let mut bases = HashSet::new();
    owners.iter().any(|o| !bases.insert(o.base_branch.as_str()))
}

/// Every changed path with its owners, in the groups' order (needs you first, then flow order).
fn by_path(changes: &ProjectChanges) -> BTreeMap<String, Vec<FileOwner>> {
    let mut map: BTreeMap<String, Vec<FileOwner>> = BTreeMap::new();
    for group in &changes.groups {
        let t = &group.ticket;
        for entry in &group.files {
            map.entry(entry.file.path.clone())
                .or_default()
                .push(FileOwner {
                    key: t.key.clone(),
                    status: t.status,
                    branch: t.branch.clone(),
                    base_branch: t.base_branch.clone(),
                    other_base: t.other_base,
                    needs_you: t.needs_you,
                    file: entry.file.clone(),
                    uncommitted: entry.uncommitted,
                });
        }
    }
    map
}

/// The changed files under `filter` (Changed, Needs you, Same file) as a tree, every folder open with its count:
/// folders first, then by name ignoring case, as the base's tree orders them. `All` lists nothing here (the base's
/// tree is asked for folder by folder).
pub fn changed_rows(changes: &ProjectChanges, filter: Filter) -> Vec<TreeRow> {
    #[derive(Default)]
    struct Folder {
        folders: BTreeMap<(String, String), Folder>,
        files: BTreeMap<(String, String), (String, Vec<FileOwner>)>,
        count: u32,
    }
    let order = |name: &str| (name.to_lowercase(), name.to_owned());
    let mut top = Folder::default();
    for (path, owners) in by_path(changes) {
        let keep = match filter {
            Filter::All => false,
            Filter::Changed => true,
            Filter::NeedsYou => owners.iter().any(|o| o.needs_you),
            Filter::SameFile => same_file(&owners),
        };
        if !keep {
            continue;
        }
        let mut parts: Vec<&str> = path.split('/').collect();
        let Some(name) = parts.pop() else { continue };
        let mut folder = &mut top;
        for part in parts {
            folder = folder.folders.entry(order(part)).or_default();
            folder.count += 1;
        }
        folder.files.insert(order(name), (path.clone(), owners));
    }
    fn walk(folder: &Folder, prefix: &str, depth: u32, rows: &mut Vec<TreeRow>) {
        for ((_, name), inner) in &folder.folders {
            let path = format!("{prefix}{name}");
            rows.push(folder_row(name.clone(), path.clone(), depth, inner.count));
            walk(inner, &format!("{path}/"), depth + 1, rows);
        }
        for ((_, name), (path, owners)) in &folder.files {
            rows.push(file_row(
                name.clone(),
                path.clone(),
                EntryKindView::File,
                depth,
                owners,
            ));
        }
    }
    let mut rows = Vec::new();
    walk(&top, "", 0, &mut rows);
    rows
}

// ---- The view ----

/// The Files view's left column and frame (`files_view.view`). `FilesViewPage` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FilesViewPage")]
pub struct Page {
    pub project: String,
    /// The project's base branch ("develop"): every label names it.
    pub base_branch: String,
    pub source: SourceBar,
    pub search: SearchBox,
    pub filters: Vec<FilterOption>,
    pub filter: Filter,
    /// "Loading the tickets’ changes…" while only the base is listed (`s28-09`).
    pub loading: Option<&'static str>,
    /// All: the top folder's entries; the other filters: their files as a tree, every folder open.
    pub rows: Vec<TreeRow>,
    /// No ticket is changing files (`s28-09`): the file pane says so.
    pub empty: Option<EmptyView>,
    /// The file pane before a file is picked.
    pub pick: &'static str,
    /// A ticket's agent is working: the page asks again every 5 s while the view is shown (commits have no event,
    /// FIL-10).
    pub working: bool,
}

/// The view for `rows`: `counts` is `None` while only the base is listed.
pub fn page(
    source: &ProjectSource,
    counts: Option<&ProjectCounts>,
    filter: Filter,
    rows: Vec<TreeRow>,
    search_keys: Option<String>,
    working: bool,
) -> Page {
    Page {
        project: source.project.clone(),
        base_branch: source.base_branch.clone(),
        source: source_bar(source, counts),
        search: SearchBox {
            placeholder: tab::SEARCH_PLACEHOLDER,
            keys: search_keys,
        },
        filters: filters(counts),
        filter,
        loading: counts.is_none().then_some(LOADING),
        rows,
        empty: counts.filter(|c| c.tickets == 0).map(|_| EmptyView {
            title: EMPTY_TITLE,
            text: EMPTY_TEXT.to_owned(),
        }),
        pick: tab::PICK_A_FILE,
        working,
    }
}

/// An agent is at work on the ticket right now, so its changes move without an event (FIL-10).
pub fn agent_works(status: Status) -> bool {
    matches!(
        status,
        Status::Working
            | Status::WorkingInBackground
            | Status::MergeConflictFixing
            | Status::Checking
    )
}

/// The search's matches (`files.project_search`), as the tab's with the chips.
pub fn search_view(query: &str, found: &ProjectSearch) -> SearchView {
    let rows: Vec<SearchRow> = found
        .matches
        .iter()
        .map(|m| {
            let name_from = if m.folder.is_empty() {
                0
            } else {
                m.folder.chars().count() + 1
            };
            SearchRow {
                path: m.path.clone(),
                name: tab::hit_parts(&m.name, name_from, m.hit),
                folder: tab::hit_parts(&m.folder, 0, m.hit),
                mark: mark(&m.owners),
                quiet: m.owners.is_empty(),
                tags: Vec::new(),
                child: None,
                chips: chips(&m.owners),
                branch: other_branch(&m.owners),
            }
        })
        .collect();
    let query = query.trim();
    SearchView {
        query: query.to_owned(),
        empty: (rows.is_empty() && !query.is_empty())
            .then(|| format!("No file name matches \u{201c}{query}\u{201d}")),
        more: (found.total as usize > rows.len()).then(|| {
            format!(
                "{} of {} shown \u{b7} type more of the name",
                rows.len(),
                found.total
            )
        }),
        rows,
    }
}

// ---- The picker ----

/// What a picked row shows. `FilesViewPick` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "FilesViewPick")]
pub enum Pick {
    /// The base branch with every ticket's changes (the default).
    Base,
    /// A ticket's worktree or an epic's branch: its own Files tab's tree (`files.*`).
    Ticket { key: String },
}

/// One row of the picker: "SHOP-29" "Coupon field" "Dev · 2 files". `FilesViewPickerRow` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FilesViewPickerRow")]
pub struct PickerRow {
    pub pick: Pick,
    /// The ticket's key; `None` for the base branch and an epic (its key is in its title).
    pub key: Option<String>,
    pub title: String,
    pub detail: Option<String>,
}

/// A labelled group of rows. `FilesViewPickerSection` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FilesViewPickerSection")]
pub struct PickerSection {
    pub title: &'static str,
    pub rows: Vec<PickerRow>,
}

/// The picker (FIL-07, `s28-04`). `FilesViewPicker` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FilesViewPicker")]
pub struct Picker {
    pub placeholder: &'static str,
    /// BASE BRANCH, EPICS · THE EPIC’S BRANCH (when there are any), TICKETS · THEIR WORKTREE (when there are any).
    pub sections: Vec<PickerSection>,
    /// "No worktree yet: SHOP-12, SHOP-44 (Grill) … and 3 in Backlog. A ticket gets its worktree when it reaches Dev."
    pub no_worktree: Option<String>,
}

/// "40.1".
fn short(key: &TicketKey) -> String {
    match key.child {
        Some(child) => format!("{}.{child}", key.number),
        None => key.to_string(),
    }
}

fn epic_row(epic: &EpicSource) -> PickerRow {
    let list = |keys: &[TicketKey]| keys.iter().map(short).collect::<Vec<_>>().join(", ");
    let detail: Vec<String> = [
        (!epic.merged.is_empty()).then(|| format!("{} merged", list(&epic.merged))),
        (!epic.on_top.is_empty()).then(|| format!("{} on top", list(&epic.on_top))),
    ]
    .into_iter()
    .flatten()
    .collect();
    PickerRow {
        pick: Pick::Ticket {
            key: epic.key.to_string(),
        },
        key: None,
        title: format!("{} \u{b7} {}", epic.title, epic.key),
        detail: (!detail.is_empty()).then(|| detail.join(" \u{b7} ")),
    }
}

/// "Dev · 3 files", "Dev · release/2.3 · 2 files", "PR open · 1 file".
fn ticket_row(t: &OwnerTicket) -> PickerRow {
    let place = match t.status {
        Status::PrOpen => Some("PR open".to_owned()),
        _ => t.stage.clone(),
    };
    let detail: Vec<String> = [
        place,
        t.other_base.then(|| t.base_branch.clone()),
        Some(plural(t.files, "file", "files")),
    ]
    .into_iter()
    .flatten()
    .collect();
    PickerRow {
        pick: Pick::Ticket {
            key: t.key.to_string(),
        },
        key: Some(t.key.to_string()),
        title: t.title.clone(),
        detail: Some(detail.join(" \u{b7} ")),
    }
}

/// The tickets with no worktree yet, by where they are: "SHOP-12, SHOP-44 (Grill), SHOP-35, SHOP-20 (Ready) and 3 in
/// Backlog", then why ("when it reaches Dev": `first_stage`, the flow's first agent stage).
pub fn no_worktree_line(tickets: &[NoWorktree], first_stage: &str) -> Option<String> {
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    let mut backlog = 0;
    for t in tickets {
        let place = match &t.stage {
            Some(stage) => stage.clone(),
            None if t.status.set() == StatusSet::Backlog => {
                backlog += 1;
                continue;
            }
            None => "Ready".to_owned(),
        };
        match groups.iter_mut().find(|(p, _)| *p == place) {
            Some((_, keys)) => keys.push(t.key.to_string()),
            None => groups.push((place, vec![t.key.to_string()])),
        }
    }
    if groups.is_empty() && backlog == 0 {
        return None;
    }
    let named = groups
        .iter()
        .map(|(place, keys)| format!("{} ({place})", keys.join(", ")))
        .collect::<Vec<_>>()
        .join(", ");
    let list = match (named.is_empty(), backlog) {
        (_, 0) => named,
        (true, n) => format!("{n} in Backlog"),
        (false, n) => format!("{named} and {n} in Backlog"),
    };
    Some(format!(
        "No worktree yet: {list}. A ticket gets its worktree when it reaches {first_stage}."
    ))
}

pub fn picker(sources: &ProjectSources, first_stage: &str) -> Picker {
    let mut sections = vec![PickerSection {
        title: BASE_SECTION,
        rows: vec![PickerRow {
            pick: Pick::Base,
            key: None,
            title: every_change(&sources.source.base_branch),
            detail: Some(THE_DEFAULT.to_owned()),
        }],
    }];
    if !sources.epics.is_empty() {
        sections.push(PickerSection {
            title: EPICS_SECTION,
            rows: sources.epics.iter().map(epic_row).collect(),
        });
    }
    if !sources.tickets.is_empty() {
        sections.push(PickerSection {
            title: TICKETS_SECTION,
            rows: sources.tickets.iter().map(ticket_row).collect(),
        });
    }
    Picker {
        placeholder: FIND,
        sections,
        no_worktree: no_worktree_line(&sources.no_worktree, first_stage),
    }
}

/// A ticket or an epic picked in the picker (`s28-04`, "One ticket’s worktree"): its own Files tab, read-only, with
/// the back link to the base and "Open SHOP-29". `FilesViewPicked` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FilesViewPicked")]
pub struct Picked {
    pub key: String,
    /// "main · every ticket’s changes": back to the base.
    pub back: String,
    /// The tab's frame, its subtitle ending "· read-only".
    pub tab: TabView,
    /// "Open SHOP-29": its sheet on the Files tab.
    pub open: String,
    /// "Only SHOP-29’s changes are marked: the same tree as its Files tab. Open SHOP-29 opens its sheet on that tab."
    pub note: String,
}

pub fn picked(base_branch: &str, mut tab: TabView) -> Picked {
    let key = tab.key.clone();
    tab.source.subtitle = format!("{} \u{b7} {READ_ONLY}", tab.source.subtitle);
    Picked {
        back: every_change(base_branch),
        open: format!("Open {key}"),
        note: format!(
            "Only {key}\u{2019}s changes are marked: the same tree as its Files tab. Open {key} opens its sheet on \
             that tab."
        ),
        tab,
        key,
    }
}

// ---- The example and the gallery ----

/// The example's shop-web (page 28) as the view models read it: the tests' and the gallery's data.
pub mod example {
    use otaq_core::board::files::{ChangedEntry, FilesFrom};
    use otaq_core::board::project_files::ChangeGroup;
    use otaq_core::git::diff::ChangedFile;

    use super::*;

    pub fn key(k: &str) -> TicketKey {
        k.parse().unwrap()
    }

    pub fn file(status: FileStatus, path: &str, added: u32, removed: u32) -> ChangedFile {
        ChangedFile {
            status,
            path: path.to_owned(),
            old_path: None,
            added,
            removed,
            binary: false,
            size: None,
            old_size: None,
            similarity: None,
        }
    }

    pub fn base(branch: &str) -> ProjectSource {
        ProjectSource {
            project: "SHOP".to_owned(),
            base_branch: branch.to_owned(),
            base_sha: "a1b2c3d4".to_owned(),
            base_short: "a1b2c3d".to_owned(),
        }
    }

    pub fn ticket(k: &str, title: &str, status: Status, stage: &str) -> OwnerTicket {
        OwnerTicket {
            key: key(k),
            title: title.to_owned(),
            status,
            stage: (!stage.is_empty()).then(|| stage.to_owned()),
            from: FilesFrom::Worktree,
            branch: otaq_core::naming::branch(k),
            base_branch: "main".to_owned(),
            other_base: false,
            needs_you: status.is_needs_you_family(),
            epic: None,
            files: 0,
            added: 0,
            removed: 0,
        }
    }

    pub fn group(mut t: OwnerTicket, files: Vec<ChangedFile>) -> ChangeGroup {
        t.files = u32::try_from(files.len()).unwrap();
        ChangeGroup {
            ticket: t,
            files: files
                .into_iter()
                .map(|file| ChangedEntry {
                    file,
                    uncommitted: false,
                    tags: Vec::new(),
                    child: None,
                })
                .collect(),
        }
    }

    /// The example's shop-web (page 28): SHOP-22 needs you (3 files, middleware uncommitted), SHOP-18 your check,
    /// SHOP-29 and SHOP-19 both on pricing.ts, SHOP-19 deleting legacy-price.ts, SHOP-36 on release/2.3.
    pub fn shop_web() -> ProjectChanges {
        let mut shop22 = group(
            ticket(
                "SHOP-22",
                "Fix flaky login redirect",
                Status::NeedsYou,
                "Dev",
            ),
            vec![
                file(FileStatus::Modified, "e2e/login.spec.ts", 6, 1),
                file(FileStatus::Modified, "src/app/login/page.tsx", 3, 3),
                file(FileStatus::Modified, "src/middleware.ts", 4, 1),
            ],
        );
        shop22.files[2].uncommitted = true;
        let mut shop18 = ticket("SHOP-18", "Checkout API", Status::WaitingForYou, "Review");
        shop18.needs_you = true;
        let mut shop36 = ticket("SHOP-36", "VAT rounding hotfix", Status::Working, "Dev");
        shop36.other_base = true;
        shop36.base_branch = "release/2.3".to_owned();
        ProjectChanges {
            source: base("main"),
            groups: vec![
                shop22,
                group(
                    shop18,
                    vec![
                        file(FileStatus::Modified, "prisma/schema.prisma", 1, 0),
                        file(
                            FileStatus::Modified,
                            "src/app/api/checkout/coupon/route.ts",
                            2,
                            1,
                        ),
                    ],
                ),
                group(
                    ticket("SHOP-29", "Coupon field", Status::Working, "Dev"),
                    vec![file(FileStatus::Modified, "src/lib/pricing.ts", 7, 2)],
                ),
                group(
                    ticket("SHOP-19", "Price table", Status::Checking, "Security"),
                    vec![
                        file(FileStatus::Deleted, "src/lib/legacy-price.ts", 0, 31),
                        file(FileStatus::Modified, "src/lib/pricing.ts", 5, 2),
                    ],
                ),
                group(
                    shop36,
                    vec![file(FileStatus::Modified, "src/lib/vat.ts", 3, 1)],
                ),
            ],
            counts: ProjectCounts {
                tickets: 5,
                changed: 8,
                needs_you: 5,
                needs_you_tickets: 2,
                same_file: 1,
            },
        }
    }

    pub fn tab_of(k: &str) -> TabView {
        TabView {
            key: k.to_owned(),
            source: tab::SourceView {
                kind: tab::SourceKind::Worktree,
                title: format!("{k}\u{2019}s worktree"),
                subtitle: format!("{} \u{b7} from main", otaq_core::naming::branch(k)),
                legend: Vec::new(),
            },
            search: SearchBox {
                placeholder: tab::SEARCH_PLACEHOLDER,
                keys: None,
            },
            filters: [tab::ALL_FILES.to_owned(), tab::changed_label(2)],
            changed: 2,
            opens_on: tab::Filter::All,
            rows: Vec::new(),
            pick: tab::PICK_A_FILE,
        }
    }
}

/// The gallery's Files view (`gallery.files_view`): shop-web on All with src/lib open and pricing.ts picked, Needs you,
/// the loading and empty states, the picker and SHOP-29's worktree. `FilesViewGallery` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FilesViewGallery")]
pub struct Gallery {
    pub all: Page,
    /// The rows of All with src and src/lib open, as the page draws them.
    pub all_rows: Vec<TreeRow>,
    pub needs_you: Page,
    pub loading: Page,
    /// The bare base's top rows while the tickets' changes load.
    pub loading_rows: Vec<TreeRow>,
    pub empty: Page,
    pub picker: Picker,
    pub picked: Picked,
}

pub fn gallery() -> Gallery {
    use example::*;
    let changes = shop_web();
    let owners = by_path(&changes);
    let entry = |name: &str, path: &str, kind, changed_below| ProjectEntry {
        name: name.to_owned(),
        path: path.to_owned(),
        kind,
        owners: owners.get(path).cloned().unwrap_or_default(),
        changed_below,
        not_on_base: false,
    };
    let top = entry_rows(
        &[
            entry("e2e", "e2e", EntryKind::Folder, 1),
            entry("prisma", "prisma", EntryKind::Folder, 1),
            entry("public", "public", EntryKind::Folder, 0),
            entry("src", "src", EntryKind::Folder, 6),
            entry("package.json", "package.json", EntryKind::File, 0),
            entry("README.md", "README.md", EntryKind::File, 0),
        ],
        0,
    );
    let src = entry_rows(
        &[
            entry("app", "src/app", EntryKind::Folder, 2),
            entry("lib", "src/lib", EntryKind::Folder, 3),
            entry("middleware.ts", "src/middleware.ts", EntryKind::File, 0),
        ],
        1,
    );
    let lib = entry_rows(
        &[
            entry(
                "legacy-price.ts",
                "src/lib/legacy-price.ts",
                EntryKind::File,
                0,
            ),
            entry("money.ts", "src/lib/money.ts", EntryKind::File, 0),
            entry("pricing.ts", "src/lib/pricing.ts", EntryKind::File, 0),
            entry("vat.ts", "src/lib/vat.ts", EntryKind::File, 0),
        ],
        2,
    );
    let bare = |e: &ProjectEntry| ProjectEntry {
        owners: Vec::new(),
        changed_below: 0,
        ..e.clone()
    };
    let loading_rows = entry_rows(
        &[
            bare(&entry("e2e", "e2e", EntryKind::Folder, 0)),
            bare(&entry("prisma", "prisma", EntryKind::Folder, 0)),
            bare(&entry("src", "src", EntryKind::Folder, 0)),
            bare(&entry("package.json", "package.json", EntryKind::File, 0)),
        ],
        0,
    );
    let mut all_rows = top[..3].to_vec();
    all_rows.push(top[3].clone());
    all_rows.push(src[0].clone());
    all_rows.push(src[1].clone());
    all_rows.extend(lib);
    all_rows.push(src[2].clone());
    all_rows.extend(top[4..].iter().cloned());
    let counts = changes.counts;
    let source = changes.source.clone();
    let keys = Some("\u{2325}\u{2318}P".to_owned());
    let sources = ProjectSources {
        source: source.clone(),
        epics: vec![EpicSource {
            key: key("SHOP-40"),
            title: "Wishlist".to_owned(),
            status: Status::Working,
            branch: otaq_core::naming::branch("SHOP-40"),
            merged: vec![key("SHOP-40.1")],
            on_top: vec![key("SHOP-40.2")],
        }],
        tickets: changes.groups.iter().map(|g| g.ticket.clone()).collect(),
        no_worktree: vec![NoWorktree {
            key: key("SHOP-12"),
            title: String::new(),
            status: Status::Working,
            stage: Some("Grill".to_owned()),
        }],
    };
    Gallery {
        all: page(&source, Some(&counts), Filter::All, top, keys.clone(), true),
        all_rows,
        needs_you: page(
            &source,
            Some(&counts),
            Filter::NeedsYou,
            changed_rows(&changes, Filter::NeedsYou),
            keys.clone(),
            true,
        ),
        loading: page(&source, None, Filter::All, Vec::new(), keys.clone(), false),
        loading_rows,
        empty: page(
            &source,
            Some(&ProjectCounts::default()),
            Filter::All,
            Vec::new(),
            keys,
            false,
        ),
        picker: picker(&sources, "Dev"),
        picked: picked("main", tab_of("SHOP-29")),
    }
}

#[cfg(test)]
mod tests {
    use super::example::*;
    use super::*;

    fn texts(rows: &[TreeRow]) -> Vec<String> {
        rows.iter()
            .map(|r| {
                let mut parts = vec![format!("{}{}", "  ".repeat(r.depth as usize), r.name)];
                parts.extend(r.changed_below.clone());
                parts.extend(r.mark.as_ref().map(|m| m.letter.to_owned()));
                parts.extend(r.chips.iter().map(|c| c.key.clone()));
                parts.extend(r.branch.clone());
                parts.extend(r.mark.as_ref().and_then(|m| m.counts.clone()));
                parts.join(" ")
            })
            .collect()
    }

    /// `s28-03`'s source bar and filters; `s28-09`'s loading and empty ones; the Needs you tooltip.
    #[test]
    fn the_source_bar_and_the_filters_count() {
        let counts = ProjectCounts {
            tickets: 8,
            changed: 11,
            needs_you: 6,
            needs_you_tickets: 4,
            same_file: 1,
        };
        let bar = source_bar(&base("main"), Some(&counts));
        assert_eq!(bar.title, "main \u{b7} every ticket\u{2019}s changes");
        assert_eq!(bar.subtitle, "the base branch \u{b7} 8 tickets, 11 files");
        let shown: Vec<String> = filters(Some(&counts))
            .into_iter()
            .map(|f| f.label)
            .collect();
        assert_eq!(shown, ["All", "Changed 11", "Needs you 6", "Same file 1"]);
        let needs = &filters(Some(&counts))[2];
        assert_eq!(needs.tooltip.as_deref(), Some("Needs you \u{b7} 6 files"));
        assert_eq!(
            needs.detail.as_deref(),
            Some("changed by the 4 tickets that need you")
        );
        // Loading: the base at once, the counts as "…".
        let loading: Vec<String> = filters(None).into_iter().map(|f| f.label).collect();
        assert_eq!(
            loading,
            [
                "All",
                "Changed \u{2026}",
                "Needs you \u{2026}",
                "Same file \u{2026}"
            ]
        );
        let p = page(&base("main"), None, Filter::All, Vec::new(), None, false);
        assert_eq!(
            p.loading,
            Some("Loading the tickets\u{2019} changes\u{2026}")
        );
        assert_eq!(p.empty, None);
        // Nothing changing.
        let none = ProjectCounts::default();
        assert_eq!(
            source_bar(&base("main"), Some(&none)).subtitle,
            "the base branch \u{b7} no ticket is changing files"
        );
        let p = page(
            &base("main"),
            Some(&none),
            Filter::All,
            Vec::new(),
            Some("⌥⌘P".into()),
            false,
        );
        let empty = p.empty.unwrap();
        assert_eq!(
            (empty.title, empty.text.as_str()),
            (
                "No ticket is changing files right now.",
                "Start one from the board, or ask the Project agent."
            )
        );
        assert_eq!(
            filters(Some(&none))
                .iter()
                .map(|f| f.label.as_str())
                .collect::<Vec<_>>(),
            ["All", "Changed 0", "Needs you 0", "Same file 0"]
        );
        assert_eq!(
            (p.search.placeholder, p.search.keys.as_deref()),
            ("Search files by name", Some("⌥⌘P"))
        );
    }

    /// billing-api's base is develop: every label says develop (FIL-02, `s28-08`).
    #[test]
    fn billing_api_says_develop() {
        let counts = ProjectCounts {
            tickets: 1,
            changed: 1,
            ..ProjectCounts::default()
        };
        let mut source = base("develop");
        source.project = "BILL".to_owned();
        let p = page(&source, Some(&counts), Filter::All, Vec::new(), None, false);
        assert_eq!(p.base_branch, "develop");
        assert_eq!(
            p.source.title,
            "develop \u{b7} every ticket\u{2019}s changes"
        );
        assert_eq!(p.source.subtitle, "the base branch \u{b7} 1 ticket, 1 file");
        let sources = ProjectSources {
            source,
            epics: Vec::new(),
            tickets: Vec::new(),
            no_worktree: Vec::new(),
        };
        let pick = picker(&sources, "Dev");
        assert_eq!(
            pick.sections[0].rows[0].title,
            "develop \u{b7} every ticket\u{2019}s changes"
        );
        assert_eq!(pick.sections.len(), 1);
        assert_eq!(pick.no_worktree, None);
        let picked = picked("develop", tab_of("BILL-7"));
        assert_eq!(picked.back, "develop \u{b7} every ticket\u{2019}s changes");
    }

    /// The base's entries (`files.project_tree`): chips in status colours, marks, +/− for one owner only, folder
    /// counts, the other base's tag, a deleted file struck through, an unchanged file quieter.
    #[test]
    fn the_tree_has_chips_and_counts() {
        let changes = shop_web();
        let owners = by_path(&changes);
        let entry = |name: &str, path: &str, kind, changed_below| ProjectEntry {
            name: name.to_owned(),
            path: path.to_owned(),
            kind,
            owners: owners.get(path).cloned().unwrap_or_default(),
            changed_below,
            not_on_base: false,
        };
        let rows = entry_rows(
            &[
                entry(
                    "legacy-price.ts",
                    "src/lib/legacy-price.ts",
                    EntryKind::File,
                    0,
                ),
                entry("money.ts", "src/lib/money.ts", EntryKind::File, 0),
                entry("pricing.ts", "src/lib/pricing.ts", EntryKind::File, 0),
                entry("vat.ts", "src/lib/vat.ts", EntryKind::File, 0),
            ],
            2,
        );
        assert_eq!(
            texts(&rows),
            [
                "    legacy-price.ts D SHOP-19 \u{2212}31",
                "    money.ts",
                "    pricing.ts M SHOP-29 SHOP-19",
                "    vat.ts M SHOP-36 release/2.3 +3 \u{2212}1",
            ]
        );
        assert!(rows[0].struck && !rows[2].struck);
        assert!(rows[1].quiet && !rows[2].quiet);
        assert_eq!(
            rows[2].chips.iter().map(|c| c.tone).collect::<Vec<_>>(),
            [Tone::Working, Tone::Working]
        );
        let folder = entry_rows(&[entry("src", "src", EntryKind::Folder, 8)], 0);
        assert_eq!(texts(&folder), ["src 8 changed"]);
        assert_eq!(folder[0].mark, None);
        // SHOP-22 needs you: turquoise; SHOP-18 in your check: also needs you; its middleware is uncommitted.
        let mid = entry_rows(
            &[entry(
                "middleware.ts",
                "src/middleware.ts",
                EntryKind::File,
                0,
            )],
            1,
        );
        assert_eq!(mid[0].chips[0].tone, Tone::Attention);
        assert_eq!(
            mid[0].mark.as_ref().unwrap().uncommitted,
            Some("uncommitted")
        );
        assert_eq!(tone(Status::WaitingForYou, false), Tone::Review);
        assert_eq!(tone(Status::PrOpen, false), Tone::Muted);
        assert_eq!(tone(Status::CheckFailed, false), Tone::Error);
    }

    /// Changed, Needs you and Same file list their files as a tree, every folder open with its count.
    #[test]
    fn each_filter_lists_its_files() {
        let changes = shop_web();
        assert_eq!(
            texts(&changed_rows(&changes, Filter::Changed)),
            [
                "e2e 1 changed",
                "  login.spec.ts M SHOP-22 +6 \u{2212}1",
                "prisma 1 changed",
                "  schema.prisma M SHOP-18 +1",
                "src 6 changed",
                "  app 2 changed",
                "    api 1 changed",
                "      checkout 1 changed",
                "        coupon 1 changed",
                "          route.ts M SHOP-18 +2 \u{2212}1",
                "    login 1 changed",
                "      page.tsx M SHOP-22 +3 \u{2212}3",
                "  lib 3 changed",
                "    legacy-price.ts D SHOP-19 \u{2212}31",
                "    pricing.ts M SHOP-29 SHOP-19",
                "    vat.ts M SHOP-36 release/2.3 +3 \u{2212}1",
                "  middleware.ts M SHOP-22 +4 \u{2212}1",
            ]
        );
        assert_eq!(
            texts(&changed_rows(&changes, Filter::NeedsYou)),
            [
                "e2e 1 changed",
                "  login.spec.ts M SHOP-22 +6 \u{2212}1",
                "prisma 1 changed",
                "  schema.prisma M SHOP-18 +1",
                "src 3 changed",
                "  app 2 changed",
                "    api 1 changed",
                "      checkout 1 changed",
                "        coupon 1 changed",
                "          route.ts M SHOP-18 +2 \u{2212}1",
                "    login 1 changed",
                "      page.tsx M SHOP-22 +3 \u{2212}3",
                "  middleware.ts M SHOP-22 +4 \u{2212}1",
            ]
        );
        assert_eq!(
            texts(&changed_rows(&changes, Filter::SameFile)),
            [
                "src 1 changed",
                "  lib 1 changed",
                "    pricing.ts M SHOP-29 SHOP-19",
            ]
        );
        assert!(changed_rows(&changes, Filter::All).is_empty());
        // A ticket on another base never makes a Same file with one on the project's base.
        let mut apart = shop_web();
        apart.groups[4].files[0].file.path = "src/lib/pricing.ts".to_owned();
        assert_eq!(texts(&changed_rows(&apart, Filter::SameFile)).len(), 3);
    }

    /// The search's matches carry the owners' chips and the other base's tag.
    #[test]
    fn search_matches_have_chips() {
        let owners = by_path(&shop_web());
        let found = ProjectSearch {
            source: base("main"),
            query: "pri".to_owned(),
            matches: vec![otaq_core::board::project_files::ProjectMatch {
                path: "src/lib/pricing.ts".to_owned(),
                name: "pricing.ts".to_owned(),
                folder: "src/lib".to_owned(),
                hit: [8, 11],
                owners: owners["src/lib/pricing.ts"].clone(),
                not_on_base: false,
            }],
            total: 1,
        };
        let v = search_view("pri", &found);
        assert_eq!(v.rows[0].chips.len(), 2);
        assert_eq!(v.rows[0].name[0].text, "pri");
        assert!(v.rows[0].name[0].hit);
        assert_eq!(v.empty, None);
        let none = ProjectSearch {
            matches: Vec::new(),
            total: 0,
            ..found
        };
        assert_eq!(
            search_view("zzq", &none).empty.as_deref(),
            Some("No file name matches \u{201c}zzq\u{201d}")
        );
    }

    /// `s28-04`: BASE BRANCH, the epic with its children, the tickets in flow order with stage and files, the line
    /// naming the tickets with no worktree yet.
    #[test]
    fn the_picker_has_its_sections_in_order() {
        let mut shop36 = ticket("SHOP-36", "VAT rounding hotfix", Status::Working, "Dev");
        shop36.other_base = true;
        shop36.base_branch = "release/2.3".to_owned();
        let with_files = |mut t: OwnerTicket, n| {
            t.files = n;
            t
        };
        let sources = ProjectSources {
            source: base("main"),
            epics: vec![EpicSource {
                key: key("SHOP-40"),
                title: "Wishlist".to_owned(),
                status: Status::Working,
                branch: otaq_core::naming::branch("SHOP-40"),
                merged: vec![key("SHOP-40.1")],
                on_top: vec![key("SHOP-40.2")],
            }],
            tickets: vec![
                with_files(
                    ticket(
                        "SHOP-22",
                        "Fix flaky login redirect",
                        Status::NeedsYou,
                        "Dev",
                    ),
                    3,
                ),
                with_files(ticket("SHOP-29", "Coupon field", Status::Working, "Dev"), 2),
                with_files(
                    ticket("SHOP-19", "Price table", Status::Checking, "Security"),
                    2,
                ),
                with_files(ticket("SHOP-26", "Add sitemap", Status::PrOpen, "Ship"), 1),
                with_files(shop36, 2),
            ],
            no_worktree: vec![
                NoWorktree {
                    key: key("SHOP-12"),
                    title: String::new(),
                    status: Status::Working,
                    stage: Some("Grill".to_owned()),
                },
                NoWorktree {
                    key: key("SHOP-44"),
                    title: String::new(),
                    status: Status::Working,
                    stage: Some("Grill".to_owned()),
                },
                NoWorktree {
                    key: key("SHOP-35"),
                    title: String::new(),
                    status: Status::Queued,
                    stage: None,
                },
                NoWorktree {
                    key: key("SHOP-20"),
                    title: String::new(),
                    status: Status::Queued,
                    stage: None,
                },
            ]
            .into_iter()
            .chain((1..=3).map(|n| NoWorktree {
                key: key(&format!("SHOP-5{n}")),
                title: String::new(),
                status: Status::Backlog,
                stage: None,
            }))
            .collect(),
        };
        let p = picker(&sources, "Dev");
        assert_eq!(p.placeholder, "Find a ticket or epic");
        let shown: Vec<(&str, Vec<String>)> = p
            .sections
            .iter()
            .map(|s| {
                (
                    s.title,
                    s.rows
                        .iter()
                        .map(|r| {
                            [r.key.clone(), Some(r.title.clone()), r.detail.clone()]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(" | ")
                        })
                        .collect(),
                )
            })
            .collect();
        assert_eq!(
            shown,
            [
                (
                    "BASE BRANCH",
                    vec!["main \u{b7} every ticket\u{2019}s changes | the default".to_owned()]
                ),
                (
                    "EPICS \u{b7} THE EPIC\u{2019}S BRANCH",
                    vec!["Wishlist \u{b7} SHOP-40 | 40.1 merged \u{b7} 40.2 on top".to_owned()]
                ),
                (
                    "TICKETS \u{b7} THEIR WORKTREE",
                    vec![
                        "SHOP-22 | Fix flaky login redirect | Dev \u{b7} 3 files".to_owned(),
                        "SHOP-29 | Coupon field | Dev \u{b7} 2 files".to_owned(),
                        "SHOP-19 | Price table | Security \u{b7} 2 files".to_owned(),
                        "SHOP-26 | Add sitemap | PR open \u{b7} 1 file".to_owned(),
                        "SHOP-36 | VAT rounding hotfix | Dev \u{b7} release/2.3 \u{b7} 2 files"
                            .to_owned(),
                    ]
                ),
            ]
        );
        assert_eq!(
            p.no_worktree.as_deref(),
            Some(
                "No worktree yet: SHOP-12, SHOP-44 (Grill), SHOP-35, SHOP-20 (Ready) and 3 in Backlog. A ticket \
                 gets its worktree when it reaches Dev."
            )
        );
        assert_eq!(
            p.sections[2].rows[1].pick,
            Pick::Ticket {
                key: "SHOP-29".to_owned()
            }
        );
        assert_eq!(p.sections[0].rows[0].pick, Pick::Base);
        assert_eq!(
            no_worktree_line(&sources.no_worktree[4..], "Dev").as_deref(),
            Some("No worktree yet: 3 in Backlog. A ticket gets its worktree when it reaches Dev.")
        );
    }

    /// One ticket's worktree (`s28-04`): its tab's tree, read-only, the back link, Open SHOP-29 and the note.
    #[test]
    fn one_ticket_is_its_tab_with_the_back_link() {
        let p = picked("main", tab_of("SHOP-29"));
        assert_eq!(p.back, "main \u{b7} every ticket\u{2019}s changes");
        assert_eq!(p.tab.source.title, "SHOP-29\u{2019}s worktree");
        assert_eq!(
            p.tab.source.subtitle,
            format!(
                "{} \u{b7} from main \u{b7} read-only",
                otaq_core::naming::branch("SHOP-29")
            )
        );
        assert_eq!(p.tab.filters, ["All files", "Changed \u{b7} 2"]);
        assert_eq!(p.open, "Open SHOP-29");
        assert_eq!(
            p.note,
            "Only SHOP-29\u{2019}s changes are marked: the same tree as its Files tab. Open SHOP-29 opens its sheet \
             on that tab."
        );
    }

    /// FIL-10: the page asks again while an agent works, never for a ticket waiting on you or the PR.
    #[test]
    fn working_agents_keep_it_fresh() {
        assert!(agent_works(Status::Working));
        assert!(agent_works(Status::Checking));
        assert!(!agent_works(Status::NeedsYou));
        assert!(!agent_works(Status::WaitingForYou));
        assert!(!agent_works(Status::PrOpen));
    }
}
