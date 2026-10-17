//! The one Rust module that names Tauri (UIT-02), so a Tauri upgrade touches only this file and `ui/src/tauri.ts`.
//!
//! The surface is small and generic, so screens never edit it: one request call (a method name and JSON params in, a
//! JSON reply or a [`RequestError`] out, dispatched by [`crate::api`]), one event subscription ([`crate::events`]),
//! one output channel per terminal ([`crate::terminal`], answered with the attach's number), and the window: opening
//! it with its own title bar, the caption buttons, keeping its place ([`crate::window`]), the system folder picker and
//! folders dropped on the window (Add project). The WebView2 key switch ([`webview_keys_off`]) is set up here too.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
) -> Result<u64, String> {
    let sink = Box::new(move |feed| channel.send(feed).is_ok());
    crate::terminal::attach(&app, &session, sink).await
}

/// The system folder picker (Add project, T-0233): the folder chosen, or `None` when cancelled.
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle, title: String) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title(title)
        .pick_folder(move |folder| {
            let _ = tx.send(folder.and_then(|f| f.into_path().ok()));
        });
    rx.await
        .ok()
        .flatten()
        .map(|path| path.display().to_string())
}

// --- The window (T-0214) ---------------------------------------------------------------------------------------

/// The window's label, which `capabilities/default.json` names.
const MAIN_WINDOW: &str = "main";

#[tauri::command]
fn window(window: tauri::WebviewWindow, action: crate::window::Action) -> Result<(), String> {
    use crate::window::Action;
    match action {
        Action::Minimize => window.minimize(),
        Action::ToggleMaximize if window.is_maximized().unwrap_or(false) => window.unmaximize(),
        Action::ToggleMaximize => window.maximize(),
        Action::Close => window.close(),
        Action::Focus => window
            .unminimize()
            .and_then(|()| window.show())
            .and_then(|()| window.set_focus()),
    }
    .map_err(|e| e.to_string())
}

/// The window's place as last seen, saved when the app quits.
struct Placement {
    file: Option<PathBuf>,
    last: Mutex<Option<crate::window::Placement>>,
}

/// Opens the main window with its own title bar, where it was last time (d0074, UI-01).
fn open_window(app: &tauri::App, placement: &Placement) -> tauri::Result<()> {
    use crate::window::{MIN_SIZE, Rect};
    let saved = placement.file.as_deref().and_then(crate::window::load);
    let screens: Vec<Rect> = app
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let (position, size) = (
                m.position().to_logical(m.scale_factor()),
                m.size().to_logical(m.scale_factor()),
            );
            Rect {
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
            }
        })
        .collect();
    let opening = crate::window::opening(saved, &screens);
    *placement.last.lock().unwrap_or_else(|e| e.into_inner()) = saved;

    let builder = tauri::WebviewWindowBuilder::new(app, MAIN_WINDOW, tauri::WebviewUrl::default())
        .title(otaq_core::naming::PRODUCT_NAME)
        .inner_size(opening.width, opening.height)
        .min_inner_size(MIN_SIZE.0, MIN_SIZE.1)
        // The webview's own zoom keys stay off (UIT-06).
        .zoom_hotkeys_enabled(false)
        // The terminal's paste reads the clipboard (`navigator.clipboard.readText()`): without this, WebView2 asks
        // "tauri.localhost wants to see text and images copied to the clipboard" on the first paste and that paste
        // is lost; WebKitGTK refuses the read. WKWebView needs nothing.
        .enable_clipboard_access();
    let builder = match opening.position {
        Some((x, y)) => builder.position(x, y),
        None => builder.center(),
    };
    // macOS: the page reaches the window's top edge and the traffic lights sit inset in its header row. Elsewhere:
    // no system title bar; the page draws the caption buttons.
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(
            crate::window::TRAFFIC_LIGHTS.0,
            crate::window::TRAFFIC_LIGHTS.1,
        ));
    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);
    let window = builder.build()?;
    // After it is placed: maximized in the builder, it would keep the saved corner with the screen's size.
    if opening.maximized {
        window.maximize()?;
    }
    // Where it opened counts as its place until it moves.
    note_placement(&window.as_ref().window(), placement);
    Ok(())
}

/// Notes the window's place after each move or resize: its normal bounds stay as they were while it is maximized,
/// minimized or full screen.
fn note_placement(window: &tauri::Window, placement: &Placement) {
    let maximized = window.is_maximized().unwrap_or(false);
    let mut last = placement.last.lock().unwrap_or_else(|e| e.into_inner());
    if maximized
        || window.is_minimized().unwrap_or(false)
        || window.is_fullscreen().unwrap_or(false)
    {
        if let Some(last) = last.as_mut() {
            last.maximized = maximized;
        }
        return;
    }
    let (Ok(position), Ok(size), Ok(scale)) = (
        window.outer_position(),
        window.inner_size(),
        window.scale_factor(),
    ) else {
        return;
    };
    let (position, size) = (
        position.to_logical::<f64>(scale),
        size.to_logical::<f64>(scale),
    );
    *last = Some(crate::window::Placement {
        bounds: crate::window::Rect {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        },
        maximized: false,
    });
}

fn save_placement(placement: &Placement) {
    let last = *placement.last.lock().unwrap_or_else(|e| e.into_inner());
    if let (Some(file), Some(last)) = (&placement.file, last)
        && let Err(e) = crate::window::save(file, &last)
    {
        eprintln!(
            "couldn't keep the window's place in {}: {e}",
            file.display()
        );
    }
}

// --- Running ----------------------------------------------------------------------------------------------------

/// Opens the window and runs until the app quits. Tauri's async calls run on the app's own `runtime`; the window's
/// place is kept in `placement` (none: not kept).
pub fn run(
    app: Arc<App>,
    runtime: tokio::runtime::Handle,
    placement: Option<PathBuf>,
) -> Result<(), String> {
    tauri::async_runtime::set(runtime);
    let placement = Arc::new(Placement {
        file: placement,
        last: Mutex::new(None),
    });
    let (on_setup, on_window, on_exit) = (placement.clone(), placement.clone(), placement);
    let dropped_on = app.clone();
    let exiting = app.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app)
        .invoke_handler(tauri::generate_handler![
            request,
            subscribe_events,
            attach_terminal,
            window,
            pick_folder
        ])
        .setup(move |tauri_app| {
            open_window(tauri_app, &on_setup)?;
            webview_keys_off(tauri_app)?;
            Ok(())
        })
        .on_window_event(move |window, event| match event {
            tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) => {
                note_placement(window, &on_window)
            }
            tauri::WindowEvent::CloseRequested { .. } => save_placement(&on_window),
            // A folder dropped anywhere on the window opens Add project with it (*02 → No projects yet*).
            tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
                if let Some(path) = paths.first() {
                    crate::api::folder_dropped(&dropped_on, path);
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .map_err(|e| format!("the window couldn't open: {e}"))?
        .run(move |_, event| {
            if let tauri::RunEvent::Exit = event {
                save_placement(&on_exit);
                exiting.usage.flush_before_exit();
            }
        });
    Ok(())
}

/// The webview's own keys off (UIT-06), so the §21.3 map is the only one. On Windows, WebView2's browser accelerator
/// keys (Ctrl+F find, Ctrl+P print, F5 / Ctrl+R reload, zoom, Ctrl+Shift+I devtools, F7 caret browsing, Alt+← back…)
/// through `AreBrowserAcceleratorKeysEnabled = false`: Tauri doesn't expose this setting (wry has it, Tauri doesn't
/// pass it), so it goes through `webview2-com`; a Tauri upgrade means checking it again. WKWebView and WebKitGTK act on
/// none of these keys themselves; the zoom keys are off in the window builder ([`open_window`]) and the page swallows
/// the rest (`ui/src/app/keys.ts`).
fn webview_keys_off(tauri_app: &tauri::App) -> Result<(), tauri::Error> {
    #[cfg(windows)]
    {
        use tauri::Manager;
        if let Some(window) = tauri_app.get_webview_window(MAIN_WINDOW) {
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
