//! Small helpers shared by views.

use gpui::{Styled, px, relative};

use crate::theme::TextStyle;

/// Applies a named text style from the type scale.
pub trait TextStyleExt: Styled + Sized {
    fn text_style_of(self, style: TextStyle) -> Self {
        let this = self
            .font_family(style.family)
            .text_size(px(style.size))
            .font_weight(style.weight);
        match style.line_height {
            Some(lh) => this.line_height(relative(lh)),
            None => this,
        }
    }
}

impl<T: Styled + Sized> TextStyleExt for T {}
