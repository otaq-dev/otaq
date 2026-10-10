//! Reusable components matching `crewboard.pen → 01 · Components`: Button, Icon Button, Kbd, Banner and Toast
//! (T-0016); Status Pill, Meta Chip, badges, tabs, segmented control, toggle, inputs, fields, dropdown,
//! checklist item, context menu, tooltip and confirm dialog (T-0030); the number stepper (T-0082).

pub mod answer_options;
pub mod banner;
pub mod button;
pub mod card;
pub mod dialog;
pub mod epic_card;
pub mod field;
pub mod kbd;
pub mod markdown;
pub mod menu;
pub mod pill;
pub mod stack_list;
pub mod stepper;
pub mod tabs;
pub mod text_input;
pub mod toast;
pub mod unsent;

/// An id for the `n`th part of an element with id `parent` (tabs, check boxes inside a row).
pub fn sub_id(parent: &gpui::ElementId, n: usize) -> gpui::ElementId {
    gpui::ElementId::Name(format!("{parent:?}/{n}").into())
}

/// Binds the components' keys. Call once at start-up.
pub fn bind_keys(cx: &mut gpui::App) {
    text_input::bind_keys(cx);
    menu::bind_keys(cx);
    dialog::bind_keys(cx);
    answer_options::bind_keys(cx);
}

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
