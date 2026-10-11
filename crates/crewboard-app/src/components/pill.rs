//! Status Pill, Meta Chip and count badges (*01 · Components → Primitives* and *Badges & Inputs → Badges &
//! pills*).

use gpui::{
    App, FontWeight, Hsla, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px,
};

use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_MONO, Status, radius, text};
use crate::ui::TextStyleExt;
use crate::view_models::status;

/// A status as a pill: 12 px icon + 11/500 label on the status background (never colour alone).
#[derive(IntoElement)]
pub struct StatusPill {
    status: Status,
    label: SharedString,
}

impl StatusPill {
    /// A ticket's status, with its legend colours and label.
    pub fn of(status: crewboard_core::domain::Status) -> Self {
        let (legend, label) = status::pill(status);
        Self {
            status: legend,
            label: label.into(),
        }
    }

    /// A legend entry with its own or a completed label ("Running :4020", "Conflicts with main").
    pub fn legend(status: Status, label: Option<SharedString>) -> Self {
        Self {
            status,
            label: label.unwrap_or_else(|| status.label().into()),
        }
    }
}

impl RenderOnce for StatusPill {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let style = ActiveTheme::get(cx).status(self.status);
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(5.))
            .pt(px(3.))
            .pb(px(3.))
            .pl(px(6.))
            .pr(px(7.))
            .rounded(px(radius::SM))
            .bg(style.bg)
            .text_color(style.fg)
            .child(icon(style.icon, px(12.)).text_color(style.fg))
            .child(div().text_style_of(text::PILL).child(self.label))
    }
}

/// How a Meta Chip is tinted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChipTone {
    #[default]
    Plain,
    /// Green outline: a running process (`▶ :4020`).
    Running,
    /// Amber: a stale last signal ("4 min ago").
    Warning,
}

/// Key/value metadata in mono type with an 11 px icon: dependencies, branch, tokens, port, PR number.
#[derive(IntoElement)]
pub struct MetaChip {
    icon: IconName,
    label: SharedString,
    tone: ChipTone,
}

impl MetaChip {
    pub fn new(icon: IconName, label: impl Into<SharedString>) -> Self {
        Self {
            icon,
            label: label.into(),
            tone: ChipTone::Plain,
        }
    }

    pub fn tone(mut self, tone: ChipTone) -> Self {
        self.tone = tone;
        self
    }
}

impl RenderOnce for MetaChip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let (fg, border, bg): (Hsla, Hsla, Option<Hsla>) = match self.tone {
            ChipTone::Plain => (c.text_secondary, c.border, None),
            ChipTone::Running => (c.st_approved, c.st_approved, None),
            ChipTone::Warning => (c.st_paused, c.st_paused, Some(c.st_paused_bg)),
        };
        let mut chip = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .px(px(6.))
            .py(px(2.))
            .rounded(px(radius::SM))
            .border_1()
            .border_color(border)
            .text_color(fg)
            .child(icon(self.icon, px(11.)).text_color(fg))
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .whitespace_nowrap()
                    .child(self.label),
            );
        if let Some(bg) = bg {
            chip = chip.bg(bg);
        }
        chip
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountTone {
    /// Orange: items needing you.
    Attention,
    Neutral,
    Accent,
}

/// A small rounded count (inbox, switcher, tab counts).
#[derive(IntoElement)]
pub struct CountBadge {
    count: u32,
    tone: CountTone,
}

impl CountBadge {
    pub fn new(count: u32, tone: CountTone) -> Self {
        Self { count, tone }
    }
}

impl RenderOnce for CountBadge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let (bg, fg) = match self.tone {
            CountTone::Attention => (c.st_attn, c.text_on_accent),
            CountTone::Neutral => (c.bg_column, c.text_secondary),
            CountTone::Accent => (c.accent, c.text_on_accent),
        };
        div()
            .flex()
            .flex_none()
            .justify_center()
            .px(px(6.))
            .py(px(1.))
            .rounded(px(9.))
            .bg(bg)
            .text_color(fg)
            .font_family(FONT_MONO)
            .text_size(px(11.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(SharedString::from(self.count.to_string()))
    }
}
