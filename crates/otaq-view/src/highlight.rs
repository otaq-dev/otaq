//! Syntax highlighting for the Files tab (REV-02; d0017, d0055) with `syntect`'s bundled grammars, chosen by the
//! file's extension. Scopes are reduced to a few kinds coloured with the app's own tokens (the design colours
//! keywords purple, strings magenta, functions blue, numbers and types amber), so light and dark follow the theme.
//!
//! A hunk is highlighted as one run of lines, so strings and comments that span lines inside it come out right;
//! what begins before the hunk can't be known and reads as plain text.

use std::ops::Range;
use std::sync::OnceLock;

use syntect::parsing::{ParseState, Scope, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet};

/// What a span of code is, for its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
    Plain,
    Keyword,
    String,
    Number,
    Comment,
    Function,
    Type,
}

/// A line's spans: byte ranges of the line with their kind. Spans cover the whole line, in order.
pub type Spans = Vec<(Range<usize>, Token)>;

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// The grammar for a path, by extension. TypeScript reads as JavaScript (the bundled set has no TypeScript).
fn syntax_for(path: &str) -> Option<&'static SyntaxReference> {
    let set = syntaxes();
    let file = path.rsplit('/').next().unwrap_or(path);
    let ext = file.rsplit_once('.').map_or(file, |(_, e)| e);
    let ext = match ext {
        "ts" | "tsx" | "mts" | "cts" | "jsx" | "mjs" | "cjs" => "js",
        other => other,
    };
    set.find_syntax_by_extension(ext)
        .or_else(|| set.find_syntax_by_extension(file))
        .filter(|s| s.name != "Plain Text")
}

/// Highlights consecutive `lines` of `path` (no line breaks in them). Unknown file types give one plain span per
/// line.
pub fn highlight(path: &str, lines: &[&str]) -> Vec<Spans> {
    let plain = |line: &str| vec![(0..line.len(), Token::Plain)];
    let Some(syntax) = syntax_for(path) else {
        return lines.iter().map(|l| plain(l)).collect();
    };
    let set = syntaxes();
    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        let text = format!("{line}\n");
        let Ok(ops) = state.parse_line(&text, set) else {
            out.push(plain(line));
            state = ParseState::new(syntax);
            stack = ScopeStack::new();
            continue;
        };
        out.push(spans(line, &ops, &mut stack));
    }
    out
}

fn spans(line: &str, ops: &[(usize, ScopeStackOp)], stack: &mut ScopeStack) -> Spans {
    let mut spans: Spans = Vec::new();
    let mut at = 0;
    let push = |range: Range<usize>, token: Token, spans: &mut Spans| {
        if range.start >= range.end {
            return;
        }
        match spans.last_mut() {
            Some((last, kind)) if *kind == token && last.end == range.start => last.end = range.end,
            _ => spans.push((range, token)),
        }
    };
    for (offset, op) in ops {
        let offset = (*offset).min(line.len());
        push(at..offset, classify(stack.as_slice()), &mut spans);
        at = at.max(offset);
        let _ = stack.apply(op);
    }
    push(at..line.len(), classify(stack.as_slice()), &mut spans);
    if spans.is_empty() {
        spans.push((0..line.len(), Token::Plain));
    }
    spans
}

/// The innermost scope that names a kind wins.
fn classify(scopes: &[Scope]) -> Token {
    for scope in scopes.iter().rev() {
        let name = scope.build_string();
        let kind = if name.starts_with("comment") {
            Token::Comment
        } else if name.starts_with("string") {
            Token::String
        } else if name.starts_with("constant.numeric") || name.starts_with("constant.language") {
            Token::Number
        } else if name.starts_with("keyword") || name.starts_with("storage") {
            Token::Keyword
        } else if name.starts_with("entity.name.function")
            || name.starts_with("support.function")
            || name.starts_with("variable.function")
            || name.starts_with("meta.function-call")
        {
            Token::Function
        } else if name.starts_with("entity.name.type")
            || name.starts_with("entity.name.class")
            || name.starts_with("support.type")
            || name.starts_with("support.class")
        {
            Token::Type
        } else {
            continue;
        };
        return kind;
    }
    Token::Plain
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(path: &str, line: &str) -> Vec<(String, Token)> {
        highlight(path, &[line])[0]
            .iter()
            .map(|(r, t)| (line[r.clone()].to_owned(), *t))
            .filter(|(s, _)| !s.trim().is_empty())
            .collect()
    }

    #[test]
    fn rust_keywords_strings_and_numbers() {
        let k = kinds("src/lib.rs", "let x = \"hi\"; // note 42");
        assert!(k.contains(&("let".into(), Token::Keyword)), "{k:?}");
        assert!(
            k.iter()
                .any(|(s, t)| s.contains("hi") && *t == Token::String),
            "{k:?}"
        );
        assert!(
            k.iter()
                .any(|(s, t)| s.contains("note") && *t == Token::Comment),
            "{k:?}"
        );
    }

    #[test]
    fn typescript_reads_as_javascript() {
        let k = kinds("api/route.ts", "import { x } from \"next/server\"");
        assert!(k.contains(&("import".into(), Token::Keyword)), "{k:?}");
        assert!(
            k.iter()
                .any(|(s, t)| s.contains("next/server") && *t == Token::String),
            "{k:?}"
        );
    }

    #[test]
    fn unknown_files_are_plain_and_spans_cover_the_line() {
        let line = "whatever: 1";
        assert_eq!(
            highlight("notes.unknownext", &[line])[0],
            vec![(0..line.len(), Token::Plain)]
        );
        let spans = &highlight("main.go", &["func main() { return }"])[0];
        assert_eq!(spans.first().unwrap().0.start, 0);
        assert_eq!(spans.last().unwrap().0.end, "func main() { return }".len());
        for pair in spans.windows(2) {
            assert_eq!(pair[0].0.end, pair[1].0.start);
        }
    }

    #[test]
    fn a_string_spanning_lines_in_one_hunk() {
        let out = highlight("a.py", &["x = \"\"\"start", "middle", "end\"\"\""]);
        assert_eq!(out[1], vec![(0..6, Token::String)]);
    }
}
