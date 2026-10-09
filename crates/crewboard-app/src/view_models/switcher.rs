//! The project switcher (UI-01; d0024, d0058; *02 · App shell & project switcher*): the rail keeps the user's
//! order so tiles never jump; the quick switcher (⌘P) sorts projects that need you first, then working ones,
//! then idle ones, and filters as you type. ⌘1–9 switch to the rail's first nine projects. A project that
//! newly needs you while another is shown gets an orange ring for 10 s, then keeps its badge.

use std::time::{Duration, Instant};

use crewboard_core::board::snapshot::ProjectInfo;

use crate::keymap::{KeyAction, Platform};

/// How long a project's tile rings after it newly needs you.
pub const RING: Duration = Duration::from_secs(10);

/// Below this window width the rail shows tiles; above it, a 232 px sidebar with names and counts.
pub const SIDEBAR_FROM_WIDTH: f32 = 1600.;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RailItem {
    pub key: String,
    pub name: String,
    pub repo_path: String,
    pub agents_working: u32,
    pub needs_you: u32,
    pub selected: bool,
    /// Nothing running and nothing waiting: drawn quietly.
    pub idle: bool,
    pub ringing: bool,
    /// "⌘2" for the second project.
    pub shortcut: Option<String>,
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
            .map(|(i, p)| RailItem {
                key: p.key.clone(),
                name: p.name.clone(),
                repo_path: p.repo_path.clone(),
                agents_working: p.agents_working,
                needs_you: p.needs_you,
                selected: self.selected.as_deref() == Some(p.key.as_str()),
                idle: p.agents_working == 0 && p.needs_you == 0,
                ringing: self
                    .ring_until
                    .iter()
                    .any(|(k, until)| *k == p.key && *until > now),
                shortcut: (i < 9)
                    .then(|| KeyAction::Project(i as u8 + 1).keys(platform))
                    .flatten(),
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
