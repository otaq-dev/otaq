//! The Crewboard desktop app on Tauri 2 (d0074, d0075): a window whose page is React with TypeScript, built with Vite
//! from `ui/`. Only [`door`] names Tauri (UIT-02); everything else here is plain Rust the door calls into.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod door;
mod events;
#[cfg(test)]
mod generated;
mod service;
#[cfg(test)]
mod stylesheet;
mod terminal;
mod window;

use std::sync::Arc;

use crewboard_core::client::CommandLauncher;
use crewboard_core::data_dir::DataDir;
use events::EventHub;
use service::{ServiceLink, Settings};

/// What the door hands to every call from the page.
pub struct App {
    events: Arc<EventHub>,
    service: ServiceLink,
    /// The rail's switcher, kept between requests so a project that newly needs you rings (T-0219).
    switcher: std::sync::Mutex<crewboard_view::view_models::switcher::SwitcherModel>,
    terminals: terminal::Terminals,
}

fn main() {
    // Before any other thread or the webview exists (Q39).
    let os = std::env::consts::OS;
    if let Some((name, value)) =
        window::webkit_env(os, std::env::var_os(window::WEBKIT_DMABUF).as_deref())
    {
        // SAFETY: the app has no other thread yet, so nothing reads the environment while it changes.
        unsafe { std::env::set_var(name, value) };
    }
    // One runtime for the whole app; the door hands it to Tauri, so its async calls run here too.
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("can't start the app's runtime: {e}");
            std::process::exit(1);
        }
    };
    let events = Arc::new(EventHub::default());
    // The `crewboard` command next to this program starts the service when none runs (d0055).
    let data = DataDir::from_env().map_err(|e| e.to_string());
    let placement = data.as_ref().ok().map(window::placement_file);
    let settings = data.and_then(|data| {
        let launcher = CommandLauncher::next_to_current_exe().map_err(|e| e.to_string())?;
        Ok(Settings::new(data, Arc::new(launcher)))
    });
    let service = {
        let _inside = runtime.enter();
        ServiceLink::start(settings, events.clone())
    };
    let app = Arc::new(App {
        events,
        service,
        switcher: Default::default(),
        terminals: terminal::Terminals::default(),
    });
    if let Err(e) = door::run(app, runtime.handle().clone(), placement) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
