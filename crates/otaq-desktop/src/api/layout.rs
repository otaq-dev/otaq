//! `layout.*`: the window's layout (SHL-01, T-0327): the size classes, the pane's widths and the main area's tabs. The
//! page asks at start and again on `settings_changed`, for the user's keys in the tabs' hint and the strip's tooltip
//! (KEY-08); the rest is fixed for the life of the app.

use otaq_view::view_models::layout;
use serde_json::Value;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, _params: Value) -> Result<Value, RequestError> {
    match method {
        "layout.view" => reply(&layout::view(&super::keymap::labels(app).await)),
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
        assert_eq!(
            view,
            serde_json::to_value(layout::view(&otaq_view::key_labels::KeyLabels::default())).unwrap()
        );
        assert_eq!(view["pane_defaults"]["w1440"], 400.0);
        assert_eq!(view["classes"][0]["class"], "w1280");
        assert!(request(&h.app, "layout.nothing", Value::Null).await.is_err());
        h.service.stop_all().await;
    }
}
