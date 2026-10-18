//! `otaq notify-check` (hidden, T-0370): the bundle check's tool, kept for debugging. It says whether this process
//! runs from `Otaq.app` and under which bundle ID. In the bundle it reads macOS's permission state, asks once if
//! macOS hasn't asked yet, posts one test notification and waits up to 60 s for its click, saying where the click
//! arrived (d0090). Outside the bundle it says so and calls nothing of macOS's notification API (that call would kill
//! the process).

use otaq_core::naming;
use otaq_core::protocol::ErrorBody;

pub fn run() -> Result<(), ErrorBody> {
    match std::env::current_exe() {
        Ok(exe) => println!(
            "Program: {}",
            crate::daemon::service_program(&exe).display()
        ),
        Err(e) => println!("Program: unknown ({e})"),
    }
    imp::run()
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

    use otaq_core::attention::bundle;
    use otaq_core::attention::notification_center::{self as center, ClickListener, Permission};
    use otaq_core::naming;
    use otaq_core::protocol::{ErrorBody, ErrorKind};

    /// How long the user has to answer macOS's question.
    const ANSWER_WAIT: Duration = Duration::from_secs(120);
    /// How long the check waits for a click on its notification.
    const CLICK_WAIT: Duration = Duration::from_secs(60);

    pub fn run() -> Result<(), ErrorBody> {
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
            println!("{}", super::outside(&why));
            return Ok(());
        };
        let bundle = in_bundle.bundle();
        println!(
            "In {}: yes ({}), bundle ID {}",
            naming::bundle_folder_name(),
            bundle.root.display(),
            bundle.id
        );

        let mut permission = center::permission(&in_bundle)
            .ok_or_else(|| internal("macOS didn't say the permission state".to_owned()))?;
        println!("Permission: {}", describe(permission));
        if permission == Permission::NotAsked {
            println!(
                "Asking macOS for permission: answer its question (waiting up to {} s)…",
                ANSWER_WAIT.as_secs()
            );
            let granted = center::request_permission(&in_bundle, ANSWER_WAIT)
                .map_err(|e| internal(format!("macOS refused the question: {e}")))?;
            println!(
                "macOS answered: {}",
                if granted { "allowed" } else { "not allowed" }
            );
            permission = center::permission(&in_bundle).unwrap_or(permission);
            println!("Permission now: {}", describe(permission));
        }
        if permission != Permission::Allowed {
            println!(
                "Not posting: allow {} in System Settings → Notifications, then run this again",
                naming::PRODUCT_NAME
            );
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
            .map_err(|e| internal(format!("macOS refused the notification: {e}")))?;
        println!(
            "Posted \"{title}\" / \"Click me\". Click it within {} s…",
            CLICK_WAIT.as_secs()
        );
        match clicks.wait(CLICK_WAIT) {
            Some(clicked) if clicked == id => {
                println!("Click: arrived in this process (its delegate)");
            }
            Some(other) => println!(
                "Click: arrived in this process (its delegate), on another notification ({other})"
            ),
            None => println!(
                "Click: no click reached this process; macOS may have opened {}",
                naming::app_binary_name()
            ),
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

    fn internal(message: String) -> ErrorBody {
        ErrorBody::new(ErrorKind::Internal, message)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use otaq_core::protocol::ErrorBody;

    pub fn run() -> Result<(), ErrorBody> {
        println!("{}", super::outside("this isn't macOS"));
        Ok(())
    }
}
