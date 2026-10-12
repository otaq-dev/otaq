//! Terminal output for the page: the door opens one channel per session and hands its sending end here as a
//! [`Sink`]. Attaching (the stored scrollback, then live output) comes with T-0216.

use crate::App;
use crate::events::Sink;

pub async fn attach(_app: &App, session: &str, _sink: Sink) -> Result<(), String> {
    Err(format!("no terminal session {session}"))
}
