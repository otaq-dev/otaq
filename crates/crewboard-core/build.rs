//! Generates the build ID (`BUILD_ID`), shared by the app, the CLI and the service through
//! `crewboard_core::protocol::build_id()`. The app compares it with the running service's ID to offer
//! "Service update ready" after a rebuild (d0022).
//!
//! Format: `<git short hash>[-dirty].<build time in Unix seconds>`, for example `7fb155e9a26d.1791504000`.
//! The script emits no `rerun-if-changed`, so cargo reruns it whenever a file of this crate changes: every
//! rebuild of the core (which holds the service) gets a new ID, while app-only changes keep it.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn main() {
    let hash = git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "nogit".to_owned());
    let dirty = match git(&["status", "--porcelain", "--untracked-files=no"]) {
        Some(s) if !s.is_empty() => "-dirty",
        _ => "",
    };
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    println!("cargo:rustc-env=BUILD_ID={hash}{dirty}.{secs}");
}
