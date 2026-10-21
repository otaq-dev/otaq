//! `flow_preview.*`: the flow preview over the main area (FLW-04, T-0420; `flow_preview.rs` in `otaq-view` says what
//! it shows). `flow_preview.view { project, choices }` reads the project's open proposal (`flow.proposal`) with what
//! it is compared against, its tickets, the agents and the agent tools, and builds the view with the user's picks;
//! `flow_preview.apply`, `flow_preview.cancel` and `flow_preview.ask` answer it (`flow.apply`,
//! `flow.cancel_proposal`, `flow.ask_for_changes`), each delivered to the Project agent that proposed it.
//! `flow_preview.propose { project, source }` is the way in for Settings → Flow's actions (`flow.propose_from`): the
//! page opens the preview on the project it names. Onboarding's Flow step asks the same view with `onboarding`, for
//! its own buttons (ONB-06: Use the default · Ask for changes · Use this flow). `flow_preview.compare { project,
//! choices, first }` is Flow setup's (ASK-07): every open proposal of the project (`onboarding.flow_setup`), side by
//! side once there are two, or one beside a tool still working or stopped; Use this flow is `flow_preview.apply` with
//! that proposal's number.

use otaq_core::board::tickets::Ticket;
use otaq_core::flow::apply::{ApplyParams, ProposeFrom, ProposeFromParams};
use otaq_core::flow::proposal::Proposal;
use otaq_core::flow::propose::AskForChangesParams;
use otaq_core::flow::{Flow, FlowVersion};
use otaq_core::protocol::{ProjectParams, Reply, Request};
use otaq_view::view_models::flow_preview::{self, Choices, View};
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "flow_preview.view" => reply(&view(app, parse(params)?).await?),
        "flow_preview.compare" => reply(&compare(app, parse(params)?).await?),
        "flow_preview.apply" => reply(&apply(app, parse(params)?).await?),
        "flow_preview.cancel" => {
            let p: ProjectOnly = parse(params)?;
            match app
                .service
                .send(Request::FlowCancelProposal(ProjectParams { project: p.project }))
                .await?
            {
                Reply::Ok => reply(&()),
                other => Err(unexpected("flow.cancel_proposal", &other)),
            }
        }
        "flow_preview.ask" => reply(&ask(app, parse(params)?).await?),
        "flow_preview.propose" => reply(&propose(app, parse(params)?).await?),
        "flow_preview.open" => reply(&open(app).await?),
        _ => Err(unknown(method)),
    }
}

/// Exports the page's types (`generated.rs`' one line for the preview).
#[cfg(test)]
pub fn export_types(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    ViewParams::export_all(cfg)?;
    OnboardingPreview::export_all(cfg)?;
    CompareParams::export_all(cfg)?;
    // Also writes `FlowCompareProposal`.
    flow_preview::CompareView::export_all(cfg)?;
    Load::export_all(cfg)?;
    ApplyRequest::export_all(cfg)?;
    AskRequest::export_all(cfg)?;
    ProposeRequest::export_all(cfg)?;
    Proposed::export_all(cfg)?;
    Ok(())
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(format!("bad parameters: {e}")))
}

fn unexpected(what: &str, other: &Reply) -> RequestError {
    RequestError::plain(format!("unexpected reply to {what}: {other:?}"))
}

#[derive(Debug, Clone, Deserialize)]
struct ProjectOnly {
    project: String,
}

/// `{"project": "API", "choices": {…}}`: the project's preview with the user's picks so far.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "FlowPreviewParams")]
pub struct ViewParams {
    pub project: String,
    #[serde(default)]
    #[ts(optional)]
    pub choices: Option<Choices>,
    /// Shown in onboarding's Flow step (ONB-06).
    #[serde(default)]
    #[ts(optional)]
    pub onboarding: Option<OnboardingPreview>,
}

/// The Flow step's preview: the proposal it showed first, so a later one reads "Updated after your answer".
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(rename = "FlowPreviewOnboarding")]
pub struct OnboardingPreview {
    #[serde(default)]
    pub first: Option<u32>,
}

/// `{"project": "SHOP", "choices": {"3": {…}}, "first": {"a-cc": 2}}`: a project's proposals in Flow setup, with the
/// picks so far by proposal and, by flow agent, the proposal the page showed first (a later one reads "updated after
/// your answer").
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "FlowCompareParams")]
pub struct CompareParams {
    pub project: String,
    #[serde(default)]
    #[ts(optional)]
    pub choices: Option<BTreeMap<u32, Choices>>,
    #[serde(default)]
    #[ts(optional)]
    pub first: Option<BTreeMap<String, u32>>,
}

/// What `flow_preview.view` answers: the preview, or none once the proposal is gone (applied, cancelled or replaced
/// from elsewhere): the page closes it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowPreviewLoad")]
pub struct Load {
    pub view: Option<View>,
}

/// Apply: the proposal it answers and the picks.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "FlowPreviewApplyParams")]
pub struct ApplyRequest {
    pub project: String,
    pub proposal: u32,
    #[serde(default)]
    pub choices: Choices,
}

/// Ask for more changes: the line for the agent.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "FlowPreviewAskParams")]
pub struct AskRequest {
    pub project: String,
    pub proposal: u32,
    pub text: String,
}

/// Where a proposal of the user's comes from (Settings → Flow's actions; `flow.propose_from`'s `ProposeFrom`).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(tag = "from", rename_all = "snake_case")]
#[ts(rename = "FlowPreviewSource")]
pub enum Source {
    /// Back to the previous flow (or an earlier `version`).
    Previous {
        #[serde(default)]
        #[ts(optional)]
        version: Option<u32>,
    },
    /// Copy from another project…
    Project { project: String },
    /// Use a preset…, Reset to the default flow (`otaq-default.yaml`).
    Preset { file: String },
    /// Import flow…: the file's name and text.
    File { name: String, yaml: String },
    /// Apply the shipped agents file again.
    AgentsFile,
}

impl From<Source> for ProposeFrom {
    fn from(source: Source) -> Self {
        match source {
            Source::Previous { version } => Self::Previous { version },
            Source::Project { project } => Self::Project { project },
            Source::Preset { file } => Self::Preset { file },
            Source::File { name, yaml } => Self::File { name, yaml },
            Source::AgentsFile => Self::AgentsFile,
        }
    }
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "FlowPreviewProposeParams")]
pub struct ProposeRequest {
    pub project: String,
    pub source: Source,
}

/// The proposal opened: the page opens the preview on its project.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowPreviewProposed")]
pub struct Proposed {
    pub project: String,
    pub proposal: u32,
}

async fn proposal(app: &App, project: &str) -> Result<Option<Proposal>, RequestError> {
    match app
        .service
        .send(Request::FlowProposal(ProjectParams {
            project: project.to_owned(),
        }))
        .await
    {
        Ok(Reply::FlowProposal(p)) => Ok(p.map(|p| *p)),
        Ok(other) => Err(unexpected("flow.proposal", &other)),
        Err(e) if e.not_found => Ok(None),
        Err(e) => Err(e),
    }
}

async fn current(app: &App, project: &str) -> Option<FlowVersion> {
    match app
        .service
        .send(Request::FlowCurrent(ProjectParams {
            project: project.to_owned(),
        }))
        .await
    {
        Ok(Reply::FlowVersion(v)) => Some(*v),
        _ => None,
    }
}

/// The flow a proposal is compared with (FLW-06): the waiting change's, else the current one.
async fn base(app: &App, project: &str, current: Option<&FlowVersion>) -> Flow {
    let waiting = match app
        .service
        .send(Request::FlowWaiting(ProjectParams {
            project: project.to_owned(),
        }))
        .await
    {
        Ok(Reply::FlowWaiting(Some(w))) => otaq_core::flow::yaml::read(&w.yaml).ok(),
        _ => None,
    };
    waiting
        .or_else(|| current.and_then(|v| v.file().ok()))
        .map_or_else(otaq_core::flow::default_flow, |f| f.flow)
}

async fn tickets(app: &App, project: &str) -> Result<Vec<Ticket>, RequestError> {
    match app
        .service
        .send(Request::TicketList(ProjectParams {
            project: project.to_owned(),
        }))
        .await?
    {
        Reply::Tickets(list) => Ok(list),
        other => Err(unexpected("ticket.list", &other)),
    }
}

pub async fn view(app: &App, params: ViewParams) -> Result<Load, RequestError> {
    let Some(proposal) = proposal(app, &params.project).await? else {
        return Ok(Load { view: None });
    };
    let current = current(app, &proposal.project).await;
    let base = base(app, &proposal.project, current.as_ref()).await;
    let tickets = tickets(app, &proposal.project).await?;
    let agents = super::settings::agent_list(app).await?;
    let tools = super::settings::agent_tools(app, false).await?;
    let choices = params.choices.unwrap_or_default();
    let view = flow_preview::view(&flow_preview::Facts {
            proposal: &proposal,
            current: current.as_ref(),
            base: &base,
            tickets: &tickets,
            agents: &agents,
            tools: &tools,
            choices: &choices,
            now: otaq_core::store::now_ms(),
        });
    Ok(Load {
        view: Some(match params.onboarding {
            Some(o) => flow_preview::onboarding(view, &proposal, o.first),
            None => view,
        }),
    })
}

/// Flow setup's proposals (ASK-07), each built as [`view`] builds one.
pub async fn compare(app: &App, params: CompareParams) -> Result<flow_preview::CompareView, RequestError> {
    let setup = match app
        .service
        .send(Request::OnboardingFlowSetup(ProjectParams {
            project: params.project.clone(),
        }))
        .await?
    {
        Reply::FlowSetup(setup) => *setup,
        other => return Err(unexpected("onboarding.flow_setup", &other)),
    };
    let current = current(app, &setup.project).await;
    let base = base(app, &setup.project, current.as_ref()).await;
    let tickets = tickets(app, &setup.project).await?;
    let agents = super::settings::agent_list(app).await?;
    let tools = super::settings::agent_tools(app, false).await?;
    let picks = params.choices.unwrap_or_default();
    let none = Choices::default();
    let now = otaq_core::store::now_ms();
    Ok(flow_preview::compare(
        &setup,
        |proposal| {
            flow_preview::view(&flow_preview::Facts {
                proposal,
                current: current.as_ref(),
                base: &base,
                tickets: &tickets,
                agents: &agents,
                tools: &tools,
                choices: picks.get(&proposal.id).unwrap_or(&none),
                now,
            })
        },
        &params.first.unwrap_or_default(),
    ))
}

/// Apply: the service checks the proposal again with the picks, and refuses with why (shown in the preview).
async fn apply(app: &App, p: ApplyRequest) -> Result<Proposed, RequestError> {
    match app
        .service
        .send(Request::FlowApply(ApplyParams {
            project: p.project.clone(),
            proposal: p.proposal,
            choices: p.choices.apply_choices(),
        }))
        .await?
    {
        Reply::FlowApplied(applied) => Ok(Proposed {
            project: p.project,
            proposal: applied.proposal,
        }),
        other => Err(unexpected("flow.apply", &other)),
    }
}

/// Ask for more changes (`flow.ask_for_changes`): the proposal closes and the line goes to the Project agent.
async fn ask(app: &App, p: AskRequest) -> Result<(), RequestError> {
    let text = p.text.trim();
    if text.is_empty() {
        return Err(RequestError::plain("Say what the agent should change"));
    }
    match app
        .service
        .send(Request::FlowAskForChanges(AskForChangesParams {
            project: p.project,
            proposal: p.proposal,
            text: text.to_owned(),
        }))
        .await?
    {
        Reply::Ok => Ok(()),
        other => Err(unexpected("flow.ask_for_changes", &other)),
    }
}

/// The projects with an open proposal, newest first: the preview opens on the first when the app starts or resyncs,
/// so a proposal made while it was closed isn't missed.
async fn open(app: &App) -> Result<Vec<String>, RequestError> {
    let projects = match app.service.send(Request::ProjectList).await? {
        Reply::Projects(list) => list,
        other => return Err(unexpected("project.list", &other)),
    };
    let mut open = Vec::new();
    for project in projects {
        if let Some(p) = proposal(app, &project.key).await? {
            open.push((p.created_at, project.key));
        }
    }
    open.sort_by_key(|o| std::cmp::Reverse(o.0));
    Ok(open.into_iter().map(|(_, key)| key).collect())
}

async fn propose(app: &App, p: ProposeRequest) -> Result<Proposed, RequestError> {
    match app
        .service
        .send(Request::FlowProposeFrom(ProposeFromParams {
            project: p.project.clone(),
            source: p.source.into(),
        }))
        .await?
    {
        Reply::FlowProposal(Some(proposal)) => Ok(Proposed {
            project: proposal.project.clone(),
            proposal: proposal.id,
        }),
        other => Err(unexpected("flow.propose_from", &other)),
    }
}

#[cfg(test)]
mod tests {
    use otaq_core::board::projects::AddProject;
    use otaq_core::git::testing::TempRepos;
    use serde_json::json;

    use super::*;
    use crate::api::request;
    use crate::service::testing::{Harness, connected};

    async fn shop_web(h: &Harness, repos: &TempRepos) {
        h.wait_for("connected", connected).await;
        h.app
            .service
            .send(Request::ProjectAdd(AddProject {
                path: repos.repo("shop-web", 1).display().to_string(),
                ..Default::default()
            }))
            .await
            .unwrap();
    }

    fn view_of(load: &Value) -> &Value {
        &load["view"]
    }

    /// A preset proposed from Settings → Flow opens a whole-flow preview; Cancel drops it and the view goes.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_preset_opens_the_preview_and_cancel_drops_it() {
        let h = Harness::new(None);
        let repos = TempRepos::new();
        shop_web(&h, &repos).await;
        let proposed = request(
            &h.app,
            "flow_preview.propose",
            json!({ "project": "SHOP", "source": { "from": "preset", "file": "solo-autopilot.yaml" } }),
        )
        .await
        .unwrap();
        assert_eq!(proposed["project"], "SHOP");
        let load = request(&h.app, "flow_preview.view", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        let view = view_of(&load);
        assert_eq!(view["title"], "Solo autopilot for shop-web");
        assert_eq!(view["subtitle"], "Preset \u{b7} nothing changes until you apply it");
        assert!(view["table"]["rows"].as_array().is_some_and(|r| !r.is_empty()));
        assert!(view["ask"].is_null(), "no agent to ask");
        assert_eq!(view["cancel"], "Cancel");
        let open = request(&h.app, "flow_preview.open", Value::Null).await.unwrap();
        assert_eq!(open, json!(["SHOP"]));

        request(&h.app, "flow_preview.cancel", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        let load = request(&h.app, "flow_preview.view", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        assert!(view_of(&load).is_null(), "the preview closes");
        h.service.stop_all().await;
    }

    /// The board's column titles.
    async fn columns(h: &Harness) -> Vec<String> {
        let params = json!({ "project": "SHOP", "width": 1376.0, "height": 856.0,
                             "done_collapsed": null, "dev_unfolded": false });
        let view = request(&h.app, "board.view", params).await.unwrap();
        view["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["title"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    /// Onboarding's Flow step (ONB-04, ONB-06): a preset picked in the step opens the preview with the step's buttons;
    /// Use this flow applies it at once on the empty board, which then shows that flow's columns.
    #[tokio::test(flavor = "multi_thread")]
    async fn onboarding_a_preset_then_use_this_flow_shows_its_columns() {
        let h = Harness::new(None);
        let repos = TempRepos::new();
        shop_web(&h, &repos).await;
        assert_eq!(
            columns(&h).await,
            ["Backlog", "Ready for Dev", "Dev", "Validate", "Review", "Done"],
            "the default flow"
        );
        let proposed = request(
            &h.app,
            "flow_preview.propose",
            json!({ "project": "SHOP", "source": { "from": "preset", "file": "grill-first.yaml" } }),
        )
        .await
        .unwrap();
        let load = request(
            &h.app,
            "flow_preview.view",
            json!({ "project": "SHOP", "onboarding": { "first": proposed["proposal"] } }),
        )
        .await
        .unwrap();
        let view = view_of(&load);
        assert_eq!(view["title"], "Grill first for shop-web");
        assert_eq!(view["cancel"], "Use the default");
        assert_eq!(view["apply"]["label"], "Use this flow");
        assert!(view["ask"].is_null(), "a preset is the user's: no agent to ask");
        assert_eq!(view["applies"], "Applies now: shop-web has no tickets yet.");
        request(
            &h.app,
            "flow_preview.apply",
            json!({ "project": "SHOP", "proposal": view["proposal"], "choices": view["choices"] }),
        )
        .await
        .unwrap();
        assert_eq!(
            columns(&h).await,
            ["Backlog", "Ready for Dev", "Grill", "Dev", "Validate", "Review", "Done"]
        );
        h.service.stop_all().await;
    }

    /// Onboarding's Use the default cancels the proposal: the board keeps the default flow.
    #[tokio::test(flavor = "multi_thread")]
    async fn onboarding_use_the_default_cancels_the_proposal() {
        let h = Harness::new(None);
        let repos = TempRepos::new();
        shop_web(&h, &repos).await;
        request(
            &h.app,
            "flow_preview.propose",
            json!({ "project": "SHOP", "source": { "from": "preset", "file": "solo-autopilot.yaml" } }),
        )
        .await
        .unwrap();
        request(&h.app, "flow_preview.cancel", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        let load = request(
            &h.app,
            "flow_preview.view",
            json!({ "project": "SHOP", "onboarding": {} }),
        )
        .await
        .unwrap();
        assert!(view_of(&load).is_null(), "the proposal is gone");
        assert_eq!(
            columns(&h).await,
            ["Backlog", "Ready for Dev", "Dev", "Validate", "Review", "Done"]
        );
        h.service.stop_all().await;
    }

    /// Apply sends the proposal and the picks; the preset is the project's flow after it.
    #[tokio::test(flavor = "multi_thread")]
    async fn apply_applies_the_open_proposal() {
        let h = Harness::new(None);
        let repos = TempRepos::new();
        shop_web(&h, &repos).await;
        let proposed = request(
            &h.app,
            "flow_preview.propose",
            json!({ "project": "SHOP", "source": { "from": "preset", "file": "grill-first.yaml" } }),
        )
        .await
        .unwrap();
        let load = request(&h.app, "flow_preview.view", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        let view = view_of(&load);
        assert_eq!(view["apply"]["enabled"], true, "{view}");
        // An answer to a proposal that was replaced meanwhile is refused.
        assert!(
            request(
                &h.app,
                "flow_preview.apply",
                json!({ "project": "SHOP", "proposal": 999, "choices": view["choices"] }),
            )
            .await
            .is_err()
        );
        request(
            &h.app,
            "flow_preview.apply",
            json!({ "project": "SHOP", "proposal": proposed["proposal"], "choices": view["choices"] }),
        )
        .await
        .unwrap();
        let Reply::FlowVersion(current) = h
            .app
            .service
            .send(Request::FlowCurrent(ProjectParams {
                project: "SHOP".into(),
            }))
            .await
            .unwrap()
        else {
            panic!("a version")
        };
        assert_eq!(current.preset.as_deref(), Some("grill-first.yaml"));
        let load = request(&h.app, "flow_preview.view", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        assert!(view_of(&load).is_null());
        h.service.stop_all().await;
    }

    /// Ask for more changes needs a line, and goes to `flow.ask_for_changes`, which keeps it for a Project agent's
    /// proposal: the user's own (a preset) is refused, and stays open.
    #[tokio::test(flavor = "multi_thread")]
    async fn ask_needs_a_line_and_a_project_agent() {
        let h = Harness::new(None);
        let repos = TempRepos::new();
        shop_web(&h, &repos).await;
        let err = request(
            &h.app,
            "flow_preview.ask",
            json!({ "project": "SHOP", "proposal": 1, "text": "  " }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.message, "Say what the agent should change");
        let proposed = request(
            &h.app,
            "flow_preview.propose",
            json!({ "project": "SHOP", "source": { "from": "preset", "file": "team-pr.yaml" } }),
        )
        .await
        .unwrap();
        assert!(
            request(
                &h.app,
                "flow_preview.ask",
                json!({ "project": "SHOP", "proposal": proposed["proposal"], "text": "skip Review" }),
            )
            .await
            .is_err()
        );
        let load = request(&h.app, "flow_preview.view", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        assert!(!view_of(&load).is_null(), "still open");
        h.service.stop_all().await;
    }

    /// Flow setup's proposals (ASK-07): shop-web in Flow setup with a preset looked at shows it alone, with the step's
    /// buttons; Use this flow is `flow_preview.apply` with its number, which settles Flow setup.
    #[tokio::test(flavor = "multi_thread")]
    async fn compare_lists_flow_setups_proposals_and_apply_settles_it() {
        let h = Harness::new(None);
        let repos = TempRepos::new();
        h.wait_for("connected", connected).await;
        h.app
            .service
            .send(Request::ProjectAdd(AddProject {
                path: repos.repo("shop-web", 1).display().to_string(),
                key: Some("SHOP".into()),
                flow_setup: true,
                ..Default::default()
            }))
            .await
            .unwrap();
        let empty = request(&h.app, "flow_preview.compare", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        assert_eq!(empty["compare"], false);
        assert_eq!(empty["proposals"], json!([]));
        let proposed = request(
            &h.app,
            "flow_preview.propose",
            json!({ "project": "SHOP", "source": { "from": "preset", "file": "grill-first.yaml" } }),
        )
        .await
        .unwrap();
        let shown = request(&h.app, "flow_preview.compare", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        assert_eq!(shown["compare"], false);
        assert!(shown["closes_others"].is_null());
        let one = &shown["proposals"][0];
        assert!(one["tool"].is_null() && one["agent"].is_null(), "the user's own");
        assert_eq!(one["view"]["proposal"], proposed["proposal"]);
        assert_eq!(one["view"]["title"], "Grill first for shop-web");
        assert_eq!(one["view"]["cancel"], "Use the default");
        assert_eq!(one["view"]["apply"]["label"], "Use this flow");
        request(
            &h.app,
            "flow_preview.apply",
            json!({ "project": "SHOP", "proposal": proposed["proposal"], "choices": one["view"]["choices"] }),
        )
        .await
        .unwrap();
        assert_eq!(
            columns(&h).await,
            ["Backlog", "Ready for Dev", "Grill", "Dev", "Validate", "Review", "Done"]
        );
        let after = request(&h.app, "flow_preview.compare", json!({ "project": "SHOP" }))
            .await
            .unwrap();
        assert_eq!(after["proposals"], json!([]));
        h.service.stop_all().await;
    }
}
