//! The request table: the door's one request call lands here and is dispatched by method name. Each screen adds its
//! own `api/<area>.rs` and one line below: `"<area>.<name>" => <area>::<name>(app, params).await,`.

mod service;
mod theme;

use serde::Serialize;
use serde_json::Value;

use crate::App;
pub use crate::service::RequestError;
#[cfg(test)]
pub use theme::{ActiveTheme, ThemeParams};

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "service.connection" => service::connection(app, params).await,
        "service.status" => service::status(app, params).await,
        "service.restart" => service::restart(app, params).await,
        "theme.active" => theme::active(app, params).await,
        _ => Err(RequestError::plain(format!("unknown request: {method}"))),
    }
}

/// A handler's answer as the page receives it.
pub fn reply(value: &impl Serialize) -> Result<Value, RequestError> {
    serde_json::to_value(value).map_err(|e| RequestError::plain(e.to_string()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::service::testing::{Harness, connected};

    #[tokio::test(flavor = "multi_thread")]
    async fn the_table_dispatches_by_method_name_to_the_real_service() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;

        let connection = request(&h.app, "service.connection", Value::Null)
            .await
            .unwrap();
        assert_eq!(connection["state"], "connected");
        assert_eq!(connection["compatibility"], "same");

        let status = request(&h.app, "service.status", Value::Null)
            .await
            .unwrap();
        assert_eq!(status["pid"], std::process::id());
        assert_eq!(status["build_id"], connection["build_id"]);

        let unknown = request(&h.app, "board.nothing", json!({})).await;
        assert_eq!(
            unknown,
            Err(RequestError::plain("unknown request: board.nothing"))
        );
        h.service.stop_all().await;
    }
}
