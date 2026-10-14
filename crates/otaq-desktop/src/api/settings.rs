//! `settings.*`: Settings, global and per project (T-0234; SET-01, PRJ-03, PRJ-04, d0023). The page asks for the whole
//! view (`settings.view`), sends one edit at a time (`settings.edit`: a field and its value; the rules are in
//! `otaq_view::view_models::settings_page`) and gets the fresh view back. The slow parts load on their own: the
//! tools check, the PR provider, the branches. Everything is stored by the service, never in a repository.

use otaq_core::board::settings::{GlobalSettings, ProjectSettings, SettingProblem};
use otaq_core::board::snapshot::ProjectInfo;
use otaq_core::protocol::{
    KeyParams, ProjectParams, ProjectSettingsParams, ProjectUpdateParams, Reply, Request,
};
use otaq_view::view_models::editor_fields::base_choices;
use otaq_view::view_models::menu::MenuItem;
use otaq_view::view_models::settings::data_paths;
use otaq_view::view_models::settings_page::{
    self as page, GlobalInputs, Os, ProjectChange, ProjectInputs, Reveal,
};
use otaq_view::icons::IconName;
use serde::Deserialize;
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "settings.view" => {
            let p: SettingsOpen = parse(params)?;
            reply(&view(app, p.project.as_deref()).await?)
        }
        "settings.edit" => edit(app, parse(params)?).await,
        "settings.tools" => reply(&page::tools(match app.service.send(Request::ToolsCheck).await {
            Ok(Reply::ToolsCheck(report)) => Ok(report),
            Ok(other) => Err(unexpected("tools.check", &other).message),
            Err(e) => Err(e.message),
        })),
        "settings.provider" => {
            let p: SettingsProject = parse(params)?;
            let request = Request::ProjectPrProvider(ProjectParams { project: p.project });
            match app.service.send(request).await? {
                Reply::PrProvider(provider) => reply(&page::provider(Some(&provider))),
                other => Err(unexpected("project.pr_provider", &other)),
            }
        }
        "settings.branches" => branches(app, parse(params)?).await,
        "settings.remove_preview" => remove_preview(app, parse(params)?).await,
        "settings.remove" => {
            let p: SettingsProject = parse(params)?;
            app.service
                .send(Request::ProjectRemove(KeyParams { key: p.project }))
                .await?;
            reply(&view(app, None).await?)
        }
        "settings.credits" => reply(&page::credits()),
        "settings.open_source" => {
            let p: SettingsLink = parse(params)?;
            // Only the credits' own links: the page can't make the app open anything else.
            if !page::is_credit_source(&p.url) {
                return Err(RequestError::plain(format!("not a credits link: {}", p.url)));
            }
            open::that_detached(&p.url).map_err(|e| RequestError::plain(e.to_string()))?;
            reply(&Value::Null)
        }
        "settings.reveal" => {
            let p: SettingsRevealParams = parse(params)?;
            let data = app
                .service
                .data()
                .ok_or_else(|| RequestError::plain("no data folder"))?;
            // The folder that holds it, in the file manager (the database file's own folder).
            let folder = match p.target {
                Reveal::Database => data.database_path().parent().map(ToOwned::to_owned),
                Reveal::Logs => Some(data.logs_dir()),
            }
            .ok_or_else(|| RequestError::plain("no folder to show"))?;
            open::that_detached(&folder).map_err(|e| RequestError::plain(e.to_string()))?;
            reply(&Value::Null)
        }
        _ => Err(unknown(method)),
    }
}

/// `settings.view`: the global page, or a project's page when `project` names one.
#[derive(Debug, Clone, Default, Deserialize, TS)]
pub struct SettingsOpen {
    #[serde(default)]
    pub project: Option<String>,
}

/// One edit: `project` names the project page it was made on (none: the global page).
#[derive(Debug, Clone, Deserialize, TS)]
pub struct SettingsEdit {
    #[serde(default)]
    pub project: Option<String>,
    pub field: String,
    pub value: page::Value,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct SettingsProject {
    pub project: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct SettingsLink {
    pub url: String,
}

#[derive(Debug, Clone, Copy, Deserialize, TS)]
pub struct SettingsRevealParams {
    pub target: Reveal,
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    let params = if params.is_null() {
        Value::Object(Default::default())
    } else {
        params
    };
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

fn unexpected(method: &str, reply: &Reply) -> RequestError {
    RequestError::plain(format!("unexpected answer to {method}: {reply:?}"))
}

async fn global(app: &App) -> Result<(GlobalSettings, Vec<SettingProblem>), RequestError> {
    match app.service.send(Request::SettingsGet).await? {
        Reply::GlobalSettings(r) => Ok((r.settings, r.problems)),
        other => Err(unexpected("settings.get", &other)),
    }
}

async fn projects(app: &App) -> Result<Vec<ProjectInfo>, RequestError> {
    match app.service.send(Request::ProjectList).await? {
        Reply::Projects(list) => Ok(list),
        other => Err(unexpected("project.list", &other)),
    }
}

async fn project_settings(
    app: &App,
    key: &str,
) -> Result<(ProjectSettings, Vec<SettingProblem>), RequestError> {
    let request = Request::ProjectSettingsGet(ProjectParams {
        project: key.to_owned(),
    });
    match app.service.send(request).await? {
        Reply::ProjectSettings(r) => Ok((r.settings, r.problems)),
        other => Err(unexpected("project_settings.get", &other)),
    }
}

/// The nav and the page: the global page, or `project`'s page.
async fn view(app: &App, project: Option<&str>) -> Result<page::View, RequestError> {
    let (settings, problems) = global(app).await?;
    let list = projects(app).await?;
    // The nav marks the projects with a flagged value.
    let mut alerts = Vec::new();
    let mut open = None;
    for info in &list {
        let (p_settings, p_problems) = project_settings(app, &info.key).await?;
        if !p_problems.is_empty() {
            alerts.push(info.key.clone());
        }
        if project == Some(info.key.as_str()) {
            open = Some((info.clone(), p_settings, p_problems));
        }
    }
    let shown = match open {
        Some((info, p_settings, p_problems)) => page::project_page(&ProjectInputs {
            info: &info,
            settings: &p_settings,
            problems: &p_problems,
            global: &settings,
        }),
        None => {
            let scheduler = match app.service.send(Request::SchedulerStatus).await {
                Ok(Reply::SchedulerStatus(s)) => Some(s),
                _ => None,
            };
            let cleanup_count = match app.service.send(Request::CleanupScan).await {
                Ok(Reply::CleanupScan(items)) => items.len(),
                _ => 0,
            };
            page::global_page(&GlobalInputs {
                settings: &settings,
                problems: &problems,
                scheduler: scheduler.as_ref(),
                data: app.service.data().map(|d| data_paths(&d)),
                cleanup_count,
                os: Os::current(),
            })
        }
    };
    Ok(page::view(shown, &list, &alerts))
}

/// Applies one edit and answers with the fresh view. A refused value comes back as the request's error.
async fn edit(app: &App, edit: SettingsEdit) -> Result<Value, RequestError> {
    match &edit.project {
        None => {
            let (mut settings, _) = global(app).await?;
            page::edit_global(&mut settings, &edit.field, &edit.value).map_err(RequestError::plain)?;
            // The service applies it at once (limits, notifications) and announces `settings_changed`, on which
            // the theme and the terminals follow.
            app.service.send(Request::SettingsSet(settings)).await?;
        }
        Some(key) => {
            let (mut settings, _) = project_settings(app, key).await?;
            match page::edit_project(&mut settings, &edit.field, &edit.value)
                .map_err(RequestError::plain)?
            {
                ProjectChange::Settings => {
                    let request = Request::ProjectSettingsSet(ProjectSettingsParams {
                        project: key.clone(),
                        settings,
                    });
                    app.service.send(request).await?;
                }
                ProjectChange::Project(update) => {
                    let request = Request::ProjectUpdate(ProjectUpdateParams {
                        project: key.clone(),
                        update,
                    });
                    app.service.send(request).await?;
                }
                ProjectChange::Nothing => {}
            }
        }
    }
    reply(&view(app, edit.project.as_deref()).await?)
}

/// The base branch dropdown: local branches, then remote ones not checked out, the current one checked.
async fn branches(app: &App, p: SettingsProject) -> Result<Value, RequestError> {
    let current = projects(app)
        .await?
        .into_iter()
        .find(|i| i.key == p.project)
        .map(|i| i.base_branch)
        .unwrap_or_default();
    let request = Request::ProjectBranches(ProjectParams { project: p.project });
    let Reply::Branches(branches) = app.service.send(request).await? else {
        return Err(RequestError::plain("unexpected answer to project.branches"));
    };
    let (local, remote) = base_choices(&branches.local, &branches.remote);
    let mut items = vec![MenuItem::header("LOCAL")];
    for b in local {
        let checked = b == current;
        items.push(MenuItem::new(b.clone(), b).icon(IconName::GitBranch).checked(checked));
    }
    if !remote.is_empty() {
        items.push(MenuItem::header("REMOTE"));
        for (full, name) in remote {
            let checked = name == current;
            items.push(MenuItem::new(full, name).icon(IconName::GitBranch).checked(checked));
        }
    }
    reply(&items)
}

/// The Remove project confirmation, with the project's ticket count.
async fn remove_preview(app: &App, p: SettingsProject) -> Result<Value, RequestError> {
    let info = projects(app)
        .await?
        .into_iter()
        .find(|i| i.key == p.project)
        .ok_or_else(|| RequestError::plain(format!("no project {}", p.project)))?;
    let request = Request::BoardSnapshot(ProjectParams {
        project: p.project.clone(),
    });
    let tickets = match app.service.send(request).await? {
        Reply::Board(board) => board.tickets().count(),
        _ => 0,
    };
    reply(&page::remove_dialog(&info, tickets))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::api::request;
    use crate::service::testing::{Harness, connected};

    #[tokio::test(flavor = "multi_thread")]
    async fn the_global_page_loads_and_an_edit_is_stored() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let view = request(&h.app, "settings.view", Value::Null).await.unwrap();
        assert_eq!(view["page"]["page"], "global");
        assert_eq!(view["page"]["title"], "General");
        assert_eq!(view["nav"][0]["label"], "Agent limits");
        let data = &view["page"]["sections"][7];
        assert_eq!(data["id"], "data");
        assert_eq!(data["rows"][0]["control"]["kind"], "path");

        let edited = request(
            &h.app,
            "settings.edit",
            json!({ "field": "terminal_renderer", "value": "dom" }),
        )
        .await
        .unwrap();
        let renderer = edited["page"]["sections"][8]["rows"][3]["control"]["value"].clone();
        assert_eq!(renderer, "dom");
        // VAL-13: rounds accept only 1–5.
        let edited = request(&h.app, "settings.edit", json!({ "field": "validation_rounds", "value": 9 }))
            .await
            .unwrap();
        assert_eq!(edited["page"]["sections"][3]["rows"][1]["control"]["value"], 5);
        let (stored, _) = global(&h.app).await.unwrap();
        assert_eq!(
            stored.terminal_renderer,
            otaq_core::board::settings::TerminalRenderer::Dom
        );

        let refused = request(&h.app, "settings.edit", json!({ "field": "notifications", "value": "no" }))
            .await
            .unwrap_err();
        assert!(refused.message.contains("on or off"), "{refused:?}");
        let credits = request(&h.app, "settings.credits", Value::Null).await.unwrap();
        assert_eq!(credits["title"], "Theme credits");
        assert!(request(&h.app, "settings.open_source", json!({ "url": "https://example.com" }))
            .await
            .is_err());
        h.service.stop_all().await;
    }
}
