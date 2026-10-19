//! The window's layout (SHL-01, d0087, *02 → Row · Window 1440 × 900 (0086 · V)*): project rail | Project agent pane
//! | main area with the **Board | Files** tabs, while [`FILES_VIEW`] is on. The pane's widths per window size class, its drag range and its
//! hidden strip; the tabs' texts and the user's keys. The page asks at start (`layout.view`) and again on
//! `settings_changed` (KEY-08: a key changed in Settings shows at once); only the drag of a width, which tab shows and
//! whether the pane is hidden stay on the page, per window.
//!
//! The ticket panel's sizes are `panel.rs`'s; it opens over the main area (SHL-05) and uses [`SizeClass`] too.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::key_labels::KeyLabels;
use crate::keymap::KeyAction;

/// Which of the design's three windows a window is closest to (Q-C): the pane's default width and the panel's
/// remembered width are kept per class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SizeClass {
    W1280,
    W1440,
    W1920,
}

/// A class and the window width it ends below (`None`: the last class), narrowest first. The page picks a class
/// from this table, so [`size_class`] is the only rule.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[ts(rename = "SizeClassBound")]
pub struct ClassBound {
    pub class: SizeClass,
    pub below: Option<f32>,
}

/// The classes split at the midpoints between the design's widths: below 1360 px → 1280, below 1680 → 1440, else
/// 1920.
pub const CLASSES: [ClassBound; 3] = [
    ClassBound {
        class: SizeClass::W1280,
        below: Some(1360.),
    },
    ClassBound {
        class: SizeClass::W1440,
        below: Some(1680.),
    },
    ClassBound {
        class: SizeClass::W1920,
        below: None,
    },
];

/// The class of a window `window_width` px wide.
pub fn size_class(window_width: f32) -> SizeClass {
    CLASSES
        .iter()
        .find(|c| c.below.is_none_or(|below| window_width < below))
        .map_or(SizeClass::W1920, |c| c.class)
}

/// One width per size class.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
pub struct ClassWidths {
    pub w1280: f32,
    pub w1440: f32,
    pub w1920: f32,
}

impl ClassWidths {
    pub fn get(self, class: SizeClass) -> f32 {
        match class {
            SizeClass::W1280 => self.w1280,
            SizeClass::W1440 => self.w1440,
            SizeClass::W1920 => self.w1920,
        }
    }
}

/// The pane's width until the user drags it, per class (SHL-01: 320 / 400 / 460 px).
pub const PANE_DEFAULTS: ClassWidths = ClassWidths {
    w1280: 320.,
    w1440: 400.,
    w1920: 460.,
};
/// The pane's drag range (SHL-01 and CHANGES-v §1; the 02 notes' "340–560" is older).
pub const PANE_MIN: f32 = 320.;
pub const PANE_MAX: f32 = 560.;
/// The hidden pane (Hide / show the pane, ⌘B): a strip this wide (*18 → Hidden*).
pub const PANE_STRIP: f32 = 28.;

/// A dragged pane width kept within its range.
pub fn clamp_pane(width: f32) -> f32 {
    width.clamp(PANE_MIN, PANE_MAX)
}

/// The pane's width in a window of `class`: the dragged one for that class, else its default.
pub fn pane_width(class: SizeClass, dragged: Option<f32>) -> f32 {
    dragged.map_or(PANE_DEFAULTS.get(class), clamp_pane)
}

/// Whether the main area has its Files tab. Off while the Files view is parked (d0087 changelog 2026-10-18, d0090):
/// the shell's tabs are the Board alone, with no tab row, and ⇧⌘O / ⌥⌘P are neither bound nor listed. The code
/// stays; the `_for` functions take the switch so the tests run both ways.
pub const FILES_VIEW: bool = false;

/// The main area's tabs (*Main tabs*), each with its ⇧⌘B / ⇧⌘O.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MainTab {
    Board,
    Files,
}

impl MainTab {
    pub const ALL: [MainTab; 2] = [MainTab::Board, MainTab::Files];

    /// The tabs the shell has: the Board alone while the Files view is off.
    pub fn shown(files_view: bool) -> &'static [MainTab] {
        if files_view {
            &Self::ALL
        } else {
            &Self::ALL[..1]
        }
    }

    /// The tab a stored layout opens on: one the shell no longer has (Files while it's off) opens on the Board.
    pub fn open(self, files_view: bool) -> MainTab {
        if Self::shown(files_view).contains(&self) {
            self
        } else {
            Self::Board
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Board => "Board",
            Self::Files => "Files",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Board => IconName::Columns3,
            Self::Files => IconName::FolderTree,
        }
    }

    pub fn action(self) -> KeyAction {
        match self {
            Self::Board => KeyAction::ShowBoard,
            Self::Files => KeyAction::ShowFiles,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "MainTabView")]
pub struct TabView {
    pub tab: MainTab,
    pub label: &'static str,
    pub icon: IconName,
}

/// What the frame draws and its sizes. `LayoutView` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "LayoutView")]
pub struct View {
    pub classes: Vec<ClassBound>,
    pub pane_defaults: ClassWidths,
    pub pane_min: f32,
    pub pane_max: f32,
    pub strip: f32,
    pub tabs: Vec<TabView>,
    /// Whether the tab row is drawn: not for one tab, which would switch nothing.
    pub tab_row: bool,
    /// "⇧⌘B · ⇧⌘O" at the tabs' right end.
    pub tab_keys: String,
    /// The Files tab until M24 (Q-F).
    pub files_empty: &'static str,
    /// The strip's name and tooltip: "Show the pane" with the user's key for it (⌘B).
    pub show_pane: &'static str,
    /// The pane's right edge, for screen readers (←/→ resize it too).
    pub pane_resize: &'static str,
    pub pane_keys: Option<String>,
}

pub fn view(keys: &KeyLabels) -> View {
    view_for(keys, FILES_VIEW)
}

/// [`view`] with the Files view on or off.
pub fn view_for(labels: &KeyLabels, files_view: bool) -> View {
    let tabs = MainTab::shown(files_view);
    let keys: Vec<String> = tabs
        .iter()
        .filter_map(|t| labels.label(t.action()))
        .collect();
    View {
        classes: CLASSES.to_vec(),
        pane_defaults: PANE_DEFAULTS,
        pane_min: PANE_MIN,
        pane_max: PANE_MAX,
        strip: PANE_STRIP,
        tabs: tabs
            .iter()
            .map(|&tab| TabView {
                tab,
                label: tab.label(),
                icon: tab.icon(),
            })
            .collect(),
        tab_row: tabs.len() > 1,
        tab_keys: keys.join(" \u{b7} "),
        files_empty: "The Files view comes in a later update.",
        show_pane: "Show the pane",
        pane_resize: "Pane width",
        pane_keys: labels.label(KeyAction::TogglePane),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_labels::fixtures;
    use crate::keymap::Platform;

    #[test]
    fn the_class_splits_at_the_midpoints() {
        for (width, class) in [
            (800., SizeClass::W1280),
            (1280., SizeClass::W1280),
            (1359., SizeClass::W1280),
            (1360., SizeClass::W1440),
            (1440., SizeClass::W1440),
            (1679., SizeClass::W1440),
            (1680., SizeClass::W1920),
            (1920., SizeClass::W1920),
            (2560., SizeClass::W1920),
        ] {
            assert_eq!(size_class(width), class, "{width} px");
        }
    }

    #[test]
    fn the_pane_defaults_per_class_and_keeps_a_drag_within_320_to_560() {
        assert_eq!(pane_width(SizeClass::W1280, None), 320.);
        assert_eq!(pane_width(SizeClass::W1440, None), 400.);
        assert_eq!(pane_width(SizeClass::W1920, None), 460.);
        assert_eq!(pane_width(SizeClass::W1440, Some(500.)), 500.);
        assert_eq!(pane_width(SizeClass::W1440, Some(200.)), PANE_MIN);
        assert_eq!(pane_width(SizeClass::W1920, Some(900.)), PANE_MAX);
        assert_eq!(clamp_pane(319.), 320.);
        assert_eq!(clamp_pane(561.), 560.);
    }

    #[test]
    fn the_page_gets_the_tabs_and_their_keys() {
        let mac = view_for(&fixtures::base(), true);
        assert!(mac.tab_row);
        assert_eq!(mac.tab_keys, "\u{21e7}\u{2318}B \u{b7} \u{21e7}\u{2318}O");
        assert_eq!(
            mac.tabs.iter().map(|t| t.label).collect::<Vec<_>>(),
            ["Board", "Files"]
        );
        assert_eq!(mac.pane_keys.as_deref(), Some("\u{2318}B"));
        assert_eq!(
            view_for(&KeyLabels::base(Platform::WinLinux), true).tab_keys,
            "Ctrl+Shift+B \u{b7} Ctrl+Shift+O"
        );
        assert_eq!(mac.files_empty, "The Files view comes in a later update.");
        assert_eq!(mac.classes.len(), 3);
        assert_eq!(mac.strip, 28.);
    }

    /// KEY-08: the strip's and the tabs' keys are the user's: a recorded key, none once removed.
    #[test]
    fn the_strip_and_the_tabs_show_the_users_keys() {
        let custom = view(&fixtures::replaced("toggle_pane", None, 0, "⌥B"));
        assert_eq!(custom.pane_keys.as_deref(), Some("⌥B"));
        assert_eq!(
            view(&fixtures::removed("toggle_pane", None, 0)).pane_keys,
            None
        );
        assert_eq!(
            view(&fixtures::letters_off()).pane_keys.as_deref(),
            Some("⌘B")
        );
        // Board's ⇧⌘B removed: its `g` `b` shows; with letters off too, nothing.
        let removed = fixtures::removed("show_board", None, 0);
        assert_eq!(
            view_for(&removed, true).tab_keys,
            "g b \u{b7} \u{21e7}\u{2318}O"
        );
    }

    /// d0087 (parked 2026-10-18): with the switch off the shell has the Board alone and draws no tab row.
    #[test]
    fn a_parked_files_view_leaves_the_board_alone_without_a_tab_row() {
        // Parked until M24: the app gets the switch off.
        assert_eq!(view(&fixtures::base()), view_for(&fixtures::base(), false));
        for platform in [Platform::Mac, Platform::WinLinux] {
            let off = view_for(&KeyLabels::base(platform), false);
            assert_eq!(
                off.tabs.iter().map(|t| t.tab).collect::<Vec<_>>(),
                [MainTab::Board]
            );
            assert!(!off.tab_row);
        }
        assert_eq!(
            view_for(&fixtures::base(), false).tab_keys,
            "\u{21e7}\u{2318}B"
        );
        assert_eq!(MainTab::shown(false), [MainTab::Board]);
        assert_eq!(MainTab::shown(true), MainTab::ALL);
    }

    /// A stored Files tab opens on the Board while the Files view is off, and as itself while it's on.
    #[test]
    fn a_stored_files_tab_reads_as_the_board_while_parked() {
        assert_eq!(MainTab::Files.open(false), MainTab::Board);
        assert_eq!(MainTab::Board.open(false), MainTab::Board);
        assert_eq!(MainTab::Files.open(true), MainTab::Files);
        assert_eq!(MainTab::Board.open(true), MainTab::Board);
    }
}
