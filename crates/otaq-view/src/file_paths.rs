//! File paths in terminal output (d0117 point 8): which pieces of a line could name a file, with the line and column
//! after it (`src/pricing.ts`, `src/pricing.ts:42`, `src/pricing.ts:42:7`, `src/pricing.ts(42,7)`,
//! `src/pricing.ts:42-50`, an absolute path, a quoted path with spaces). These are only candidates: one is a link once
//! the desktop found an existing file of that name inside the session's folder.

use serde::Serialize;
use ts_rs::TS;

/// The longest path looked at; longer runs are no paths.
const MAX_PATH: usize = 1024;

/// A piece of a line that may name a file. `start` and `end` count UTF-16 units (as the page's strings do), `end`
/// exclusive, and cover the path with its line and column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCandidate {
    pub start: u32,
    pub end: u32,
    /// As written, without a leading `./`: relative to the session's folder, or absolute.
    pub path: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// A file path the page shows as a link (`FileLink` on the page): where it is in the line (UTF-16 units, `end`
/// exclusive), the file inside the session's folder, and the line to open it at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct FileLink {
    pub start: u32,
    pub end: u32,
    /// Relative to the session's folder, `/`-separated.
    pub path: String,
    pub line: Option<u32>,
}

/// The pieces of `text` that may name a file, in order. A quoted path with spaces (`"my notes.md"`) is a candidate
/// beside the unquoted pieces inside it; [`links`] keeps the longer once both are checked.
pub fn candidates(text: &str) -> Vec<PathCandidate> {
    let chars = Chars::new(text);
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !is_path_char(chars.at(i)) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_path_char(chars.at(i)) {
            i += 1;
        }
        out.extend(unquoted(&chars, start, i));
    }
    out.extend(quoted(&chars));
    out.sort_by_key(|c| (c.start, std::cmp::Reverse(c.end)));
    out
}

/// The links among `found` (candidates whose file exists, with its path inside the folder): where two overlap, the
/// longer wins (a quoted path over the pieces inside it).
pub fn links(found: Vec<(PathCandidate, String)>) -> Vec<FileLink> {
    let mut found = found;
    found.sort_by_key(|(c, _)| (std::cmp::Reverse(c.end - c.start), c.start));
    let mut kept: Vec<FileLink> = Vec::new();
    for (c, path) in found {
        if kept.iter().any(|k| c.start < k.end && k.start < c.end) {
            continue;
        }
        kept.push(FileLink {
            start: c.start,
            end: c.end,
            path,
            line: c.line,
        });
    }
    kept.sort_by_key(|l| l.start);
    kept
}

/// Characters a path is made of; a space only inside quotes.
fn is_path_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '@' | '+' | '~' | '%' | '[' | ']')
}

/// The text as characters with their UTF-16 offsets.
struct Chars {
    chars: Vec<char>,
    /// `offsets[i]`: the UTF-16 offset of `chars[i]`; one more at the end.
    offsets: Vec<u32>,
}

impl Chars {
    fn new(text: &str) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let mut offsets = Vec::with_capacity(chars.len() + 1);
        let mut at = 0u32;
        for c in &chars {
            offsets.push(at);
            at += c.len_utf16() as u32;
        }
        offsets.push(at);
        Self { chars, offsets }
    }

    fn len(&self) -> usize {
        self.chars.len()
    }

    fn at(&self, i: usize) -> char {
        self.chars[i]
    }

    fn get(&self, i: usize) -> Option<char> {
        self.chars.get(i).copied()
    }

    fn text(&self, from: usize, to: usize) -> String {
        self.chars[from..to].iter().collect()
    }
}

/// The candidate in the run `from..to` of path characters, with the line and column written after it.
fn unquoted(chars: &Chars, from: usize, to: usize) -> Option<PathCandidate> {
    // `https://…`: the run after the scheme's colon is an address, not a path.
    if from > 0 && chars.at(from - 1) == ':' && chars.get(from) == Some('/') {
        return None;
    }
    let (from, to) = trim(chars, from, to)?;
    let path = normalized(&chars.text(from, to))?;
    // A line and column only right after the whole run: `a.ts.:3` has none.
    let (line, column, end) = position(chars, to);
    Some(PathCandidate {
        start: chars.offsets[from],
        end: chars.offsets[end],
        path,
        line,
        column,
    })
}

/// Quoted paths with spaces: `"…"`, `'…'` or `` `…` ``, opened and closed outside a word (so `don't` opens nothing).
fn quoted(chars: &Chars) -> Vec<PathCandidate> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let q = chars.at(i);
        let opens = matches!(q, '"' | '\'' | '`') && (i == 0 || !chars.at(i - 1).is_alphanumeric());
        if !opens {
            i += 1;
            continue;
        }
        let Some(close) = (i + 1..chars.len()).find(|&j| chars.at(j) == q) else {
            break;
        };
        let closes = chars.get(close + 1).is_none_or(|c| !c.is_alphanumeric());
        let inner = chars.text(i + 1, close);
        // Without a space the unquoted run already has it.
        if closes && inner.contains(' ') {
            let (path_end, line, column) = inner_position(&inner);
            let written: String = inner.chars().take(path_end).collect();
            if written.contains(' ')
                && written.chars().all(|c| c == ' ' || is_path_char(c))
                && !written.starts_with(' ')
                && !written.ends_with(' ')
                && let Some(path) = normalized(&written)
            {
                out.push(PathCandidate {
                    start: chars.offsets[i + 1],
                    end: chars.offsets[close],
                    path,
                    line,
                    column,
                });
            }
        }
        i = close + 1;
    }
    out
}

/// Inside quotes: the path's length in characters, then the line and column written after it (`:42`, `:42:7`,
/// `(42,7)`), which must end the quoted text.
fn inner_position(inner: &str) -> (usize, Option<u32>, Option<u32>) {
    let chars = Chars::new(inner);
    for at in 1..chars.len() {
        if !matches!(chars.at(at), ':' | '(') {
            continue;
        }
        if let (Some(line), column, end) = position(&chars, at)
            && end == chars.len()
        {
            return (at, Some(line), column);
        }
    }
    (chars.len(), None, None)
}

/// Drops what ends a sentence or opens a bracket around the path: trailing `.`, a `[` or `]` with no partner.
fn trim(chars: &Chars, mut from: usize, mut to: usize) -> Option<(usize, usize)> {
    loop {
        let before = (from, to);
        while to > from && chars.at(to - 1) == '.' {
            to -= 1;
        }
        let text = chars.text(from, to);
        let opens = text.matches('[').count();
        let closes = text.matches(']').count();
        if to > from && chars.at(to - 1) == ']' && closes > opens {
            to -= 1;
        }
        if to > from && chars.at(from) == '[' && opens > closes {
            from += 1;
        }
        if to > from + 1
            && chars.at(from) == '['
            && chars.at(to - 1) == ']'
            && opens == 1
            && closes == 1
        {
            from += 1;
            to -= 1;
        }
        if (from, to) == before {
            break;
        }
    }
    (to > from).then_some((from, to))
}

/// `:42`, `:42:7`, `:42-50` or `(42,7)` / `(42, 7)` / `(42)` at `at`: the line, the column and where it ends (`at`
/// when there is none).
fn position(chars: &Chars, at: usize) -> (Option<u32>, Option<u32>, usize) {
    let digits = |from: usize| -> (Option<u32>, usize) {
        let mut i = from;
        while chars.get(i).is_some_and(|c| c.is_ascii_digit()) {
            i += 1;
        }
        let n = chars.text(from, i).parse::<u32>().ok().filter(|n| *n > 0);
        (n, i)
    };
    match chars.get(at) {
        Some(':') => {
            let (Some(line), end) = digits(at + 1) else {
                return (None, None, at);
            };
            match chars.get(end) {
                Some(':') => match digits(end + 1) {
                    (Some(column), after) => (Some(line), Some(column), after),
                    _ => (Some(line), None, end),
                },
                Some('-') => match digits(end + 1) {
                    (Some(_), after) => (Some(line), None, after),
                    _ => (Some(line), None, end),
                },
                _ => (Some(line), None, end),
            }
        }
        Some('(') => {
            let (Some(line), end) = digits(at + 1) else {
                return (None, None, at);
            };
            match chars.get(end) {
                Some(')') => (Some(line), None, end + 1),
                Some(',') => {
                    let from = if chars.get(end + 1) == Some(' ') {
                        end + 2
                    } else {
                        end + 1
                    };
                    match digits(from) {
                        (Some(column), after) if chars.get(after) == Some(')') => {
                            (Some(line), Some(column), after + 1)
                        }
                        _ => (None, None, at),
                    }
                }
                _ => (None, None, at),
            }
        }
        _ => (None, None, at),
    }
}

/// The path as a candidate: no leading `./`, no `.` or `..` part, no empty part, not a folder (`src/`), not `~`, with
/// a letter in it, and looking like a file: a folder in it (`src/a`) or a name with an extension (`a.ts`, `.env`).
fn normalized(written: &str) -> Option<String> {
    let mut path = written;
    while let Some(rest) = path.strip_prefix("./") {
        path = rest;
    }
    if path.is_empty() || path.len() > MAX_PATH || path.ends_with('/') || path.starts_with('~') {
        return None;
    }
    if path.starts_with("//") || !path.chars().any(char::is_alphabetic) {
        return None;
    }
    let relative = path.strip_prefix('/').unwrap_or(path);
    let parts: Vec<&str> = relative.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return None;
    }
    let name = parts.last()?;
    let has_extension = name
        .rfind('.')
        .is_some_and(|dot| name[dot + 1..].chars().any(char::is_alphabetic));
    (path.contains('/') || has_extension).then(|| path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The candidates' paths with their lines and columns.
    fn found(text: &str) -> Vec<(String, Option<u32>, Option<u32>)> {
        candidates(text)
            .into_iter()
            .map(|c| (c.path, c.line, c.column))
            .collect()
    }

    fn one(
        path: &str,
        line: Option<u32>,
        column: Option<u32>,
    ) -> Vec<(String, Option<u32>, Option<u32>)> {
        vec![(path.to_owned(), line, column)]
    }

    /// The text a candidate covers.
    fn covered(text: &str) -> Vec<String> {
        let units: Vec<u16> = text.encode_utf16().collect();
        candidates(text)
            .iter()
            .map(|c| String::from_utf16(&units[c.start as usize..c.end as usize]).unwrap())
            .collect()
    }

    #[test]
    fn a_path_alone_with_a_line_and_with_a_column() {
        assert_eq!(found("src/pricing.ts"), one("src/pricing.ts", None, None));
        assert_eq!(
            found("see src/pricing.ts:42"),
            one("src/pricing.ts", Some(42), None)
        );
        assert_eq!(
            found("src/pricing.ts:42:7 error"),
            one("src/pricing.ts", Some(42), Some(7))
        );
        assert_eq!(
            covered("at src/pricing.ts:42:7 here"),
            vec!["src/pricing.ts:42:7"]
        );
    }

    #[test]
    fn the_other_forms_of_a_line() {
        assert_eq!(
            found("src/a.ts(42,7): error"),
            one("src/a.ts", Some(42), Some(7))
        );
        assert_eq!(found("src/a.ts(42, 7)"), one("src/a.ts", Some(42), Some(7)));
        assert_eq!(found("src/a.ts(42)"), one("src/a.ts", Some(42), None));
        assert_eq!(found("src/a.ts:42-50"), one("src/a.ts", Some(42), None));
        assert_eq!(covered("src/a.ts:42-50,"), vec!["src/a.ts:42-50"]);
        assert_eq!(covered("src/a.ts(42,7): x"), vec!["src/a.ts(42,7)"]);
    }

    #[test]
    fn absolute_paths_and_a_leading_dot_slash() {
        assert_eq!(
            found("/Users/me/shop/src/a.ts:3"),
            one("/Users/me/shop/src/a.ts", Some(3), None)
        );
        assert_eq!(found("./src/a.ts"), one("src/a.ts", None, None));
        assert_eq!(covered("./src/a.ts"), vec!["./src/a.ts"]);
    }

    #[test]
    fn trailing_punctuation_is_not_part_of_the_path() {
        assert_eq!(found("Edited src/a.ts."), one("src/a.ts", None, None));
        assert_eq!(found("src/a.ts, src/b.ts"), {
            let mut both = one("src/a.ts", None, None);
            both.extend(one("src/b.ts", None, None));
            both
        });
        assert_eq!(found("(src/a.ts)"), one("src/a.ts", None, None));
        assert_eq!(found("src/a.ts: changed"), one("src/a.ts", None, None));
        assert_eq!(covered("src/a.ts: changed"), vec!["src/a.ts"]);
        assert_eq!(covered("at src/a.ts:42."), vec!["src/a.ts:42"]);
        assert_eq!(covered("[src/a.ts]"), vec!["src/a.ts"]);
        assert_eq!(
            found("app/[id]/page.tsx"),
            one("app/[id]/page.tsx", None, None)
        );
    }

    #[test]
    fn outside_the_folder_folders_and_non_paths_are_no_candidates() {
        assert!(found("../outside.ts").is_empty());
        assert!(found("src/../../outside.ts").is_empty());
        assert!(found("src/lib/").is_empty());
        assert!(found("~/notes.md").is_empty());
        assert!(found("1/2 and 3.5 and v1.2.3").is_empty());
        assert!(found("Makefile and words").is_empty());
        assert!(
            found("https://example.com/a/b.ts")
                .iter()
                .all(|(p, _, _)| p != "//example.com/a/b.ts")
        );
        assert!(
            !found("https://example.com/a/b.ts")
                .iter()
                .any(|(p, _, _)| p.starts_with('/'))
        );
    }

    #[test]
    fn a_path_with_spaces_only_in_quotes() {
        assert_eq!(found("my notes/plan.md"), one("notes/plan.md", None, None));
        let quoted = found("open \"my notes/plan.md:3\" now");
        assert!(quoted.contains(&("my notes/plan.md".to_owned(), Some(3), None)));
        assert!(covered("'docs/a b.md'").contains(&"docs/a b.md".to_owned()));
        assert!(found("don't src isn't").is_empty());
    }

    #[test]
    fn offsets_count_utf16_units() {
        let text = "✻ 📦 src/a.ts:4";
        let c = &candidates(text)[0];
        let units: Vec<u16> = text.encode_utf16().collect();
        assert_eq!(
            String::from_utf16(&units[c.start as usize..c.end as usize]).unwrap(),
            "src/a.ts:4"
        );
    }

    #[test]
    fn the_longer_of_two_overlapping_links_wins() {
        let text = "\"my notes/plan.md\"";
        let found: Vec<(PathCandidate, String)> = candidates(text)
            .into_iter()
            .map(|c| (c.clone(), c.path))
            .collect();
        let kept = links(found);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].path, "my notes/plan.md");
    }
}
