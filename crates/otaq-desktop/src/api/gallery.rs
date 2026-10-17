//! `gallery.*`: the design gallery (T-0217), a developer-only page for comparing the foundations and the components
//! with *crewboard.pen → 01* by hand. It opens over the app when the app starts with `OTAQ_GALLERY=1`, as the
//! GPUI app's gallery did.

use std::ffi::OsStr;

use otaq_core::naming;
use otaq_view::view_models::{
    board_page, card_page, cleanup, decision_change, decision_item, gallery, inbox_page, lead_status, lead_tab,
    onboarding_page, settings_page, tool_avatar,
};
use serde_json::Value;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(_app: &App, method: &str, _params: Value) -> Result<Value, RequestError> {
    match method {
        "gallery.wanted" => reply(&wanted(std::env::var_os(env_var()).as_deref())),
        "gallery.view" => reply(&gallery::view()),
        // The card grids (T-0218): every Ticket Card and Epic Card variant from the fixtures.
        "gallery.cards" => reply(&card_page::gallery()),
        // Every Decision Item variant and the two review lists (T-0287).
        "gallery.decisions" => reply(&decision_item::gallery()),
        // The Change… and Add a fix child dialogs (T-0288).
        "gallery.decision_change" => reply(&decision_change::gallery()),
        // The Lead Status, the track label and the epic card with a lead (T-0289).
        "gallery.leads" => reply(&lead_status::gallery()),
        // The epic panel's Lead tab in each state (T-0290).
        "gallery.lead_tabs" => reply(&lead_tab::gallery()),
        // The epic lead's inbox items and the suggestion's line (T-0292).
        "gallery.inbox_lead" => reply(&inbox_page::lead_gallery()),
        // Settings' Environment variables row in each state (T-0300).
        "gallery.env_vars" => reply(&settings_page::env_vars_gallery()),
        // Settings' warm build switch in each state (T-0301).
        "gallery.warm_switch" => reply(&settings_page::warm_switch_gallery()),
        // Cleanup's warm builds section in each state (T-0302).
        "gallery.cleanup_warm" => reply(&cleanup::warm_gallery()),
        // The add-project form's folder trust line and failed state, the note and the failed warning (T-0306).
        "gallery.trust" => reply(&onboarding_page::trust_gallery()),
        // The four tool avatars and the line saying they are hidden while one tool is connected (T-0321).
        "gallery.tool_avatars" => reply(&tool_avatar::gallery()),
        // The Review column's states, the drop verdicts, the empty board and the agent chip (T-0324).
        "gallery.board_review" => reply(&board_page::gallery()),
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
        let leads = crate::api::request(&h.app, "gallery.leads", Value::Null)
            .await
            .unwrap();
        assert_eq!(leads["statuses"][0]["view"]["full"], "Wishlist lead · idle");
        assert!(cards["epics"].as_array().is_some_and(|e| !e.is_empty()));
        let tabs = crate::api::request(&h.app, "gallery.lead_tabs", Value::Null)
            .await
            .unwrap();
        assert_eq!(tabs["tabs"][0]["view"]["status"]["full"], "Wishlist lead · idle");
        let env = crate::api::request(&h.app, "gallery.env_vars", Value::Null)
            .await
            .unwrap();
        assert_eq!(env["cells"][0]["row"]["control"]["kind"], "env_vars");
        let warm = crate::api::request(&h.app, "gallery.cleanup_warm", Value::Null)
            .await
            .unwrap();
        assert_eq!(warm["cells"][0]["page"]["warm"]["total"], "41.6 GB on disk");
        let switch = crate::api::request(&h.app, "gallery.warm_switch", Value::Null)
            .await
            .unwrap();
        assert_eq!(switch["cells"][0]["row"]["control"]["kind"], "warm_build");
        let trust = crate::api::request(&h.app, "gallery.trust", Value::Null)
            .await
            .unwrap();
        assert_eq!(trust["banners"][0]["banners"][0]["kind"], "trust_note");
        assert!(trust["forms"][2]["form"]["trust_error"].is_string());
        let avatars = crate::api::request(&h.app, "gallery.tool_avatars", Value::Null)
            .await
            .unwrap();
        assert_eq!(avatars["avatars"][1]["background"], "tool-cx");
        assert_eq!(avatars["shown"], false);
        let board = crate::api::request(&h.app, "gallery.board_review", Value::Null)
            .await
            .unwrap();
        assert_eq!(board["review"][0]["column"]["sections"][1]["id"], "approved");
        h.service.stop_all().await;
    }
}
