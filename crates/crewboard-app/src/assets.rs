//! Everything bundled into the app binary: fonts and icons, so the app looks the same on every machine
//! (SDD §21.2).
//!
//! Fonts: Geist and Geist Mono (Vercel), SIL Open Font License 1.1 (`assets/fonts/LICENSE-geist.txt`), from the
//! `geist` npm package 1.7.2: Geist Regular/Medium/SemiBold/Bold and Geist Mono Regular/Medium.
//! Icons: Lucide, ISC (see [`crate::icons`]).

use std::borrow::Cow;

use gpui::{App, AssetSource, Result, SharedString};

use crate::icons::IconName;

/// Serves the embedded icons to GPUI's SVG renderer.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(IconName::from_path(path).map(|icon| Cow::Borrowed(icon.bytes())))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(IconName::ALL
            .iter()
            .map(|i| i.path())
            .filter(|p| p.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}

pub const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/Geist-Regular.ttf"),
    include_bytes!("../assets/fonts/Geist-Medium.ttf"),
    include_bytes!("../assets/fonts/Geist-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Geist-Bold.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Regular.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Medium.ttf"),
];

/// Registers the bundled fonts with GPUI's text system.
pub fn load_fonts(cx: &App) -> Result<()> {
    cx.text_system()
        .add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_load_by_path() {
        let bytes = Assets.load("icons/hand.svg").unwrap().unwrap();
        assert!(bytes.starts_with(b"<!--") || bytes.starts_with(b"<svg"));
        assert!(Assets.load("icons/nope.svg").unwrap().is_none());
        assert_eq!(Assets.list("icons/").unwrap().len(), IconName::ALL.len());
    }

    #[test]
    fn fonts_are_truetype() {
        for font in FONTS {
            assert_eq!(&font[..4], &[0, 1, 0, 0], "TrueType header");
        }
    }
}
