//! `settings.*`: Settings, global and per project (T-0234; SET-01, PRJ-03, PRJ-04, d0023). The page asks for the whole
//! view (`settings.view`), sends one edit at a time (`settings.edit`: a field and its value; the rules are in
//! `otaq_view::view_models::settings_page`) and gets the fresh view back. The slow parts load on their own: the
//! tools check, the PR provider, the branches. Everything is stored by the service, never in a repository.

use otaq_core::board::settings::{GlobalSettings, ProjectSettings, SettingProblem};
use otaq_core::board::snapshot::ProjectInfo;
use otaq_core::protocol::{
    KeyParams, ProjectParams, ProjectSettingsParams, ProjectUpdateParams, Reply, Request,
};
use otaq_view::view_models::about;
use otaq_view::view_models::editor_fields::base_choices;
use otaq_view::view_models::licenses;
use otaq_view::view_models::menu::MenuItem;
use otaq_view::view_models::settings::data_paths;
use otaq_view::view_models::settings_page::{
    self as page, GlobalInputs, Os, ProjectChange, ProjectInputs, Reveal, WarmInputs,
};
use otaq_view::view_models::warm_build;
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
        // Settings → About (d0071): the section, Otaq's own license, the third-party notices.
        "settings.about" => reply(&crate::about::view()),
        "settings.license" => reply(&about::license(crate::about::LICENSE)),
        "settings.licenses" => reply(&licenses::view(crate::about::NOTICES, cfg!(target_os = "macos"))),
        "settings.open_source" => {
            let p: SettingsLink = parse(params)?;
            // Only the links the credits and the notices show: the page can't make the app open anything else.
            if !page::is_credit_source(&p.url) && !crate::about::notices().links_to(&p.url) {
                return Err(RequestError::plain(format!("not a credits or notices link: {}", p.url)));
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
        Some((info, p_settings, p_problems)) => {
            // The warm build switch (T-0301): whether the disk can clone, and the project's warm build.
            let warm = super::recovery::warm_list(app).await.ok();
            page::project_page(&ProjectInputs {
                info: &info,
                settings: &p_settings,
                problems: &p_problems,
                global: &settings,
                warm: warm.as_ref().map(|list| WarmInputs {
                    list,
                    now: otaq_core::store::now_ms(),
                }),
            })
        }
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
            // Turning the warm build switch off clears the project's warm build after saving (the service does it;
            // the page asked first). Its toast comes once it is gone, after any copy it waits for (WRM-06).
            let clears = if edit.field == "warm_builds" && edit.value == page::Value::Flag(false) {
                warm_build_of(app, key).await
            } else {
                None
            };
            match page::edit_project(&mut settings, &edit.field, &edit.value)
                .map_err(RequestError::plain)?
            {
                ProjectChange::Settings => {
                    let request = Request::ProjectSettingsSet(ProjectSettingsParams {
                        project: key.clone(),
                        settings,
                    });
                    app.service.send(request).await?;
                    if let Some((repo, name)) = clears {
                        let toast = warm_build::switched_off_toast(&name);
                        super::recovery::follow_clear(app, &repo, &name, toast);
                    }
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

/// The project's own warm build, by its folder, and the project's name; `None` when it has none.
async fn warm_build_of(app: &App, key: &str) -> Option<(String, String)> {
    let list = super::recovery::warm_list(app).await.ok()?;
    let build = list
        .builds
        .into_iter()
        .find(|b| b.project.as_deref() == Some(key))?;
    let name = projects(app)
        .await
        .ok()?
        .into_iter()
        .find(|p| p.key == key)
        .map_or_else(|| build.repo.clone(), |p| p.name);
    Some((build.repo, name))
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

        // Settings → About (d0071).
        let about = request(&h.app, "settings.about", Value::Null).await.unwrap();
        assert_eq!(about["product"], otaq_core::naming::PRODUCT_NAME);
        assert!(about["version_info"].as_str().unwrap().contains(env!("CARGO_PKG_VERSION")));
        let license = request(&h.app, "settings.license", Value::Null).await.unwrap();
        assert_eq!(license["title"], "MIT License");
        let notices = request(&h.app, "settings.licenses", Value::Null).await.unwrap();
        assert!(notices["rust"].as_array().unwrap().len() > 3);
        h.service.stop_all().await;
    }

    /// The Environment variables row (T-0300; ENV-01, ENV-03): a row is added, edited and removed through
    /// `settings.edit`, stored by the service, and a repeated name comes back marked.
    #[tokio::test(flavor = "multi_thread")]
    async fn environment_variables_are_saved_and_a_duplicate_is_marked() {
        use otaq_core::board::projects::AddProject;
        use otaq_core::board::settings::EnvVar;
        use otaq_core::git::testing::TempRepos;

        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let path = repos.repo("shop-web", 1);
        let project = match h
            .app
            .service
            .send(Request::ProjectAdd(AddProject {
                path: path.display().to_string(),
                key: None,
                base_branch: None,
            }))
            .await
            .unwrap()
        {
            Reply::ProjectAdded { project: p, .. } => p.key,
            other => panic!("{other:?}"),
        };
        let edit = |field: &str, value: Value| {
            request(
                &h.app,
                "settings.edit",
                json!({ "project": project, "field": field, "value": value }),
            )
        };
        let env_row = |view: &Value| -> Value {
            view["page"]["sections"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|s| s["rows"].as_array().unwrap())
                .find(|r| r["field"] == "env_vars")
                .unwrap()["control"]
                .clone()
        };
        let view = edit("env_vars.add", json!({ "name": "CI", "value": "true" }))
            .await
            .unwrap();
        assert_eq!(env_row(&view)["rows"][0]["name"], "CI");
        let view = edit("env_vars.add", json!({ "name": "CI", "value": "false" }))
            .await
            .unwrap();
        let rows = env_row(&view)["rows"].clone();
        assert_eq!(rows[0]["problem"], Value::Null);
        assert_eq!(
            rows[1]["problem"],
            "CI is already set above. Only the first row applies; remove one of them."
        );
        assert_eq!(view["nav"].as_array().unwrap().last().unwrap()["alert"], true);
        edit("env_vars.1.name", json!("DEBUG")).await.unwrap();
        edit("env_vars.0.value", json!("1")).await.unwrap();
        let (stored, problems) = project_settings(&h.app, &project).await.unwrap();
        assert_eq!(stored.env_vars, [EnvVar::new("CI", "1"), EnvVar::new("DEBUG", "false")]);
        assert!(problems.is_empty(), "{problems:?}");
        let view = edit("env_vars.remove", json!(0)).await.unwrap();
        assert_eq!(env_row(&view)["rows"][0]["name"], "DEBUG");
        h.service.stop_all().await;
    }

    /// The warm build switch (T-0301; WRM-05): turning it off with a warm build shows the confirmation's texts,
    /// saves, clears it and toasts; turning it on asks nothing.
    #[tokio::test(flavor = "multi_thread")]
    async fn turning_the_warm_build_switch_off_clears_the_warm_build() {
        use otaq_core::board::projects::AddProject;
        use otaq_core::git::testing::TempRepos;

        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let path = repos.repo("shop-web", 1);
        let project = match h
            .app
            .service
            .send(Request::ProjectAdd(AddProject {
                path: path.display().to_string(),
                key: None,
                base_branch: None,
            }))
            .await
            .unwrap()
        {
            Reply::ProjectAdded { project: p, .. } => p.key,
            other => panic!("{other:?}"),
        };
        let open = || request(&h.app, "settings.view", json!({ "project": project }));
        let edit = |value: bool| {
            request(
                &h.app,
                "settings.edit",
                json!({ "project": project, "field": "warm_builds", "value": value }),
            )
        };
        let switch = |view: &Value| -> Value {
            view["page"]["sections"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|s| s["rows"].as_array().unwrap())
                .find(|r| r["field"] == "warm_builds")
                .unwrap()["control"]
                .clone()
        };
        let view = open().await.unwrap();
        let control = switch(&view);
        assert_eq!(control["kind"], "warm_build");
        if control["disabled"] == true {
            // The temporary folder's disk can't clone: drawn off with the reason; nothing more to try here.
            assert_eq!(control["on"], false);
            assert!(control["reason"].as_str().unwrap().starts_with("Unavailable on this disk: "));
            eprintln!("skipped: the temporary folder's disk can't clone");
            h.service.stop_all().await;
            return;
        }
        assert_eq!(control["on"], true);
        assert_eq!(control["confirm"], Value::Null, "nothing to delete: no question");

        // A warm build of the project's (its folder is the repo's name).
        let data = h.app.service.data().unwrap();
        let folder = data.warm_path("shop-web").unwrap();
        std::fs::create_dir_all(folder.join("out")).unwrap();
        std::fs::write(folder.join("out").join("built.bin"), vec![0u8; 4096]).unwrap();
        let control = switch(&open().await.unwrap());
        assert_eq!(control["confirm"]["title"], "Clear shop-web’s warm build (up to 4 KB)?");
        assert_eq!(control["confirm"]["confirm"], "Turn off and clear up to 4 KB");

        let control = switch(&edit(false).await.unwrap());
        assert_eq!(control["on"], false);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !h.app.notices.view().toasts.iter().any(|t| t.title == "Cleared shop-web’s warm build") {
            assert!(std::time::Instant::now() < deadline, "no toast");
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(!folder.exists());
        let (stored, _) = project_settings(&h.app, &project).await.unwrap();
        assert!(!stored.warm_builds);

        let control = switch(&edit(true).await.unwrap());
        assert_eq!(control["on"], true);
        assert_eq!(control["confirm"], Value::Null);
        h.service.stop_all().await;
    }
}
