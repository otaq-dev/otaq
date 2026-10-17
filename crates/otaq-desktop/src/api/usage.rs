//! `usage.*`: the "Help improve Otaq" dialog (d0085). `usage.dialog` says whether it shows (the user hasn't answered
//! yet) with its words; `usage.choose` stores the answer. Settings → Data's switch edits the same setting through
//! `settings.edit`. After either, [`sync`] makes the sender follow the stored answer.

use otaq_core::board::settings::{GlobalSettings, UsageData};
use otaq_core::protocol::{Reply, Request};
use otaq_view::view_models::usage_data::{self, Dialog};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "usage.dialog" => reply(&dialog(app).await?),
        "usage.choose" => {
            let p: UsageChoice =
                serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))?;
            choose(app, p.on).await?;
            Ok(Value::Null)
        }
        _ => Err(unknown(method)),
    }
}

/// The answer to `usage.dialog`: whether the dialog opens, and its words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "UsageDataAsk")]
pub struct Ask {
    pub show: bool,
    pub dialog: Dialog,
}

/// What `usage.choose` takes: the button pressed (Esc counts as No thanks).
#[derive(Debug, Clone, Copy, Deserialize, TS)]
pub struct UsageChoice {
    pub on: bool,
}

async fn settings(app: &App) -> Result<GlobalSettings, RequestError> {
    match app.service.send(Request::SettingsGet).await? {
        Reply::GlobalSettings(reply) => Ok(reply.settings),
        other => Err(RequestError::plain(format!(
            "unexpected answer to settings.get: {other:?}"
        ))),
    }
}

async fn dialog(app: &App) -> Result<Ask, RequestError> {
    Ok(Ask {
        show: settings(app).await?.usage_data == UsageData::NotAsked,
        dialog: usage_data::dialog(),
    })
}

async fn choose(app: &App, on: bool) -> Result<(), RequestError> {
    let mut settings = settings(app).await?;
    settings.usage_data = if on { UsageData::On } else { UsageData::Off };
    app.service.send(Request::SettingsSet(settings)).await?;
    Ok(())
}

/// The sender follows the stored answer: on only after an explicit yes.
pub async fn sync(app: &App) -> Result<(), RequestError> {
    app.usage.set_on(settings(app).await?.usage_data.sends());
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::api::request;
    use crate::service::testing::{Harness, connected};

    #[tokio::test(flavor = "multi_thread")]
    async fn the_dialog_shows_until_answered_and_the_switch_follows() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;

        let ask = request(&h.app, "usage.dialog", Value::Null).await.unwrap();
        assert_eq!(ask["show"], true, "not asked yet");
        assert_eq!(ask["dialog"]["accept_label"], "Send anonymous data");
        assert!(!h.app.usage.is_on());

        request(&h.app, "usage.choose", json!({ "on": false })).await.unwrap();
        let ask = request(&h.app, "usage.dialog", Value::Null).await.unwrap();
        assert_eq!(ask["show"], false, "asked once");
        assert!(!h.app.usage.is_on(), "No thanks sends nothing");

        request(&h.app, "usage.choose", json!({ "on": true })).await.unwrap();
        assert!(h.app.usage.is_on());

        // Settings → Data's switch turns it off again.
        request(
            &h.app,
            "settings.edit",
            json!({ "field": "usage_data", "value": false }),
        )
        .await
        .unwrap();
        assert!(!h.app.usage.is_on());
        h.service.stop_all().await;
    }
}
