//! `window.*`: what the page needs to draw the window's own title bar (UI-01). The window operations themselves
//! (caption buttons, dragging) are Tauri calls and go through the door.

use serde_json::Value;

use super::{RequestError, reply, unknown};
use crate::App;
use crate::window::Chrome;

pub async fn request(_app: &App, method: &str, _params: Value) -> Result<Value, RequestError> {
    match method {
        "window.chrome" => reply(&Chrome::current()),
        _ => Err(unknown(method)),
    }
}
