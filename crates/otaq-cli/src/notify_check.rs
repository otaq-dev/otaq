//! `otaq notify-check` (hidden, T-0370): the bundle check from the command, kept for debugging. The check itself is
//! [`otaq_core::attention::notify_check`]; the app runs the same one with `--notify-check`.

use otaq_core::protocol::{ErrorBody, ErrorKind};

pub fn run() -> Result<(), ErrorBody> {
    otaq_core::attention::notify_check::run(&mut |line| println!("{line}"))
        .map_err(|e| ErrorBody::new(ErrorKind::Internal, e))
}
