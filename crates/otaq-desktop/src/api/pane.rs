//! `pane.*`: the Project agent pane (SHL-04, PAG-01, PAG-04, PAG-05, PAG-06; d0087, d0068, d0018; *18 · Project
//! agents & terminals → Row · Pane: empty, hidden, tool problems, menu (0087)*). `pane.view` answers everything the
//! pane draws for a project and the agent it shows (`agent_pane.rs`); the others are its clicks: Send (the first
//! message starts the agent, d0087), Rename, End agent (asks first mid-turn), Close ended agents and Resume. Which
//! agent each project shows, + and the empty pane are the page's own state: nothing here starts without a message
//! or a click. The empty pane names the Project agent role's agent (T-0342, AGM-01), which [`project_agent`] finds
//! for Ask the agent too.

use otaq_core::agents::{Agent, Role};
use otaq_core::project_agents::{CreatedAndChanged, ProjectAgent, ProjectAgentList};
use otaq_core::protocol::{
    IdParams, KeyParams, ProjectAgentCloseParams, ProjectAgentOpenParams, ProjectAgentRenameParams,
    ProjectParams, Reply, Request,
};
use otaq_view::view_models::agent_pane::{self as vm, EditedTicket, PaneView};
use otaq_view::view_models::project_agent_page::{self as agent_page, CloseDialog};
use otaq_view::view_models::toasts::{NewToast, ToastLevel};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "pane.view" => reply(&view(app, parse(params)?).await?),
        "pane.send" => reply(&send(app, parse(params)?).await?),
        "pane.rename" => reply(&rename(app, parse(params)?).await?),
        "pane.end" => reply(&end(app, parse(params)?).await?),
        "pane.close_ended" => reply(&close_ended(app, parse(params)?).await?),
        "pane.resume" => reply(&resume(app, parse(params)?).await?),
        "pane.start_flow" => reply(&start_flow(app, parse(params)?).await?),
        _ => Err(unknown(method)),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

fn unexpected(what: &str, other: &Reply) -> RequestError {
    RequestError::plain(format!("unexpected answer to {what}: {other:?}"))
}

fn toast_error(app: &App, title: &str, error: &RequestError) {
    app.notices.toast(NewToast::new(
        ToastLevel::Error,
        title,
        Some(error.message.clone()),
    ));
}

async fn agents(app: &App, project: &str) -> Result<ProjectAgentList, RequestError> {
    match app
        .service
        .send(Request::ProjectAgentList(ProjectParams {
            project: project.to_owned(),
        }))
        .await?
    {
        Reply::ProjectAgents(list) => Ok(*list),
        other => Err(unexpected("project_agent.list", &other)),
    }
}

async fn created_of(app: &App, id: &str) -> Result<CreatedAndChanged, RequestError> {
    match app
        .service
        .send(Request::ProjectAgentCreated(IdParams { id: id.to_owned() }))
        .await?
    {
        Reply::CreatedAndChanged(c) => Ok(*c),
        other => Err(unexpected("project_agent.created", &other)),
    }
}

/// The tickets it edited, with their titles and places (one that is gone is left out).
async fn edited_of(app: &App, created: &CreatedAndChanged) -> Vec<EditedTicket> {
    let mut edited = Vec::new();
    for key in &created.edited {
        if let Ok(Reply::Ticket(t)) = app
            .service
            .send(Request::TicketGet(KeyParams {
                key: key.to_string(),
            }))
            .await
        {
            edited.push(EditedTicket {
                key: t.key,
                title: t.title,
                status: t.status,
            });
        }
    }
    edited
}

/// The Project agent role's agent (AGM-01): the one every Project agent starts on. `None` before it exists.
pub(super) async fn project_agent(app: &App) -> Result<Option<Agent>, RequestError> {
    Ok(project_agent_and_list(app).await?.0)
}

/// The agent tools as the pane reads them ([`super::editor::agent_tools`]).
pub(super) async fn tools(app: &App) -> otaq_view::view_models::tool_problems::Tools {
    super::editor::agent_tools(app).await
}

/// [`project_agent`] and every agent.
async fn project_agent_and_list(app: &App) -> Result<(Option<Agent>, Vec<Agent>), RequestError> {
    let list = match app.service.send(Request::AgentsList).await? {
        Reply::Agents(list) => *list,
        other => return Err(unexpected("agents.list", &other)),
    };
    let default = list
        .agents
        .iter()
        .find(|a| a.role == Some(Role::ProjectAgent))
        .cloned();
    Ok((default, list.agents))
}

async fn project_name(app: &App, project: &str) -> Result<String, RequestError> {
    match app.service.send(Request::ProjectList).await? {
        Reply::Projects(list) => Ok(list
            .into_iter()
            .find(|p| p.key == project)
            .map_or_else(|| project.to_owned(), |p| p.name)),
        other => Err(unexpected("project.list", &other)),
    }
}

/// What `pane.view` takes: the project, the agent the page shows (none for the empty pane) and the window's width
/// (Created and changed folds to one line at the 1280 class).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PaneParams")]
pub struct Params {
    pub project: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub window_width: Option<f32>,
    /// The empty pane after "New agent on …" (M23): the named agent its first message starts.
    #[serde(default)]
    #[ts(optional)]
    pub start_on: Option<String>,
}

pub async fn view(app: &App, params: Params) -> Result<PaneView, RequestError> {
    let agents = agents(app, &params.project).await?;
    let name = project_name(app, &params.project).await?;
    let (new_agent, named) = project_agent_and_list(app).await?;
    let start_on = params
        .start_on
        .as_deref()
        .and_then(|id| named.iter().find(|a| a.id == id));
    let tools = super::editor::agent_tools(app).await;
    let shown = params
        .agent
        .as_deref()
        .filter(|id| agents.open.iter().any(|a| a.id == *id));
    let (created, edited) = match shown {
        Some(id) => {
            let created = created_of(app, id).await?;
            let edited = edited_of(app, &created).await;
            (Some(created), edited)
        }
        None => (None, Vec::new()),
    };
    Ok(vm::view(&vm::Facts {
        agents: &agents,
        shown,
        created: created.as_ref(),
        edited: &edited,
        project_name: &name,
        new_agent: new_agent.as_ref(),
        window_width: params.window_width,
        now_ms: otaq_core::store::now_ms(),
        keys: super::keymap::labels(app).await,
        tools: &tools,
        agent_list: &named,
        start_on,
        project: &params.project,
    }))
}

/// What `pane.send` takes: the empty pane's first message.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PaneSendParams")]
pub struct SendParams {
    pub project: String,
    pub message: String,
    /// The empty pane's `starts_on` after "New agent on …" (M23); `None` for the Project agent default.
    #[serde(default)]
    #[ts(optional)]
    pub agent: Option<String>,
}

/// Send in the empty pane: the message is the go-ahead, and the new agent starts with it as its first prompt
/// (`project_agent.open`, d0087). The new agent's id, which the pane shows next.
pub async fn send(app: &App, params: SendParams) -> Result<String, RequestError> {
    match app
        .service
        .send(Request::ProjectAgentOpen(ProjectAgentOpenParams {
            project: params.project,
            message: params.message,
            agent: params.agent,
        }))
        .await
    {
        Ok(Reply::ProjectAgent(agent)) => Ok(agent.id),
        Ok(other) => Err(unexpected("project_agent.open", &other)),
        Err(e) => {
            toast_error(app, "The Project agent didn\u{2019}t start", &e);
            Err(e)
        }
    }
}

/// What `pane.rename` takes: the agent and the user's name for it (which always wins, PAG-04).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PaneRenameParams")]
pub struct RenameParams {
    pub id: String,
    pub name: String,
}

/// An empty name changes nothing.
pub async fn rename(app: &App, params: RenameParams) -> Result<(), RequestError> {
    let name = params.name.trim().to_owned();
    if name.is_empty() {
        return Ok(());
    }
    if let Err(e) = app
        .service
        .send(Request::ProjectAgentRename(ProjectAgentRenameParams {
            id: params.id,
            name,
        }))
        .await
    {
        toast_error(app, "The Project agent wasn\u{2019}t renamed", &e);
        return Err(e);
    }
    Ok(())
}

/// What `pane.end` takes: the shown agent, and `confirm` after End agent's question.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PaneEndParams")]
pub struct EndParams {
    pub project: String,
    pub id: String,
    #[serde(default)]
    pub confirm: bool,
}

/// How End agent went: done (it moves to Recently ended), or it is mid-turn and the user is asked first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "outcome", rename_all = "snake_case")]
#[ts(rename = "PaneEnded")]
pub enum Ended {
    Done,
    /// *Close an agent mid-turn · confirm*: "Stop and close".
    Ask { dialog: CloseDialog },
}

async fn find_agent(app: &App, project: &str, id: &str) -> Result<ProjectAgent, RequestError> {
    let list = agents(app, project).await?;
    list.open
        .into_iter()
        .chain(list.recent)
        .find(|a| a.id == id)
        .ok_or_else(|| RequestError::plain(format!("no such Project agent: {id}")))
}

pub async fn end(app: &App, params: EndParams) -> Result<Ended, RequestError> {
    match app
        .service
        .send(Request::ProjectAgentClose(ProjectAgentCloseParams {
            id: params.id.clone(),
            confirm: params.confirm,
        }))
        .await
    {
        Ok(_) => Ok(Ended::Done),
        Err(e) if e.refusal.as_deref() == Some("needs_confirmation") => {
            let agent = find_agent(app, &params.project, &params.id).await?;
            Ok(Ended::Ask {
                dialog: agent_page::close_dialog(&agent, &created_of(app, &params.id).await?),
            })
        }
        Err(e) => {
            toast_error(app, "The Project agent didn\u{2019}t end", &e);
            Err(e)
        }
    }
}

/// What `pane.close_ended` takes: the shown project.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PaneProjectParams")]
pub struct ProjectOnly {
    pub project: String,
}

/// … → Close ended agents: they leave the switcher and stay in Recently ended.
pub async fn close_ended(app: &App, params: ProjectOnly) -> Result<(), RequestError> {
    match app
        .service
        .send(Request::ProjectAgentCloseEnded(ProjectParams {
            project: params.project,
        }))
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => {
            toast_error(app, "The ended agents didn\u{2019}t close", &e);
            Err(e)
        }
    }
}

/// What `pane.resume` takes: an ended Project agent (its footer, or Recently ended).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PaneResumeParams")]
pub struct ResumeParams {
    pub id: String,
}

/// Resume (the click is the go-ahead, d0018): its conversation continues. The agent's id, which the pane shows.
pub async fn resume(app: &App, params: ResumeParams) -> Result<String, RequestError> {
    match app
        .service
        .send(Request::ProjectAgentResume(IdParams { id: params.id }))
        .await
    {
        Ok(Reply::ProjectAgent(agent)) => Ok(agent.id),
        Ok(other) => Err(unexpected("project_agent.resume", &other)),
        Err(e) => {
            toast_error(app, "The Project agent didn\u{2019}t resume", &e);
            Err(e)
        }
    }
}

/// What `pane.start_flow` answers: the onboarding agent and the session its terminal attaches to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PaneFlowStarted")]
pub struct FlowStarted {
    /// The Project agent's id.
    pub id: String,
    /// Its session (`<Terminal session …>`); `None` only if it ended at once.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub session: Option<String>,
}

/// **Look and propose a flow** (`onboarding.ask` since M35): asks the first connected tool for the project's flow, or
/// returns its running flow agent. A refusal (no tool connected, the tool at its limit, the project's flow already
/// set up) comes back as the error, with no toast: the Flow step shows it. A thin stand-in until T-0434 asks each
/// tool from its own row (ASK-05).
pub async fn start_flow(app: &App, params: ProjectOnly) -> Result<FlowStarted, RequestError> {
    let project = ProjectParams {
        project: params.project,
    };
    let tool = match app
        .service
        .send(Request::OnboardingSources(project.clone()))
        .await?
    {
        Reply::OnboardingSources(sources) => sources.tools.first().copied(),
        other => return Err(unexpected("onboarding.sources", &other)),
    };
    let Some(tool) = tool else {
        return Err(RequestError::plain("No agent tool is connected"));
    };
    match app
        .service
        .send(Request::OnboardingAsk(otaq_core::protocol::OnboardingAskParams {
            project: project.project,
            tool,
        }))
        .await?
    {
        Reply::ProjectAgent(agent) => Ok(FlowStarted {
            session: agent.session.as_ref().map(|s| s.id.clone()),
            id: agent.id,
        }),
        other => Err(unexpected("onboarding.ask", &other)),
    }
}

#[cfg(test)]
mod tests {
    use otaq_core::board::projects::AddProject;
    use otaq_core::git::testing::TempRepos;
    use serde_json::json;

    use super::*;
    use crate::api::request as table;
    use crate::service::testing::{Harness, connected};

    #[tokio::test(flavor = "multi_thread")]
    async fn the_empty_pane_starts_nothing_and_send_needs_words() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let path = repos.repo("pocket-notes", 1);
        let project = match h
            .app
            .service
            .send(Request::ProjectAdd(AddProject {
                path: path.display().to_string(),
                key: None,
                base_branch: None,
                flow_setup: false,
            }))
            .await
            .unwrap()
        {
            Reply::ProjectAdded { project: p, .. } => p.key,
            other => panic!("{other:?}"),
        };

        let view = table(
            &h.app,
            "pane.view",
            json!({ "project": project, "window_width": 1280 }),
        )
        .await
        .unwrap();
        assert_eq!(view["agent"], Value::Null);
        assert_eq!(view["opens_on"], Value::Null);
        assert_eq!(view["empty"]["title"], "Ask the Project agent");
        assert_eq!(view["empty"]["hint"], "Enter starts Claude Code");
        assert_eq!(view["empty"]["line"]["agent"], "Claude Code");
        assert!(
            view["empty"]["text"]
                .as_str()
                .unwrap()
                .contains("in pocket-notes (never edits)")
        );
        assert_eq!(view["switcher"]["rows"], json!([]));
        assert_eq!(view["agent_waits"], false);
        // An agent the page remembers but the project doesn't have: the empty pane.
        let gone = table(
            &h.app,
            "pane.view",
            json!({ "project": project, "agent": "gone" }),
        )
        .await
        .unwrap();
        assert_eq!(gone["agent"], Value::Null);

        // The Project agent role moved to Strong: the empty pane says so.
        let Reply::Agent(strong) = h
            .app
            .service
            .send(Request::AgentsAdd(otaq_core::agents::AgentFields {
                name: "Strong".into(),
                instructions: String::new(),
                skills: Vec::new(),
                model: otaq_core::agents::AgentModel::Tool {
                    tool: otaq_core::agent_tools::Tool::ALL[0],
                    model: None,
                },
                effort: None,
            }))
            .await
            .unwrap()
        else {
            panic!("no agent");
        };
        h.app
            .service
            .send(Request::AgentsMoveRole(otaq_core::protocol::MoveRoleParams {
                role: Role::ProjectAgent,
                agent: strong.id.clone(),
            }))
            .await
            .unwrap();
        let view = table(&h.app, "pane.view", json!({ "project": project }))
            .await
            .unwrap();
        assert_eq!(view["empty"]["line"]["agent"], "Strong");
        assert_eq!(view["empty"]["line"]["note"], "\u{b7} starts when you send");
        assert_eq!(view["empty"]["hint"], "Enter starts Strong");

        // Send without words is refused; nothing was started.
        assert!(
            table(
                &h.app,
                "pane.send",
                json!({ "project": project, "message": "  " })
            )
            .await
            .is_err()
        );
        let list = agents(&h.app, &project).await.unwrap();
        assert!(list.open.is_empty() && list.recent.is_empty());

        // Close ended agents with none is a no-op; an empty rename changes nothing.
        table(&h.app, "pane.close_ended", json!({ "project": project }))
            .await
            .unwrap();
        table(&h.app, "pane.rename", json!({ "id": "x", "name": " " }))
            .await
            .unwrap();
        h.service.stop_all().await;
    }
}
