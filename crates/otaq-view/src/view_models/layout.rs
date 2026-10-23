//! The window's layout (SHB-01, d0113, *24 → Row · The Project agent*): project rail | the **Project agent panel**,
//! only while it is open, 420 px wide and pushing the board | the main area. There is no always-visible pane, no strip
//! and no Board | Files tab row: the project Files view is the toolbar's third view (FIL-01). The page asks once at
//! start (`layout.view`); only whether the panel is open stays on the page, per window.
//!
//! The ticket sheet's sizes are `panel.rs`'s; it opens over the main area (SHL-05) and uses [`SizeClass`] too.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::view_models::switcher::RAIL_WIDTH;

/// Which of the design's three windows a window is closest to (Q-C): the ticket sheet's remembered width is kept per
/// class.
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

/// The Project agent panel's width while it is open (SHB-01, `s24-05`): the same at every window size.
pub const AGENT_PANEL: f32 = 420.;

/// The main area's width in a window `window_width` px wide: all of it after the rail, less the panel while it is
/// open (it pushes the board, d0113).
pub fn main_width(window_width: f32, agent_open: bool) -> f32 {
    let panel = if agent_open { AGENT_PANEL } else { 0. };
    (window_width - RAIL_WIDTH as f32 - panel).max(0.)
}

/// What the frame draws and its sizes. `LayoutView` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "LayoutView")]
pub struct View {
    pub classes: Vec<ClassBound>,
    pub rail: f32,
    /// The Project agent panel's width while it is open.
    pub agent_panel: f32,
}

pub fn view() -> View {
    View {
        classes: CLASSES.to_vec(),
        rail: RAIL_WIDTH as f32,
        agent_panel: AGENT_PANEL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// SHB-01 (d0113): without a pane the main area takes the width after the 64 px rail; the open panel takes its
    /// 420 px from it at every window size.
    #[test]
    fn the_main_area_takes_the_width_the_pane_had() {
        for (window, closed, open) in [
            (1280., 1216., 796.),
            (1440., 1376., 956.),
            (1920., 1856., 1436.),
        ] {
            assert_eq!(main_width(window, false), closed, "{window} px");
            assert_eq!(main_width(window, true), open, "{window} px, panel open");
        }
        assert_eq!(main_width(300., true), 0.);
        let view = view();
        assert_eq!((view.rail, view.agent_panel), (64., 420.));
        assert_eq!(view.classes.len(), 3);
    }
}
