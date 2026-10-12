//! The request table: the door's one request call lands here and is dispatched by method name. Each screen adds its
//! own `api/<area>.rs` and one line below (T-0211, T-0212).

use serde_json::Value;

use crate::App;

pub async fn request(_app: &App, method: &str, _params: Value) -> Result<Value, String> {
    // T-0211 starts the table: `"<area>.<name>" => <area>::<name>(app, params).await,`.
    Err(format!("unknown request: {method}"))
}
