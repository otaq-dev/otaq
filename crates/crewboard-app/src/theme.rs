//! The visual language from *crewboard.pen → 01 · Foundations* (SDD §21.2, brief 01): colour tokens for light and
//! dark, the status styles (colour + icon + label; colour is never the only signal), the type scale, spacing,
//! radius and density. Every screen reads its colours from the active [`Theme`], never from literals.
//!
//! The theme follows the OS appearance by default; [`ThemeChoice`] overrides it (the setting is stored in T-0045).
//! Switching is live: set a new choice and every window redraws.

use gpui::{App, FontWeight, Global, Hsla, Pixels, WindowAppearance, px, rgba};

use crate::icons::IconName;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

/// The user's theme setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    /// The appearance to draw with, given the OS (window) appearance.
    pub fn resolve(self, os: WindowAppearance) -> Appearance {
        match self {
            Self::Light => Appearance::Light,
            Self::Dark => Appearance::Dark,
            Self::System => match os {
                WindowAppearance::Light | WindowAppearance::VibrantLight => Appearance::Light,
                WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
            },
        }
    }
}

fn color(hex: u32) -> Hsla {
    rgba(hex).into()
}

macro_rules! color_tokens {
    ($($field:ident = $name:literal : $light:literal / $dark:literal;)*) => {
        /// Every colour token of *01 · Foundations → Color Tokens*, resolved for one appearance.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Colors {
            $(pub $field: Hsla,)*
        }

        impl Colors {
            /// Token names as they appear in the design (`bg-app`, `st-attn`, …).
            pub const NAMES: &[&str] = &[$($name),*];

            pub fn new(appearance: Appearance) -> Self {
                match appearance {
                    Appearance::Light => Self { $($field: color($light),)* },
                    Appearance::Dark => Self { $($field: color($dark),)* },
                }
            }

            /// Looks a token up by its design name.
            pub fn get(&self, name: &str) -> Option<Hsla> {
                match name {
                    $($name => Some(self.$field),)*
                    _ => None,
                }
            }
        }
    };
}

// Values as RGBA hex, copied from the design's variables (light / dark).
color_tokens! {
    bg_app = "bg-app": 0xF6F6F4FF / 0x0E0F11FF;
    bg_panel = "bg-panel": 0xFFFFFFFF / 0x15161AFF;
    bg_column = "bg-column": 0xEEEEEBFF / 0x121316FF;
    bg_card = "bg-card": 0xFFFFFFFF / 0x1C1E23FF;
    bg_raised = "bg-raised": 0xFFFFFFFF / 0x23262CFF;
    bg_hover = "bg-hover": 0xEDEDEAFF / 0x262930FF;
    bg_pressed = "bg-pressed": 0xE4E4E0FF / 0x2E323AFF;
    border = "border": 0xE2E2DEFF / 0x272A31FF;
    border_strong = "border-strong": 0xCCCCC6FF / 0x3A3E47FF;
    text_primary = "text-primary": 0x1A1B1EFF / 0xECEDEFFF;
    text_secondary = "text-secondary": 0x5B5F67FF / 0xA5A9B2FF;
    text_muted = "text-muted": 0x8C9099FF / 0x6C707AFF;
    text_on_accent = "text-on-accent": 0xFFFFFFFF / 0xFFFFFFFF;
    accent = "accent": 0x4C57D6FF / 0x6E78F7FF;
    accent_hover = "accent-hover": 0x3F49C2FF / 0x828BFFFF;
    accent_soft = "accent-soft": 0xECEEFDFF / 0x1E2142FF;
    focus = "focus": 0x4C57D6FF / 0x8A93FFFF;
    selection = "selection": 0x4C57D633 / 0x6E78F74D;
    st_attn = "st-attn": 0xD4500AFF / 0xFF8A3DFF;
    st_attn_bg = "st-attn-bg": 0xFFF0E5FF / 0x3A2112FF;
    st_error = "st-error": 0xCC2F3BFF / 0xFF6670FF;
    st_error_bg = "st-error-bg": 0xFDEBEDFF / 0x3A191CFF;
    st_working = "st-working": 0x2C6AD6FF / 0x6AA6FFFF;
    st_working_bg = "st-working-bg": 0xEAF1FDFF / 0x15233BFF;
    st_investigate = "st-investigate": 0x7650D2FF / 0xB196FFFF;
    st_investigate_bg = "st-investigate-bg": 0xF2EEFCFF / 0x251C3BFF;
    st_queued = "st-queued": 0x646A75FF / 0x9AA0ABFF;
    st_queued_bg = "st-queued-bg": 0xEFF0F2FF / 0x23262CFF;
    st_review = "st-review": 0x0B8478FF / 0x3CD3C0FF;
    st_review_bg = "st-review-bg": 0xE4F5F2FF / 0x0F2E2AFF;
    st_paused = "st-paused": 0x9A6600FF / 0xF2BE4AFF;
    st_paused_bg = "st-paused-bg": 0xFAF2DAFF / 0x33290EFF;
    st_approved = "st-approved": 0x25854AFF / 0x4CC77AFF;
    st_approved_bg = "st-approved-bg": 0xE6F4EAFF / 0x12301CFF;
    st_done = "st-done": 0x9A9DA4FF / 0x5C6069FF;
    term_bg = "term-bg": 0xFAFAF8FF / 0x0A0B0DFF;
    term_fg = "term-fg": 0x2A2C31FF / 0xD6D8DDFF;
    diff_add_bg = "diff-add-bg": 0xE5F6EAFF / 0x11291AFF;
    diff_del_bg = "diff-del-bg": 0xFCEBECFF / 0x2E1517FF;
}

/// The bundled fonts (*Type · Spacing · Icons*): Geist for the UI, Geist Mono for code, terminal, IDs and numbers.
pub const FONT_UI: &str = "Geist";
pub const FONT_MONO: &str = "Geist Mono";

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

/// The 4 px grid: `s-1` … `s-6`.
pub mod space {
    pub const S1: f32 = 4.;
    pub const S2: f32 = 8.;
    pub const S3: f32 = 12.;
    pub const S4: f32 = 16.;
    pub const S5: f32 = 24.;
    pub const S6: f32 = 32.;
}

/// Radius: 4 for pills and inputs, 6 for buttons and cards, 8–10 for panels and dialogs.
pub mod radius {
    pub const SM: f32 = 4.;
    pub const MD: f32 = 6.;
    pub const LG: f32 = 8.;
    pub const DIALOG: f32 = 10.;
}

/// Density: column width, card padding, header and list row heights.
pub mod density {
    pub const COLUMN: f32 = 220.;
    pub const COLUMN_COMPACT: f32 = 184.;
    /// Card padding: 10 vertical × 12 horizontal.
    pub const CARD_PADDING: (f32, f32) = (10., 12.);
    pub const HEADER: f32 = 44.;
    pub const ROW: f32 = 28.;
}

/// Icon sizes: 14 px in the UI, 12 px inside pills (Lucide, 1.5 stroke).
pub const ICON: Pixels = px(14.);
pub const ICON_SMALL: Pixels = px(12.);

/// How loudly a status asks for attention (*Status Legend*).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Loud: orange = blocked on you; red = broke and waits for your click.
    NeedsYou,
    /// Medium: your turn, but nothing is stuck.
    Review,
    /// Calm.
    Working,
    /// Fades back: finished or not started.
    Done,
}

/// Which pair of status colours a status uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    Attn,
    Error,
    Review,
    Paused,
    Working,
    Queued,
    Approved,
    Done,
    Epic,
    Pinned,
}

/// Every entry of the *Status Legend*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    NeedsYou,
    AsksYou,
    Idle,
    Interrupted,
    StartInterrupted,
    ParentClosed,
    ReadyForReview,
    PrOpen,
    Conflicts,
    ChangedSinceReview,
    AgentWorking,
    WorkingInBackground,
    Queued,
    Parked,
    Running,
    Asked,
    SentBack,
    WaitingForLimitReset,
    Backlog,
    Approved,
    Done,
    Merged,
    ClosedNotMerged,
    Epic,
    Pinned,
}

/// A status as drawn: colours, icon and label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusStyle {
    pub tier: Tier,
    pub fg: Hsla,
    pub bg: Hsla,
    pub icon: IconName,
}

impl Status {
    pub const ALL: [Status; 25] = [
        Self::NeedsYou,
        Self::AsksYou,
        Self::Idle,
        Self::Interrupted,
        Self::StartInterrupted,
        Self::ParentClosed,
        Self::ReadyForReview,
        Self::PrOpen,
        Self::Conflicts,
        Self::ChangedSinceReview,
        Self::AgentWorking,
        Self::WorkingInBackground,
        Self::Queued,
        Self::Parked,
        Self::Running,
        Self::Asked,
        Self::SentBack,
        Self::WaitingForLimitReset,
        Self::Backlog,
        Self::Approved,
        Self::Done,
        Self::Merged,
        Self::ClosedNotMerged,
        Self::Epic,
        Self::Pinned,
    ];

    /// Tier, palette and icon, as in the legend.
    pub fn spec(self) -> (Tier, Palette, IconName) {
        use IconName as I;
        use Palette as P;
        use Tier as T;
        match self {
            Self::NeedsYou => (T::NeedsYou, P::Attn, I::Hand),
            Self::AsksYou => (T::NeedsYou, P::Attn, I::MessageCircleQuestionMark),
            Self::Idle => (T::NeedsYou, P::Attn, I::CirclePause),
            Self::Interrupted => (T::NeedsYou, P::Error, I::ZapOff),
            Self::StartInterrupted => (T::NeedsYou, P::Error, I::TriangleAlert),
            Self::ParentClosed => (T::NeedsYou, P::Error, I::CircleSlash),
            Self::ReadyForReview => (T::Review, P::Review, I::Eye),
            Self::PrOpen => (T::Review, P::Review, I::GitPullRequest),
            Self::Conflicts => (T::Review, P::Paused, I::GitCompareArrows),
            Self::ChangedSinceReview => (T::Review, P::Paused, I::GitCommitHorizontal),
            Self::AgentWorking => (T::Working, P::Working, I::Activity),
            Self::WorkingInBackground => (T::Working, P::Working, I::Cpu),
            Self::Queued => (T::Working, P::Queued, I::Hourglass),
            Self::Parked => (T::Working, P::Paused, I::SquareParking),
            Self::Running => (T::Working, P::Approved, I::Play),
            Self::Asked => (T::Working, P::Working, I::MessagesSquare),
            Self::SentBack => (T::Working, P::Paused, I::Undo2),
            Self::WaitingForLimitReset => (T::Working, P::Queued, I::Hourglass),
            Self::Backlog => (T::Done, P::Queued, I::CircleDashed),
            Self::Approved => (T::Done, P::Approved, I::Check),
            Self::Done => (T::Done, P::Done, I::CircleCheck),
            Self::Merged => (T::Done, P::Done, I::GitMerge),
            Self::ClosedNotMerged => (T::Done, P::Done, I::CircleSlash),
            Self::Epic => (T::Done, P::Epic, I::Layers),
            Self::Pinned => (T::Done, P::Pinned, I::Pin),
        }
    }

    /// The legend's label. Labels with live parts ("Running :4020", "Asked SHOP-40.1", "Epic · 1 / 3 merged",
    /// "Conflicts with main") are completed by the views that know those values.
    pub fn label(self) -> &'static str {
        match self {
            Self::NeedsYou => "Needs you",
            Self::AsksYou => "Asks you",
            Self::Idle => "Idle",
            Self::Interrupted => "Interrupted",
            Self::StartInterrupted => "Start interrupted",
            Self::ParentClosed => "Parent closed",
            Self::ReadyForReview => "Ready for Review",
            Self::PrOpen => "PR open",
            Self::Conflicts => "Conflicts with",
            Self::ChangedSinceReview => "Changed since review",
            Self::AgentWorking => "Agent Working",
            Self::WorkingInBackground => "Working in background",
            Self::Queued => "Queued",
            Self::Parked => "Parked · has work",
            Self::Running => "Running",
            Self::Asked => "Asked",
            Self::SentBack => "Sent back",
            Self::WaitingForLimitReset => "Waiting for limit reset",
            Self::Backlog => "Backlog",
            Self::Approved => "Approved",
            Self::Done => "Done",
            Self::Merged => "Merged",
            Self::ClosedNotMerged => "Closed · not merged",
            Self::Epic => "Epic",
            Self::Pinned => "Pinned",
        }
    }
}

/// Everything a view needs to draw in one appearance.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub appearance: Appearance,
    pub colors: Colors,
}

impl Theme {
    pub fn new(appearance: Appearance) -> Self {
        Self {
            appearance,
            colors: Colors::new(appearance),
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

/// The theme in use, as a GPUI global: the user's choice plus the theme resolved from it.
pub struct ActiveTheme {
    pub choice: ThemeChoice,
    pub theme: Theme,
}

impl Global for ActiveTheme {}

impl ActiveTheme {
    /// Installs the theme for the given OS appearance. Call once at start-up.
    pub fn init(choice: ThemeChoice, os: WindowAppearance, cx: &mut App) {
        cx.set_global(Self {
            choice,
            theme: Theme::new(choice.resolve(os)),
        });
    }

    pub fn get(cx: &App) -> &Theme {
        &cx.global::<Self>().theme
    }

    /// Re-resolves the theme (after the OS appearance or the choice changed) and redraws every window.
    pub fn update(choice: ThemeChoice, os: WindowAppearance, cx: &mut App) {
        let appearance = choice.resolve(os);
        let changed = {
            let active = cx.global::<Self>();
            active.choice != choice || active.theme.appearance != appearance
        };
        if changed {
            cx.set_global(Self {
                choice,
                theme: Theme::new(appearance),
            });
            cx.refresh_windows();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Hsla {
        let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap();
        let v = if s.len() == 7 { (v << 8) | 0xFF } else { v };
        color(v)
    }

    #[test]
    fn every_token_resolves_in_both_appearances() {
        assert_eq!(Colors::NAMES.len(), 39);
        for appearance in [Appearance::Light, Appearance::Dark] {
            let colors = Colors::new(appearance);
            for name in Colors::NAMES {
                assert!(colors.get(name).is_some(), "{name}");
            }
        }
        assert_eq!(Colors::new(Appearance::Light).get("no-such-token"), None);
    }

    #[test]
    fn token_values_match_the_design() {
        let light = Theme::new(Appearance::Light);
        let dark = Theme::new(Appearance::Dark);
        assert_eq!(light.colors.get("bg-app"), Some(hex("#F6F6F4")));
        assert_eq!(dark.colors.get("bg-app"), Some(hex("#0E0F11")));
        assert_eq!(light.colors.accent, hex("#4C57D6"));
        assert_eq!(dark.colors.st_attn, hex("#FF8A3D"));
        assert_eq!(light.colors.selection, hex("#4C57D633"));
        assert_eq!(dark.colors.diff_del_bg, hex("#2E1517"));
    }

    #[test]
    fn choice_resolves_against_the_os() {
        assert_eq!(
            ThemeChoice::System.resolve(WindowAppearance::VibrantDark),
            Appearance::Dark
        );
        assert_eq!(
            ThemeChoice::System.resolve(WindowAppearance::Light),
            Appearance::Light
        );
        assert_eq!(
            ThemeChoice::Light.resolve(WindowAppearance::Dark),
            Appearance::Light
        );
        assert_eq!(
            ThemeChoice::Dark.resolve(WindowAppearance::Light),
            Appearance::Dark
        );
    }

    #[test]
    fn statuses_carry_colour_and_icon() {
        let theme = Theme::new(Appearance::Light);
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
