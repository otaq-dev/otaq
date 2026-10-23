//! `layout.*`: the window's layout (SHB-01, T-0327, T-0458): the size classes, the rail's and the Project agent
//! panel's widths. The page asks once at start; it is fixed for the life of the app.

use otaq_view::view_models::layout;
use serde_json::Value;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(_app: &App, method: &str, _params: Value) -> Result<Value, RequestError> {
    match method {
        "layout.view" => reply(&layout::view()),
        _ => Err(unknown(method)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::request;
    use crate::service::testing::Harness;

    #[tokio::test(flavor = "multi_thread")]
    async fn the_table_answers_layout_view_for_this_platform() {
        let h = Harness::new(None);
        let view = request(&h.app, "layout.view", Value::Null).await.unwrap();
        assert_eq!(view, serde_json::to_value(layout::view()).unwrap());
        assert_eq!(view["agent_panel"], 420.0);
        assert_eq!(view["classes"][0]["class"], "w1280");
        assert!(request(&h.app, "layout.nothing", Value::Null).await.is_err());
        h.service.stop_all().await;
    }
}
