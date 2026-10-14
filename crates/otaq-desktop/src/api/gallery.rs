//! `gallery.*`: the design gallery (T-0217), a developer-only page for comparing the foundations and the components
//! with *crewboard.pen → 01* by hand. It opens over the app when the app starts with `OTAQ_GALLERY=1`, as the
//! GPUI app's gallery did.

use std::ffi::OsStr;

use otaq_core::naming;
use otaq_view::view_models::{card_page, gallery};
use serde_json::Value;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(_app: &App, method: &str, _params: Value) -> Result<Value, RequestError> {
    match method {
        "gallery.wanted" => reply(&wanted(std::env::var_os(env_var()).as_deref())),
        "gallery.view" => reply(&gallery::view()),
        // The card grids (T-0218): every Ticket Card and Epic Card variant from the fixtures.
        "gallery.cards" => reply(&card_page::gallery()),
        _ => Err(unknown(method)),
    }
}

/// `OTAQ_GALLERY`.
fn env_var() -> String {
    naming::env_var("GALLERY")
}

/// Whether the variable asks for the gallery: set, and neither empty nor `0`.
fn wanted(value: Option<&OsStr>) -> bool {
    value.is_some_and(|v| !v.is_empty() && v != "0")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_variable_opens_the_gallery_unless_empty_or_zero() {
        assert_eq!(env_var(), format!("{}GALLERY", naming::env_var("")));
        assert!(wanted(Some(OsStr::new("1"))));
        assert!(wanted(Some(OsStr::new("yes"))));
        for off in [None, Some(OsStr::new("")), Some(OsStr::new("0"))] {
            assert!(!wanted(off), "{off:?}");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_view_is_served_by_the_request_table() {
        let h = crate::service::testing::Harness::new(None);
        let view = crate::api::request(&h.app, "gallery.view", Value::Null)
            .await
            .unwrap();
        assert_eq!(view["legend"].as_array().map(Vec::len), Some(4));
        assert_eq!(view["keyboard_map"][0]["title"], "Attention");
        assert_eq!(view["priority"]["chips"][0]["short"], "P1");
        let cards = crate::api::request(&h.app, "gallery.cards", Value::Null)
            .await
            .unwrap();
        assert_eq!(cards["variants"][0]["card"]["key"], "SHOP-31");
        assert!(cards["epics"].as_array().is_some_and(|e| !e.is_empty()));
        h.service.stop_all().await;
    }
}
