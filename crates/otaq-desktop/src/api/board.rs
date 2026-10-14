//! `board.*`: the board (UI-02, TKT-06, CLS-03, CLS-06, REV-14; d0004, d0037, d0041, d0047, d0052, d0066;
//! *03 · Board — shop-web*, *03 · Board & columns*). `board.view` builds the six columns for the page from the
//! project's snapshot; a drag is the UIT-04 way: `board.drag` answers once, at drag start, with every column's
//! verdict from the core's `check_move`, so the page draws the whole drag itself; `board.drop` says what the drop
//! does (and carries out a plain move); `board.park` is the Park confirmation's button.
//!
//! T-0221 (UI-02, EPC-01, TKT-06, TKT-08, TKT-09, CLS-01; d0047, d0060, d0065, d0033): `board.search` is the header's
//! search and filter field (the board takes the same filter with `board.view`); `board.menu` and `board.menu_choose`
//! are the card's right-click menu; `board.delete_dialog` and `board.delete` are the one delete confirmation of a
//! never-started ticket, which the card menu, the panel's "…" menu and the Project agent's summary all open.

use otaq_core::board::moves::DropTarget;
use otaq_core::board::snapshot::BoardSnapshot;
use otaq_core::board::tickets::{Base, Ticket};
use otaq_core::protocol::{CloseParams, KeyParams, MoveParams, ProjectParams, Reply, Request};
use otaq_view::keymap::Platform;
use otaq_view::view_models::board_page::{
    self, BoardPage, ColumnId, DragAnswer, DropOutcome, DropStep, Params, ProjectFacts,
};
use otaq_view::view_models::card_menu::{self, Choice};
use otaq_view::view_models::delete::{self as delete_text, DeleteText};
use otaq_view::view_models::empty_board;
use otaq_view::view_models::epic_actions;
use otaq_view::view_models::menu::MenuItem;
use otaq_view::view_models::search_page::{self, FilterState, SearchView};
use otaq_view::view_models::toasts::{NewToast, ToastLevel};
use serde::Deserialize;
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "board.view" => reply(&view(app, parse(params)?).await?),
        "board.drag" => reply(&drag(app, parse(params)?).await?),
        "board.drop" => reply(&drop(app, parse(params)?).await?),
        "board.park" => reply(&park(app, parse(params)?).await?),
        "board.search" => reply(&search(app, parse(params)?).await?),
        "board.menu" => reply(&menu(app, parse(params)?).await?),
        "board.menu_choose" => reply(&menu_choose(app, parse(params)?).await?),
        "board.delete_dialog" => reply(&delete_dialog(app, parse(params)?).await),
        "board.delete" => reply(&delete(app, parse(params)?).await?),
        _ => Err(unknown(method)),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

fn unexpected(method: &str, other: &Reply) -> RequestError {
    RequestError::plain(format!("unexpected answer to {method}: {other:?}"))
}

/// What `board.drag` and `board.park` take: the ticket.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "BoardTicketParams")]
pub struct TicketParams {
    pub key: String,
}

/// What `board.drop` takes: the ticket and the column it was dropped on.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "BoardDropParams")]
pub struct DropParams {
    pub key: String,
    pub column: ColumnId,
}

/// What `board.search` takes: the project, the filter as the page keeps it, and the open ticket.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "SearchParams")]
pub struct SearchParams {
    pub project: String,
    pub filter: FilterState,
    pub selected: Option<String>,
}

/// What `board.menu_choose` takes: the card and the chosen item's id.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "CardMenuParams")]
pub struct MenuParams {
    pub key: String,
    pub id: String,
}

async fn snapshot(app: &App, project: &str) -> Result<BoardSnapshot, RequestError> {
    match app
        .service
        .send(Request::BoardSnapshot(ProjectParams {
            project: project.to_owned(),
        }))
        .await?
    {
        Reply::Board(board) => Ok(board),
        other => Err(unexpected("board.snapshot", &other)),
    }
}

async fn ticket(app: &App, key: &str) -> Result<Ticket, RequestError> {
    match app
        .service
        .send(Request::TicketGet(KeyParams {
            key: key.to_owned(),
        }))
        .await?
    {
        Reply::Ticket(ticket) => Ok(*ticket),
        other => Err(unexpected("ticket.get", &other)),
    }
}

pub async fn view(app: &App, params: Params) -> Result<BoardPage, RequestError> {
    let board = snapshot(app, &params.project).await?;
    let tickets: Vec<Ticket> = board.tickets().cloned().collect();
    let project = &board.project;
    // Cards a Project agent created name it by its current name (T-0243, d0068).
    let agent_names: Vec<(String, String)> = match app
        .service
        .send(Request::ProjectAgentList(ProjectParams {
            project: params.project.clone(),
        }))
        .await
    {
        Ok(Reply::ProjectAgents(list)) => list
            .open
            .into_iter()
            .chain(list.recent)
            .map(|a| (a.id, a.name))
            .collect(),
        _ => Vec::new(),
    };
    // A ticket interrupted while validating says Resume starts a fresh validation (T-0244).
    let was_validating = match app.service.send(Request::StartupReport).await {
        Ok(Reply::StartupReport(report)) => report.was_validating,
        _ => Vec::new(),
    };
    let mut page = board_page::page(
        &tickets,
        &ProjectFacts {
            name: &project.name,
            repo_path: &project.repo_path,
            base_branch: &project.base_branch,
            agent_names: &agent_names,
            was_validating: &was_validating,
        },
        &params,
        otaq_core::store::now_ms(),
        Platform::current(),
    );
    // The empty board's Auto-merge line (T-0240) follows the project's settings.
    if let Some(empty) = page.empty.as_mut()
        && let Ok(Reply::ProjectSettings(reply)) = app
            .service
            .send(Request::ProjectSettingsGet(ProjectParams {
                project: params.project.clone(),
            }))
            .await
    {
        empty.auto_merge = Some(empty_board::auto_merge_line(
            reply.effective.auto_merge,
            reply.effective.auto_merge_inactive,
        ));
    }
    Ok(page)
}

/// Asked once when a drag starts; the page asks nothing more until the drop.
pub async fn drag(app: &App, params: TicketParams) -> Result<DragAnswer, RequestError> {
    Ok(board_page::drag_answer(&ticket(app, &params.key).await?))
}

/// The branch an approve merges into: the service's own proposal, else the ticket's base.
async fn merge_target(app: &App, ticket: &Ticket) -> String {
    let proposal = app
        .service
        .send(Request::TicketMergeProposal(KeyParams {
            key: ticket.key.to_string(),
        }))
        .await;
    match (proposal, &ticket.base) {
        (Ok(Reply::MergeProposal(p)), _) => p.target,
        (_, Base::Branch(branch)) => branch.clone(),
        (_, Base::Epic(epic)) => otaq_core::naming::branch(&epic.to_string()),
    }
}

pub async fn drop(app: &App, params: DropParams) -> Result<DropOutcome, RequestError> {
    let ticket = ticket(app, &params.key).await?;
    let column = params.column.column();
    Ok(match board_page::drop_step(&ticket, column) {
        DropStep::Nothing => DropOutcome::Nothing,
        DropStep::NotYet(message) => DropOutcome::NotYet { message },
        DropStep::Park => DropOutcome::Park {
            dialog: board_page::park_dialog(&ticket),
        },
        DropStep::Move => {
            app.service
                .send(Request::TicketMove(MoveParams {
                    key: params.key,
                    target: DropTarget::of(column),
                    confirmed: false,
                }))
                .await?;
            DropOutcome::Moved
        }
        DropStep::Approve => {
            let project = ticket.key.project.to_string();
            let (board, target, provider) = tokio::join!(
                snapshot(app, &project),
                merge_target(app, &ticket),
                app.service.send(Request::ProjectPrProvider(ProjectParams {
                    project: project.clone(),
                })),
            );
            let tickets: Vec<Ticket> = board?.tickets().cloned().collect();
            let provider = match provider {
                Ok(Reply::PrProvider(p)) => Some(p),
                _ => None,
            };
            DropOutcome::Approve {
                menu: board_page::approve_menu(&ticket, &target, &tickets, provider.as_ref()),
            }
        }
    })
}

/// The Park confirmation's button: the agent stops; branch, worktree and conversation are kept (CLS-06).
pub async fn park(app: &App, params: TicketParams) -> Result<(), RequestError> {
    app.service
        .send(Request::TicketPark(CloseParams {
            key: params.key,
            confirm: true,
        }))
        .await?;
    Ok(())
}

/// The header's field: chips, the count, and the suggestions with theirs.
pub async fn search(app: &App, params: SearchParams) -> Result<SearchView, RequestError> {
    let board = snapshot(app, &params.project).await?;
    let tickets: Vec<Ticket> = board.tickets().cloned().collect();
    let selected = params.selected.and_then(|k| k.parse().ok());
    Ok(search_page::view(
        &params.filter,
        &tickets,
        selected.as_ref(),
        Platform::current(),
    ))
}

/// The card's right-click menu: only the items its ticket allows (Delete… disabled once started).
pub async fn menu(app: &App, params: TicketParams) -> Result<Vec<MenuItem>, RequestError> {
    let ticket = ticket(app, &params.key).await?;
    let board = snapshot(app, &ticket.key.project.to_string()).await?;
    let tickets: Vec<Ticket> = board.tickets().cloned().collect();
    let has_epics = !epic_actions::epic_choices(&tickets).is_empty();
    Ok(card_menu::items(&ticket, has_epics))
}

/// A chosen item: Pin, Unpin and the priorities are sent here; the dialogs are the page's to open.
pub async fn menu_choose(app: &App, params: MenuParams) -> Result<Choice, RequestError> {
    let ticket = ticket(app, &params.key).await?;
    let choice = card_menu::choice(&params.id, &ticket);
    if choice == Choice::Sent
        && let Some(request) = card_menu::request(&params.id, &ticket)
    {
        app.service.send(request).await?;
    }
    Ok(choice)
}

/// The confirmation's texts from the service's preview; a refusal (the ticket started) comes back in the dialog.
pub async fn delete_dialog(app: &App, params: TicketParams) -> DeleteText {
    match app
        .service
        .send(Request::TicketDeletePreview(KeyParams {
            key: params.key.clone(),
        }))
        .await
    {
        Ok(Reply::DeletePreview(preview)) => {
            let text = delete_text::text(&preview);
            match agent_creator(app, &params.key).await {
                Some(name) => delete_text::created_by_agent(text, &name),
                None => text,
            }
        }
        Ok(other) => delete_text::refused(
            &params.key,
            unexpected("ticket.delete_preview", &other).message,
        ),
        Err(e) => delete_text::refused(&params.key, e.message),
    }
}

/// The current name of the Project agent that created the ticket, if one did (T-0243).
async fn agent_creator(app: &App, key: &str) -> Option<String> {
    let ticket = ticket(app, key).await.ok()?;
    let otaq_core::domain::Actor::ProjectAgent(id) = &ticket.created_by else {
        return None;
    };
    match app
        .service
        .send(Request::ProjectAgentList(ProjectParams {
            project: ticket.key.project.to_string(),
        }))
        .await
    {
        Ok(Reply::ProjectAgents(list)) => list
            .open
            .into_iter()
            .chain(list.recent)
            .find(|a| &a.id == id)
            .map(|a| a.name),
        _ => None,
    }
}

/// The confirm click: the service deletes (refusing a started ticket) and a toast says what went.
pub async fn delete(app: &App, params: TicketParams) -> Result<(), RequestError> {
    match app
        .service
        .send(Request::TicketDelete(KeyParams { key: params.key }))
        .await?
    {
        Reply::DeletePreview(preview) => {
            app.notices.toast(NewToast::new(
                ToastLevel::Success,
                delete_text::deleted(&preview),
                None,
            ));
            Ok(())
        }
        other => Err(unexpected("ticket.delete", &other)),
    }
}

#[cfg(test)]
mod tests {
    use otaq_core::board::tickets::NewTicketInput;
    use otaq_core::git::testing::TempRepos;
    use otaq_core::board::projects::AddProject;
    use serde_json::json;

    use super::*;
    use crate::api::request;
    use crate::service::testing::{Harness, connected};

    async fn send(h: &Harness, r: Request) -> Reply {
        h.app.service.send(r).await.unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_board_and_a_drag_come_from_the_real_service() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let Reply::Project(project) = send(
            &h,
            Request::ProjectAdd(AddProject {
                path: repos.repo("shop-web", 1).display().to_string(),
                key: None,
                base_branch: None,
            }),
        )
        .await
        else {
            panic!("no project");
        };
        let params = json!({ "project": project.key, "width": 1376.0, "height": 856.0,
                             "done_collapsed": null, "dev_unfolded": false });

        // No tickets yet: the empty board over the six columns.
        let view = request(&h.app, "board.view", params.clone()).await.unwrap();
        assert_eq!(view["empty"]["title"], "shop-web is ready");
        assert_eq!(
            view["empty"]["auto_merge"],
            "Auto-merge is off. Validated tickets wait for your review; turn it on in project settings."
        );
        assert_eq!(view["columns"].as_array().map(Vec::len), Some(6));

        let mut keys = Vec::new();
        for title in ["Update footer links", "Old idea"] {
            let Reply::Ticket(t) = send(
                &h,
                Request::TicketCreate(NewTicketInput {
                    project: project.key.clone(),
                    kind: Default::default(),
                    title: title.into(),
                    body: String::new(),
                    priority: None,
                    epic: None,
                    base_branch: None,
                    external_id: None,
                }),
            )
            .await
            else {
                panic!("no ticket");
            };
            keys.push(t.key.to_string());
        }
        let view = request(&h.app, "board.view", params.clone()).await.unwrap();
        assert_eq!(view["empty"], Value::Null);
        assert_eq!(view["columns"][0]["count"], "2");
        assert_eq!(view["columns"][0]["cards"][0]["kind"], "ticket");

        // One answer per drag, every column at once.
        let drag = request(&h.app, "board.drag", json!({ "key": keys[0] }))
            .await
            .unwrap();
        assert_eq!(drag["columns"][0]["verdict"]["kind"], "home");
        assert_eq!(drag["columns"][1]["verdict"]["kind"], "allowed");
        assert_eq!(
            drag["columns"][3]["verdict"]["title"],
            "Only agents move tickets to Ready for Review"
        );

        // A refused drop does nothing.
        let outcome = request(
            &h.app,
            "board.drop",
            json!({ "key": keys[0], "column": "ready_for_review" }),
        )
        .await
        .unwrap();
        assert_eq!(outcome["kind"], "nothing");

        // Reopen: a closed ticket dragged out of Done goes back to Backlog (CLS-03).
        send(
            &h,
            Request::TicketClose(CloseParams {
                key: keys[1].clone(),
                confirm: true,
            }),
        )
        .await;
        let outcome = request(
            &h.app,
            "board.drop",
            json!({ "key": keys[1], "column": "backlog" }),
        )
        .await
        .unwrap();
        assert_eq!(outcome["kind"], "moved");
        let view = request(&h.app, "board.view", params).await.unwrap();
        assert_eq!(view["columns"][0]["count"], "2");
        h.service.stop_all().await;
    }

    /// T-0221: the filter hides cards on the board and fills the header's field; the card menu pins and sets the
    /// priority; the delete confirmation words the service's preview, deletes, and is refused for a started ticket.
    #[tokio::test(flavor = "multi_thread")]
    async fn search_card_menu_and_delete_come_from_the_real_service() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let Reply::Project(project) = send(
            &h,
            Request::ProjectAdd(AddProject {
                path: repos.repo("shop-web", 1).display().to_string(),
                key: None,
                base_branch: None,
            }),
        )
        .await
        else {
            panic!("no project");
        };
        let mut keys = Vec::new();
        for title in ["Checkout API", "Footer links"] {
            let Reply::Ticket(t) = send(
                &h,
                Request::TicketCreate(NewTicketInput {
                    project: project.key.clone(),
                    kind: Default::default(),
                    title: title.into(),
                    body: String::new(),
                    priority: None,
                    epic: None,
                    base_branch: None,
                    external_id: None,
                }),
            )
            .await
            else {
                panic!("no ticket");
            };
            keys.push(t.key.to_string());
        }

        // Search: the board hides what doesn't match; the field turns a finished token into a chip.
        let view = request(
            &h.app,
            "board.view",
            json!({ "project": project.key, "width": 1376.0, "height": 856.0, "done_collapsed": null,
                    "dev_unfolded": false, "filter": { "tokens": [], "text": "checkout" } }),
        )
        .await
        .unwrap();
        assert_eq!(view["columns"][0]["count"], "1/2");
        assert_eq!(view["columns"][0]["hidden"], "1 hidden by filter");
        let field = request(
            &h.app,
            "board.search",
            json!({ "project": project.key, "filter": { "tokens": [], "text": "is:backlog " },
                    "selected": null }),
        )
        .await
        .unwrap();
        assert_eq!(field["filter"]["tokens"], json!(["is:backlog"]));
        assert_eq!(field["results"], "2 results");

        // The card menu: Pin is sent here, Delete opens the dialog.
        let items = request(&h.app, "board.menu", json!({ "key": keys[1] }))
            .await
            .unwrap();
        assert_eq!(items[0]["label"], "Pin to top");
        let pin = request(
            &h.app,
            "board.menu_choose",
            json!({ "key": keys[1], "id": items[0]["id"] }),
        )
        .await
        .unwrap();
        assert_eq!(pin["kind"], "sent");
        let Reply::Ticket(pinned) = send(&h, Request::TicketGet(KeyParams { key: keys[1].clone() })).await
        else {
            panic!("no ticket");
        };
        assert!(pinned.flags.is_pinned());
        let delete_item = items
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["label"] == "Delete…")
            .unwrap();
        assert_eq!(delete_item["disabled"], false);
        let chosen = request(
            &h.app,
            "board.menu_choose",
            json!({ "key": keys[1], "id": delete_item["id"] }),
        )
        .await
        .unwrap();
        assert_eq!(chosen["kind"], "delete");

        // The delete confirmation, then the delete.
        let dialog = request(&h.app, "board.delete_dialog", json!({ "key": keys[1] }))
            .await
            .unwrap();
        assert_eq!(dialog["title"], format!("Delete {} Footer links?", keys[1]));
        assert_eq!(dialog["confirm"], "Delete ticket");
        assert_eq!(dialog["error"], Value::Null);
        request(&h.app, "board.delete", json!({ "key": keys[1] }))
            .await
            .unwrap();
        let gone = h
            .app
            .service
            .send(Request::TicketGet(KeyParams { key: keys[1].clone() }))
            .await;
        assert!(gone.is_err(), "the ticket is gone");
        h.service.stop_all().await;
    }
}
