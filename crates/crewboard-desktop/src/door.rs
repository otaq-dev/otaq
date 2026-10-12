//! The one Rust module that names Tauri (UIT-02), so a Tauri upgrade touches only this file and `ui/src/tauri.ts`.
//!
//! The surface is small and generic, so screens never edit it: one request call (a method name and JSON params in, a
//! JSON reply or a [`RequestError`] out, dispatched by [`crate::api`]), one event subscription ([`crate::events`])
//! and one output channel per terminal session ([`crate::terminal`]). The window operations join it with T-0214; the
//! WebView2 key switch ([`webview_keys_off`]) is set up here too.

use std::sync::Arc;

use serde_json::Value;
use tauri::State;
use tauri::ipc::Channel;

use crate::App;
use crate::api::RequestError;

type AppState<'a> = State<'a, Arc<App>>;

#[tauri::command]
async fn request(app: AppState<'_>, method: String, params: Value) -> Result<Value, RequestError> {
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

/// Opens the window and runs until the app quits. Tauri's async calls run on the app's own `runtime`.
pub fn run(app: Arc<App>, runtime: tokio::runtime::Handle) -> Result<(), String> {
    tauri::async_runtime::set(runtime);
    tauri::Builder::default()
        .manage(app)
        .invoke_handler(tauri::generate_handler![
            request,
            subscribe_events,
            attach_terminal
        ])
        .setup(|tauri_app| {
            webview_keys_off(tauri_app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .map_err(|e| format!("the window couldn't open: {e}"))
}

/// The webview's own keys off (UIT-06), so the §21.3 map is the only one. On Windows, WebView2's browser accelerator
/// keys (Ctrl+F find, Ctrl+P print, F5 / Ctrl+R reload, zoom, Ctrl+Shift+I devtools, F7 caret browsing, Alt+← back…)
/// through `AreBrowserAcceleratorKeysEnabled = false`: Tauri doesn't expose this setting (wry has it, Tauri doesn't
/// pass it), so it goes through `webview2-com`; a Tauri upgrade means checking it again. WKWebView and WebKitGTK act on
/// none of these keys themselves; the zoom keys are off in `tauri.conf.json` and the page swallows the rest
/// (`ui/src/app/keys.ts`).
fn webview_keys_off(tauri_app: &tauri::App) -> Result<(), tauri::Error> {
    #[cfg(windows)]
    {
        use tauri::Manager;
        if let Some(window) = tauri_app.get_webview_window("main") {
            window.with_webview(|webview| unsafe {
                use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
                use windows_core::Interface;
                if let Ok(settings) = webview
                    .controller()
                    .CoreWebView2()
                    .and_then(|w| w.Settings())
                    .and_then(|s| s.cast::<ICoreWebView2Settings3>())
                {
                    let _ = settings.SetAreBrowserAcceleratorKeysEnabled(false);
                }
            })?;
        }
    }
    #[cfg(not(windows))]
    let _ = tauri_app;
    Ok(())
}
