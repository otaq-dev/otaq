//! "What and why" in read mode (TKT-03, d0019): Markdown kept simple. Paragraphs are body text; fenced code
//! blocks stay intact in Geist Mono on the terminal background. Nothing else is interpreted.

use gpui::{App, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px, relative};

use crate::theme::{ActiveTheme, FONT_MONO, radius};

/// One block of a Markdown body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Paragraph(String),
    /// The lines between the fences, as written.
    Code(String),
}

/// Splits a body into paragraphs (blank-line separated) and fenced code blocks. An unclosed fence runs to the
/// end.
pub fn blocks(text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let mut code: Option<Vec<&str>> = None;
    let flush = |paragraph: &mut Vec<&str>, out: &mut Vec<Block>| {
        if !paragraph.is_empty() {
            out.push(Block::Paragraph(paragraph.join("\n")));
            paragraph.clear();
        }
    };
    for line in text.lines() {
        let fence = line.trim_start().starts_with("```");
        match (&mut code, fence) {
            (Some(lines), true) => {
                out.push(Block::Code(lines.join("\n")));
                code = None;
            }
            (Some(lines), false) => lines.push(line),
            (None, true) => {
                flush(&mut paragraph, &mut out);
                code = Some(Vec::new());
            }
            (None, false) if line.trim().is_empty() => flush(&mut paragraph, &mut out),
            (None, false) => paragraph.push(line),
        }
    }
    if let Some(lines) = code {
        out.push(Block::Code(lines.join("\n")));
    }
    flush(&mut paragraph, &mut out);
    out
}

#[derive(IntoElement)]
pub struct Markdown {
    text: String,
}

impl Markdown {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for Markdown {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(blocks(&self.text).into_iter().map(|block| {
                match block {
                    Block::Paragraph(text) => div()
                        .text_size(px(13.))
                        .line_height(relative(1.5))
                        .text_color(c.text_primary)
                        .child(SharedString::from(text))
                        .into_any_element(),
                    Block::Code(code) => div()
                        .px(px(8.))
                        .py(px(6.))
                        .rounded(px(radius::SM))
                        .bg(c.term_bg)
                        .font_family(FONT_MONO)
                        .text_size(px(12.))
                        .line_height(relative(1.45))
                        .text_color(c.term_fg)
                        .children(code.lines().map(|l| {
                            div()
                                .whitespace_nowrap()
                                .child(SharedString::from(l.to_owned()))
                        }))
                        .into_any_element(),
                }
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_blocks_stay_intact() {
        let body = "Build the checkout page.\nIt posts the cart.\n\n```ts\nrouter.push(session.url)\n  // redirect\n```\nAfter.";
        assert_eq!(
            blocks(body),
            [
                Block::Paragraph("Build the checkout page.\nIt posts the cart.".into()),
                Block::Code("router.push(session.url)\n  // redirect".into()),
                Block::Paragraph("After.".into()),
            ]
        );
        assert_eq!(blocks("```\nopen"), [Block::Code("open".into())]);
        assert!(blocks("").is_empty());
    }
}
