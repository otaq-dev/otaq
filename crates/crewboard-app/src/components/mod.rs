//! Reusable components matching `crewboard.pen → 01 · Components` (T-0016): Button, Icon Button, Kbd, Banner and
//! Toast. The remaining components (Status Pill, Meta Chip, tabs, inputs, menus, dialogs, cards) come in M2.

pub mod banner;
pub mod button;
pub mod kbd;
pub mod toast;

use gpui::{BoxShadow, Hsla, point, px};

/// The 2 px focus ring drawn outside the element (the design's outer stroke), as a spread shadow so it never
/// changes the layout.
pub fn focus_ring(color: Hsla) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color,
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(2.),
        inset: false,
    }]
}

/// The two-layer shadow of floating layers (toasts, menus, dialogs).
pub fn floating_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: gpui::rgba(0x0000001F).into(),
            offset: point(px(0.), px(8.)),
            blur_radius: px(24.),
            spread_radius: px(0.),
            inset: false,
        },
        BoxShadow {
            color: gpui::rgba(0x00000014).into(),
            offset: point(px(0.), px(1.)),
            blur_radius: px(3.),
            spread_radius: px(0.),
            inset: false,
        },
    ]
}
