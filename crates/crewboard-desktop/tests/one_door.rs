//! UIT-02: `src/door.rs` is the only Rust module that names Tauri. This test reads every file under `src/` and fails
//! if another one names `tauri` (or a `tauri_*` crate) outside a comment. `build.rs` is the build script, not a module,
//! and calls `tauri_build` by necessity.

use std::fs;
use std::path::{Path, PathBuf};

const DOOR: &str = "door.rs";

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

/// The 1-based lines of `src` that name Tauri in code: `tauri` at the start of an identifier, ignoring `//` comments.
fn lines_naming_tauri(src: &str) -> Vec<usize> {
    src.lines()
        .enumerate()
        .filter(|(_, line)| {
            let code = line.split("//").next().unwrap_or_default();
            code.match_indices("tauri").any(|(at, _)| {
                !code[..at]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
            })
        })
        .map(|(i, _)| i + 1)
        .collect()
}

#[test]
fn only_the_door_names_tauri() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let door = src.join(DOOR);
    assert!(files.contains(&door), "the door module is missing");
    assert!(
        !lines_naming_tauri(&fs::read_to_string(&door).expect("readable door")).is_empty(),
        "the door should name Tauri"
    );

    let mut offenders = Vec::new();
    for file in files.iter().filter(|f| **f != door) {
        let text = fs::read_to_string(file).expect("readable source");
        for line in lines_naming_tauri(&text) {
            offenders.push(format!("{}:{line}", file.display()));
        }
    }
    assert!(
        offenders.is_empty(),
        "only src/{DOOR} may name Tauri (UIT-02); move these calls into it:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_scanner_finds_code_and_skips_comments() {
    let src = "use tauri::State;\n// tauri in a comment\nlet x = 1; // tauri::State\nfn tauri_like() {}\nmy_tauri();\n#[tauri::command]\n";
    assert_eq!(lines_naming_tauri(src), [1, 4, 6]);
}
