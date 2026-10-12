//! `notices.*`: the banner area and the toasts (T-0231, [`crate::notices`]), what their buttons do on the user's
//! click, and what a notification click opens.

use crewboard_core::protocol::{Reply, Request, ResumeAllParams};
use crewboard_view::view_models::banners::{PERMISSION_DOCS_URL, Situation, SituationKind};
use crewboard_view::view_models::limit_banner::{RESUME_FAILED, resumed};
use crewboard_view::view_models::restart::RestartDialog;
use crewboard_view::view_models::toasts::{NewToast, ToastLevel};
use serde::Deserialize;
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;
use crate::notices::OpenTarget;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "notices.view" => reply(&app.notices.view()),
        "notices.dismiss_banner" => dismiss_banner(app, parse(params)?).await,
        "notices.toast" => reply(&app.notices.toast(parse(params)?)),
        "notices.dismiss_toast" => {
            app.notices.dismiss_toast(parse::<ToastId>(params)?.id);
            Ok(Value::Null)
        }
        "notices.hold_toast" => {
            let hold: HoldToast = parse(params)?;
            app.notices.hold_toast(hold.id, hold.held);
            Ok(Value::Null)
        }
        "notices.restart_dialog" => {
            let (working, waiting) = app.notices.agent_counts();
            reply(&RestartDialog { working, waiting }.view())
        }
        "notices.restart" => restart(app).await,
        "notices.resume_after_limit" => resume_after_limit(app).await,
        "notices.resume_all" => resume_all(app).await,
        "notices.open_docs" => {
            open::that_detached(PERMISSION_DOCS_URL).map_err(|e| {
                RequestError::plain(format!("couldn't open {PERMISSION_DOCS_URL}: {e}"))
            })?;
            Ok(Value::Null)
        }
        "notices.permission_warning" => {
            app.notices.raise(Situation::PermissionWarning);
            Ok(Value::Null)
        }
        "notices.open_at_start" => reply(&app.notices.take_open_at_start()),
        "notices.open_target" => open_target(parse(params)?),
        _ => Err(unknown(method)),
    }
}

fn parse<T: for<'de> Deserialize<'de>>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

/// `{"kind": "permission_warning"}`: the banner the user closed.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "BannerDismiss")]
pub struct DismissBanner {
    pub kind: SituationKind,
}

/// `{"id": 3}`: a toast.
#[derive(Debug, Clone, Deserialize, TS)]
pub struct ToastId {
    pub id: u64,
}

/// `{"id": 3, "held": true}`: the pointer came onto a toast (or left it).
#[derive(Debug, Clone, Deserialize, TS)]
pub struct HoldToast {
    pub id: u64,
    pub held: bool,
}

/// `{"key": "SHOP-22"}` from an `open_from_notification` event; no key for a grouped notification.
#[derive(Debug, Clone, Deserialize, TS)]
pub struct NotificationKey {
    #[serde(default)]
    pub key: Option<String>,
}

/// Closing the permission-mode warning is remembered in the app's database (SET-01).
async fn dismiss_banner(app: &App, params: DismissBanner) -> Result<Value, RequestError> {
    app.notices.dismiss_banner(params.kind);
    if params.kind == SituationKind::PermissionWarning {
        app.service.send(Request::DismissPermissionWarning).await?;
    }
    Ok(Value::Null)
}

/// Restarts the service with this build, or starts it when stopped. Sent only after the user's click (confirmed
/// first when agents would be interrupted, d0022); the outcome shows as a toast or the summary banner.
async fn restart(app: &App) -> Result<Value, RequestError> {
    let before = app.notices.begin_restart();
    let outcome = app.service.restart().await;
    let now = app.service.states().borrow().clone();
    app.notices.end_restart(outcome, before, &now);
    Ok(Value::Null)
}

/// Resume all after the usage limit (USG-06): the user's click tells every waiting agent to continue.
async fn resume_after_limit(app: &App) -> Result<Value, RequestError> {
    let toast = match app.service.send(Request::UsageResumeAll).await {
        Ok(Reply::Resumed(keys)) => {
            let (title, text) = resumed(keys.len());
            NewToast::new(ToastLevel::Success, title, Some(text))
        }
        Ok(_) => return Ok(Value::Null),
        Err(e) => NewToast::new(ToastLevel::Error, RESUME_FAILED, Some(e.message)),
    };
    app.notices.toast(toast);
    if let Ok(Reply::Usage(summary)) = app
        .service
        .send(Request::UsageSummary(Default::default()))
        .await
    {
        app.notices.on_usage(summary.plan, summary.limit_waiting);
    }
    Ok(Value::Null)
}

/// The user confirmed Resume all: every interrupted session continues, and the summary banner goes.
async fn resume_all(app: &App) -> Result<Value, RequestError> {
    app.notices.clear(SituationKind::SessionsInterrupted);
    if let Err(e) = app
        .service
        .send(Request::TicketsResumeAll(ResumeAllParams::default()))
        .await
    {
        app.notices.toast(NewToast::new(
            ToastLevel::Error,
            RESUME_FAILED,
            Some(e.message),
        ));
    }
    Ok(Value::Null)
}

fn open_target(params: NotificationKey) -> Result<Value, RequestError> {
    let target = match params.key {
        None => OpenTarget::inbox(),
        Some(key) => key
            .parse()
            .map(|key| OpenTarget::ticket(&key))
            .map_err(|_| RequestError::plain(format!("not a ticket key: {key}")))?,
    };
    reply(&target)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::api::request;
    use crate::service::testing::{Harness, connected};

    #[tokio::test(flavor = "multi_thread")]
    async fn the_banner_area_and_the_toasts_through_the_table() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;

        let view = request(&h.app, "notices.view", Value::Null).await.unwrap();
        assert_eq!(view["banners"], json!([]));
        assert_eq!(view["only_restart_allowed"], false);

        let id = request(
            &h.app,
            "notices.toast",
            json!({"level": "error", "title": "gh is not logged in",
                   "link": {"kind": "copy", "label": "Copy command", "text": "gh auth login"}}),
        )
        .await
        .unwrap();
        h.wait_for_event("the toast", |e| {
            e["kind"] == "notices"
                && e["notices"]["toasts"]
                    .as_array()
                    .is_some_and(|t| t.iter().any(|t| t["title"] == "gh is not logged in"))
        })
        .await;
        request(&h.app, "notices.hold_toast", json!({"id": id, "held": true}))
            .await
            .unwrap();
        request(&h.app, "notices.dismiss_toast", json!({"id": id}))
            .await
            .unwrap();
        let view = request(&h.app, "notices.view", Value::Null).await.unwrap();
        assert!(
            view["toasts"]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t["id"] != id)
        );

        // The permission-mode warning: raised after the first run, closed for good.
        request(&h.app, "notices.permission_warning", Value::Null)
            .await
            .unwrap();
        let view = request(&h.app, "notices.view", Value::Null).await.unwrap();
        assert_eq!(view["banners"][0]["kind"], "permission_warning");
        assert_eq!(view["banners"][0]["buttons"][0]["label"], "Read the docs");
        request(
            &h.app,
            "notices.dismiss_banner",
            json!({"kind": "permission_warning"}),
        )
        .await
        .unwrap();
        let view = request(&h.app, "notices.view", Value::Null).await.unwrap();
        assert_eq!(view["banners"], json!([]));
        match h.app.service.send(Request::OnboardingState).await.unwrap() {
            Reply::OnboardingState(state) => assert!(state.permission_warning_dismissed),
            other => panic!("{other:?}"),
        }

        let dialog = request(&h.app, "notices.restart_dialog", Value::Null)
            .await
            .unwrap();
        assert_eq!(dialog["title"], "Restart the background service?");
        assert_eq!(dialog["confirm"], "Restart service");

        assert_eq!(
            request(&h.app, "notices.open_target", json!({"key": "SHOP-22"}))
                .await
                .unwrap(),
            json!({"project": "SHOP", "ticket": "SHOP-22"})
        );
        assert_eq!(
            request(&h.app, "notices.open_target", json!({}))
                .await
                .unwrap(),
            json!({"project": null, "ticket": null})
        );
        assert_eq!(
            request(&h.app, "notices.open_at_start", Value::Null)
                .await
                .unwrap(),
            Value::Null
        );
        h.service.stop_all().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn restart_now_restarts_only_when_asked_and_says_how_it_went() {
        let h = Harness::new(Some("older.1"));
        h.wait_for("connected", connected).await;
        // The in-process service runs another build: the update-ready banner, and nothing restarts by itself.
        h.wait_for_event("the update banner", |e| {
            e["kind"] == "notices" && e["notices"]["banners"][0]["kind"] == "service_update_ready"
        })
        .await;
        assert_eq!(h.service.started.lock().unwrap().len(), 1);
        request(&h.app, "notices.restart", Value::Null)
            .await
            .unwrap();
        h.wait_for_event("the restart toast", |e| {
            e["kind"] == "notices"
                && e["notices"]["toasts"].as_array().is_some_and(|t| {
                    t.iter()
                        .any(|t| t["title"] == "Background service restarted")
                })
        })
        .await;
        h.wait_for_event("a busy button", |e| {
            e["kind"] == "notices" && e["notices"]["banners"][0]["buttons"][0]["label"] == "Restarting…"
        })
        .await;
        assert_eq!(
            h.service.started.lock().unwrap().len(),
            2,
            "a new service was started"
        );
        h.service.stop_all().await;
    }
}
