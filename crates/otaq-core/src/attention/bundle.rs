//! Whether a program runs from inside `Otaq.app` (d0072, d0090): its real path (links resolved) is
//! `<X>.app/Contents/MacOS/<program>` and the bundle's `Info.plist` names Otaq's bundle ID. macOS ties a process to
//! its bundle by that real path, and its notification API works only inside a bundle. Plain path checks, so they run
//! on every platform; the macOS side ([`super::notification_center`]) adds that macOS agrees.

use std::path::{Path, PathBuf};

use crate::naming;

/// The bundle a program runs from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    /// The `.app` folder.
    pub root: PathBuf,
    /// `CFBundleIdentifier` from its `Info.plist`.
    pub id: String,
}

/// The bundle `exe` sits in, or `None` outside one (`cargo run`, `target/release`, tests).
pub fn bundle_of(exe: &Path) -> Option<Bundle> {
    let real = exe.canonicalize().ok()?;
    let macos = real.parent()?;
    let contents = macos.parent()?;
    let root = contents.parent()?;
    let layout = macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && root.extension()? == "app";
    if !layout {
        return None;
    }
    let plist = std::fs::read_to_string(contents.join("Info.plist")).ok()?;
    let id = plist_bundle_id(&plist)?;
    is_otaq_id(&id).then(|| Bundle {
        root: root.to_path_buf(),
        id,
    })
}

/// The bundle the running program sits in.
pub fn current() -> Option<Bundle> {
    bundle_of(&std::env::current_exe().ok()?)
}

/// Otaq's bundle ID, or a check bundle's `<ID>.<suffix>` (`install-local.sh --bundle-id`, T-0370), so a check can ask
/// macOS without using up the real ID's one question.
pub fn is_otaq_id(id: &str) -> bool {
    id.strip_prefix(naming::BUNDLE_ID)
        .is_some_and(|rest| rest.is_empty() || rest.len() > 1 && rest.starts_with('.'))
}

/// `CFBundleIdentifier` from an XML `Info.plist`, as `install-local.sh` writes it.
fn plist_bundle_id(plist: &str) -> Option<String> {
    let after = plist.split_once("<key>CFBundleIdentifier</key>")?.1;
    let value = after.trim_start().strip_prefix("<string>")?;
    let id = value.split_once("</string>")?.0.trim();
    (!id.is_empty()).then(|| id.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake `Otaq.app` under `dir` with `id` in its `Info.plist` and an empty `otaq` program; returns the program.
    fn fake_bundle(dir: &Path, id: &str) -> PathBuf {
        let contents = dir.join(naming::bundle_folder_name()).join("Contents");
        std::fs::create_dir_all(contents.join("MacOS")).unwrap();
        std::fs::write(
            contents.join("Info.plist"),
            format!(
                "<plist version=\"1.0\"><dict>\n\t<key>CFBundleIdentifier</key>\n\t<string>{id}</string>\n</dict></plist>\n"
            ),
        )
        .unwrap();
        let exe = contents.join("MacOS").join(naming::COMMAND_NAME);
        std::fs::write(&exe, "").unwrap();
        exe
    }

    #[test]
    fn a_program_in_the_bundle_is_in_it() {
        let dir = tempfile::tempdir().unwrap();
        let exe = fake_bundle(dir.path(), naming::BUNDLE_ID);
        let bundle = bundle_of(&exe).expect("in the bundle");
        assert_eq!(bundle.id, naming::BUNDLE_ID);
        assert_eq!(
            bundle.root,
            dir.path()
                .canonicalize()
                .unwrap()
                .join(naming::bundle_folder_name())
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_into_the_bundle_counts_by_its_real_path() {
        let dir = tempfile::tempdir().unwrap();
        let exe = fake_bundle(dir.path(), naming::BUNDLE_ID);
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let link = bin.join(naming::COMMAND_NAME);
        std::os::unix::fs::symlink(&exe, &link).unwrap();
        assert!(bundle_of(&link).is_some());
    }

    #[test]
    fn a_check_bundle_counts_and_another_app_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let check = fake_bundle(
            &dir.path().join("check"),
            &format!("{}.check", naming::BUNDLE_ID),
        );
        assert!(bundle_of(&check).is_some());
        let other = fake_bundle(&dir.path().join("other"), "com.example.notes");
        assert_eq!(bundle_of(&other), None);
        let lookalike = fake_bundle(&dir.path().join("like"), &format!("{}x", naming::BUNDLE_ID));
        assert_eq!(bundle_of(&lookalike), None);
    }

    #[test]
    fn a_plain_build_is_outside() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("release").join(naming::COMMAND_NAME);
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, "").unwrap();
        assert_eq!(bundle_of(&exe), None);
        assert_eq!(bundle_of(&dir.path().join("missing")), None);
    }
}
