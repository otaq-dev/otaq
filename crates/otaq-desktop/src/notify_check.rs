//! `otaq-app --notify-check` (hidden, T-0370): the bundle check from the bundle's main program, as macOS starts it
//! (`open -n Otaq.app --args --notify-check`). It runs [`otaq_core::attention::notify_check`], writes each line to
//! stdout and to `notify-check.log` in the data folder, and quits without a window or the service. Kept for
//! debugging, like `otaq notify-check`.

use std::io::Write;

use otaq_core::data_dir::DataDir;

/// The argument that runs the check instead of the app.
const FLAG: &str = "--notify-check";

/// Whether the app was started for the check.
pub fn requested() -> bool {
    std::env::args_os().skip(1).any(|arg| arg == FLAG)
}

/// Runs the check; returns the exit code.
pub fn run() -> i32 {
    let log = DataDir::from_env()
        .ok()
        .map(|data| data.root().join("notify-check.log"));
    let mut file = log.as_ref().and_then(|path| {
        std::fs::create_dir_all(path.parent()?).ok()?;
        std::fs::File::create(path).ok()
    });
    let mut say = |line: &str| {
        println!("{line}");
        if let Some(file) = file.as_mut() {
            let _ = writeln!(file, "{line}");
        }
    };
    if let Some(path) = &log {
        say(&format!("Log: {}", path.display()));
    }
    match otaq_core::attention::notify_check::run(&mut say) {
        Ok(()) => 0,
        Err(e) => {
            say(&format!("error: {e}"));
            1
        }
    }
}
