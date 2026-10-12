//! The theme in GPUI terms: the token colours as `Hsla`, the type scale with GPUI font weights, icon sizes and the
//! active theme as a GPUI global. The data (the 15 themes, the status legend, spacing, radius and density) is in
//! [`crewboard_view::theme`] and re-exported here. Switching is live: set a new theme and every window redraws.

use gpui::{App, FontWeight, Global, Hsla, Pixels, WindowAppearance, px, rgba};

pub use crewboard_view::theme::*;

use crate::icons::IconName;

/// Every colour token, resolved for one theme.
pub type Colors = crewboard_view::theme::Colors<Hsla>;

/// The OS (window) appearance.
pub fn appearance(os: WindowAppearance) -> Appearance {
    match os {
        WindowAppearance::Light | WindowAppearance::VibrantLight => Appearance::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
    }
}

fn color(hex: u32) -> Hsla {
    rgba(hex).into()
}

/// A theme's colour tokens.
pub fn colors(theme: ThemeId) -> Colors {
    crewboard_view::theme::Colors::new(theme).map(color)
}

/// A named text style from the type scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub family: &'static str,
    pub size: f32,
    pub weight: FontWeight,
    /// Line height as a multiple of the size, where the scale gives one.
    pub line_height: Option<f32>,
    /// Letter spacing in px, as designed. GPUI has no letter spacing, so it is recorded but not drawn.
    pub letter_spacing: f32,
}

const fn style(family: &'static str, size: f32, weight: FontWeight) -> TextStyle {
    TextStyle {
        family,
        size,
        weight,
        line_height: None,
        letter_spacing: 0.,
    }
}

/// *Type · Spacing · Icons → Type scale*.
pub mod text {
    use super::*;

    /// Display / dialog title: 20 / 600 / -0.3.
    pub const DISPLAY: TextStyle = TextStyle {
        letter_spacing: -0.3,
        ..style(FONT_UI, 20., FontWeight::SEMIBOLD)
    };
    /// Header · project: 15 / 600.
    pub const HEADER: TextStyle = style(FONT_UI, 15., FontWeight::SEMIBOLD);
    /// Column title: 12 / 600.
    pub const COLUMN_TITLE: TextStyle = style(FONT_UI, 12., FontWeight::SEMIBOLD);
    /// Card title: 13 / 500 / 1.35.
    pub const CARD_TITLE: TextStyle = TextStyle {
        line_height: Some(1.35),
        ..style(FONT_UI, 13., FontWeight::MEDIUM)
    };
    /// Body: 13 / 400 / 1.5.
    pub const BODY: TextStyle = TextStyle {
        line_height: Some(1.5),
        ..style(FONT_UI, 13., FontWeight::NORMAL)
    };
    /// Metadata: 11 / 400.
    pub const METADATA: TextStyle = style(FONT_UI, 11., FontWeight::NORMAL);
    /// Code / IDs: 12 mono.
    pub const CODE: TextStyle = style(FONT_MONO, 12., FontWeight::NORMAL);
    /// Terminal: 13 mono / 1.45.
    pub const TERMINAL: TextStyle = TextStyle {
        line_height: Some(1.45),
        ..style(FONT_MONO, 13., FontWeight::NORMAL)
    };
    /// Status pill label: 11 / 500 (*Status Pill* component).
    pub const PILL: TextStyle = style(FONT_UI, 11., FontWeight::MEDIUM);
}

/// Icon sizes: 14 px in the UI, 12 px inside pills (Lucide, 1.5 stroke).
pub const ICON: Pixels = px(14.);
pub const ICON_SMALL: Pixels = px(12.);

/// A status as drawn: colours, icon and label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusStyle {
    pub tier: Tier,
    pub fg: Hsla,
    pub bg: Hsla,
    pub icon: IconName,
}

/// Everything a view needs to draw in one theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub id: ThemeId,
    pub appearance: Appearance,
    pub colors: Colors,
    /// The terminal's 16 colours as RGB.
    pub ansi: [u32; 16],
}

impl Theme {
    pub fn new(id: ThemeId) -> Self {
        Self {
            id,
            appearance: id.appearance(),
            colors: colors(id),
            ansi: ansi(id),
        }
    }

    /// The colours of a palette: (foreground, background).
    pub fn palette(&self, palette: Palette) -> (Hsla, Hsla) {
        let c = &self.colors;
        match palette {
            Palette::Attn => (c.st_attn, c.st_attn_bg),
            Palette::Error => (c.st_error, c.st_error_bg),
            Palette::Review => (c.st_review, c.st_review_bg),
            Palette::Paused => (c.st_paused, c.st_paused_bg),
            Palette::Working => (c.st_working, c.st_working_bg),
            Palette::Queued => (c.st_queued, c.st_queued_bg),
            Palette::Approved => (c.st_approved, c.st_approved_bg),
            Palette::Done => (c.st_done, c.st_queued_bg),
            Palette::Epic => (c.st_investigate, c.st_investigate_bg),
            Palette::Pinned => (c.accent, c.accent_soft),
        }
    }

    pub fn status(&self, status: Status) -> StatusStyle {
        let (tier, palette, icon) = status.spec();
        let (fg, bg) = self.palette(palette);
        StatusStyle { tier, fg, bg, icon }
    }
}

/// The theme in use, as a GPUI global.
pub struct ActiveTheme {
    pub theme: Theme,
}

impl Global for ActiveTheme {}

impl ActiveTheme {
    /// Installs the theme. Call once at start-up.
    pub fn init(id: ThemeId, cx: &mut App) {
        cx.set_global(Self {
            theme: Theme::new(id),
        });
    }

    pub fn get(cx: &App) -> &Theme {
        &cx.global::<Self>().theme
    }

    /// Switches to `id` and redraws every window, if it isn't the theme in use already.
    pub fn update(id: ThemeId, cx: &mut App) {
        if cx.global::<Self>().theme.id != id {
            cx.set_global(Self {
                theme: Theme::new(id),
            });
            cx.refresh_windows();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_appearances_map_to_light_and_dark() {
        assert_eq!(appearance(WindowAppearance::Light), Appearance::Light);
        assert_eq!(
            appearance(WindowAppearance::VibrantLight),
            Appearance::Light
        );
        assert_eq!(appearance(WindowAppearance::Dark), Appearance::Dark);
        assert_eq!(appearance(WindowAppearance::VibrantDark), Appearance::Dark);
        assert_eq!(colors(ThemeId::Nord).get("bg-app"), Some(color(0x242933FF)));
    }

    #[test]
    fn statuses_carry_colour_and_icon() {
        let theme = Theme::new(ThemeId::CrewboardLight);
        let needs_you = theme.status(Status::NeedsYou);
        assert_eq!(needs_you.tier, Tier::NeedsYou);
        assert_eq!(needs_you.fg, theme.colors.st_attn);
        assert_eq!(needs_you.bg, theme.colors.st_attn_bg);
        assert_eq!(needs_you.icon, IconName::Hand);
        let merged = theme.status(Status::Merged);
        assert_eq!(merged.tier, Tier::Done);
        assert_eq!(
            (merged.fg, merged.bg),
            (theme.colors.st_done, theme.colors.st_queued_bg)
        );
        // Every tier has statuses, and every status has a label.
        for tier in [Tier::NeedsYou, Tier::Review, Tier::Working, Tier::Done] {
            assert!(Status::ALL.iter().any(|s| s.spec().0 == tier));
        }
        assert!(Status::ALL.iter().all(|s| !s.label().is_empty()));
        // The M12 row of every tier (*Pills · M12*): none of them is in the main rows.
        for tier in [Tier::NeedsYou, Tier::Review, Tier::Working, Tier::Done] {
            assert!(Status::M12.iter().any(|s| s.spec().0 == tier));
        }
        assert!(Status::M12.iter().all(|s| !Status::ALL.contains(s)));
        let failed_moved = theme.status(Status::FailedMoved);
        assert_eq!(
            (failed_moved.tier, failed_moved.fg, failed_moved.icon),
            (Tier::Review, theme.colors.st_error, IconName::ShieldX),
            "did not pass reads red"
        );
    }

    #[test]
    fn type_scale() {
        assert_eq!(text::DISPLAY.size, 20.);
        assert_eq!(text::DISPLAY.weight, FontWeight::SEMIBOLD);
        assert_eq!(text::CARD_TITLE.line_height, Some(1.35));
        assert_eq!(text::TERMINAL.family, FONT_MONO);
        assert_eq!(text::BODY.family, FONT_UI);
    }
}
