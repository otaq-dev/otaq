//! Icons as GPUI elements; the names and the bundled SVGs are in [`crewboard_view::icons`].

use gpui::{Pixels, Styled, Svg, svg};

pub use crewboard_view::icons::IconName;

/// An icon element; give it a colour with `.text_color(...)`.
pub fn icon(name: IconName, size: Pixels) -> Svg {
    svg().path(name.path()).size(size).flex_none()
}
