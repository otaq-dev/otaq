//! `theme.*`: the theme the page draws with (UIT-07, d0070), and the theme picker's cards and pick (UI-11). The
//! page asks again on every `settings_changed` event, so a theme picked in Settings or in another window recolors
//! this one at once.

use crewboard_core::board::settings::{GlobalSettings, Theme, ThemeSetting};
use crewboard_core::protocol::{Reply, Request};
use crewboard_view::theme::{Appearance, ThemeId};
use crewboard_view::view_models::theme_picker;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "theme.active" => active(app, params).await,
        "theme.picker" => picker(app, params).await,
        "theme.pick" => pick(app, params).await,
        _ => Err(unknown(method)),
    }
}

/// What `theme.active` and `theme.picker` take: the OS appearance now, used only until a theme is stored.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
pub struct ThemeParams {
    pub os_dark: bool,
}

/// What `theme.pick` takes: the picked theme's key (`tokyo-night`). The answer is the [`ActiveTheme`].
#[derive(Debug, Clone, Deserialize, TS)]
pub struct ThemePick {
    pub key: String,
}

/// The theme in use: its key (the page's `data-theme`, e.g. `tokyo-night`), its name, and whether it is dark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ActiveTheme {
    pub key: String,
    pub name: String,
    pub dark: bool,
}

impl ActiveTheme {
    fn of(id: ThemeId) -> Self {
        Self {
            key: id.stored().as_str().to_owned(),
            name: id.name(),
            dark: id.appearance() == Appearance::Dark,
        }
    }
}

fn params<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

async fn settings(app: &App) -> Result<GlobalSettings, RequestError> {
    match app.service.send(Request::SettingsGet).await? {
        Reply::GlobalSettings(reply) => Ok(reply.settings),
        other => Err(RequestError::plain(format!(
            "unexpected answer to settings.get: {other:?}"
        ))),
    }
}

/// The stored theme. Until one is stored (a fresh data folder, or System mode from before d0070), the one matching
/// the OS now, which is then stored: the first launch matches the OS once, later OS changes don't switch it.
async fn resolve(app: &App, params: ThemeParams) -> Result<ThemeId, RequestError> {
    let mut settings = settings(app).await?;
    let os = if params.os_dark {
        Appearance::Dark
    } else {
        Appearance::Light
    };
    let id = ThemeId::from_setting(settings.theme, os);
    if !matches!(settings.theme, ThemeSetting::Picked(_)) {
        settings.theme = ThemeSetting::Picked(id.stored());
        app.service.send(Request::SettingsSet(settings)).await?;
    }
    Ok(id)
}

pub async fn active(app: &App, params: Value) -> Result<Value, RequestError> {
    reply(&ActiveTheme::of(resolve(app, self::params(params)?).await?))
}

/// The picker's Dark and Light sections with the theme in use checked (`ThemePickerSection[]`).
pub async fn picker(app: &App, params: Value) -> Result<Value, RequestError> {
    reply(&theme_picker::sections(resolve(app, self::params(params)?).await?))
}

/// A click on a theme card: stores it at once (UI-11). The service then announces `settings_changed`, on which
/// every window recolors.
pub async fn pick(app: &App, params: Value) -> Result<Value, RequestError> {
    let ThemePick { key } = self::params(params)?;
    let theme: Theme = key
        .parse()
        .map_err(|_| RequestError::plain(format!("unknown theme: {key}")))?;
    let mut settings = settings(app).await?;
    settings.theme = ThemeSetting::Picked(theme);
    app.service.send(Request::SettingsSet(settings)).await?;
    reply(&ActiveTheme::of(ThemeId::from_stored(theme)))
}

#[cfg(test)]
mod tests {
    use crewboard_core::naming;
    use serde_json::json;

    use super::*;
    use crate::api::request;
    use crate::service::testing::{Harness, connected};

    async fn stored(h: &Harness) -> ThemeSetting {
        match h.app.service.send(Request::SettingsGet).await.unwrap() {
            Reply::GlobalSettings(reply) => reply.settings.theme,
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_fresh_data_folder_matches_the_os_once_then_keeps_its_theme() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        assert_eq!(stored(&h).await, ThemeSetting::default());

        let first = request(&h.app, "theme.active", json!({ "os_dark": true }))
            .await
            .unwrap();
        assert_eq!(first["key"], naming::THEME_DARK);
        assert_eq!(first["dark"], true);
        assert_eq!(stored(&h).await, ThemeSetting::Picked(Theme::CrewboardDark));
        // The OS turned light: the stored theme stays.
        let later = request(&h.app, "theme.active", json!({ "os_dark": false }))
            .await
            .unwrap();
        assert_eq!(later, first);
        h.service.stop_all().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_picked_theme_is_used_and_a_change_is_announced() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let Reply::GlobalSettings(reply) = h.app.service.send(Request::SettingsGet).await.unwrap()
        else {
            panic!()
        };
        let mut settings = reply.settings;
        settings.theme = ThemeSetting::Picked(Theme::TokyoNight);
        h.app
            .service
            .send(Request::SettingsSet(settings))
            .await
            .unwrap();
        // The page refetches on this event (UIT-07).
        h.wait_for_event("settings_changed", |e| {
            e["kind"] == "service" && e["name"] == "settings_changed"
        })
        .await;

        let theme = request(&h.app, "theme.active", json!({ "os_dark": false }))
            .await
            .unwrap();
        assert_eq!(
            theme,
            json!({ "key": "tokyo-night", "name": "Tokyo Night", "dark": true })
        );
        let wrong = request(&h.app, "theme.active", Value::Null).await;
        assert!(wrong.is_err());
        h.service.stop_all().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_click_in_the_picker_stores_the_theme_and_checks_its_card() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;

        let before = request(&h.app, "theme.picker", json!({ "os_dark": false }))
            .await
            .unwrap();
        assert_eq!(before[1]["rows"][0][0]["id"], naming::THEME_LIGHT);
        assert_eq!(before[1]["rows"][0][0]["selected"], true);

        let picked = request(&h.app, "theme.pick", json!({ "key": "dracula" }))
            .await
            .unwrap();
        assert_eq!(
            picked,
            json!({ "key": "dracula", "name": "Dracula", "dark": true })
        );
        assert_eq!(stored(&h).await, ThemeSetting::Picked(Theme::Dracula));
        h.wait_for_event("settings_changed", |e| {
            e["kind"] == "service" && e["name"] == "settings_changed"
        })
        .await;
        let after = request(&h.app, "theme.picker", json!({ "os_dark": false }))
            .await
            .unwrap();
        assert_eq!(after[0]["rows"][1][0]["selected"], true);
        assert_eq!(after[1]["rows"][0][0]["selected"], false);

        let unknown = request(&h.app, "theme.pick", json!({ "key": "neon" })).await;
        assert_eq!(
            unknown,
            Err(RequestError::plain("unknown theme: neon"))
        );
        assert_eq!(stored(&h).await, ThemeSetting::Picked(Theme::Dracula));
        h.service.stop_all().await;
    }
}
