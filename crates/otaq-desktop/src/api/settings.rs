//! `settings.*`: Settings, global and per project (T-0234; SET-01, PRJ-03, PRJ-04, d0023). The page asks for the whole
//! view (`settings.view`), sends one edit at a time (`settings.edit`: a field and its value; the rules are in
//! `otaq_view::view_models::settings_page`) and gets the fresh view back. The slow parts load on their own: the
//! tools check, the PR provider, the branches. Everything is stored by the service, never in a repository.

use otaq_core::board::settings::{GlobalSettings, ProjectSettings, SettingProblem, WorkflowStep};
use otaq_core::board::snapshot::ProjectInfo;
use otaq_core::agent_tools::Tool;
use otaq_core::board::Refusal;
use otaq_core::named_agents::AgentList;
use otaq_core::protocol::{
    AgentDefaultParams, AgentEditParams, AutoFallbackParams, IdParams, KeyParams, ModelsParams,
    ProjectAgentDefaultParams, ProjectParams, ProjectSettingsParams, ProjectUpdateParams, Reply,
    Request, WorkflowSkillsParams,
};
use otaq_view::view_models::about;
use otaq_view::view_models::editor_fields::base_choices;
use otaq_view::view_models::licenses;
use otaq_view::view_models::menu::MenuItem;
use otaq_view::view_models::named_agents;
use otaq_view::view_models::settings::data_paths;
use otaq_view::view_models::settings_page::{
    self as page, GlobalInputs, Os, ProjectChange, ProjectInputs, Reveal, WarmInputs,
};
use otaq_view::view_models::warm_build;
use otaq_view::icons::IconName;
use serde::{Deserialize, Serialize};
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
        // Settings → Agents and the project's Agents block (T-0340; AGT-01…AGT-03).
        "settings.agents" => {
            let p: AgentsOpen = parse(params)?;
            reply(&agents_section(app, p.refresh).await?)
        }
        "settings.agent_dialog" => reply(&agent_dialog(app, parse(params)?).await?),
        // The picker's Retry (`GTuGI`): reads the tool's list again, then the dialog as it was (T-0361).
        "settings.agent_models_retry" => {
            let p: AgentModelsRetry = parse(params)?;
            let copy = match app
                .service
                .send(Request::AgentsModelsRetry(ModelsParams { tool: p.tool }))
                .await?
            {
                Reply::Models(copy) => copy,
                other => return Err(unexpected("agents.models_retry", &other)),
            };
            visit(app).read(p.tool, copy.is_some());
            reply(&agent_dialog(app, p.dialog).await?)
        }
        // "Use fallback agents automatically" (AGT-08; T-0361).
        "settings.auto_fallback" => {
            let p: AutoFallbackSet = parse(params)?;
            app.service
                .send(Request::SettingsAutoFallback(AutoFallbackParams { on: p.on }))
                .await?;
            reply(&agents_section(app, false).await?)
        }
        "settings.agent_save" => reply(&agent_save(app, parse(params)?).await?),
        "settings.agent_delete_dialog" => {
            let p: AgentIdParams = parse(params)?;
            reply(&delete_dialog(app, &p.id).await?)
        }
        "settings.agent_delete" => {
            let p: AgentIdParams = parse(params)?;
            reply(&agent_delete(app, &p.id).await?)
        }
        "settings.agent_default" => {
            let p: AgentDefaultSet = parse(params)?;
            app.service
                .send(Request::AgentsSetDefault(AgentDefaultParams {
                    role: p.role.into(),
                    agent: p.agent,
                }))
                .await?;
            reply(&agents_section(app, false).await?)
        }
        "settings.project_agents" => {
            let p: SettingsProject = parse(params)?;
            reply(&project_agents(app, &p.project).await?)
        }
        "settings.project_agent_default" => {
            let p: ProjectAgentDefaultSet = parse(params)?;
            app.service
                .send(Request::ProjectSetAgentDefault(ProjectAgentDefaultParams {
                    project: p.project.clone(),
                    role: p.role.into(),
                    agent: p.agent,
                }))
                .await?;
            reply(&project_agents(app, &p.project).await?)
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

/// `settings.agents`: `refresh` reads the tool's model list again first (`agents.models_refresh`): the page asks
/// for it once when the section opens (d0086).
#[derive(Debug, Clone, Copy, Default, Deserialize, TS)]
pub struct AgentsOpen {
    #[serde(default)]
    pub refresh: bool,
}

/// `settings.agent_dialog`: Edit agent for `id`, Add agent prefilled from `duplicate` ("<name> copy"), else Add
/// agent; `draft` is what the fields hold now (the page sends it on each change for the live checks). `tool`: the
/// Tool picker changed to it (`codex`): the model goes back to the tool default and a fallback on that tool goes.
#[derive(Debug, Clone, Default, Deserialize, TS)]
pub struct AgentDialogParams {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub duplicate: Option<String>,
    #[serde(default)]
    pub draft: Option<named_agents::Draft>,
    #[serde(default)]
    #[ts(type = "string | null")]
    pub tool: Option<Tool>,
}

/// `settings.agent_models_retry`: the picker's Retry for `tool`, then the dialog of `dialog`.
#[derive(Debug, Clone, Deserialize, TS)]
pub struct AgentModelsRetry {
    #[ts(type = "string")]
    pub tool: Tool,
    #[serde(default)]
    pub dialog: AgentDialogParams,
}

/// `settings.auto_fallback`: the Fallbacks block's checkbox.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
pub struct AutoFallbackSet {
    pub on: bool,
}

/// `settings.agent_save`: Add agent (`id` none) or Save.
#[derive(Debug, Clone, Deserialize, TS)]
pub struct AgentSaveParams {
    #[serde(default)]
    pub id: Option<String>,
    pub draft: named_agents::Draft,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct AgentIdParams {
    pub id: String,
}

/// `settings.agent_default`: an app-wide role default.
#[derive(Debug, Clone, Deserialize, TS)]
pub struct AgentDefaultSet {
    pub role: named_agents::RoleId,
    pub agent: String,
}

/// `settings.project_agent_default`: `agent` null = "Use app default".
#[derive(Debug, Clone, Deserialize, TS)]
pub struct ProjectAgentDefaultSet {
    pub project: String,
    pub role: named_agents::RoleId,
    #[serde(default)]
    pub agent: Option<String>,
}

/// How Add agent or Save went: saved (the fresh section), or refused (the dialog with the reason in its field).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "outcome", rename_all = "snake_case")]
#[ts(rename = "AgentSaveOutcome")]
pub enum AgentSaveOutcome {
    Saved { section: Box<named_agents::Section> },
    Refused { dialog: Box<named_agents::Dialog> },
}

/// How Delete went: deleted (the fresh section), or something started using it since the dialog asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "outcome", rename_all = "snake_case")]
#[ts(rename = "AgentDeleteOutcome")]
pub enum AgentDeleteOutcome {
    Deleted { section: Box<named_agents::Section> },
    Refused { dialog: Box<named_agents::DeleteDialog> },
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    let params = if params.is_null() {
        Value::Object(Default::default())
    } else {
        params
    };
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

async fn agent_list(app: &App) -> Result<AgentList, RequestError> {
    match app.service.send(Request::AgentsList).await? {
        Reply::Agents(list) => Ok(*list),
        other => Err(unexpected("agents.list", &other)),
    }
}

/// What the page keeps while Settings → Agents is open.
fn visit(app: &App) -> std::sync::MutexGuard<'_, named_agents::Visit> {
    app.agents_visit.lock().unwrap_or_else(|e| e.into_inner())
}

/// The agent tools (`agent_tools.list`; the avatars follow it), each tool's cached model list (T-0335), the connected
/// ones read again first when `refresh`, and the tools whose read gave none. When the tools can't be listed, the
/// first one counts as connected, as before M23.
async fn agent_tools(app: &App, refresh: bool) -> Result<named_agents::AgentTools, RequestError> {
    let connected: Vec<Tool> = match super::agent_tools::list(app, Request::AgentToolsList).await {
        Ok(list) => list
            .tools
            .iter()
            .filter(|t| t.state.is_connected())
            .map(|t| t.tool)
            .collect(),
        Err(_) => vec![named_agents::new_entry_tool()],
    };
    let mut lists = Vec::new();
    for &tool in Tool::ALL {
        let read = refresh && connected.contains(&tool);
        let params = ModelsParams { tool };
        let request = if read {
            Request::AgentsModelsRefresh(params)
        } else {
            Request::AgentsModels(params)
        };
        let copy = match app.service.send(request).await? {
            Reply::Models(copy) => copy.map(|c| c.list),
            other => return Err(unexpected("agents.models", &other)),
        };
        if read {
            visit(app).read(tool, copy.is_some());
        }
        lists.extend(copy.map(|list| (tool, list)));
    }
    Ok(named_agents::AgentTools {
        connected,
        lists,
        failed: visit(app).failed(),
    })
}

/// The section; `refresh` is the page opening: it notes the entries there are and reads the connected tools' lists.
async fn agents_section(app: &App, refresh: bool) -> Result<named_agents::Section, RequestError> {
    let list = agent_list(app).await?;
    if refresh {
        visit(app).open(&list.agents);
    }
    let tools = agent_tools(app, refresh).await?;
    let page = visit(app).page(&list.agents, list.auto_fallback);
    Ok(named_agents::section(&list.agents, &list.defaults, &tools, &page))
}

async fn agent_dialog(app: &App, p: AgentDialogParams) -> Result<named_agents::Dialog, RequestError> {
    let tools = agent_tools(app, false).await?;
    let list = agent_list(app).await?;
    let draft = match (p.draft, &p.id, p.duplicate) {
        (Some(draft), _, _) => draft,
        (None, Some(id), _) => list
            .agents
            .iter()
            .find(|a| a.id == *id)
            .map(named_agents::Draft::of)
            .ok_or_else(|| RequestError::plain(format!("no agent {id}")))?,
        (None, None, Some(of)) => match app
            .service
            .send(Request::AgentsDuplicate(IdParams { id: of }))
            .await?
        {
            Reply::AgentDraft(fields) => named_agents::Draft {
                name: fields.name,
                tool: Some(fields.tool),
                model: fields.model,
                use_for: fields.use_for,
                fallback: fields.fallback,
            },
            other => return Err(unexpected("agents.duplicate", &other)),
        },
        (None, None, None) => named_agents::Draft::default(),
    };
    let draft = match p.tool {
        Some(tool) => named_agents::change_tool(draft, tool, &list.agents, p.id.as_deref()),
        None => draft,
    };
    Ok(named_agents::dialog(
        &list.agents,
        p.id.as_deref(),
        draft,
        &tools,
        None,
    ))
}

async fn agent_save(app: &App, p: AgentSaveParams) -> Result<AgentSaveOutcome, RequestError> {
    let list = agent_list(app).await?;
    let tools = agent_tools(app, false).await?;
    let refused = |e: Option<(&str, &str)>, draft| {
        AgentSaveOutcome::Refused {
            dialog: Box::new(named_agents::dialog(
                &list.agents,
                p.id.as_deref(),
                draft,
                &tools,
                e,
            )),
        }
    };
    let checked = named_agents::dialog(&list.agents, p.id.as_deref(), p.draft.clone(), &tools, None);
    if !checked.save_enabled {
        return Ok(refused(None, p.draft));
    }
    let fields = named_agents::fields(&p.draft, &list.agents, p.id.as_deref(), &tools);
    let request = match &p.id {
        Some(id) => Request::AgentsEdit(AgentEditParams {
            id: id.clone(),
            fields,
        }),
        None => Request::AgentsAdd(fields),
    };
    match app.service.send(request).await {
        Ok(answer) => {
            // The user's own new entry is never marked "added".
            if let Reply::Agent(agent) = &answer
                && p.id.is_none()
            {
                visit(app).added_by_user(&agent.id);
            }
            Ok(AgentSaveOutcome::Saved {
                section: Box::new(agents_section(app, false).await?),
            })
        }
        Err(e) => {
            match e.refusal.as_deref() {
                Some(code) => Ok(refused(Some((code, &e.message)), p.draft)),
                None => Err(e),
            }
        }
    }
}

/// What Delete… opens: the refused dialog or the plain confirmation (`agents.in_use` asks first).
async fn delete_dialog(app: &App, id: &str) -> Result<named_agents::DeleteDialog, RequestError> {
    let list = agent_list(app).await?;
    let name = list
        .agents
        .iter()
        .find(|a| a.id == id)
        .map(|a| a.name.clone())
        .ok_or_else(|| RequestError::plain(format!("no agent {id}")))?;
    let request = Request::AgentsInUse(IdParams { id: id.to_owned() });
    match app.service.send(request).await? {
        Reply::AgentInUse(uses) => Ok(named_agents::delete_dialog(id, &name, &uses)),
        other => Err(unexpected("agents.in_use", &other)),
    }
}

async fn agent_delete(app: &App, id: &str) -> Result<AgentDeleteOutcome, RequestError> {
    let request = Request::AgentsDelete(IdParams { id: id.to_owned() });
    match app.service.send(request).await {
        Ok(_) => Ok(AgentDeleteOutcome::Deleted {
            section: Box::new(agents_section(app, false).await?),
        }),
        Err(e) => {
            if e.refusal.as_deref() == Some(Refusal::AgentInUse.as_str()) {
                Ok(AgentDeleteOutcome::Refused {
                    dialog: Box::new(delete_dialog(app, id).await?),
                })
            } else {
                Err(e)
            }
        }
    }
}

async fn project_agents(app: &App, project: &str) -> Result<page::ProjectAgentsBlock, RequestError> {
    let tools = agent_tools(app, false).await?;
    let list = agent_list(app).await?;
    let request = Request::ProjectAgentDefaults(ProjectParams {
        project: project.to_owned(),
    });
    let own = match app.service.send(request).await? {
        Reply::AgentDefaults(own) => *own,
        other => return Err(unexpected("project.agent_defaults", &other)),
    };
    Ok(page::project_agents_block(
        project,
        &list.agents,
        &list.defaults,
        &own,
        &tools,
    ))
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
    // The skills in force, for the Workflow skills note: a project's own, else the global one (T-0360).
    let mut skills: Vec<String> = WorkflowStep::ALL
        .into_iter()
        .map(|step| settings.workflow_skills.get(step).to_owned())
        .collect();
    let mut skills_project = None;
    let mut shown = match open {
        Some((info, p_settings, p_problems)) => {
            for (skill, step) in skills.iter_mut().zip(WorkflowStep::ALL) {
                let own = p_settings.workflow_skills.get(step);
                if !own.trim().is_empty() {
                    *skill = own.to_owned();
                }
            }
            skills_project = Some(info.key.clone());
            // The warm build switch (T-0301): whether the disk can clone, and the project's warm build.
            let warm = super::recovery::warm_list(app).await.ok();
            // Run command's Ask the agent names the project's Project agent (T-0342).
            let project_agent = otaq_view::view_models::agent_pane::new_agent_name(
                super::pane::project_agent(app, &info.key).await?.as_ref(),
            );
            // Ask the agent is hidden only when no agent tool is connected (RUN-02, d0086).
            let tool_connected = match super::agent_tools::list(app, Request::AgentToolsList).await {
                Ok(list) => list.tools.iter().any(|t| t.state.is_connected()),
                Err(_) => true,
            };
            page::project_page(&ProjectInputs {
                info: &info,
                settings: &p_settings,
                problems: &p_problems,
                global: &settings,
                warm: warm.as_ref().map(|list| WarmInputs {
                    list,
                    now: otaq_core::store::now_ms(),
                }),
                project_agent: &project_agent,
                tool_connected,
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
            // What macOS allows, for the status line under the notifications switch (ATT-14).
            let notification_permission = match app.service.send(Request::NotificationsState).await {
                Ok(Reply::NotificationsState(state)) => state.permission,
                _ => otaq_core::attention::notify::PermissionState::Unknown,
            };
            page::global_page(&GlobalInputs {
                settings: &settings,
                problems: &problems,
                scheduler: scheduler.as_ref(),
                data: app.service.data().map(|d| data_paths(&d)),
                cleanup_count,
                os: Os::current(),
                notification_permission,
            })
        }
    };
    if let Ok(Reply::SkillUses(uses)) = app
        .service
        .send(Request::AgentToolsWorkflowSkills(WorkflowSkillsParams {
            skills,
            project: skills_project,
        }))
        .await
    {
        page::with_skill_notes(&mut shown, &uses);
    }
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
                        settings: Box::new(settings),
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
        assert_eq!(view["nav"][0]["label"], "Agent tools");
        assert_eq!(view["nav"][1]["label"], "Agents");
        // Agent limits and Agent instructions are on the Agents page, not rows of their own.
        assert_eq!(view["nav"][2]["label"], "Workflow skills");
        // No agent tool is connected in the test service: no Workflow skills note.
        assert_eq!(view["page"]["sections"][4]["rows"][0]["control"]["notes"], json!([]));
        let data = &view["page"]["sections"][9];
        assert_eq!(data["id"], "data");
        assert_eq!(data["rows"][0]["control"]["kind"], "path");

        let edited = request(
            &h.app,
            "settings.edit",
            json!({ "field": "terminal_renderer", "value": "dom" }),
        )
        .await
        .unwrap();
        let renderer = edited["page"]["sections"][10]["rows"][3]["control"]["value"].clone();
        assert_eq!(renderer, "dom");
        // VAL-13: rounds accept only 1–5.
        let edited = request(&h.app, "settings.edit", json!({ "field": "validation_rounds", "value": 9 }))
            .await
            .unwrap();
        assert_eq!(edited["page"]["sections"][5]["rows"][1]["control"]["value"], 5);
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
        // No agent tool is connected in the test service: Run command has no Ask the agent (RUN-02, d0086).
        let run = view["page"]["sections"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|s| s["rows"].as_array().unwrap())
            .find(|r| r["field"] == "run_command")
            .unwrap()
            .clone();
        assert_eq!(run["button"], Value::Null);
        let control = switch(&view);
        assert_eq!(control["kind"], "warm_build");
        // On every disk (M19-b): where it can't clone, the build is handed over instead.
        assert_eq!(control["disabled"], false);
        assert_eq!(control["reason"], Value::Null);
        assert_eq!(control["on"], true);
        assert_eq!(control["confirm"], Value::Null, "nothing to delete: no question");

        // A warm build of the project's (its folder is the repo's name).
        let data = h.app.service.data().unwrap();
        let folder = data.warm_entry_path("shop-web", 0).unwrap();
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

    /// Settings → Agents through the service (T-0340): add, a taken name, defaults, Duplicate's draft, the three
    /// Delete outcomes and the project's block. The test service has no model list, so entries are tool default.
    #[tokio::test(flavor = "multi_thread")]
    async fn agents_are_added_checked_defaulted_and_deleted() {
        use otaq_core::board::projects::AddProject;
        use otaq_core::git::testing::TempRepos;

        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let ask = |method: &'static str, params: Value| request(&h.app, method, params);

        // Opening the section reads the list again (none in tests) and shows what's there.
        let section = ask("settings.agents", json!({ "refresh": true })).await.unwrap();
        assert_eq!(section["title"], "Agents");
        assert_eq!(section["columns"], json!(["NAME", "MODEL", "USE FOR"]));
        assert_eq!(section["rows"], json!([]));

        let add = |name: &str| {
            ask(
                "settings.agent_save",
                json!({ "draft": { "name": name, "model": null, "use_for": "Everything" } }),
            )
        };
        // No tool is connected in the test service: one-tool columns, the checkbox disabled with its reason.
        assert_eq!(section["fallbacks"]["enabled"], false);
        assert_eq!(section["fallbacks"]["on"], false);
        assert_eq!(
            section["fallbacks"]["note"],
            "Disabled: a fallback must be on another tool. Connect a second agent tool to use fallbacks."
        );
        let saved = add("Claude Code").await.unwrap();
        assert_eq!(saved["outcome"], "saved");
        assert_eq!(saved["section"]["rows"][0]["model"], "Tool default");
        assert_eq!(
            saved["section"]["rows"][0]["tool_problem"],
            "Claude Code isn't connected · its tickets wait"
        );
        // The user's own entry isn't "added" (a tool connecting adds those).
        assert_eq!(saved["section"]["rows"][0]["added"], Value::Null);
        assert_eq!(saved["section"]["fresh_note"]["title"], "One agent, so nothing picks yet");
        let cc = saved["section"]["rows"][0]["id"].as_str().unwrap().to_owned();
        assert_eq!(saved["section"]["rows"][0]["menu"][2]["enabled"], false, "the last agent");

        // A taken name, ignoring case: refused in the view's words, nothing saved.
        let refused = add("claude code").await.unwrap();
        assert_eq!(refused["outcome"], "refused");
        assert_eq!(
            refused["dialog"]["name_problem"],
            "There's already an agent named claude code"
        );
        let live = ask("settings.agent_dialog", json!({ "draft": { "name": "CLAUDE CODE" } }))
            .await
            .unwrap();
        assert_eq!(live["save_enabled"], false);
        assert_eq!(live["title"], "Add agent");
        assert_eq!(live["tool"], "Claude Code");
        assert_eq!(live["fallback"], Value::Null, "no Fallback field with one tool");
        // The Tool picker's change: the model goes back to the tool default.
        let moved = ask(
            "settings.agent_dialog",
            json!({ "draft": { "name": "X", "model": "opus" }, "tool": "codex" }),
        )
        .await
        .unwrap();
        assert_eq!(moved["draft"]["tool"], "codex");
        assert_eq!(moved["draft"]["model"], Value::Null);
        assert_eq!(moved["tool"], "Codex");
        // Retry: the test service reads no list, so the picker says it couldn't load it.
        let retried = ask(
            "settings.agent_models_retry",
            json!({ "tool": "codex", "dialog": { "draft": { "name": "X", "tool": "codex" } } }),
        )
        .await
        .unwrap();
        assert_eq!(retried["picker"]["status"]["text"], "Couldn't load Codex's models");
        assert_eq!(retried["picker"]["status"]["retry_tool"], "codex");
        // The checkbox is stored (the service, d0086) and comes back on the section.
        let on = ask("settings.auto_fallback", json!({ "on": true })).await.unwrap();
        assert_eq!(on["fallbacks"]["on"], true);
        let off = ask("settings.auto_fallback", json!({ "on": false })).await.unwrap();
        assert_eq!(off["fallbacks"]["on"], false);

        let light = add("Light").await.unwrap()["section"]["rows"][1]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let section = ask("settings.agent_default", json!({ "role": "validator", "agent": light }))
            .await
            .unwrap();
        assert_eq!(section["fresh_note"], Value::Null);
        assert_eq!(section["defaults"][1]["picker"]["value"]["name"], "Light");

        // Duplicate's draft opens Add agent prefilled.
        let copy = ask("settings.agent_dialog", json!({ "duplicate": light })).await.unwrap();
        assert_eq!(copy["title"], "Add agent");
        assert_eq!(copy["draft"]["name"], "Light copy");
        let edit = ask("settings.agent_dialog", json!({ "id": light })).await.unwrap();
        assert_eq!(edit["title"], "Edit agent · Light");
        assert_eq!(edit["delete"]["enabled"], true);

        // In use: Delete… opens the refused dialog, and Delete itself is refused the same way.
        let dialog = ask("settings.agent_delete_dialog", json!({ "id": light })).await.unwrap();
        assert_eq!(dialog["kind"], "refused");
        assert_eq!(dialog["title"], "Light can't be deleted");
        assert_eq!(dialog["lines"], json!(["App-wide · Validator default"]));
        let deleted = ask("settings.agent_delete", json!({ "id": light })).await.unwrap();
        assert_eq!(deleted["outcome"], "refused");

        // The project block: collapsed, then Developer → Light opens it expanded.
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
        ask("settings.agent_default", json!({ "role": "developer", "agent": cc }))
            .await
            .unwrap();
        let block = ask("settings.project_agents", json!({ "project": project })).await.unwrap();
        assert_eq!(block["expanded"], false);
        assert_eq!(block["collapsed_text"], "Uses app defaults");
        assert_eq!(
            block["rows"][0]["picker"]["options"][0]["name"],
            "Use app default (Claude Code)"
        );
        let block = ask(
            "settings.project_agent_default",
            json!({ "project": project, "role": "developer", "agent": light }),
        )
        .await
        .unwrap();
        assert_eq!(block["expanded"], true);
        assert_eq!(block["rows"][0]["picker"]["value"]["name"], "Light");
        let block = ask(
            "settings.project_agent_default",
            json!({ "project": project, "role": "developer", "agent": null }),
        )
        .await
        .unwrap();
        assert_eq!(block["expanded"], false);

        // Unused once its default moves: the plain confirmation, then it's gone.
        ask("settings.agent_default", json!({ "role": "validator", "agent": cc }))
            .await
            .unwrap();
        let dialog = ask("settings.agent_delete_dialog", json!({ "id": light })).await.unwrap();
        assert_eq!(dialog["kind"], "confirm");
        assert_eq!(dialog["title"], "Delete Light?");
        let deleted = ask("settings.agent_delete", json!({ "id": light })).await.unwrap();
        assert_eq!(deleted["outcome"], "deleted");
        assert_eq!(deleted["section"]["rows"].as_array().unwrap().len(), 1);
        h.service.stop_all().await;
    }
}
