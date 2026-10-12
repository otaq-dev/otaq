//! "What and why" in read mode (TKT-03, d0019; *08 · Ticket editor → Ticket tab · read mode*): Markdown kept
//! simple. Paragraphs are body text; fenced code blocks stay intact in Geist Mono on the terminal background.
//! Headings ("## Done when") are a semibold line and "- item" lines a bulleted list, as the acceptance criteria
//! written in a description read (TKT-04, d0067). Nothing else is interpreted. `MarkdownBlock` on the page.

use serde::Serialize;
use ts_rs::TS;

/// One block of a Markdown body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "MarkdownBlock")]
pub enum Block {
    Paragraph {
        text: String,
    },
    /// The lines between the fences, as written.
    Code {
        text: String,
    },
    /// A "#" to "######" line, without its marks.
    Heading {
        text: String,
    },
    /// Consecutive "- item" / "* item" lines (a "[ ]" / "[x]" box is dropped).
    List {
        items: Vec<String>,
    },
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
            out.push(Block::Paragraph {
                text: paragraph.join("\n"),
            });
            paragraph.clear();
        }
    };
    let push_item = |item: &str, out: &mut Vec<Block>| match out.last_mut() {
        Some(Block::List { items }) => items.push(item.to_owned()),
        _ => out.push(Block::List {
            items: vec![item.to_owned()],
        }),
    };
    for line in text.lines() {
        let fence = line.trim_start().starts_with("```");
        match (&mut code, fence) {
            (Some(lines), true) => {
                out.push(Block::Code {
                    text: lines.join("\n"),
                });
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
                    out.push(Block::Heading {
                        text: text.to_owned(),
                    });
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
        out.push(Block::Code {
            text: lines.join("\n"),
        });
    }
    flush(&mut paragraph, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> Block {
        Block::Paragraph { text: text.into() }
    }

    fn code(text: &str) -> Block {
        Block::Code { text: text.into() }
    }

    #[test]
    fn code_blocks_stay_intact() {
        let body = "Build the checkout page.\nIt posts the cart.\n\n```ts\nrouter.push(session.url)\n  // redirect\n```\nAfter.";
        assert_eq!(
            blocks(body),
            [
                p("Build the checkout page.\nIt posts the cart."),
                code("router.push(session.url)\n  // redirect"),
                p("After."),
            ]
        );
        assert_eq!(blocks("```\nopen"), [code("open")]);
        assert!(blocks("").is_empty());
    }

    /// A "Done when" list moved into the description by migration 0022 reads as a heading and a list (d0067).
    #[test]
    fn headings_and_lists() {
        let body = "Add a toggle.\n\n## Done when\n\n- Toggle works\n- [x] Choice persists\n* Defaults\n\nAfter.\n#nope";
        assert_eq!(
            blocks(body),
            [
                p("Add a toggle."),
                Block::Heading {
                    text: "Done when".into()
                },
                Block::List {
                    items: vec![
                        "Toggle works".into(),
                        "Choice persists".into(),
                        "Defaults".into()
                    ]
                },
                p("After.\n#nope"),
            ]
        );
        assert_eq!(
            blocks("```\n## kept\n- kept\n```"),
            [code("## kept\n- kept")]
        );
    }
}
