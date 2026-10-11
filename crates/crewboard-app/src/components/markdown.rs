//! "What and why" in read mode (TKT-03, d0019): Markdown kept simple. Paragraphs are body text; fenced code
//! blocks stay intact in Geist Mono on the terminal background. Headings ("## Done when") are a semibold line and
//! "- item" lines a bulleted list, as *08 · Ticket editor → Ticket tab · read mode* shows the acceptance criteria
//! written in a description (TKT-04, d0067). Nothing else is interpreted.

use gpui::{
    App, FontWeight, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px, relative,
};

use crate::theme::{ActiveTheme, FONT_MONO, radius};

/// One block of a Markdown body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Paragraph(String),
    /// The lines between the fences, as written.
    Code(String),
    /// A "#" to "######" line, without its marks.
    Heading(String),
    /// Consecutive "- item" / "* item" lines (a "[ ]" / "[x]" box is dropped).
    List(Vec<String>),
}

/// "## Done when" → "Done when".
fn heading(line: &str) -> Option<&str> {
    let line = line.trim();
    let marks = line.len() - line.trim_start_matches('#').len();
    let text = line[marks..].strip_prefix(' ')?;
    (1..=6).contains(&marks).then(|| text.trim())
}

/// "- [ ] works offline" → "works offline".
fn list_item(line: &str) -> Option<&str> {
    let line = line.trim();
    let rest = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))?;
    let rest = ["[ ] ", "[x] ", "[X] "]
        .iter()
        .find_map(|b| rest.strip_prefix(b))
        .unwrap_or(rest);
    Some(rest.trim())
}

/// Splits a body into paragraphs (blank-line separated), headings, lists and fenced code blocks. An unclosed
/// fence runs to the end.
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
    let push_item = |item: &str, out: &mut Vec<Block>| match out.last_mut() {
        Some(Block::List(items)) => items.push(item.to_owned()),
        _ => out.push(Block::List(vec![item.to_owned()])),
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
            (None, false) => {
                if let Some(text) = heading(line) {
                    flush(&mut paragraph, &mut out);
                    out.push(Block::Heading(text.to_owned()));
                } else if let Some(item) = list_item(line) {
                    flush(&mut paragraph, &mut out);
                    push_item(item, &mut out);
                } else {
                    paragraph.push(line);
                }
            }
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
                    Block::Heading(text) => div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.text_primary)
                        .child(SharedString::from(text))
                        .into_any_element(),
                    Block::List(items) => div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .children(items.into_iter().map(|item| {
                            div()
                                .flex()
                                .gap(px(8.))
                                .text_size(px(13.))
                                .line_height(relative(1.45))
                                .text_color(c.text_primary)
                                .child("•")
                                .child(div().flex_1().min_w_0().child(SharedString::from(item)))
                        }))
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

    /// A "Done when" list moved into the description by migration 0022 reads as a heading and a list (d0067).
    #[test]
    fn headings_and_lists() {
        let body = "Add a toggle.\n\n## Done when\n\n- Toggle works\n- [x] Choice persists\n* Defaults\n\nAfter.\n#nope";
        assert_eq!(
            blocks(body),
            [
                Block::Paragraph("Add a toggle.".into()),
                Block::Heading("Done when".into()),
                Block::List(vec![
                    "Toggle works".into(),
                    "Choice persists".into(),
                    "Defaults".into()
                ]),
                Block::Paragraph("After.\n#nope".into()),
            ]
        );
        assert_eq!(
            blocks("```\n## kept\n- kept\n```"),
            [Block::Code("## kept\n- kept".into())]
        );
    }
}
