//! The Otaq desktop app on Tauri 2 (d0074, d0075): a window whose page is React with TypeScript, built with Vite
//! from `ui/`. Only [`door`] names Tauri (UIT-02); everything else here is plain Rust the door calls into.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod about;
mod api;
mod door;
mod events;
#[cfg(test)]
mod generated;
mod notices;
mod notify_check;
mod service;
#[cfg(test)]
mod stylesheet;
mod terminal;
mod usage;
mod window;

use std::sync::Arc;

use events::EventHub;
use otaq_core::client::CommandLauncher;
use otaq_core::data_dir::DataDir;
use service::{ServiceLink, Settings};

/// What the door hands to every call from the page.
pub struct App {
    events: Arc<EventHub>,
    service: ServiceLink,
    /// The rail's switcher, kept between requests so a project that newly needs you rings (T-0219).
    switcher: std::sync::Mutex<otaq_view::view_models::switcher::SwitcherModel>,
    /// Each project's last-used tab of the Terminals drawer, for ⇧⌘T (T-0242, T-0331).
    drawer_tabs: std::sync::Mutex<std::collections::BTreeMap<String, String>>,
    /// The Needs-you inbox, kept between requests so resolved rows fade out (T-0230).
    inbox: std::sync::Mutex<otaq_view::view_models::inbox::InboxModel>,
    /// The ⌘J run through the inbox, while one goes (T-0230).
    inbox_run: std::sync::Mutex<Option<otaq_view::view_models::inbox_run::InboxRun>>,
    /// The Diff tabs' viewed ticks, folded summaries and Unified / Side-by-side, while the app runs (T-0225).
    review: std::sync::Mutex<otaq_view::view_models::diff::ReviewMemory>,
    terminals: terminal::Terminals,
    notices: Arc<notices::Notices>,
    /// The open Open PR / Push to PR dialogs by ticket, kept between requests (T-0227).
    pr_dialogs: std::sync::Mutex<
        std::collections::HashMap<String, otaq_view::view_models::pr_dialog::PrDialog>,
    >,
    /// The Cleanup overlay's last scan and what is ticked, kept between requests (T-0235).
    cleanup: std::sync::Mutex<otaq_view::view_models::cleanup::CleanupModel>,
    /// The open run-command dialogs by project, kept between requests (T-0232).
    run_commands: std::sync::Mutex<
        std::collections::HashMap<String, otaq_view::view_models::run_command::RunCommandModel>,
    >,
    /// Anonymous usage data, sent only after the user said yes (d0085).
    usage: usage::Usage,
    /// Settings → Agents while it is open: the entries a tool connecting added, the lists that couldn't load (T-0361).
    agents_visit: std::sync::Mutex<otaq_view::view_models::named_agents::Visit>,
}

fn main() {
    // The bundle check (T-0370): no window, no service.
    if notify_check::requested() {
        std::process::exit(notify_check::run());
    }
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
    // The `otaq` command next to this program starts the service when none runs (d0055).
    let data = DataDir::from_env().map_err(|e| e.to_string());
    let placement = data.as_ref().ok().map(window::placement_file);
    let settings = data.and_then(|data| {
        let launcher = CommandLauncher::next_to_current_exe().map_err(|e| e.to_string())?;
        Ok(Settings::new(data, Arc::new(launcher)))
    });
    let (service, notices) = {
        let _inside = runtime.enter();
        let service = ServiceLink::start(settings, events.clone());
        let notices = notices::Notices::start(&service, events.clone());
        (service, notices)
    };
    let app = Arc::new(App {
        events,
        service,
        switcher: Default::default(),
        drawer_tabs: Default::default(),
        inbox: Default::default(),
        inbox_run: Default::default(),
        review: Default::default(),
        terminals: terminal::Terminals::default(),
        notices,
        pr_dialogs: Default::default(),
        cleanup: Default::default(),
        run_commands: Default::default(),
        agents_visit: Default::default(),
        usage: usage::Usage::new(runtime.handle().clone()),
    });
    runtime.spawn(api::usage_loop(app.clone()));
    if let Err(e) = door::run(app, runtime.handle().clone(), placement) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
