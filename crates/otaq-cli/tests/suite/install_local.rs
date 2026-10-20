//! `scripts/install-local.sh` (T-0370, d0072) on a fake build: the bundle's layout, its `Info.plist` and its ad-hoc
//! signature. The fake programs are copies of `/usr/bin/true`, so no release build runs here; the real build is the
//! hand-run script. `--dest` installs into a temporary folder, and `HOME` and `OTAQ_HOME` point into it too, so
//! nothing reaches the user's folders or service. macOS only (the script builds `Otaq.app`).

#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};
use std::process::Command;

use otaq_core::agent_tools::claude::find_plugin_dir;
use otaq_core::attention::bundle;
use otaq_core::data_dir::DataDir;
use otaq_core::naming;

const CHECK_ID: &str = "io.example.check";

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/install-local.sh")
}

fn plist_value(plist: &Path, key: &str) -> String {
    let out = Command::new("plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(plist)
        .output()
        .unwrap();
    assert!(out.status.success(), "{key}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// Installs from a fake build into `<tmp>/dest` with `bundle_id`; returns the script's stdout.
fn install(tmp: &Path, bundle_id: &str) -> String {
    let build = tmp.join("build");
    std::fs::create_dir_all(&build).unwrap();
    for program in [naming::COMMAND_NAME.to_owned(), naming::app_binary_name()] {
        std::fs::copy("/usr/bin/true", build.join(program)).unwrap();
    }
    let out = Command::new("bash")
        .arg(script())
        .args(["--from".as_ref(), build.as_os_str()])
        .args(["--dest".as_ref(), tmp.join("dest").as_os_str()])
        .args(["--bundle-id", bundle_id])
        .env("HOME", tmp.join("home"))
        .env(DataDir::env_var(), tmp.join("data"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn the_bundle_has_the_layout_plist_and_signature() {
    let tmp = tempfile::tempdir().unwrap();
    let tmp = tmp.path().canonicalize().unwrap();
    let id = format!("{}.check", naming::BUNDLE_ID);
    install(&tmp, &id);

    let app = tmp.join("dest").join(naming::bundle_folder_name());
    let contents = app.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    let mut programs: Vec<_> = std::fs::read_dir(&macos)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    programs.sort();
    assert_eq!(
        programs,
        [naming::COMMAND_NAME.to_owned(), naming::app_binary_name()]
    );
    assert!(resources.join("icon.icns").is_file());
    for folder in ["claude", "copilot"] {
        assert!(
            resources
                .join("plugin")
                .join(folder)
                .join(".claude-plugin/plugin.json")
                .is_file(),
            "{folder}"
        );
    }
    assert!(
        resources
            .join("plugin/codex/.codex-plugin/plugin.json")
            .is_file()
    );
    assert!(
        resources
            .join("plugin/skills/work-ticket/SKILL.md")
            .is_file(),
        "the shipped skills are in the bundle"
    );

    let plist = contents.join("Info.plist");
    assert_eq!(plist_value(&plist, "CFBundleIdentifier"), id);
    assert_eq!(
        plist_value(&plist, "CFBundleExecutable"),
        naming::app_binary_name()
    );
    assert_eq!(plist_value(&plist, "CFBundleName"), naming::PRODUCT_NAME);
    assert_eq!(plist_value(&plist, "CFBundleIconFile"), "icon.icns");
    assert_eq!(
        plist_value(&plist, "CFBundleShortVersionString"),
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(plist_value(&plist, "LSMinimumSystemVersion"), "10.13");

    let verify = Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&app)
        .output()
        .unwrap();
    assert!(verify.status.success(), "{verify:?}");
    let details = Command::new("codesign")
        .args(["--display", "--verbose=2"])
        .arg(macos.join(naming::COMMAND_NAME))
        .output()
        .unwrap();
    let details = String::from_utf8_lossy(&details.stderr);
    assert!(details.contains(&format!("Identifier={id}\n")), "{details}");
    assert!(details.contains("Signature=adhoc"), "{details}");

    // The program, the plugin and the service's path as the code finds them from the bundle.
    let command = macos.join(naming::COMMAND_NAME);
    assert_eq!(bundle::bundle_of(&command).map(|b| b.id), Some(id));
    assert_eq!(find_plugin_dir(&macos), Some(resources.join("plugin")));
    assert!(!tmp.join("home/.local").exists(), "--dest makes no link");
}

#[test]
fn a_second_install_stops_the_old_service_and_replaces_the_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    let first = install(tmp.path(), CHECK_ID);
    assert!(!first.contains("Stopping"), "{first}");
    let second = install(tmp.path(), CHECK_ID);
    assert!(
        second.contains("Stopping the background service"),
        "{second}"
    );
    let left: Vec<_> = std::fs::read_dir(tmp.path().join("dest"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        left,
        [naming::bundle_folder_name().as_str()],
        "no staging folder left"
    );
}
