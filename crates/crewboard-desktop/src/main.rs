//! The Crewboard desktop app on Tauri 2 (d0074, d0075): a window whose page is React with TypeScript, built with Vite
//! from `ui/`. Only [`door`] names Tauri (UIT-02); everything else here is plain Rust the door calls into.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod door;
mod events;
mod terminal;

use std::sync::Arc;

use events::EventHub;

/// What the door hands to every call from the page.
pub struct App {
    events: EventHub,
}

fn main() {
    let app = Arc::new(App {
        events: EventHub::default(),
    });
    if let Err(e) = door::run(app) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
