//! Terminal colours: the 16 ANSI colours, the 256-colour cube and grey ramp, and truecolor (SES-09).
//!
//! The 16 colours, the background and the foreground come from the active theme (`term-bg`, `term-fg`, `ansi-0` …
//! `ansi-15` in *01 · Theme palettes*, d0069), so every terminal recolors with the theme. The cube and the ramp are
//! xterm's.

use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};

/// One of the 256 indexed colours: 0–15 ANSI, 16–231 the 6×6×6 cube, 232–255 the grey ramp (xterm values).
pub fn indexed(index: u8, ansi: &[u32; 16]) -> u32 {
    match index {
        0..=15 => ansi[usize::from(index)],
        16..=231 => {
            let i = index - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + u32::from(v) * 40 };
            (level(i / 36) << 16) | (level((i / 6) % 6) << 8) | level(i % 6)
        }
        232..=255 => {
            let v = 8 + u32::from(index - 232) * 10;
            (v << 16) | (v << 8) | v
        }
    }
}

/// The theme's terminal foreground, background and 16 ANSI colours (normal 0–7, bright 8–15), as RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Base {
    pub fg: u32,
    pub bg: u32,
    pub ansi: [u32; 16],
}

/// A cell colour as RGB. Colours the program set with OSC 4/10/11 win over the palette.
pub fn resolve(color: Color, overrides: &Colors, base: Base) -> u32 {
    let rgb = |c: Rgb| (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b);
    match color {
        Color::Spec(c) => rgb(c),
        Color::Indexed(i) => overrides[usize::from(i)].map_or_else(|| indexed(i, &base.ansi), rgb),
        Color::Named(name) => {
            if let Some(c) = overrides[name as usize] {
                return rgb(c);
            }
            match name {
                NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::Cursor => {
                    base.fg
                }
                NamedColor::Background => base.bg,
                NamedColor::DimForeground => dim(base.fg, base.bg),
                other => {
                    let n = other as usize;
                    if n < 16 {
                        base.ansi[n]
                    } else {
                        // DimBlack … DimWhite follow BrightForeground in the enum.
                        let dim_index = n.saturating_sub(NamedColor::DimBlack as usize) & 15;
                        dim(base.ansi[dim_index], base.bg)
                    }
                }
            }
        }
    }
}

/// The bright variant of an ANSI colour, for bold text (as xterm does).
pub fn brighten(color: Color) -> Color {
    match color {
        Color::Named(name) if (name as usize) < 8 => Color::Indexed(name as u8 + 8),
        Color::Indexed(i) if i < 8 => Color::Indexed(i + 8),
        other => other,
    }
}

/// Two thirds of the way from the background to `fg`.
pub fn dim(fg: u32, bg: u32) -> u32 {
    let mix = |shift: u32| {
        let f = (fg >> shift) & 0xFF;
        let b = (bg >> shift) & 0xFF;
        ((f * 2 + b) / 3) << shift
    };
    mix(16) | mix(8) | mix(0)
}

#[cfg(test)]
mod tests {
    use super::super::element::to_rgb;
    use super::*;
    use crate::theme::{Theme, ThemeId};

    fn base(id: ThemeId) -> Base {
        let theme = Theme::new(id);
        Base {
            fg: to_rgb(theme.colors.term_fg),
            bg: to_rgb(theme.colors.term_bg),
            ansi: theme.ansi,
        }
    }

    #[test]
    fn the_sixteen_come_from_the_theme() {
        // Crewboard's own colours are the design's (d0069): dark black is #1C1E23 now, not #3A3E47.
        let light = base(ThemeId::CrewboardLight);
        assert_eq!((light.fg, light.bg), (0x2A2C31, 0xFAFAF8));
        assert_eq!(light.ansi[0], 0x2A2C31);
        assert_eq!(light.ansi[9], 0xE0434F);
        let dark = base(ThemeId::CrewboardDark);
        assert_eq!((dark.fg, dark.bg), (0xD6D8DD, 0x0A0B0D));
        assert_eq!(dark.ansi[0], 0x1C1E23);
        assert_eq!(dark.ansi[15], 0xFFFFFF);
        let dracula = base(ThemeId::Dracula);
        let overrides = Colors::default();
        assert_eq!(
            resolve(Color::Named(NamedColor::Red), &overrides, dracula),
            0xFF5555
        );
        assert_eq!(
            resolve(Color::Named(NamedColor::Background), &overrides, dracula),
            0x282A36
        );
        assert_eq!(
            resolve(Color::Indexed(4), &overrides, base(ThemeId::SolarizedLight)),
            0x268BD2
        );
    }

    #[test]
    fn the_cube_and_ramp_match_xterm() {
        let ansi = base(ThemeId::CrewboardDark).ansi;
        assert_eq!(indexed(16, &ansi), 0x000000);
        assert_eq!(indexed(196, &ansi), 0xFF0000);
        assert_eq!(indexed(231, &ansi), 0xFFFFFF);
        assert_eq!(indexed(232, &ansi), 0x080808);
        assert_eq!(indexed(255, &ansi), 0xEEEEEE);
        assert_eq!(indexed(1, &ansi), ansi[1]);
    }

    #[test]
    fn named_and_truecolor_resolve() {
        let b = base(ThemeId::CrewboardLight);
        let overrides = Colors::default();
        let resolve = |c| resolve(c, &overrides, b);
        assert_eq!(resolve(Color::Named(NamedColor::Foreground)), b.fg);
        assert_eq!(resolve(Color::Named(NamedColor::Background)), b.bg);
        assert_eq!(resolve(Color::Named(NamedColor::Red)), b.ansi[1]);
        assert_eq!(resolve(Color::Named(NamedColor::BrightBlue)), b.ansi[12]);
        assert_eq!(
            resolve(Color::Spec(Rgb {
                r: 0xD9,
                g: 0x77,
                b: 0x57
            })),
            0xD97757
        );
        assert_eq!(resolve(Color::Indexed(208)), indexed(208, &b.ansi));
        assert_eq!(
            resolve(Color::Named(NamedColor::DimRed)),
            dim(b.ansi[1], b.bg)
        );
    }

    #[test]
    fn program_set_colours_win() {
        let b = base(ThemeId::TokyoNight);
        let mut overrides = Colors::default();
        overrides[1] = Some(Rgb { r: 1, g: 2, b: 3 });
        assert_eq!(
            resolve(Color::Named(NamedColor::Red), &overrides, b),
            0x010203
        );
        assert_eq!(resolve(Color::Indexed(1), &overrides, b), 0x010203);
    }

    #[test]
    fn bold_brightens_the_first_eight() {
        assert_eq!(
            brighten(Color::Named(NamedColor::Green)),
            Color::Indexed(10)
        );
        assert_eq!(brighten(Color::Indexed(12)), Color::Indexed(12));
    }
}
