//! The types the page reads are generated into TypeScript with `ts-rs`, never written by hand on that side (d0075
//! point 3, UIT-04). They land in `ui/src/generated/`, one file per type, committed; that folder holds only generated
//! files. One command regenerates them all:
//!
//! ```sh
//! cargo test -p crewboard-desktop regenerate_types -- --ignored
//! ```
//!
//! `generated_types_are_fresh` (part of `cargo test`, so of `scripts/check.sh`) generates them into a temporary folder
//! and fails, naming the stale files, when the committed ones differ.
//!
//! A screen adds the `TS` derive (with serde) to its own types and one line per type to [`export_all`]; ts-rs also
//! writes every type a listed type refers to.

use std::collections::BTreeMap;
use std::path::Path;

use ts_rs::{Config, ExportError, TS};

/// The committed folder, relative to this crate.
const FOLDER: &str = "ui/src/generated";

fn export_all(dir: &Path) -> Result<(), ExportError> {
    // 64-bit integers arrive as JSON numbers (serde_json), not as `bigint`.
    let cfg = Config::new().with_out_dir(dir).with_large_int("number");
    crewboard_view::keymap::Modifiers::export_all(&cfg)?;
    crewboard_view::keymap::KeyAction::export_all(&cfg)?;
    crewboard_view::view_models::panel::Tab::export_all(&cfg)?;
    crewboard_view::terminal::Phase::export_all(&cfg)?;
    // Also writes `ServiceConnection` and `ServiceCompatibility`, which it refers to.
    crate::events::PageEvent::export_all(&cfg)?;
    crate::api::RequestError::export_all(&cfg)?;
    crewboard_view::icons::IconName::export_all(&cfg)?;
    crate::api::ThemeParams::export_all(&cfg)?;
    crate::api::ActiveTheme::export_all(&cfg)?;
    crate::window::Chrome::export_all(&cfg)?;
    crate::window::Action::export_all(&cfg)?;
    Ok(())
}

/// Each file in `dir` by name, with its text. Line endings are compared as `\n`, so a Windows checkout isn't stale.
fn files(dir: &Path) -> BTreeMap<String, String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return BTreeMap::new();
    };
    entries
        .flatten()
        .map(|e| {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            (
                e.file_name().to_string_lossy().into_owned(),
                text.replace("\r\n", "\n"),
            )
        })
        .collect()
}

/// The files of `committed` that don't match `fresh`: changed, missing, or not generated at all.
fn stale_files(
    fresh: &BTreeMap<String, String>,
    committed: &BTreeMap<String, String>,
) -> Vec<String> {
    let changed = fresh
        .iter()
        .filter(|(name, text)| committed.get(*name) != Some(text))
        .map(|(name, _)| name.clone());
    let extra = committed
        .keys()
        .filter(|name| !fresh.contains_key(*name))
        .map(|name| format!("{name} (not generated)"));
    changed.chain(extra).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn committed() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(FOLDER)
    }

    #[test]
    fn generated_types_are_fresh() {
        let fresh = tempfile::tempdir().unwrap();
        export_all(fresh.path()).unwrap();
        let stale = stale_files(&files(fresh.path()), &files(&committed()));
        assert!(
            stale.is_empty(),
            "{FOLDER} is stale: {}. Run `cargo test -p {} regenerate_types -- --ignored` to regenerate.",
            stale.join(", "),
            env!("CARGO_PKG_NAME"),
        );
    }

    /// The one command that regenerates the folder (see the module docs).
    #[test]
    #[ignore = "writes into the source tree; run on purpose to regenerate"]
    fn regenerate_types() {
        let dir = committed();
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        export_all(&dir).unwrap();
    }

    #[test]
    fn stale_files_names_changed_missing_and_extra_files() {
        let map = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs
                .iter()
                .map(|(n, t)| (n.to_string(), t.to_string()))
                .collect()
        };
        let fresh = map(&[("A.ts", "a"), ("B.ts", "b"), ("C.ts", "c")]);
        let committed = map(&[("A.ts", "a"), ("B.ts", "old"), ("Hand.ts", "x")]);
        assert_eq!(
            stale_files(&fresh, &committed),
            ["B.ts", "C.ts", "Hand.ts (not generated)"]
        );
        assert!(stale_files(&fresh, &fresh).is_empty());
    }

    #[test]
    fn line_endings_dont_make_files_stale() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.ts"), "one\r\ntwo\r\n").unwrap();
        assert_eq!(files(dir.path())["A.ts"], "one\ntwo\n");
    }
}
