//! Decisions 0028 and 0076: the product name, command name, plugin namespace and env prefix come from one constant.
//! This test reads every `src/` file of the workspace crates and fails if a string literal outside
//! `otaq-core/src/naming.rs` contains the name. Comments and identifiers (such as `otaq_core`) are allowed.

use std::fs;
use std::path::{Path, PathBuf};

const NAME: &str = "otaq";

fn workspace_crates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates folder")
        .to_path_buf()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("readable folder") {
        let path = entry.expect("folder entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Returns the contents of every string literal (plain, byte and raw) in Rust source, with the line it starts on.
/// Comments and char literals are skipped. Small on purpose: it only has to be right for this repository.
fn string_literals(src: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = src.chars().collect();
    let mut found = Vec::new();
    let mut line = 1;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '\n' => {
                line += 1;
                i += 1;
            }
            '/' if next == Some('/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if next == Some('*') => {
                let mut depth = 0;
                while i < chars.len() {
                    if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                        depth += 1;
                        i += 2;
                    } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                        depth -= 1;
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        if chars[i] == '\n' {
                            line += 1;
                        }
                        i += 1;
                    }
                }
            }
            'r' if matches!(next, Some('"') | Some('#'))
                && !is_ident_char(chars.get(i.wrapping_sub(1))) =>
            {
                // Raw string: r"…", r#"…"#, br#"…"#.
                let mut j = i + 1;
                let mut hashes = 0;
                while chars.get(j) == Some(&'#') {
                    hashes += 1;
                    j += 1;
                }
                if chars.get(j) != Some(&'"') {
                    i += 1;
                    continue;
                }
                let start_line = line;
                let mut text = String::new();
                j += 1;
                while j < chars.len() {
                    if chars[j] == '"' && (1..=hashes).all(|k| chars.get(j + k) == Some(&'#')) {
                        j += 1 + hashes;
                        break;
                    }
                    if chars[j] == '\n' {
                        line += 1;
                    }
                    text.push(chars[j]);
                    j += 1;
                }
                found.push((start_line, text));
                i = j;
            }
            '"' => {
                let start_line = line;
                let mut text = String::new();
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' {
                        text.push(chars[i]);
                        i += 1;
                    }
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    text.push(chars[i]);
                    i += 1;
                }
                found.push((start_line, text));
                i += 1;
            }
            '\'' => {
                // A char literal ('"', '\'', 'x') is skipped; a lifetime ('a) is not a literal.
                if next == Some('\\') {
                    i += 2;
                    while i < chars.len() && chars[i] != '\'' {
                        i += 1;
                    }
                    i += 1;
                } else if chars.get(i + 2) == Some(&'\'') {
                    i += 3;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    found
}

fn is_ident_char(c: Option<&char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || *c == '_')
}

#[test]
fn names_are_spelled_out_only_in_the_naming_module() {
    let crates = workspace_crates();
    let allowed = crates.join("otaq-core/src/naming.rs");
    let mut files = Vec::new();
    for entry in fs::read_dir(&crates).expect("crates folder") {
        let src = entry.expect("crate").path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut files);
        }
    }
    assert!(
        files.len() >= 3,
        "expected the crates' source files, found {files:?}"
    );

    let mut offenders = Vec::new();
    for file in files.iter().filter(|f| **f != allowed) {
        let src = fs::read_to_string(file).expect("readable source");
        for (line, text) in string_literals(&src) {
            if text.to_lowercase().contains(NAME) {
                offenders.push(format!("{}:{line}: \"{text}\"", file.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "use otaq_core::naming instead of spelling out the name (d0028):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_scanner_finds_literals_and_skips_comments() {
    let src = r##"
        // "otaq" in a comment
        /* "otaq" in a block */
        let a = 'x'; let q = '"'; fn f<'a>(s: &'a str) {}
        let b = "plain OTAQ_TICKET";
        let c = r#"raw "otaq/" text"#;
    "##;
    let texts: Vec<String> = string_literals(src).into_iter().map(|(_, t)| t).collect();
    assert_eq!(texts, ["plain OTAQ_TICKET", "raw \"otaq/\" text"]);
}
