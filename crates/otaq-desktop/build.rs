use std::path::Path;
use std::process::Command;

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
    println!("cargo:rerun-if-changed=build.rs");
    tauri_build::build()
}
