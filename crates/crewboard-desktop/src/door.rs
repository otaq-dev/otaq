//! The one Rust module that names Tauri (UIT-02), so a Tauri upgrade touches only this file and `ui/src/tauri.ts`.
//!
//! The surface is small and generic, so screens never edit it: one request call (a method name and JSON params in, a
//! JSON reply out, dispatched by [`crate::api`]), one event subscription ([`crate::events`]) and one output channel
//! per terminal session ([`crate::terminal`]). The window operations join it with T-0214.

use std::sync::Arc;

use serde_json::Value;
use tauri::State;
use tauri::ipc::Channel;

use crate::App;

type AppState<'a> = State<'a, Arc<App>>;

#[tauri::command]
async fn request(app: AppState<'_>, method: String, params: Value) -> Result<Value, String> {
    crate::api::request(&app, &method, params).await
}

#[tauri::command]
fn subscribe_events(app: AppState<'_>, channel: Channel<Value>) {
    app.events
        .subscribe(Box::new(move |event| channel.send(event).is_ok()));
}

#[tauri::command]
async fn attach_terminal(
    app: AppState<'_>,
    session: String,
    channel: Channel<Value>,
) -> Result<(), String> {
    let sink = Box::new(move |chunk| channel.send(chunk).is_ok());
    crate::terminal::attach(&app, &session, sink).await
}

/// Opens the window and runs until the app quits.
pub fn run(app: Arc<App>) -> Result<(), String> {
    tauri::Builder::default()
        .manage(app)
        .invoke_handler(tauri::generate_handler![
            request,
            subscribe_events,
            attach_terminal
        ])
        .run(tauri::generate_context!())
        .map_err(|e| format!("the window couldn't open: {e}"))
}
