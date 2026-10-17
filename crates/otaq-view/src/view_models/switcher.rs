//! The project switcher (UI-01; d0024, d0058; *02 · App shell & project switcher*): the rail keeps the user's
//! order so tiles never jump; the quick switcher (⌘P) sorts projects that need you first, then working ones,
//! then idle ones, and filters as you type. ⌘1–9 switch to the rail's first nine projects. A project that
//! newly needs you while another is shown gets a turquoise ring for 10 s, then keeps its badge. The rail is the
//! 64 px column of tiles at every window width (d0087; Q-H); at its bottom Cleanup, Terminals and Settings
//! (CHANGES-v §3).

use std::time::{Duration, Instant};

use otaq_core::board::snapshot::ProjectInfo;
use otaq_core::project_agents::AgentNeedsYou;
use serde::Serialize;
use ts_rs::TS;

use crate::keymap::{KeyAction, Platform};
use crate::shell::tilde;

/// How long a project's tile rings after it newly needs you.
pub const RING: Duration = Duration::from_secs(10);

/// The rail's width at every window width: one column of tiles (d0087, as every V window draws it).
pub const RAIL_WIDTH: u32 = 64;

/// A button at the rail's bottom: its name (the tooltip and the label for screen readers) and its keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "RailButton")]
pub struct RailButton {
    pub label: &'static str,
    /// "⇧⌘T" on this platform.
    pub keys: Option<String>,
}

/// The rail's bottom after Cleanup (which keeps its own view, `shell.cleanup`): Terminals (⇧⌘T, the drawer on the
/// last-used Project terminal; no dot: agents that wait show in the pane and the inbox) and Settings (⌘,).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "RailBottom")]
pub struct RailBottom {
    pub terminals: RailButton,
    pub settings: RailButton,
}

pub fn bottom(platform: Platform) -> RailBottom {
    let button = |label, action: KeyAction| RailButton {
        label,
        keys: action.keys(platform),
    };
    RailBottom {
        terminals: button("Terminals", KeyAction::ProjectTerminal),
        settings: button("Settings", KeyAction::Settings),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct RailItem {
    pub key: String,
    pub name: String,
    pub repo_path: String,
    /// "shop-web · SHOP": the quick switcher's label, and the tooltip's title.
    pub title: String,
    /// "~/code/shop-web".
    pub path: String,
    pub agents_working: u32,
    pub needs_you: u32,
    pub selected: bool,
    /// Nothing running and nothing waiting: drawn quietly.
    pub idle: bool,
    pub ringing: bool,
    /// "⌘2" for the second project.
    pub shortcut: Option<String>,
    /// The tooltip's lines under the title: "1 needs you", "2 running", or "idle".
    pub detail: Vec<String>,
    /// The tooltip's last line: "⌘2 to switch".
    pub note: Option<String>,
}

/// One row of the quick switcher: the project, "shop-web · SHOP · current" for the shown one, and "2 running".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "QuickSwitcherRow")]
pub struct QuickRow {
    pub project: RailItem,
    pub label: String,
    pub running: Option<String>,
}

impl From<RailItem> for QuickRow {
    fn from(project: RailItem) -> Self {
        let label = if project.selected {
            format!("{} · current", project.title)
        } else {
            project.title.clone()
        };
        let running =
            (project.agents_working > 0).then(|| format!("{} running", project.agents_working));
        Self {
            project,
            label,
            running,
        }
    }
}

/// Adds the Project agents waiting for the user to their projects' counts (ATT-10, PAG-07): the project list
/// counts tickets only, the rail and the quick switcher count both, as the header does.
pub fn add_waiting_agents(projects: &mut [ProjectInfo], agents: &[AgentNeedsYou]) {
    for p in projects {
        let waiting = agents.iter().filter(|a| a.project == p.key).count();
        p.needs_you = p
            .needs_you
            .saturating_add(u32::try_from(waiting).unwrap_or(u32::MAX));
    }
}

/// The rail and quick switcher's state.
#[derive(Debug, Default)]
pub struct SwitcherModel {
    projects: Vec<ProjectInfo>,
    pub selected: Option<String>,
    ring_until: Vec<(String, Instant)>,
}

impl SwitcherModel {
    /// Takes a fresh project list. Keeps the selection if the project still exists, else selects the first.
    pub fn set_projects(&mut self, projects: Vec<ProjectInfo>, now: Instant) {
        for p in &projects {
            let before = self
                .projects
                .iter()
                .find(|o| o.key == p.key)
                .map_or(0, |o| o.needs_you);
            let shown = self.selected.as_deref() == Some(p.key.as_str());
            if p.needs_you > before && !shown && !self.projects.is_empty() {
                self.ring_until.retain(|(k, _)| *k != p.key);
                self.ring_until.push((p.key.clone(), now + RING));
            }
        }
        self.projects = projects;
        let still_there = self
            .selected
            .as_ref()
            .is_some_and(|s| self.projects.iter().any(|p| &p.key == s));
        if !still_there {
            self.selected = self.projects.first().map(|p| p.key.clone());
        }
    }

    pub fn projects(&self) -> &[ProjectInfo] {
        &self.projects
    }

    pub fn current(&self) -> Option<&ProjectInfo> {
        let key = self.selected.as_ref()?;
        self.projects.iter().find(|p| &p.key == key)
    }

    pub fn select(&mut self, key: &str) -> bool {
        if self.projects.iter().any(|p| p.key == key) {
            self.selected = Some(key.to_owned());
            self.ring_until.retain(|(k, _)| k != key);
            true
        } else {
            false
        }
    }

    /// ⌘1–9: the project at that rail position.
    pub fn project_for_digit(&self, digit: u8) -> Option<&str> {
        let i = usize::from(digit.checked_sub(1)?);
        (digit <= 9).then(|| self.projects.get(i).map(|p| p.key.as_str()))?
    }

    /// How long until the next ring is over, so the owner can redraw then; `None` while nothing rings.
    pub fn next_ring_end(&self, now: Instant) -> Option<Duration> {
        self.ring_until
            .iter()
            .filter(|(_, until)| *until > now)
            .map(|(_, until)| *until - now)
            .min()
    }

    /// Drops rings that are over; true when something changed.
    pub fn expire(&mut self, now: Instant) -> bool {
        let before = self.ring_until.len();
        self.ring_until.retain(|(_, until)| *until > now);
        before != self.ring_until.len()
    }

    /// The rail, in the user's (stored) order.
    pub fn rail(&self, now: Instant, platform: Platform) -> Vec<RailItem> {
        self.projects
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let shortcut = (i < 9)
                    .then(|| KeyAction::Project(i as u8 + 1).keys(platform))
                    .flatten();
                let mut detail = Vec::new();
                if p.needs_you > 0 {
                    detail.push(format!("{} needs you", p.needs_you));
                }
                if p.agents_working > 0 {
                    detail.push(format!("{} running", p.agents_working));
                }
                if detail.is_empty() {
                    detail.push("idle".to_owned());
                }
                RailItem {
                    key: p.key.clone(),
                    name: p.name.clone(),
                    repo_path: p.repo_path.clone(),
                    title: format!("{} · {}", p.name, p.key),
                    path: tilde(&p.repo_path),
                    agents_working: p.agents_working,
                    needs_you: p.needs_you,
                    selected: self.selected.as_deref() == Some(p.key.as_str()),
                    idle: p.agents_working == 0 && p.needs_you == 0,
                    ringing: self
                        .ring_until
                        .iter()
                        .any(|(k, until)| *k == p.key && *until > now),
                    note: shortcut.as_ref().map(|k| format!("{k} to switch")),
                    shortcut,
                    detail,
                }
            })
            .collect()
    }

    /// The quick switcher's rows for `filter`: needs-you first, then working, then idle; rail order inside.
    pub fn quick(&self, filter: &str, now: Instant, platform: Platform) -> Vec<RailItem> {
        let needle = filter.trim().to_lowercase();
        let mut rows: Vec<RailItem> = self
            .rail(now, platform)
            .into_iter()
            .filter(|r| {
                needle.is_empty()
                    || r.name.to_lowercase().contains(&needle)
                    || r.key.to_lowercase().contains(&needle)
            })
            .collect();
        rows.sort_by_key(|r| {
            if r.needs_you > 0 {
                0
            } else if r.agents_working > 0 {
                1
            } else {
                2
            }
        });
        rows
    }
}

/// The quick switcher's highlighted row.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct QuickCursor {
    pub index: usize,
}

impl QuickCursor {
    pub fn down(&mut self, rows: usize) {
        if rows > 0 {
            self.index = (self.index + 1) % rows;
        }
    }

    pub fn up(&mut self, rows: usize) {
        if rows > 0 {
            self.index = (self.index + rows - 1) % rows;
        }
    }

    /// Keeps the cursor on a row after the list shrank.
    pub fn clamp(&mut self, rows: usize) {
        if self.index >= rows {
            self.index = rows.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(key: &str, name: &str, working: u32, needs: u32) -> ProjectInfo {
        ProjectInfo {
            key: key.into(),
            name: name.into(),
            repo_path: format!("/code/{name}"),
            base_branch: "main".into(),
            agents_working: working,
            needs_you: needs,
        }
    }

    fn sample() -> Vec<ProjectInfo> {
        vec![
            project("SHOP", "shop-web", 2, 6),
            project("CREW", "crew", 0, 0),
            project("BILL", "billing-api", 1, 1),
            project("PORT", "customer-portal-frontend", 3, 0),
        ]
    }

    #[test]
    fn the_rail_ends_with_terminals_and_settings() {
        let mac = bottom(Platform::Mac);
        assert_eq!(mac.terminals.label, "Terminals");
        assert_eq!(mac.terminals.keys.as_deref(), Some("⇧⌘T"));
        assert_eq!(mac.settings.label, "Settings");
        assert_eq!(mac.settings.keys.as_deref(), Some("⌘,"));
        assert_eq!(
            bottom(Platform::WinLinux).settings.keys.as_deref(),
            Some("Ctrl+,")
        );
        assert_eq!(RAIL_WIDTH, 64);
    }

    #[test]
    fn waiting_project_agents_count_in_their_project() {
        let agent = |project: &str, id: &str| AgentNeedsYou {
            project: project.into(),
            project_name: String::new(),
            agent: id.into(),
            name: "Import GitHub issues".into(),
            epic: None,
            status: otaq_core::domain::Status::AsksYou,
            reason: String::new(),
            question: None,
            since: 0,
        };
        let mut projects = sample();
        add_waiting_agents(&mut projects, &[agent("CREW", "a1"), agent("SHOP", "a2")]);
        let counts: Vec<_> = projects.iter().map(|p| p.needs_you).collect();
        assert_eq!(counts, [7, 1, 1, 0]);
    }

    #[test]
    fn rail_keeps_the_order_and_counts() {
        let now = Instant::now();
        let mut m = SwitcherModel::default();
        m.set_projects(sample(), now);
        let rail = m.rail(now, Platform::Mac);
        let keys: Vec<_> = rail.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, ["SHOP", "CREW", "BILL", "PORT"]);
        assert!(rail[0].selected, "the first project is selected");
        assert!(rail[1].idle && !rail[0].idle);
        assert_eq!((rail[2].needs_you, rail[2].agents_working), (1, 1));
        assert_eq!(rail[1].shortcut.as_deref(), Some("⌘2"));
    }

    #[test]
    fn quick_switcher_sorts_and_filters() {
        let now = Instant::now();
        let mut m = SwitcherModel::default();
        m.set_projects(sample(), now);
        let keys = |rows: Vec<RailItem>| rows.into_iter().map(|r| r.key).collect::<Vec<_>>();
        assert_eq!(
            keys(m.quick("", now, Platform::Mac)),
            ["SHOP", "BILL", "PORT", "CREW"]
        );
        assert_eq!(keys(m.quick("bill", now, Platform::Mac)), ["BILL"]);
        assert_eq!(keys(m.quick("PORT", now, Platform::Mac)), ["PORT"]);
        assert!(m.quick("nothing", now, Platform::Mac).is_empty());
    }

    #[test]
    fn digits_and_selection() {
        let now = Instant::now();
        let mut m = SwitcherModel::default();
        m.set_projects(sample(), now);
        assert_eq!(m.project_for_digit(3), Some("BILL"));
        assert_eq!(m.project_for_digit(5), None);
        assert_eq!(m.project_for_digit(0), None);
        assert!(m.select("BILL"));
        assert!(!m.select("NOPE"));
        assert_eq!(m.current().unwrap().key, "BILL");
        // The selected project disappears: the first one takes over.
        m.set_projects(vec![project("SHOP", "shop-web", 0, 0)], now);
        assert_eq!(m.selected.as_deref(), Some("SHOP"));
    }

    #[test]
    fn another_project_needing_you_rings_for_ten_seconds() {
        let now = Instant::now();
        let mut m = SwitcherModel::default();
        m.set_projects(sample(), now);
        let mut next = sample();
        next[2].needs_you = 2;
        next[0].needs_you = 7;
        m.set_projects(next, now);
        let rail = m.rail(now, Platform::Mac);
        assert!(rail[2].ringing, "BILL isn't shown, so it rings");
        assert!(!rail[0].ringing, "the shown project doesn't ring");
        assert!(!m.expire(now + Duration::from_secs(5)));
        assert!(m.expire(now + RING + Duration::from_millis(1)));
        assert!(!m.rail(now + RING, Platform::Mac)[2].ringing);
        assert_eq!(
            m.rail(now + RING, Platform::Mac)[2].needs_you,
            2,
            "the badge stays"
        );
    }

    #[test]
    fn tooltips_and_quick_rows_say_what_waits() {
        let now = Instant::now();
        let mut m = SwitcherModel::default();
        m.set_projects(sample(), now);
        let rail = m.rail(now, Platform::Mac);
        assert_eq!(rail[2].title, "billing-api · BILL");
        assert_eq!(rail[2].detail, ["1 needs you", "1 running"]);
        assert_eq!(rail[2].note.as_deref(), Some("⌘3 to switch"));
        assert_eq!(rail[1].detail, ["idle"]);
        assert_eq!(rail[2].path, "/code/billing-api");
        let rows: Vec<QuickRow> = m
            .quick("", now, Platform::Mac)
            .into_iter()
            .map(QuickRow::from)
            .collect();
        assert_eq!(rows[0].label, "shop-web · SHOP · current");
        assert_eq!(rows[0].running.as_deref(), Some("2 running"));
        assert_eq!(rows[3].label, "crew · CREW");
        assert_eq!(rows[3].running, None);
    }

    #[test]
    fn the_next_ring_end_is_reported_until_rings_are_over() {
        let now = Instant::now();
        let mut m = SwitcherModel::default();
        m.set_projects(sample(), now);
        assert_eq!(m.next_ring_end(now), None);
        let mut next = sample();
        next[1].needs_you = 1;
        m.set_projects(next, now + Duration::from_secs(2));
        assert_eq!(
            m.next_ring_end(now + Duration::from_secs(4)),
            Some(RING - Duration::from_secs(2))
        );
        assert_eq!(m.next_ring_end(now + RING + Duration::from_secs(2)), None);
    }

    #[test]
    fn cursor_wraps() {
        let mut c = QuickCursor::default();
        c.up(3);
        assert_eq!(c.index, 2);
        c.down(3);
        assert_eq!(c.index, 0);
        c.index = 5;
        c.clamp(2);
        assert_eq!(c.index, 1);
    }
}
