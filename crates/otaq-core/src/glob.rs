//! Path globs, as a project's schema paths are written (M19-e, d0087): `db/migrations/**`, `prisma/schema.prisma`,
//! `src/**/*.sql`. Paths are relative to the repository root and use `/` between folders, as git prints them.
//!
//! - `*` matches any characters within one folder name, `?` one character.
//! - `**` as a whole path part matches any number of folders, none included.
//! - `[abc]`, `[a-z]` and `[!abc]` match one character from (or not from) the set.
//!
//! Small on purpose: it only has to cover the paths a project names.

/// Why `pattern` can't be used as a schema path, if it can't: it must be a relative path inside the repository
/// and a well-formed glob.
pub fn problem(pattern: &str) -> Option<String> {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return Some("A schema path entry is empty".to_owned());
    }
    if trimmed.contains('\\') {
        return Some(format!("{trimmed}: use / between folders"));
    }
    let absolute = trimmed.starts_with('/')
        || trimmed.as_bytes().get(1) == Some(&b':')
        || std::path::Path::new(trimmed).is_absolute();
    if absolute || trimmed.split('/').any(|part| part == "..") {
        return Some(format!(
            "{trimmed} must be a path inside the repository, like db/migrations/**"
        ));
    }
    let chars: Vec<char> = trimmed.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            match class_end(&chars, i) {
                Some(end) => i = end,
                None => return Some(format!("{trimmed}: a [ has no matching ]")),
            }
        }
        i += 1;
    }
    None
}

/// Whether `path` (relative, `/` between folders) matches `pattern`. A pattern with a [`problem`] matches nothing.
pub fn matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim();
    if problem(pattern).is_some() {
        return false;
    }
    let parts: Vec<&str> = pattern.split('/').filter(|p| !p.is_empty()).collect();
    let names: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    match_parts(&parts, &names)
}

fn match_parts(parts: &[&str], names: &[&str]) -> bool {
    match parts.split_first() {
        None => names.is_empty(),
        Some((&"**", rest)) => (0..=names.len()).any(|skip| match_parts(rest, &names[skip..])),
        Some((part, rest)) => names
            .split_first()
            .is_some_and(|(name, names)| match_name(part, name) && match_parts(rest, names)),
    }
}

/// One folder or file name against one pattern part (no `/` in either).
fn match_name(part: &str, name: &str) -> bool {
    let pattern: Vec<char> = part.chars().collect();
    let name: Vec<char> = name.chars().collect();
    match_chars(&pattern, &name)
}

fn match_chars(pattern: &[char], name: &[char]) -> bool {
    let Some((&first, rest)) = pattern.split_first() else {
        return name.is_empty();
    };
    match first {
        '*' => (0..=name.len()).any(|skip| match_chars(rest, &name[skip..])),
        '?' => !name.is_empty() && match_chars(rest, &name[1..]),
        '[' => {
            let Some(end) = class_end(pattern, 0) else {
                return false;
            };
            name.first().is_some_and(|c| in_class(&pattern[1..end], *c))
                && match_chars(&pattern[end + 1..], &name[1..])
        }
        c => name.first() == Some(&c) && match_chars(rest, &name[1..]),
    }
}

/// The index of the `]` closing the class that opens at `start`; a `]` right after `[` or `[!` is a member.
fn class_end(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start + 1;
    if chars.get(i) == Some(&'!') {
        i += 1;
    }
    if chars.get(i) == Some(&']') {
        i += 1;
    }
    while i < chars.len() {
        if chars[i] == ']' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Whether `c` is in the class body (what is between `[` and `]`).
fn in_class(body: &[char], c: char) -> bool {
    let (negated, body) = match body.split_first() {
        Some(('!', rest)) => (true, rest),
        _ => (false, body),
    };
    let mut found = false;
    let mut i = 0;
    while i < body.len() {
        if body.get(i + 1) == Some(&'-') && i + 2 < body.len() {
            found |= (body[i]..=body[i + 2]).contains(&c);
            i += 3;
        } else {
            found |= body[i] == c;
            i += 1;
        }
    }
    found != negated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_and_bad_patterns() {
        for good in [
            "db/migrations/**",
            "prisma/schema.prisma",
            "src/**/*.sql",
            "db/[0-9]*.sql",
            "a/[]]b",
        ] {
            assert_eq!(problem(good), None, "{good}");
        }
        for bad in [
            "",
            "  ",
            "/etc/x",
            "C:/db",
            "../db/**",
            "db/../x",
            "db\\migrations",
            "db/[0-9.sql",
        ] {
            assert!(problem(bad).is_some(), "{bad}");
        }
    }

    #[test]
    fn matching() {
        let yes = [
            ("db/migrations/**", "db/migrations/0001.sql"),
            ("db/migrations/**", "db/migrations/2026/01/x.sql"),
            ("db/migrations/**", "db/migrations"),
            ("src/**/*.sql", "src/a.sql"),
            ("src/**/*.sql", "src/x/y/a.sql"),
            ("prisma/schema.prisma", "prisma/schema.prisma"),
            ("db/[0-9]*.sql", "db/7_add.sql"),
            ("db/[!x]?.sql", "db/ab.sql"),
        ];
        for (pattern, path) in yes {
            assert!(matches(pattern, path), "{pattern} {path}");
        }
        let no = [
            ("db/migrations/**", "db/seed.sql"),
            ("src/*.sql", "src/x/a.sql"),
            ("prisma/schema.prisma", "prisma/schema.prisma.bak"),
            ("db/[0-9]*.sql", "db/a.sql"),
            ("db/[!x]?.sql", "db/xb.sql"),
            ("../db/**", "db/x"),
        ];
        for (pattern, path) in no {
            assert!(!matches(pattern, path), "{pattern} {path}");
        }
    }
}
