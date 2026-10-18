//! `otaq-app --notify-check` (T-0370) outside `Otaq.app`: it says so on stdout and in `notify-check.log` in the data
//! folder, exits 0 without a window or the service, and calls nothing of macOS's notification API.

use std::process::Command;

use otaq_core::data_dir::DataDir;
use otaq_core::naming;

#[test]
fn outside_the_bundle_it_says_so_and_quits() {
    let home = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_otaq-app"))
        .arg("--notify-check")
        .env(DataDir::env_var(), home.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let outside = format!("outside {}", naming::bundle_folder_name());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(&outside), "{stdout}");
    let log = std::fs::read_to_string(home.path().join("notify-check.log")).unwrap();
    assert!(log.contains(&outside), "{log}");
    assert!(!home.path().join("logs").exists(), "no service started");
}
