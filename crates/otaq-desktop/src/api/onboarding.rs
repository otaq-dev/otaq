//! `onboarding.*`: first run and Add project (UI-08, UI-11, SET-02, PRJ-01, PRJ-02, PRJ-03; d0011, d0021, d0023,
//! d0069, d0070; *14 · First run & onboarding*, *02 → No projects yet*). The steps' words come once
//! (`onboarding.texts`); the tools step runs the service's tools check and answers the checklist; the Add project
//! form sends what the user picked and typed and gets the form back as Rust words it (the folder's check, the
//! detected values, the key's live check, the connected tools that keep folder trust); `onboarding.add` adds the
//! project or says why not, and which tools' folder trust wasn't recorded and why (PRJ-07, d0085, d0086). The Tools
//! step's agent tool rows act through `agent_tools.*` (Sign in…, Show, Hide). A folder dropped on the window reaches
//! the page as a `files_dropped` event ([`crate::drops`]). `onboarding.sources` answers the Flow go-ahead's line
//! for a project (ONB-03, ONB-04, ONB-08; d0108): what the agent will look at, or "Nothing to look at yet".
//! `onboarding.flow` answers the whole Flow step for the project just added (ONB-04; the go-ahead, the presets), asking
//! what there is once. The step's proposal is `flow_preview.view` with its onboarding buttons (ONB-06).
//! `onboarding.flow_dialog` answers the same go-ahead, shorter, for the Add project dialog after a project is added
//! later (ONB-08); its proposal opens in the usual preview.
//! `onboarding.at_work` answers the card beside the onboarding agent's terminal (ONB-05): working, stopped at the
//! limit, ended without a proposal; before the start, whether the Project agent's tool isn't signed in.

use otaq_core::board::projects::AddProject;
use otaq_core::board::snapshot::ProjectInfo;
use otaq_core::agent_tools::SetupOutcome as Outcome;
use otaq_core::agent_tools::Tool;
use otaq_core::protocol::{OnboardingState, PathParams, ProjectParams, Reply, Request};
use otaq_view::view_models::add_project::AddProjectModel;
use otaq_view::view_models::flow_at_work;
use otaq_view::view_models::flow_sources::{self, View as FlowLook};
use otaq_view::view_models::onboarding::OnboardingModel;
use otaq_view::view_models::onboarding_page::{self, AddProjectPage, FlowStep, Texts, Tools};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "onboarding.start" => reply(&start(app).await?),
        "onboarding.texts" => reply(&texts(app).await),
        "onboarding.tools" => reply(&tools(app).await?),
        "onboarding.dismiss_warning" => {
            app.service.send(Request::DismissPermissionWarning).await?;
            Ok(Value::Null)
        }
        "onboarding.form" => reply(&form(app, parse(params)?).await?.1),
        "onboarding.add" => reply(&add(app, parse(params)?).await?),
        "onboarding.sources" => reply(&sources(app, parse(params)?).await?),
        "onboarding.flow" => reply(&flow(app, parse(params)?).await?),
        "onboarding.flow_dialog" => reply(&flow_dialog(app, parse(params)?).await?),
        "onboarding.at_work" => reply(&at_work(app, parse(params)?).await?),
        _ => Err(unknown(method)),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

fn unexpected(method: &str, other: &Reply) -> RequestError {
    RequestError::plain(format!("unexpected answer to {method}: {other:?}"))
}

/// Whether onboarding opens at start: first run means no project is stored yet (UI-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "OnboardingStart")]
pub struct Start {
    pub show: bool,
}

async fn state(app: &App) -> Result<OnboardingState, RequestError> {
    match app.service.send(Request::OnboardingState).await? {
        Reply::OnboardingState(state) => Ok(state),
        other => Err(unexpected("onboarding.state", &other)),
    }
}

async fn start(app: &App) -> Result<Start, RequestError> {
    Ok(Start {
        show: !state(app).await?.has_projects,
    })
}

async fn texts(app: &App) -> Texts {
    onboarding_page::texts(&super::keymap::labels(app).await)
}

/// Runs the service's tools check and answers the checklist (Re-check asks again).
async fn tools(app: &App) -> Result<Tools, RequestError> {
    let state = state(app).await?;
    let mut model = OnboardingModel::new(state.permission_warning_dismissed);
    model.start_check();
    let result = match app.service.send(Request::ToolsCheck).await {
        Ok(Reply::ToolsCheck(report)) => Ok(report),
        Ok(other) => Err(format!("unexpected answer: {other:?}")),
        Err(e) => Err(e.message),
    };
    model.finish_check(result);
    Ok(onboarding_page::tools(&model, state.has_projects))
}

/// What the Add project form holds: the folder picked (or dropped), and the key and base branch as typed.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(rename = "AddProjectParams")]
pub struct FormParams {
    pub folder: Option<String>,
    pub key: String,
    /// The user typed in the key field: the suggestion no longer replaces it.
    pub key_edited: bool,
    pub base_branch: String,
    /// The user typed in the base-branch field (else the detected one is used).
    pub base_edited: bool,
    /// The onboarding card (else the dialog).
    pub first_run: bool,
}

async fn projects(app: &App) -> Result<Vec<ProjectInfo>, RequestError> {
    match app.service.send(Request::ProjectList).await? {
        Reply::Projects(projects) => Ok(projects),
        other => Err(unexpected("project.list", &other)),
    }
}

/// The form for what the page holds: the folder checked by the service, the typed values on top of the detected
/// ones. Also returns the model and the projects, for Add.
async fn form(
    app: &App,
    params: FormParams,
) -> Result<((AddProjectModel, Vec<ProjectInfo>), AddProjectPage), RequestError> {
    let projects = projects(app).await?;
    let mut model = AddProjectModel {
        trust_tools: super::agent_tools::trust_tools(app).await,
        ..AddProjectModel::default()
    };
    if let Some(folder) = params.folder.clone() {
        model.pick(folder.clone());
        let result = match app
            .service
            .send(Request::ProjectProbe(PathParams { path: folder }))
            .await
        {
            Ok(Reply::FolderProbe(probe)) => Ok(probe),
            Ok(other) => Err((None, format!("unexpected answer: {other:?}"))),
            Err(e) => Err((e.refusal, e.message)),
        };
        model.probed(result, &projects, &super::keymap::labels(app).await);
    }
    if params.key_edited {
        model.set_key(&params.key);
    }
    if params.base_edited {
        model.set_base(&params.base_branch);
    }
    let page = onboarding_page::add_project(&model, &projects, params.first_run);
    Ok(((model, projects), page))
}

/// The answer to `onboarding.add`: the new project's key, or the form with the reason under the key or the form.
/// When the project was added but a tool's trust wasn't recorded, the form says which and why, and the page waits
/// for its Continue before going on as after `Added` (PRJ-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "AddProjectOutcome")]
pub enum AddOutcome {
    Added {
        key: String,
    },
    AddedUntrusted {
        key: String,
        form: Box<AddProjectPage>,
    },
    Refused {
        form: Box<AddProjectPage>,
    },
}

async fn add(app: &App, params: FormParams) -> Result<AddOutcome, RequestError> {
    let first_run = params.first_run;
    let ((mut model, projects), page) = form(app, params).await?;
    let Some(probe) = model.probe.clone().filter(|_| page.can_add) else {
        return Ok(AddOutcome::Refused {
            form: Box::new(page),
        });
    };
    match app
        .service
        .send(Request::ProjectAdd(AddProject {
            path: probe.repo_path,
            key: Some(model.key.clone()),
            base_branch: Some(model.base_branch.clone()),
            // The user's Add project: the project stays in Flow setup until its flow is settled (ASK-04).
            flow_setup: true,
        }))
        .await
    {
        // A service whose tools keep no folder trust answers none: nothing to say.
        Ok(Reply::ProjectAdded {
            project,
            trust: Some(Outcome::Failed { reason }),
        }) => {
            model.added_untrusted(failed_tools(app, &project.key, reason).await);
            Ok(AddOutcome::AddedUntrusted {
                key: project.key,
                form: Box::new(onboarding_page::add_project(&model, &projects, first_run)),
            })
        }
        Ok(Reply::ProjectAdded { project, .. }) => Ok(AddOutcome::Added { key: project.key }),
        Ok(other) => Err(unexpected("project.add", &other)),
        Err(e) => {
            model.add_failed(e.refusal.as_deref(), e.message);
            Ok(AddOutcome::Refused {
                form: Box::new(onboarding_page::add_project(&model, &projects, first_run)),
            })
        }
    }
}

/// `{"project": "SHOP"}`: the project whose Flow go-ahead is shown.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "FlowLookParams")]
pub struct SourcesParams {
    pub project: String,
}

/// The go-ahead's line for the project, from what there is to look at (the service reads no content).
async fn sources(app: &App, params: SourcesParams) -> Result<FlowLook, RequestError> {
    match app
        .service
        .send(Request::OnboardingSources(ProjectParams {
            project: params.project,
        }))
        .await?
    {
        Reply::OnboardingSources(sources) => Ok(flow_sources::view(&sources)),
        other => Err(unexpected("onboarding.sources", &other)),
    }
}

/// The Flow go-ahead's types (T-0427).
#[cfg(test)]
pub fn export_sources_types(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    SourcesParams::export_all(cfg)?;
    FlowLook::export_all(cfg)?;
    Ok(())
}

/// The Flow step for the project (`{"project": "SHOP"}`, the same params as `onboarding.sources`): its go-ahead with
/// the line for what there is, and the presets.
async fn flow(app: &App, params: SourcesParams) -> Result<FlowStep, RequestError> {
    let projects = projects(app).await?;
    let project = projects
        .iter()
        .find(|p| p.key == params.project)
        .ok_or_else(|| RequestError::plain(format!("no such project: {}", params.project)))?;
    let (key, name) = (project.key.clone(), project.name.clone());
    let look = sources(app, params).await?;
    Ok(onboarding_page::flow_step(&key, &name, &look))
}

/// The Add project dialog's go-ahead for the project just added (`{"project": "BILL"}`): "billing-api’s flow", like
/// for the first project added before it (ONB-08).
async fn flow_dialog(app: &App, params: SourcesParams) -> Result<FlowStep, RequestError> {
    let projects = projects(app).await?;
    let project = projects
        .iter()
        .find(|p| p.key == params.project)
        .ok_or_else(|| RequestError::plain(format!("no such project: {}", params.project)))?;
    let (key, name) = (project.key.clone(), project.name.clone());
    let look = sources(app, params).await?;
    Ok(onboarding_page::flow_dialog(&key, &name, &projects, &look))
}

/// The Flow step's types (T-0429): the step and its gallery.
#[cfg(test)]
pub fn export_flow_step_types(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    FlowStep::export_all(cfg)?;
    onboarding_page::FlowGallery::export_all(cfg)?;
    Ok(())
}

/// `{"project": "SHOP", "agent": "a1"}`: the onboarding agent at work; without `agent`, the check before its start.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "FlowAtWorkParams")]
pub struct AtWorkParams {
    pub project: String,
    #[serde(default)]
    #[ts(optional)]
    pub agent: Option<String>,
}

/// The agent at work's card (ONB-05). Without an agent: the not-signed-in card when the Project agent's tool is
/// Found · not signed in (d0086), else `None` (Look and propose a flow starts it). With one: its card now (working,
/// stopped at its tool's limit, or ended), from the project's agents, ended ones included.
async fn at_work(app: &App, params: AtWorkParams) -> Result<Option<flow_at_work::View>, RequestError> {
    let tools = super::pane::tools(app).await;
    let Some(id) = params.agent else {
        let tool = super::pane::project_agent(app)
            .await?
            .and_then(|agent| tools.tool_of(&agent));
        return Ok(tool
            .filter(|t| flow_at_work::not_signed_in(*t, &tools))
            .map(flow_at_work::cannot_start));
    };
    let list = match app
        .service
        .send(Request::ProjectAgentList(ProjectParams {
            project: params.project,
        }))
        .await?
    {
        Reply::ProjectAgents(list) => *list,
        other => return Err(unexpected("project_agent.list", &other)),
    };
    let agent = list
        .open
        .iter()
        .chain(&list.recent)
        .find(|a| a.id == id)
        .ok_or_else(|| RequestError::plain(format!("no such Project agent: {id}")))?;
    Ok(Some(flow_at_work::view(agent, &tools)))
}

/// The agent at work's types (T-0430): its card, params and gallery.
#[cfg(test)]
pub fn export_flow_at_work_types(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    AtWorkParams::export_all(cfg)?;
    flow_at_work::View::export_all(cfg)?;
    flow_at_work::Gallery::export_all(cfg)?;
    Ok(())
}

/// Which tools' trust failed for the project just added, from the service's notices (each tool's reason); the
/// combined reason under the first tool that keeps folder trust when they can't be read.
async fn failed_tools(app: &App, key: &str, reason: String) -> Vec<(Tool, String)> {
    let failed: Vec<(Tool, String)> = state(app)
        .await
        .map(|s| s.trust_failed)
        .unwrap_or_default()
        .into_iter()
        .filter(|f| f.project_key == key)
        .map(|f| (f.tool, f.reason))
        .collect();
    if !failed.is_empty() {
        return failed;
    }
    let tool = super::agent_tools::trust_tools(app)
        .await
        .first()
        .copied()
        .unwrap_or(Tool::ALL[0]);
    vec![(tool, reason)]
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use otaq_core::git::testing::TempRepos;
    use serde_json::json;

    use super::*;
    use crate::api::request;
    use crate::service::testing::{Harness, connected};

    #[tokio::test(flavor = "multi_thread")]
    async fn first_run_adds_a_project_from_the_real_service() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();

        let start = request(&h.app, "onboarding.start", Value::Null)
            .await
            .unwrap();
        assert_eq!(start["show"], true, "no project yet: onboarding opens");
        let texts = request(&h.app, "onboarding.texts", Value::Null)
            .await
            .unwrap();
        assert_eq!(texts["theme"]["title"], "Pick a look");
        // `onboarding.tools` isn't asked here: the tools check runs `claude --version` (the view is tested in
        // otaq-view with made-up reports).

        // A folder that isn't a repository: the frame's text, Add stays off.
        let plain = tempfile::tempdir().unwrap();
        let form = |folder: &Path| {
            json!({ "folder": folder.display().to_string(), "key": "", "key_edited": false,
                    "base_branch": "", "base_edited": false, "first_run": true })
        };
        let refused = request(&h.app, "onboarding.form", form(plain.path()))
            .await
            .unwrap();
        assert_eq!(refused["error"]["title"], "Not a git repository");
        assert_eq!(refused["can_add"], false);

        // A repository: the suggested key and the detected base branch; Add lands it.
        let repo = repos.repo("shop-web", 1);
        let valid = request(&h.app, "onboarding.form", form(&repo))
            .await
            .unwrap();
        assert_eq!(valid["folder_look"], "valid");
        assert_eq!(valid["key"]["value"], "SHOP");
        assert_eq!(valid["can_add"], true);
        let added = request(&h.app, "onboarding.add", form(&repo))
            .await
            .unwrap();
        assert_eq!(added, json!({ "kind": "added", "key": "SHOP" }));

        // The same folder again: already added.
        let again = request(&h.app, "onboarding.form", form(&repo))
            .await
            .unwrap();
        assert_eq!(
            again["error"]["title"],
            format!("shop-web is already in {}", otaq_core::naming::PRODUCT_NAME)
        );
        let start = request(&h.app, "onboarding.start", Value::Null)
            .await
            .unwrap();
        assert_eq!(start["show"], false);
        h.service.stop_all().await;
    }

    /// A service whose agents start as a stand-in (`sh` running `cat`; no agent tool runs) and whose tools PATH holds
    /// only a `claude` that prints a version (Claude Code connected), with shop-web in Flow setup. The last part keeps
    /// the temporary folders.
    async fn flow_world() -> (Harness, TempRepos, tempfile::TempDir) {
        use otaq_core::agent_tools::LaunchContext;
        use otaq_core::agent_tools::claude::ClaudeLaunch;

        let bin = tempfile::tempdir().unwrap();
        let claude = bin.path().join("claude");
        std::fs::write(&claude, "#!/bin/sh\necho '2.1.4 (Claude Code)'\n").unwrap();
        std::fs::set_permissions(&claude, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let path = bin.path().as_os_str().to_owned();
        let h = Harness::configured(move |c| {
            c.tools.path = path.clone();
            c.launch = Some(LaunchContext {
                bin_dir: "/nonexistent".into(),
                plugin_dir: "/nonexistent/plugin".into(),
                path: std::env::var_os("PATH").unwrap_or_default(),
                claude: ClaudeLaunch {
                    program: vec!["sh".into(), "-c".into(), "exec cat".into(), "sh".into()],
                    config_dir: None,
                },
            });
        });
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let repo = repos.repo("shop-web", 1);
        h.app
            .service
            .send(Request::ProjectAdd(AddProject {
                path: repo.display().to_string(),
                key: Some("SHOP".into()),
                base_branch: None,
                flow_setup: true,
            }))
            .await
            .unwrap();
        // The service's check at start finds the stand-in `claude`.
        tokio::time::timeout(std::time::Duration::from_secs(20), async {
            loop {
                let sources = h
                    .app
                    .service
                    .send(Request::OnboardingSources(ProjectParams {
                        project: "SHOP".into(),
                    }))
                    .await;
                if matches!(sources, Ok(Reply::OnboardingSources(s)) if s.any()) {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("Claude Code connected");
        (h, repos, bin)
    }

    async fn at_work(h: &Harness, agent: &str) -> Value {
        request(
            &h.app,
            "onboarding.at_work",
            json!({ "project": "SHOP", "agent": agent }),
        )
        .await
        .unwrap()
    }

    async fn agent_of(h: &Harness, id: &str) -> otaq_core::project_agents::ProjectAgent {
        h.service.agents().0.get(id).await.unwrap()
    }

    /// The agent's hook, as the service handles it: the signals map it, the Project agents apply it.
    async fn hook(h: &Harness, id: &str, event: &str, payload: Value) {
        let (agents, signals, _) = h.service.agents();
        let agent = agents.get(id).await.unwrap();
        let caller = otaq_core::caller::Caller::ProjectAgent {
            project: agent.project.clone(),
            agent: agent.id.clone(),
            session: agent.session.as_ref().unwrap().id.clone(),
        };
        for signal in signals
            .hook(&caller, agent.tool, event, &payload)
            .await
            .unwrap()
        {
            agents.hook(id, &signal).await.unwrap();
        }
    }

    /// T-0430 (ONB-05, d0088): the onboarding agent at work; the plan limit stops it, Try again (its tool's Resume)
    /// continues the same session; Stop and use a preset ends it into Recently ended.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_agent_at_work_stops_at_the_limit_and_try_again_continues_the_same_session() {
        use otaq_core::usage::limit::{LimitHit, LimitKind};

        let (h, _repos, _bin) = flow_world().await;
        // Before the start: its tool isn't Found · not signed in, so Look and propose a flow starts it.
        let check = request(&h.app, "onboarding.at_work", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        assert_eq!(check, Value::Null);
        let started = request(&h.app, "pane.start_flow", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        let id = started["id"].as_str().unwrap().to_owned();
        let session = started["session"].as_str().unwrap().to_owned();

        let card = at_work(&h, &id).await;
        assert_eq!(card["kind"], "working");
        assert_eq!(card["title"], "Reading how you work");
        assert_eq!(card["elapsed"], "Working · 0 min");
        assert_eq!(card["stop"], "Stop and use a preset");

        // The plan limit stops it: the stopped card, Try again off until the reset.
        hook(&h, &id, "UserPromptSubmit", json!({ "prompt": "/flow" })).await;
        let hit = LimitHit {
            kind: LimitKind::Session,
            text: "You've hit your session limit".into(),
        };
        hook(
            &h,
            &id,
            "StopFailure",
            json!({ "error": "rate_limit", "last_assistant_message": hit.text }),
        )
        .await;
        let tool = agent_of(&h, &id).await.tool;
        let now = otaq_core::store::now_ms();
        let resets_at = now + 72 * 60_000;
        h.service
            .agents()
            .2
            .record_hit_reset(tool, &hit, Some(resets_at), now)
            .await
            .unwrap();
        assert!(agent_of(&h, &id).await.limit_stopped);
        let card = at_work(&h, &id).await;
        assert_eq!(card["kind"], "stopped");
        assert_eq!(card["title"], "The agent stopped");
        assert!(
            card["problem"]
                .as_str()
                .unwrap()
                .contains(" limit reached · resets "),
            "{card}"
        );
        assert_eq!(card["skip"], "Skip, use the default");
        assert_eq!(card["use_preset"], "Use a preset instead");
        assert_eq!(card["retry"]["action"]["kind"], "resume_tool");
        let retry = &card["retry"];
        let at = otaq_view::view_models::clock::reset_label(resets_at, now);
        assert_eq!(retry["label"], format!("Try again at {at}"));
        assert_eq!(retry["enabled"], false, "off until the limit resets");

        // Try again (once the limit resets): "continue" goes to the same session, which keeps running.
        request(
            &h.app,
            "limits.resume_tool",
            json!({ "tool": retry["action"]["tool"] }),
        )
        .await
        .unwrap();
        let card = at_work(&h, &id).await;
        assert_eq!((&card["kind"], &card["retry"]["enabled"]), (&json!("stopped"), &json!(true)));
        let typed = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let seen = h
                    .service
                    .sessions()
                    .attach(&session)
                    .map(|s| String::from_utf8_lossy(&s.scrollback).contains("continue"))
                    .unwrap_or(false);
                if seen {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
        .await;
        assert!(typed.is_ok(), "continue typed into the same session");
        hook(&h, &id, "UserPromptSubmit", json!({ "prompt": "continue" })).await;
        let agent = agent_of(&h, &id).await;
        assert_eq!(agent.session.as_ref().unwrap().id, session, "the same session");
        assert_eq!(at_work(&h, &id).await["kind"], "working");

        // Stop and use a preset: the agent ends and goes to Recently ended.
        let ended = request(
            &h.app,
            "pane.end",
            json!({ "project": "SHOP", "id": id, "confirm": true }),
        )
        .await
        .unwrap();
        assert_eq!(ended["outcome"], "done");
        let list = h.service.agents().0.list("SHOP").await.unwrap();
        assert!(list.open.iter().all(|a| a.id != id));
        assert_eq!(list.recent.first().map(|a| a.id.as_str()), Some(id.as_str()));
        h.service.stop_all().await;
    }

    /// T-0430 (ONB-05, d0088): its session ends before a proposal (a crash, or `/exit`): the ended card, and Try
    /// again resumes the same conversation.
    #[tokio::test(flavor = "multi_thread")]
    async fn ending_without_a_proposal_shows_its_card_and_try_again_resumes_the_conversation() {
        let (h, _repos, _bin) = flow_world().await;
        let started = request(&h.app, "pane.start_flow", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        let id = started["id"].as_str().unwrap().to_owned();
        let session = started["session"].as_str().unwrap().to_owned();
        let conversation = agent_of(&h, &id).await.tool_session_id;
        assert!(conversation.is_some());

        h.service.sessions().stop(&session).await.unwrap();
        let ended = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let card = at_work(&h, &id).await;
                if card["kind"] == "ended" {
                    return card;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("the ended card");
        assert_eq!(ended["title"], "The agent ended without a proposal");
        assert_eq!(ended["foot"], "Read-only · the session ended");
        assert_eq!(ended["retry"]["label"], "Try again");
        assert_eq!(
            ended["retry"]["action"],
            json!({ "kind": "resume", "agent": id })
        );

        let resumed = request(&h.app, "pane.resume", json!({ "id": id }))
            .await
            .unwrap();
        assert_eq!(resumed, json!(id));
        let agent = agent_of(&h, &id).await;
        assert_eq!(agent.tool_session_id, conversation, "the same conversation");
        assert_eq!(at_work(&h, &id).await["kind"], "working");
        h.service.stop_all().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_failed_trust_still_adds_the_project_and_says_why() {
        // Claude Code's config file isn't JSON: it is never overwritten, and the trust fails.
        let claude = tempfile::tempdir().unwrap();
        let config =
            otaq_core::agent_tools::claude::trust::ClaudeConfig::locate(Some(claude.path().into()), None)
                .unwrap();
        std::fs::write(&config.file, "not json").unwrap();
        let h = Harness::with_claude(None, Some(config.clone()));
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let repo = repos.repo("pocket-notes", 1);
        let form = json!({ "folder": repo.display().to_string(), "key": "NOTE", "key_edited": true,
                           "base_branch": "", "base_edited": false, "first_run": false });
        let valid = request(&h.app, "onboarding.form", form.clone())
            .await
            .unwrap();
        // No agent tool is connected in the test service (an empty tools PATH), so the line names none; the trust
        // is still recorded, and its failure named per tool.
        assert_eq!(valid["trust_line"], Value::Null);
        assert_eq!(valid["trust_error"], Value::Null);

        let added = request(&h.app, "onboarding.add", form).await.unwrap();
        assert_eq!(added["kind"], "added_untrusted");
        assert_eq!(added["key"], "NOTE");
        assert_eq!(
            added["form"]["trust_error"],
            "Claude Code’s trust couldn’t be recorded: Claude Code's config file couldn't be read. Its agents will \
             wait on Claude Code’s trust screen."
        );
        assert_eq!(added["form"]["continue_label"], "Continue");
        assert_eq!(added["form"]["can_add"], false);
        assert_eq!(
            std::fs::read_to_string(&config.file).unwrap(),
            "not json",
            "left as it was"
        );
        // The board's warning, after the service's trust_changed.
        h.wait_for_event("the failed warning", |e| {
            e["kind"] == "notices" && e["notices"]["banners"][0]["kind"] == "trust_failed"
        })
        .await;
        let view = request(&h.app, "notices.view", Value::Null).await.unwrap();
        assert_eq!(
            view["banners"][0]["title"],
            "Claude Code doesn’t trust NOTE’s folder:"
        );
        assert_eq!(view["banners"][0]["dismissable"], false);
        h.service.stop_all().await;
    }
}
