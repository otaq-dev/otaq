//! The bundle check (T-0370, d0090), shared by `otaq notify-check` and `otaq-app --notify-check`: it says which
//! program runs and whether it runs from `Otaq.app` under which bundle ID. In the bundle it reads macOS's permission
//! state, asks once if macOS hasn't asked yet, posts one test notification and waits up to 60 s for its click,
//! saying where the click arrived. Outside the bundle it says so and calls nothing of macOS's notification API (that
//! call would kill the process). Each line goes to `say`; an error ends the check.

use std::path::{Path, PathBuf};

use crate::naming;

/// Runs the check, handing every line to `say`.
pub fn run(say: &mut dyn FnMut(&str)) -> Result<(), String> {
    match std::env::current_exe() {
        Ok(exe) => {
            let real = exe.canonicalize().unwrap_or(exe);
            say(&format!("Program: {}", real.display()));
            if in_temp_folder_now(&real) {
                say(
                    "This bundle is in a temporary folder; macOS refuses notifications for apps there.",
                );
            }
        }
        Err(e) => say(&format!("Program: unknown ({e})")),
    }
    imp::run(say)
}

/// Whether `path` is in a temporary folder, with this process's `$TMPDIR` ([`in_temp_folder`]).
pub(crate) fn in_temp_folder_now(path: &Path) -> bool {
    let tmpdir = std::env::var_os("TMPDIR").map(PathBuf::from);
    in_temp_folder(path, tmpdir.as_deref())
}

/// Whether `path` is in a temporary folder (`/tmp`, `/var/folders`, `$TMPDIR`): LaunchServices marks an app there
/// `in-temp-dir`, and macOS's notification service then refuses it (the bundle check, 2026-10-18).
fn in_temp_folder(path: &Path, tmpdir: Option<&Path>) -> bool {
    let tmpdir = tmpdir.map(|dir| dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf()));
    [
        "/tmp",
        "/private/tmp",
        "/var/folders",
        "/private/var/folders",
    ]
    .iter()
    .map(Path::new)
    .chain(tmpdir.as_deref())
    .any(|dir| path.starts_with(dir))
}

/// The line for a process outside the bundle; `why` says what was found instead.
fn outside(why: &str) -> String {
    format!(
        "outside {}: {why}; macOS's notification API isn't called",
        naming::bundle_folder_name()
    )
}

#[cfg(target_os = "macos")]
mod imp {
    use std::time::Duration;

    use crate::attention::bundle;
    use crate::attention::notification_center::{self as center, ClickListener, Permission};
    use crate::naming;

    /// How long the user has to answer macOS's question.
    const ANSWER_WAIT: Duration = Duration::from_secs(120);
    /// How long the check waits for a click on its notification.
    const CLICK_WAIT: Duration = Duration::from_secs(60);

    pub fn run(say: &mut dyn FnMut(&str)) -> Result<(), String> {
        let main_id = center::main_bundle_id();
        let Some(in_bundle) = center::in_bundle() else {
            let why = match (bundle::current(), main_id) {
                (None, _) => "not in a bundle with its ID".to_owned(),
                (Some(found), Some(id)) => {
                    format!("{} has {}, macOS says {id}", found.root.display(), found.id)
                }
                (Some(found), None) => format!(
                    "{} has {}, macOS gives no bundle ID",
                    found.root.display(),
                    found.id
                ),
            };
            say(&super::outside(&why));
            return Ok(());
        };
        let bundle = in_bundle.bundle();
        say(&format!(
            "In {}: yes ({}), bundle ID {}",
            naming::bundle_folder_name(),
            bundle.root.display(),
            bundle.id
        ));

        let mut permission = center::permission(&in_bundle)
            .ok_or_else(|| "macOS didn't say the permission state".to_owned())?;
        say(&format!("Permission: {}", describe(permission)));
        if permission == Permission::NotAsked {
            say(&format!(
                "Asking macOS for permission: answer its question (waiting up to {} s)…",
                ANSWER_WAIT.as_secs()
            ));
            let granted = center::request_permission(&in_bundle, ANSWER_WAIT).map_err(|e| {
                if e.contains("not allowed") {
                    say(
                        "macOS doesn't accept this program as the bundle's app (see `log show --predicate 'process == \"usernoted\"'`).",
                    );
                }
                format!("macOS refused the question: {e}")
            })?;
            say(&format!(
                "macOS answered: {}",
                if granted { "allowed" } else { "not allowed" }
            ));
            permission = center::permission(&in_bundle).unwrap_or(permission);
            say(&format!("Permission now: {}", describe(permission)));
        }
        if permission != Permission::Allowed {
            say(&format!(
                "Not posting: allow {} in System Settings → Notifications, then run this again",
                naming::PRODUCT_NAME
            ));
            return Ok(());
        }

        let clicks = ClickListener::new(&in_bundle);
        let id = format!(
            "{}-notify-check-{}",
            naming::COMMAND_NAME,
            std::process::id()
        );
        let title = format!("{} · notification check", naming::PRODUCT_NAME);
        center::post(&in_bundle, &id, &title, "Click me")
            .map_err(|e| format!("macOS refused the notification: {e}"))?;
        say(&format!(
            "Posted \"{title}\" / \"Click me\". Click it within {} s…",
            CLICK_WAIT.as_secs()
        ));
        match clicks.wait(CLICK_WAIT) {
            Some(clicked) if clicked == id => say("Click: arrived in this process (its delegate)"),
            Some(other) => say(&format!(
                "Click: arrived in this process (its delegate), on another notification ({other})"
            )),
            None => say(&format!(
                "Click: no click reached this process; macOS may have opened {}",
                naming::app_binary_name()
            )),
        }
        Ok(())
    }

    fn describe(permission: Permission) -> &'static str {
        match permission {
            Permission::NotAsked => "not asked",
            Permission::Allowed => "allowed",
            Permission::TurnedOff => "turned off",
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn run(say: &mut dyn FnMut(&str)) -> Result<(), String> {
        say(&super::outside("this isn't macOS"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_folders_are_recognised() {
        for path in [
            "/tmp/m25/check/Notes.app/Contents/MacOS/notes",
            "/private/tmp/m25/check/Notes.app",
            "/private/var/folders/xy/T/Notes.app",
        ] {
            assert!(in_temp_folder(Path::new(path), None), "{path}");
        }
        assert!(in_temp_folder(
            Path::new("/scratch/tmp-own/Notes.app"),
            Some(Path::new("/scratch/tmp-own"))
        ));
        assert!(!in_temp_folder(
            Path::new("/Users/me/m25-check/Notes.app"),
            None
        ));
        assert!(!in_temp_folder(Path::new("/tmpfoo/Notes.app"), None));
    }
}
