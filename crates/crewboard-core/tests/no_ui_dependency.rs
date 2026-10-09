//! Decision 0002: `crewboard-core` has no UI dependency. This test asks cargo for the core's dependency
//! tree (normal and build dependencies) and fails if a UI framework appears in it.

use std::process::Command;

const UI_CRATES: &[&str] = &["gpui"];

#[test]
fn core_does_not_depend_on_a_ui_framework() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--offline",
            "--package",
            env!("CARGO_PKG_NAME"),
            "--edges",
            "normal,build",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout).expect("utf-8 output");
    assert!(
        tree.lines().any(|l| l.starts_with("tracing ")),
        "unexpected tree:\n{tree}"
    );

    let ui: Vec<&str> = tree
        .lines()
        .filter(|line| {
            let name = line.split_whitespace().next().unwrap_or_default();
            UI_CRATES.contains(&name)
        })
        .collect();
    assert!(
        ui.is_empty(),
        "crewboard-core must not depend on {ui:?} (d0002)"
    );
}
