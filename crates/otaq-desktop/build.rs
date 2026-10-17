use std::path::Path;
use std::process::Command;

/// Otaq's Aptabase app (EU region), d0085.
const APTABASE_APP_KEY: &str = "A-EU-5140360391";

fn main() {
    // The commit Settings → About shows (d0071): the short hash of HEAD, `unknown` outside a git checkout.
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|hash| !hash.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=BUILD_COMMIT={commit}");
    // A new commit or a checkout changes HEAD (or the branch it points to); rebuild so the hash follows.
    if let Ok(out) = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
    {
        let git_dir = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        let head = Path::new(&git_dir).join("HEAD");
        println!("cargo:rerun-if-changed={}", head.display());
        if let Ok(head_text) = std::fs::read_to_string(&head)
            && let Some(branch) = head_text.trim().strip_prefix("ref: ")
        {
            println!(
                "cargo:rerun-if-changed={}",
                Path::new(&git_dir).join(branch).display()
            );
        }
    }
    // Anonymous usage data (d0085): the Aptabase App Key, and the host of a self-hosted one. Release builds get Otaq's
    // own key (App Keys ship inside every client, so it isn't a secret); debug builds get none and never send anything.
    // `OTAQ_APTABASE_APP_KEY` / `OTAQ_APTABASE_HOST` override them.
    let release = std::env::var("PROFILE").is_ok_and(|p| p == "release");
    for (from, to, default) in [
        (
            "OTAQ_APTABASE_APP_KEY",
            "USAGE_APP_KEY",
            release.then_some(APTABASE_APP_KEY),
        ),
        ("OTAQ_APTABASE_HOST", "USAGE_HOST", None),
    ] {
        println!("cargo:rerun-if-env-changed={from}");
        if let Some(value) = std::env::var(from).ok().or(default.map(str::to_owned)) {
            println!("cargo:rustc-env={to}={value}");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
    tauri_build::build()
}
