//! `drawer.*`: the Terminals & agents drawer and its header button (UI-09, UI-01, RUN-06, PAG-01; d0068, d0058, d0019;
//! *02 → Terminals & agents button*, *17 · Project terminals*, *18 → + menu*). The drawer shows one tab per Project agent
//! and per Project terminal of the shown project, + New (New terminal / New Project agent) and Recently ended. The
//! last-used tab of each project is kept here, so ⇧⌘T finds the last-used terminal (or opens one).

use crewboard_core::project_agents::{ProjectAgent, ProjectAgentList};
use crewboard_core::protocol::{
    IdParams, ProjectAgentCloseParams, ProjectParams, ProjectTerminalParams, Reply, Request,
    SessionIdParams,
};
use crewboard_core::terminals::shell::ProjectTerminal;
use crewboard_view::keymap::{KeyAction, Platform};
use crewboard_view::shell::tilde;
use crewboard_view::view_models::menu::MenuItem;
use crewboard_view::view_models::project_terminal::{
    self as vm, LastUsed, NEW_AGENT, NEW_TERMINAL, Tab, agent_tab_id, terminal_tab_id,
};
use crewboard_view::view_models::toasts::{NewToast, ToastLevel};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "drawer.button" => reply(&button(app, parse(params)?).await?),
        "drawer.view" => reply(&view(app, parse(params)?).await?),
        "drawer.select" => reply(&select(app, parse(params)?)),
        "drawer.last_terminal" => reply(&last_terminal(app, parse(params)?).await?),
        "drawer.new" => reply(&new(app, parse(params)?).await?),
        "drawer.close" => reply(&close(app, parse(params)?).await?),
        "drawer.resume" => reply(&resume(app, parse(params)?).await?),
        _ => Err(unknown(method)),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

fn unexpected(what: &str, other: &Reply) -> RequestError {
    RequestError::plain(format!("unexpected answer to {what}: {other:?}"))
}

/// What most `drawer.*` requests take: the shown project.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "DrawerParams")]
pub struct Params {
    pub project: String,
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

async fn terminals(app: &App, project: &str) -> Result<Vec<ProjectTerminal>, RequestError> {
    match app
        .service
        .send(Request::ProjectTerminalList(ProjectParams {
            project: project.to_owned(),
        }))
        .await?
    {
        Reply::ProjectTerminals(list) => Ok(list),
        other => Err(unexpected("project_terminal.list", &other)),
    }
}

/// The header button "Terminals & agents": its tooltip and the orange dot while a Project agent waits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DrawerButton")]
pub struct Button {
    pub label: &'static str,
    pub keys: Option<String>,
    pub dot: bool,
    /// "Project agent “Import GitHub issues” asks you".
    pub waiting: Option<String>,
    /// "New Project agent  ⇧⌘A".
    pub note: String,
}

pub fn button_of(agents: &[ProjectAgent]) -> Button {
    let platform = Platform::current();
    Button {
        label: "Terminals & agents",
        keys: KeyAction::ProjectTerminal.keys(platform),
        dot: vm::button_dot(agents),
        waiting: vm::waiting_line(agents),
        note: match KeyAction::NewProjectAgent.keys(platform) {
            Some(keys) => format!("New Project agent  {keys}"),
            None => "New Project agent".to_owned(),
        },
    }
}

pub async fn button(app: &App, params: Params) -> Result<Button, RequestError> {
    Ok(button_of(&agents(app, &params.project).await?.open))
}

/// The drawer: its tabs, which one shows, + New and Recently ended, and the texts around a terminal.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "DrawerView")]
pub struct View {
    pub title: &'static str,
    pub tabs: Vec<Tab>,
    /// The tab shown: the project's last-used one while it is open, else the first.
    pub selected: Option<String>,
    pub new_label: &'static str,
    pub new_menu: Vec<MenuItem>,
    pub recent_label: &'static str,
    /// Recently ended Project agents; choosing one resumes it.
    pub recent: Vec<MenuItem>,
    pub recent_empty: &'static str,
    pub close_tab: &'static str,
    pub hide: &'static str,
    pub maximize: &'static str,
    pub restore: &'static str,
    /// The hint in a new terminal's empty scrollback, until the first keystroke.
    pub hint: Vec<String>,
    /// What the drawer says with no tab open.
    pub empty: Vec<String>,
    pub height: f32,
}

pub struct Facts<'a> {
    pub agents: &'a ProjectAgentList,
    pub terminals: &'a [ProjectTerminal],
    pub remembered: Option<&'a str>,
    pub project_name: &'a str,
    pub short_repo: &'a str,
}

pub fn view_of(f: &Facts) -> View {
    let tabs = vm::tabs(&f.agents.open, f.terminals);
    let selected = f
        .remembered
        .filter(|r| tabs.iter().any(|t| t.id == *r))
        .map(str::to_owned)
        .or_else(|| tabs.first().map(|t| t.id.clone()));
    View {
        title: "Terminals & agents",
        tabs,
        selected,
        new_label: "New",
        new_menu: vm::new_menu(KeyAction::NewProjectAgent.keys(Platform::current()).as_deref()),
        recent_label: "Recently ended",
        recent: f
            .agents
            .recent
            .iter()
            .map(|a| {
                let item = MenuItem::new(a.id.clone(), a.name.clone()).icon(
                    crewboard_view::icons::IconName::Bot,
                );
                match a.ended_at {
                    Some(at) => item.shortcut(crewboard_view::view_models::clock::local_hhmm(at)),
                    None => item,
                }
            })
            .collect(),
        recent_empty: "No Project agent ended yet",
        close_tab: "Close tab",
        hide: "Hide \u{b7} terminals and agents keep running",
        maximize: "Maximize",
        restore: "Restore",
        hint: vm::hint(f.project_name, f.short_repo),
        empty: vm::empty_lines(f.short_repo),
        height: vm::DRAWER_HEIGHT,
    }
}

fn remembered(app: &App, project: &str) -> Option<String> {
    app.drawer_tabs
        .lock()
        .ok()
        .and_then(|m| m.get(project).cloned())
}

fn remember(app: &App, project: &str, tab: &str) {
    if let Ok(mut m) = app.drawer_tabs.lock() {
        m.insert(project.to_owned(), tab.to_owned());
    }
}

pub async fn view(app: &App, params: Params) -> Result<View, RequestError> {
    let agents = agents(app, &params.project).await?;
    let terminals = terminals(app, &params.project).await?;
    let (name, repo) = match app.service.send(Request::ProjectList).await? {
        Reply::Projects(list) => list
            .into_iter()
            .find(|p| p.key == params.project)
            .map(|p| (p.name, tilde(&p.repo_path)))
            .unwrap_or_else(|| (params.project.clone(), String::new())),
        other => return Err(unexpected("project.list", &other)),
    };
    let remembered = remembered(app, &params.project);
    Ok(view_of(&Facts {
        agents: &agents,
        terminals: &terminals,
        remembered: remembered.as_deref(),
        project_name: &name,
        short_repo: &repo,
    }))
}

/// What `drawer.select` and `drawer.close` take: a tab of the project's drawer.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "DrawerTabParams")]
pub struct TabParams {
    pub project: String,
    pub tab: String,
    /// Close a Project agent mid-turn after its question.
    #[serde(default)]
    pub confirm: bool,
}

/// The user picked a tab: it is the last used one now.
pub fn select(app: &App, params: TabParams) {
    remember(app, &params.project, &params.tab);
}

async fn open_terminal(app: &App, project: &str, new: bool) -> Result<String, RequestError> {
    match app
        .service
        .send(Request::ProjectTerminalOpen(ProjectTerminalParams {
            project: project.to_owned(),
            new,
        }))
        .await
    {
        Ok(Reply::ProjectTerminal(t)) => Ok(terminal_tab_id(&t.session.id)),
        Ok(other) => Err(unexpected("project_terminal.open", &other)),
        Err(e) => {
            toast_error(app, "The terminal didn\u{2019}t open", &e);
            Err(e)
        }
    }
}

fn toast_error(app: &App, title: &str, error: &RequestError) {
    app.notices.toast(NewToast::new(
        ToastLevel::Error,
        title,
        Some(error.message.clone()),
    ));
}

/// ⇧⌘T: the tab to show, the last-used terminal or a new one (`project_terminal.open {new}`).
pub async fn last_terminal(app: &App, params: Params) -> Result<String, RequestError> {
    let open = terminals(app, &params.project).await?;
    let tab = match vm::last_used(&open, remembered(app, &params.project).as_deref()) {
        LastUsed::Show(tab) => tab,
        LastUsed::OpenNew => open_terminal(app, &params.project, true).await?,
    };
    remember(app, &params.project, &tab);
    Ok(tab)
}

/// What `drawer.new` takes: the + New item (or ⇧⌘A's `new-agent`).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "DrawerNewParams")]
pub struct NewParams {
    pub project: String,
    pub item: String,
}

/// New terminal ("Terminal 3") or New Project agent (the click is the go-ahead for its tokens, d0068). The new tab.
pub async fn new(app: &App, params: NewParams) -> Result<String, RequestError> {
    let tab = match params.item.as_str() {
        NEW_TERMINAL => open_terminal(app, &params.project, true).await?,
        NEW_AGENT => match app
            .service
            .send(Request::ProjectAgentOpen(ProjectParams {
                project: params.project.clone(),
            }))
            .await
        {
            Ok(Reply::ProjectAgent(agent)) => agent_tab_id(&agent.id),
            Ok(other) => return Err(unexpected("project_agent.open", &other)),
            Err(e) => {
                toast_error(app, "The Project agent didn\u{2019}t open", &e);
                return Err(e);
            }
        },
        other => return Err(RequestError::plain(format!("not a + New item: {other}"))),
    };
    remember(app, &params.project, &tab);
    Ok(tab)
}

/// How a close went: done, or the Project agent is mid-turn and the user is asked first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "outcome", rename_all = "snake_case")]
#[ts(rename = "DrawerClosed")]
pub enum Closed {
    Done,
    Ask {
        title: String,
        confirm: &'static str,
    },
}

/// Closing a terminal's tab ends its shell; closing an agent's ends it (an unused New agent is discarded).
pub async fn close(app: &App, params: TabParams) -> Result<Closed, RequestError> {
    let request = if let Some(session) = params.tab.strip_prefix("terminal:") {
        Request::ProjectTerminalClose(SessionIdParams {
            session: session.to_owned(),
        })
    } else if let Some(id) = params.tab.strip_prefix("agent:") {
        Request::ProjectAgentClose(ProjectAgentCloseParams {
            id: id.to_owned(),
            confirm: params.confirm,
        })
    } else {
        return Err(RequestError::plain(format!("not a tab: {}", params.tab)));
    };
    match app.service.send(request).await {
        Ok(_) => Ok(Closed::Done),
        Err(e) if e.refusal.as_deref() == Some("needs_confirmation") => Ok(Closed::Ask {
            title: e.message,
            confirm: "Stop and close",
        }),
        Err(e) => {
            toast_error(app, "The tab didn\u{2019}t close", &e);
            Err(e)
        }
    }
}

/// What `drawer.resume` takes: a recently ended Project agent.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "DrawerResumeParams")]
pub struct ResumeParams {
    pub project: String,
    pub id: String,
}

/// Recently ended → an agent: its conversation continues in a new tab (d0045).
pub async fn resume(app: &App, params: ResumeParams) -> Result<String, RequestError> {
    match app
        .service
        .send(Request::ProjectAgentResume(IdParams {
            id: params.id.clone(),
        }))
        .await
    {
        Ok(Reply::ProjectAgent(agent)) => {
            let tab = agent_tab_id(&agent.id);
            remember(app, &params.project, &tab);
            Ok(tab)
        }
        Ok(other) => Err(unexpected("project_agent.resume", &other)),
        Err(e) => {
            toast_error(app, "The Project agent didn\u{2019}t resume", &e);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use crewboard_core::board::projects::AddProject;
    use crewboard_core::git::testing::TempRepos;
    use serde_json::json;

    use super::*;
    use crate::api::request as table;
    use crate::service::testing::{Harness, connected};

    #[test]
    fn the_button_names_the_keys() {
        let button = button_of(&[]);
        assert_eq!(button.label, "Terminals & agents");
        assert!(!button.dot);
        assert_eq!(button.waiting, None);
        assert!(button.note.starts_with("New Project agent"));
    }

    #[test]
    fn the_shown_tab_is_the_last_used_while_open() {
        let agents = ProjectAgentList {
            open: Vec::new(),
            recent: Vec::new(),
        };
        let facts = |remembered| Facts {
            agents: &agents,
            terminals: &[],
            remembered,
            project_name: "shop-web",
            short_repo: "~/code/shop-web",
        };
        let empty = view_of(&facts(Some("terminal:gone")));
        assert_eq!(empty.selected, None);
        assert!(empty.empty[0].contains("~/code/shop-web"));
        assert_eq!(empty.new_menu.len(), 2);
        assert!(empty.hint[0].contains("shop-web"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn terminals_open_show_and_close_through_the_drawer() {
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
            Reply::Project(p) => p.key,
            other => panic!("{other:?}"),
        };
        let p = json!({ "project": project });

        let view = table(&h.app, "drawer.view", p.clone()).await.unwrap();
        assert_eq!(view["tabs"], json!([]));
        assert_eq!(view["selected"], Value::Null);
        let button = table(&h.app, "drawer.button", p.clone()).await.unwrap();
        assert_eq!(button["dot"], false);

        // ⇧⌘T with nothing open: a new terminal, shown.
        let first = table(&h.app, "drawer.last_terminal", p.clone())
            .await
            .unwrap();
        let first = first.as_str().unwrap().to_owned();
        assert!(first.starts_with("terminal:"));
        let second = table(
            &h.app,
            "drawer.new",
            json!({ "project": project, "item": NEW_TERMINAL }),
        )
        .await
        .unwrap();
        let view = table(&h.app, "drawer.view", p.clone()).await.unwrap();
        let labels: Vec<&str> = view["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["label"].as_str().unwrap())
            .collect();
        assert_eq!(labels, ["Terminal 1", "Terminal 2"]);
        assert_eq!(view["selected"], second, "the newest pick");

        // Picking Terminal 1 makes it the last used: ⇧⌘T shows it.
        table(
            &h.app,
            "drawer.select",
            json!({ "project": project, "tab": first }),
        )
        .await
        .unwrap();
        let again = table(&h.app, "drawer.last_terminal", p.clone())
            .await
            .unwrap();
        assert_eq!(again, json!(first));

        let closed = table(
            &h.app,
            "drawer.close",
            json!({ "project": project, "tab": first }),
        )
        .await
        .unwrap();
        assert_eq!(closed["outcome"], "done");
        let view = table(&h.app, "drawer.view", p.clone()).await.unwrap();
        assert_eq!(view["tabs"].as_array().unwrap().len(), 1);
        assert_eq!(view["selected"], second, "the closed one is gone");
        assert!(
            table(&h.app, "drawer.new", json!({ "project": project, "item": "x" }))
                .await
                .is_err()
        );
        h.service.stop_all().await;
    }
}
