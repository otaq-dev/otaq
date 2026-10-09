//! The `crewboard` binary as a user or agent sees it: help, error format, exit codes.
//! Every run gets a temporary data folder, so nothing touches the real one.

use std::process::{Command, Output};

use crewboard_core::data_dir::DataDir;

fn run(args: &[&str]) -> Output {
    let home = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_crewboard"))
        .args(args)
        .env(DataDir::env_var(), home.path())
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn help_lists_daemon() {
    let out = run(&["--help"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("daemon"), "{text}");
    for flag in ["--json", "--quiet", "--silent", "--project", "--actor"] {
        assert!(text.contains(flag), "{flag} missing:\n{text}");
    }
}

#[test]
fn usage_errors_in_json() {
    let out = run(&["daemon", "frobnicate", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let parsed: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(parsed["error"]["kind"], "usage");
    assert!(
        parsed["error"]["message"]
            .as_str()
            .unwrap()
            .contains("frobnicate"),
        "{parsed}"
    );
}

#[test]
fn usage_errors_for_humans() {
    let out = run(&["daemon", "frobnicate"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("frobnicate"));
}
