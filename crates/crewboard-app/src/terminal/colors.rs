//! Terminal colours: the 16 ANSI colours, the 256-colour cube and grey ramp, and truecolor (SES-09).
//!
//! The design defines only `term-bg` and `term-fg`; the 16-colour palette is an implementation choice derived
//! from the status colours of each theme, so `ls --color` and Claude Code's diffs read like the rest of the app.

use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};

/// The 16 ANSI colours (normal 0–7, bright 8–15) as RGB.
pub fn ansi16(index: u8, dark: bool) -> u32 {
    const LIGHT: [u32; 16] = [
        0x2A2C31, 0xCC2F3B, 0x25854A, 0x9A6600, 0x2C6AD6, 0x7650D2, 0x0B8478,
        0x8C9099, // normal
        0x5B5F67, 0xE0434F, 0x2E9E58, 0xB57B00, 0x4C57D6, 0x8B66E0, 0x109A8C,
        0xA5A9B2, // bright
    ];
    const DARK: [u32; 16] = [
        0x3A3E47, 0xFF6670, 0x4CC77A, 0xF2BE4A, 0x6AA6FF, 0xB196FF, 0x3CD3C0,
        0xD6D8DD, // normal
        0x6C707A, 0xFF8A90, 0x6FE09A, 0xFFD27A, 0x8EBBFF, 0xC9B5FF, 0x6EE6D6,
        0xFFFFFF, // bright
    ];
    let palette = if dark { &DARK } else { &LIGHT };
    palette[usize::from(index & 15)]
}

/// One of the 256 indexed colours: 0–15 ANSI, 16–231 the 6×6×6 cube, 232–255 the grey ramp (xterm values).
pub fn indexed(index: u8, dark: bool) -> u32 {
    match index {
        0..=15 => ansi16(index, dark),
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

/// The theme's terminal foreground and background, and whether it is dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Base {
    pub fg: u32,
    pub bg: u32,
    pub dark: bool,
}

/// A cell colour as RGB. Colours the program set with OSC 4/10/11 win over the palette.
pub fn resolve(color: Color, overrides: &Colors, base: Base) -> u32 {
    let rgb = |c: Rgb| (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b);
    match color {
        Color::Spec(c) => rgb(c),
        Color::Indexed(i) => overrides[usize::from(i)].map_or_else(|| indexed(i, base.dark), rgb),
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
                        ansi16(n as u8, base.dark)
                    } else {
                        // DimBlack … DimWhite follow BrightForeground in the enum.
                        let dim_index = n.saturating_sub(NamedColor::DimBlack as usize) as u8;
                        dim(ansi16(dim_index, base.dark), base.bg)
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
    use super::*;

    const BASE: Base = Base {
        fg: 0x2A2C31,
        bg: 0xFAFAF8,
        dark: false,
    };

    #[test]
    fn the_cube_and_ramp_match_xterm() {
        assert_eq!(indexed(16, false), 0x000000);
        assert_eq!(indexed(196, false), 0xFF0000);
        assert_eq!(indexed(231, true), 0xFFFFFF);
        assert_eq!(indexed(232, false), 0x080808);
        assert_eq!(indexed(255, false), 0xEEEEEE);
        assert_eq!(indexed(1, true), ansi16(1, true));
    }

    #[test]
    fn named_and_truecolor_resolve() {
        let overrides = Colors::default();
        let resolve = |c| resolve(c, &overrides, BASE);
        assert_eq!(resolve(Color::Named(NamedColor::Foreground)), BASE.fg);
        assert_eq!(resolve(Color::Named(NamedColor::Background)), BASE.bg);
        assert_eq!(resolve(Color::Named(NamedColor::Red)), ansi16(1, false));
        assert_eq!(
            resolve(Color::Named(NamedColor::BrightBlue)),
            ansi16(12, false)
        );
        assert_eq!(
            resolve(Color::Spec(Rgb {
                r: 0xD9,
                g: 0x77,
                b: 0x57
            })),
            0xD97757
        );
        assert_eq!(resolve(Color::Indexed(208)), indexed(208, false));
        assert_eq!(
            resolve(Color::Named(NamedColor::DimRed)),
            dim(ansi16(1, false), BASE.bg)
        );
    }

    #[test]
    fn program_set_colours_win() {
        let mut overrides = Colors::default();
        overrides[1] = Some(Rgb { r: 1, g: 2, b: 3 });
        assert_eq!(
            resolve(Color::Named(NamedColor::Red), &overrides, BASE),
            0x010203
        );
        assert_eq!(resolve(Color::Indexed(1), &overrides, BASE), 0x010203);
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
