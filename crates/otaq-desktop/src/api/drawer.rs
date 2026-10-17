//! `drawer.*`: the Terminals drawer and its header button (UI-09, UI-01, RUN-06; d0068, d0058, d0019, d0087;
//! *02 → Terminals & agents button*, *17 · Project terminals*). The drawer shows one tab per Project terminal of the
//! shown project and + New. The last-used tab of each project is kept here, so ⇧⌘T finds the last-used terminal (or
//! opens one). Project agents live in the pane since d0087 (`pane.*`, T-0326): nothing here answers about them;
//! T-0331 removes what is left of their parts (the agent tab kind, the button's dot, the New Project agent item).

use otaq_core::project_agents::ProjectAgent;
use otaq_core::protocol::{ProjectParams, ProjectTerminalParams, Reply, Request, SessionIdParams};
use otaq_core::terminals::shell::ProjectTerminal;
use otaq_view::keymap::{KeyAction, Platform};
use otaq_view::shell::tilde;
use otaq_view::view_models::menu::MenuItem;
use otaq_view::view_models::project_terminal::{
    self as vm, LastUsed, NEW_TERMINAL, Tab, terminal_tab_id,
};
use otaq_view::view_models::toasts::{NewToast, ToastLevel};
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

/// The header button "Terminals & agents": its tooltip and the turquoise dot while a Project agent waits.
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

/// No Project agent counts here any more: a waiting one shows on the pane's strip (T-0326).
pub async fn button(_app: &App, _params: Params) -> Result<Button, RequestError> {
    Ok(button_of(&[]))
}

/// The drawer: its tabs, which one shows, + New, and the texts around a terminal.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "DrawerView")]
pub struct View {
    pub title: &'static str,
    pub tabs: Vec<Tab>,
    /// The tab shown: the project's last-used one while it is open, else the first.
    pub selected: Option<String>,
    pub new_label: &'static str,
    pub new_menu: Vec<MenuItem>,
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
    pub terminals: &'a [ProjectTerminal],
    pub remembered: Option<&'a str>,
    pub project_name: &'a str,
    pub short_repo: &'a str,
}

pub fn view_of(f: &Facts) -> View {
    let tabs = vm::tabs(&[], f.terminals);
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
        // + New makes terminals only: New Project agent is the pane's (+, ⇧⌘A).
        new_menu: vm::new_menu(None)
            .into_iter()
            .filter(|item| item.id == NEW_TERMINAL)
            .collect(),
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

/// What `drawer.new` takes: the + New item.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "DrawerNewParams")]
pub struct NewParams {
    pub project: String,
    pub item: String,
}

/// New terminal ("Terminal 3"). The new tab.
pub async fn new(app: &App, params: NewParams) -> Result<String, RequestError> {
    let tab = match params.item.as_str() {
        NEW_TERMINAL => open_terminal(app, &params.project, true).await?,
        other => return Err(RequestError::plain(format!("not a + New item: {other}"))),
    };
    remember(app, &params.project, &tab);
    Ok(tab)
}

/// Closing a terminal's tab ends its shell.
pub async fn close(app: &App, params: TabParams) -> Result<(), RequestError> {
    let Some(session) = params.tab.strip_prefix("terminal:") else {
        return Err(RequestError::plain(format!("not a tab: {}", params.tab)));
    };
    if let Err(e) = app
        .service
        .send(Request::ProjectTerminalClose(SessionIdParams {
            session: session.to_owned(),
        }))
        .await
    {
        toast_error(app, "The tab didn\u{2019}t close", &e);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use otaq_core::board::projects::AddProject;
    use otaq_core::git::testing::TempRepos;
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
        let facts = |remembered| Facts {
            terminals: &[],
            remembered,
            project_name: "shop-web",
            short_repo: "~/code/shop-web",
        };
        let empty = view_of(&facts(Some("terminal:gone")));
        assert_eq!(empty.selected, None);
        assert!(empty.empty[0].contains("~/code/shop-web"));
        assert_eq!(empty.new_menu.len(), 1, "no New Project agent: that is the pane's");
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
            Reply::ProjectAdded { project: p, .. } => p.key,
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
        assert_eq!(closed, Value::Null);
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
