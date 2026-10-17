//! `notices.*`: the banner area and the toasts (T-0231, [`crate::notices`]), what their buttons do on the user's
//! click, and what a notification click opens.

use otaq_core::board::epic_flight::EpicQueueOutcome;
use otaq_core::board::settings::AutoMerge;
use otaq_core::protocol::{
    EpicUndoQueueParams, KeyParams, ProjectParams, ProjectSettingsParams, Reply, Request,
    ResumeAllParams, ToolParams,
};
use otaq_view::view_models::banners::{PERMISSION_DOCS_URL, Situation, SituationKind};
use otaq_view::view_models::limit_banner::{RESUME_FAILED, resumed};
use otaq_view::view_models::restart::RestartDialog;
use otaq_view::view_models::epic_toast;
use otaq_view::view_models::toasts::{NewToast, ToastAction, ToastLevel};
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
        "notices.toast_action" => toast_action(app, parse(params)?).await,
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

/// `{"id": 3, "label": "Undo", "action": {"kind": "undo_epic_queue", …}}`: the user pressed a toast's button.
#[derive(Debug, Clone, Deserialize, TS)]
pub struct ToastPress {
    pub id: u64,
    pub label: String,
    pub action: ToastAction,
}

/// `{"key": "SHOP-22"}` from an `open_from_notification` event; no key for a grouped notification.
#[derive(Debug, Clone, Deserialize, TS)]
pub struct NotificationKey {
    #[serde(default)]
    pub key: Option<String>,
}

/// Closing the permission-mode warning is remembered in the app's database (SET-01), and so is closing the folder
/// trust note (PRJ-08: never shown again).
async fn dismiss_banner(app: &App, params: DismissBanner) -> Result<Value, RequestError> {
    app.notices.dismiss_banner(params.kind);
    match params.kind {
        SituationKind::PermissionWarning => {
            app.service.send(Request::DismissPermissionWarning).await?;
        }
        SituationKind::TrustNote => {
            app.service.send(Request::DismissTrustNote).await?;
        }
        _ => {}
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

/// Resume after the usage limit (USG-06; per tool from M23, USG-10): the user's click tells the limited tool's
/// waiting agents to continue.
async fn resume_after_limit(app: &App) -> Result<Value, RequestError> {
    let tool = match app
        .service
        .send(Request::UsageSummary(Default::default()))
        .await
    {
        Ok(Reply::Usage(summary)) => summary.plan.and_then(|p| p.limit).map(|l| l.tool),
        _ => None,
    };
    let Some(tool) = tool else {
        return Ok(Value::Null);
    };
    let toast = match app
        .service
        .send(Request::LimitsResumeTool(ToolParams { tool }))
        .await
    {
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

/// A toast's button, on the user's click: the toast goes, and what the action did shows in its place (EPC-15).
async fn toast_action(app: &App, press: ToastPress) -> Result<Value, RequestError> {
    app.notices.dismiss_toast(press.id);
    let toast = match press.action {
        ToastAction::UndoEpicQueue { epic, keys } => {
            let request = Request::EpicUndoQueue(EpicUndoQueueParams { key: epic, keys });
            match app.service.send(request).await {
                Ok(Reply::EpicQueueUndone(outcome)) => epic_toast::undo_toast(&outcome),
                Ok(_) => None,
                Err(e) => Some(epic_toast::action_failed(&press.label, e.message)),
            }
        }
        ToastAction::EpicChildrenAutoMerge {
            project,
            name,
            undo,
        } => Some(match epic_children_auto_merge(app, &project).await {
            Ok(()) => epic_toast::auto_merge_on_toast(&name, undo.map(|b| *b)),
            Err(e) => epic_toast::action_failed(&press.label, e.message),
        }),
    };
    if let Some(toast) = toast {
        app.notices.toast(toast);
    }
    Ok(Value::Null)
}

/// Turn on for epic children: the project's Auto-merge override becomes Epic children only, the same write as
/// Settings → the project → Auto-merge (d0064). Short-term (d0080): no other settings UI.
async fn epic_children_auto_merge(app: &App, project: &str) -> Result<(), RequestError> {
    let get = Request::ProjectSettingsGet(ProjectParams {
        project: project.to_owned(),
    });
    let Reply::ProjectSettings(current) = app.service.send(get).await? else {
        return Err(RequestError::plain("unexpected answer to project_settings.get"));
    };
    let mut settings = current.settings;
    settings.auto_merge = Some(AutoMerge::EpicChildren);
    let set = Request::ProjectSettingsSet(ProjectSettingsParams {
        project: project.to_owned(),
        settings: Box::new(settings),
    });
    app.service.send(set).await?;
    Ok(())
}

/// Shows the queue toast for an epic queue the user made (a drop, ⇧⌘D, the menu): "Queued 2 children of SHOP-45 ·
/// Undo", and with Auto-merge Off "… · Turn on for epic children · Undo" (EPC-15). The epic's name is looked up for
/// the one-child wording ("Queued SHOP-45.1 of Search").
pub(crate) async fn show_epic_queued(app: &App, outcome: &EpicQueueOutcome) {
    let get = Request::TicketGet(KeyParams {
        key: outcome.epic.to_string(),
    });
    let title = match app.service.send(get).await {
        Ok(Reply::Ticket(epic)) => epic.title,
        _ => outcome.epic.to_string(),
    };
    app.notices.toast(epic_toast::queue_toast(outcome, &title));
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

    fn toast_titles(view: &Value) -> Vec<String> {
        view["toasts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap().to_owned())
            .collect()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_queue_toast_and_turn_on_for_epic_children() {
        use otaq_core::board::projects::AddProject;
        use otaq_core::board::tickets::NewTicketInput;
        use otaq_core::domain::Kind;
        use otaq_core::git::testing::TempRepos;

        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let Reply::ProjectAdded { project, .. } = h
            .app
            .service
            .send(Request::ProjectAdd(AddProject {
                path: repos.repo("shop-web", 1).display().to_string(),
                key: None,
                base_branch: None,
            }))
            .await
            .unwrap()
        else {
            panic!("no project");
        };
        let Reply::Ticket(epic) = h
            .app
            .service
            .send(Request::TicketCreate(NewTicketInput {
                project: project.key.clone(),
                kind: Kind::Epic,
                title: "Search".into(),
                body: String::new(),
                priority: None,
                epic: None,
                base_branch: None,
                external_id: None,
            }))
            .await
            .unwrap()
        else {
            panic!("no epic");
        };
        let child = |n: u32| otaq_core::domain::TicketKey {
            child: Some(n),
            ..epic.key.clone()
        };
        let mut outcome = EpicQueueOutcome {
            epic: epic.key.clone(),
            queued: vec![child(1)],
            resumed: Vec::new(),
            auto_merge_off: false,
            project: "shop-web".into(),
            lead_started: false,
        };
        show_epic_queued(&h.app, &outcome).await;
        let view = request(&h.app, "notices.view", Value::Null).await.unwrap();
        assert_eq!(
            toast_titles(&view),
            [format!("Queued {} of Search", child(1))]
        );

        outcome.queued.push(child(2));
        outcome.auto_merge_off = true;
        show_epic_queued(&h.app, &outcome).await;
        let view = request(&h.app, "notices.view", Value::Null).await.unwrap();
        let toast = &view["toasts"][1];
        assert_eq!(
            toast["title"],
            "Queued 2 tickets. Auto-merge is off for shop-web, so each one waits for your review."
        );
        let buttons = &toast["link"]["actions"];
        assert_eq!(buttons[0]["label"], "Turn on for epic children");
        assert_eq!(buttons[1]["label"], "Undo");

        // Turn on: the project's own Auto-merge becomes Epic children only, and the toast says so, keeping Undo.
        request(
            &h.app,
            "notices.toast_action",
            json!({"id": toast["id"], "label": buttons[0]["label"], "action": buttons[0]["action"]}),
        )
        .await
        .unwrap();
        let Reply::ProjectSettings(stored) = h
            .app
            .service
            .send(Request::ProjectSettingsGet(ProjectParams {
                project: project.key.clone(),
            }))
            .await
            .unwrap()
        else {
            panic!("no settings");
        };
        assert_eq!(stored.settings.auto_merge, Some(AutoMerge::EpicChildren));
        let view = request(&h.app, "notices.view", Value::Null).await.unwrap();
        let titles = toast_titles(&view);
        assert_eq!(titles.len(), 2, "the pressed toast went: {titles:?}");
        assert_eq!(titles[1], "Auto-merge: epic children only for shop-web");
        assert_eq!(view["toasts"][1]["link"]["actions"][0]["label"], "Undo");
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
