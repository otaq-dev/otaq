//! `board.*`: the board (UI-02, TKT-06, CLS-03, CLS-06, REV-14; d0004, d0037, d0041, d0047, d0052, d0066;
//! *03 · Board — shop-web*, *03 · Board & columns*). `board.view` builds the six columns for the page from the
//! project's snapshot; a drag is the UIT-04 way: `board.drag` answers once, at drag start, with every column's
//! verdict from the core's `check_move`, so the page draws the whole drag itself; `board.drop` says what the drop
//! does (and carries out a plain move); `board.park` is the Park confirmation's button.

use crewboard_core::board::moves::DropTarget;
use crewboard_core::board::snapshot::BoardSnapshot;
use crewboard_core::board::tickets::{Base, Ticket};
use crewboard_core::protocol::{CloseParams, KeyParams, MoveParams, ProjectParams, Reply, Request};
use crewboard_view::keymap::Platform;
use crewboard_view::view_models::board_page::{
    self, BoardPage, ColumnId, DragAnswer, DropOutcome, DropStep, Params, ProjectFacts,
};
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
    Ok(board_page::page(
        &tickets,
        &ProjectFacts {
            name: &project.name,
            repo_path: &project.repo_path,
            base_branch: &project.base_branch,
        },
        &params,
        crewboard_core::store::now_ms(),
        Platform::current(),
    ))
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
        (_, Base::Epic(epic)) => crewboard_core::naming::branch(&epic.to_string()),
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

#[cfg(test)]
mod tests {
    use crewboard_core::board::tickets::NewTicketInput;
    use crewboard_core::git::testing::TempRepos;
    use crewboard_core::board::projects::AddProject;
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
}
