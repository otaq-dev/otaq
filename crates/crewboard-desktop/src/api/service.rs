//! `service.*`: the link to the background service, for the connection line and the service banners (T-0231).

use crewboard_core::protocol::{Reply, Request};
use serde_json::Value;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "service.connection" => connection(app, params).await,
        "service.status" => status(app, params).await,
        "service.restart" => restart(app, params).await,
        _ => Err(unknown(method)),
    }
}

/// The link's state now; changes arrive as `connection` events.
pub async fn connection(app: &App, _params: Value) -> Result<Value, RequestError> {
    reply(&app.service.connection())
}

/// The service's `service.status` (PID, build, agent counts).
pub async fn status(app: &App, _params: Value) -> Result<Value, RequestError> {
    match app.service.send(Request::ServiceStatus).await? {
        Reply::ServiceStatus(status) => reply(&status),
        other => Err(RequestError::plain(format!(
            "unexpected answer to service.status: {other:?}"
        ))),
    }
}

/// Restarts the service with this build, or starts it. Only sent on the user's click (d0022).
pub async fn restart(app: &App, _params: Value) -> Result<Value, RequestError> {
    app.service.restart().await.map_err(RequestError::plain)?;
    Ok(Value::Null)
}
