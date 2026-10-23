//! `files_view.*`: the project Files view (FIL-01…FIL-04, FIL-07, FIL-10; d0087; `s28-03`, `s28-04`, `s28-09`). The
//! project's base branch from git with every ticket's changes on top (`files.project_*`, `board/project_files.rs`):
//! the left column with its filters and counts, a folder as it opens, the search by name, the picker's sources, and a
//! picked ticket or epic as its own Files tab, read-only (its tree, files and All changes come from `files.*`). What
//! it says is `otaq-view`'s `files_view.rs`.

use otaq_core::board::tickets::Ticket;
use otaq_core::protocol::{
    ProjectFilesPathParams, ProjectFilesSearchParams, ProjectParams, Reply, Request,
};
use otaq_view::keymap::KeyAction;
use otaq_view::view_models::files_tab::{SearchView, TreeRow};
use otaq_view::view_models::files_view::{self as vm, Filter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "files_view.view" => reply(&view(app, parse(params)?).await?),
        "files_view.tree" => reply(&tree(app, parse(params)?).await?),
        "files_view.search" => reply(&search(app, parse(params)?).await?),
        "files_view.sources" => reply(&sources(app, parse(params)?).await?),
        "files_view.picked" => reply(&picked(app, parse(params)?).await?),
        _ => Err(unknown(method)),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

fn unexpected(what: &str, other: &Reply) -> RequestError {
    RequestError::plain(format!("unexpected answer to {what}: {other:?}"))
}

/// The shown project's view under a filter; `base_only` lists the bare base at once (the Loading state), the page
/// then asks again without it.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "FilesViewParams")]
pub struct ViewParams {
    pub project: String,
    pub filter: Filter,
    #[serde(default)]
    #[ts(optional)]
    pub base_only: Option<bool>,
}

/// What `files_view.view` answers: the view, or why the base can't be read (no repository, no base branch).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FilesViewLoad")]
pub struct Load {
    pub view: Option<vm::Page>,
    pub problem: Option<String>,
}

/// Whether one of the project's agents works now: the page then asks every 5 s (FIL-10).
async fn working(app: &App, project: &str) -> bool {
    super::panel::tickets(app, project)
        .await
        .unwrap_or_default()
        .iter()
        .any(|t: &Ticket| vm::agent_works(t.status))
}

pub async fn view(app: &App, params: ViewParams) -> Result<Load, RequestError> {
    let base_only = params.base_only.unwrap_or(false);
    let tree = |base_only| {
        app.service
            .send(Request::FilesProjectTree(ProjectFilesPathParams {
                project: params.project.clone(),
                path: String::new(),
                base_only,
            }))
    };
    let (keys, working) = tokio::join!(super::keymap::labels(app), working(app, &params.project));
    let search_keys = keys.label(KeyAction::GoToFile);
    let answer = match params.filter {
        Filter::All => tree(base_only).await.and_then(|r| match r {
            Reply::ProjectTree(t) => Ok(vm::page(
                &t.source,
                t.counts.as_ref(),
                Filter::All,
                vm::entry_rows(&t.entries, 0),
                search_keys,
                working,
            )),
            other => Err(unexpected("files.project_tree", &other)),
        }),
        filter => app
            .service
            .send(Request::FilesProjectChanges(ProjectParams {
                project: params.project.clone(),
            }))
            .await
            .and_then(|r| match r {
                Reply::ProjectChanges(c) => Ok(vm::page(
                    &c.source,
                    Some(&c.counts),
                    filter,
                    vm::changed_rows(&c, filter),
                    search_keys,
                    working,
                )),
                other => Err(unexpected("files.project_changes", &other)),
            }),
    };
    Ok(match answer {
        Ok(page) => Load {
            view: Some(page),
            problem: None,
        },
        Err(e) => Load {
            view: None,
            problem: Some(e.message),
        },
    })
}

/// A folder of the base, opened at `depth`.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "FilesViewTreeParams")]
pub struct TreeParams {
    pub project: String,
    pub path: String,
    pub depth: u32,
}

pub async fn tree(app: &App, params: TreeParams) -> Result<Vec<TreeRow>, RequestError> {
    match app
        .service
        .send(Request::FilesProjectTree(ProjectFilesPathParams {
            project: params.project,
            path: params.path,
            base_only: false,
        }))
        .await?
    {
        Reply::ProjectTree(tree) => Ok(vm::entry_rows(&tree.entries, params.depth)),
        other => Err(unexpected("files.project_tree", &other)),
    }
}

/// What is typed in the search box.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "FilesViewSearchParams")]
pub struct SearchParams {
    pub project: String,
    pub query: String,
}

pub async fn search(app: &App, params: SearchParams) -> Result<SearchView, RequestError> {
    match app
        .service
        .send(Request::FilesProjectSearch(ProjectFilesSearchParams {
            project: params.project,
            query: params.query.clone(),
        }))
        .await?
    {
        Reply::ProjectSearch(found) => Ok(vm::search_view(&params.query, &found)),
        other => Err(unexpected("files.project_search", &other)),
    }
}

/// The shown project.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "FilesViewProjectParams")]
pub struct ProjectOnly {
    pub project: String,
}

/// The picker's rows; "when it reaches Dev" names the flow's first agent stage.
pub async fn sources(app: &App, params: ProjectOnly) -> Result<vm::Picker, RequestError> {
    let (answer, flow) = tokio::join!(
        app.service
            .send(Request::FilesProjectSources(ProjectParams {
                project: params.project.clone(),
            })),
        super::board::flow(app, &params.project),
    );
    let first = flow
        .first_agent_stage()
        .map_or_else(|| "Dev".to_owned(), |s| s.name.clone());
    match answer? {
        Reply::ProjectSources(sources) => Ok(vm::picker(&sources, &first)),
        other => Err(unexpected("files.project_sources", &other)),
    }
}

/// A ticket or an epic picked in the picker, in the shown project.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "FilesViewPickedParams")]
pub struct PickedParams {
    pub project: String,
    pub key: String,
}

/// What `files_view.picked` answers: the picked source, or why it has no files to show.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FilesViewPickedLoad")]
pub struct PickedLoad {
    pub picked: Option<vm::Picked>,
    pub problem: Option<String>,
}

/// Its own Files tab's frame (`files.view`), read-only, with the back link to the base.
pub async fn picked(app: &App, params: PickedParams) -> Result<PickedLoad, RequestError> {
    let (tab, base) = tokio::join!(
        super::files::view(
            app,
            super::files::Params {
                key: params.key.clone(),
            },
        ),
        app.service
            .send(Request::FilesProjectTree(ProjectFilesPathParams {
                project: params.project.clone(),
                path: String::new(),
                base_only: true,
            })),
    );
    let tab = tab?;
    let base_branch = match base {
        Ok(Reply::ProjectTree(tree)) => tree.source.base_branch,
        Ok(other) => return Err(unexpected("files.project_tree", &other)),
        Err(e) => {
            return Ok(PickedLoad {
                picked: None,
                problem: Some(e.message),
            });
        }
    };
    Ok(match tab.view {
        Some(view) => PickedLoad {
            picked: Some(vm::picked(&base_branch, view)),
            problem: None,
        },
        None => PickedLoad {
            picked: None,
            problem: tab.problem,
        },
    })
}

#[cfg(test)]
pub fn export_types(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    ViewParams::export_all(cfg)?;
    Load::export_all(cfg)?;
    TreeParams::export_all(cfg)?;
    SearchParams::export_all(cfg)?;
    ProjectOnly::export_all(cfg)?;
    PickedParams::export_all(cfg)?;
    PickedLoad::export_all(cfg)?;
    vm::Picker::export_all(cfg)?;
    vm::Gallery::export_all(cfg)
}
