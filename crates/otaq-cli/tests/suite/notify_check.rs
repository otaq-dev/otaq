//! `otaq notify-check` (T-0370) outside `Otaq.app`: it says so, exits 0 and calls nothing of macOS's notification API
//! (which would kill a process outside a bundle). It isn't listed in `--help`.

use std::process::Command;

use otaq_core::data_dir::DataDir;
use otaq_core::naming;

#[test]
fn outside_the_bundle_it_says_so_and_touches_nothing() {
    let home = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_otaq"))
        .arg("notify-check")
        .env(DataDir::env_var(), home.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(&format!("outside {}", naming::bundle_folder_name())),
        "{stdout}"
    );
    assert!(!stdout.contains("Posted"), "{stdout}");
}

#[test]
fn it_is_hidden_from_help() {
    let out = Command::new(env!("CARGO_BIN_EXE_otaq"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&out.stdout).contains("notify-check"));
}
