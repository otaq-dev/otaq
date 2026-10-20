//! Skill folders for tests (any crate's): written under a temporary folder the test owns, never in a real home.

use std::path::{Path, PathBuf};

/// A skill folder `<root>/team-skills/<name>/` whose SKILL.md is named `name`; returns the folder.
pub fn skill_folder(root: &Path, name: &str) -> PathBuf {
    let folder = root.join("team-skills").join(name);
    std::fs::create_dir_all(&folder).expect("skill folder");
    std::fs::write(
        folder.join(super::folder::SKILL_FILE),
        format!("---\nname: {name}\ndescription: {name}\n---\nDo the step.\n"),
    )
    .expect("SKILL.md");
    folder
}
