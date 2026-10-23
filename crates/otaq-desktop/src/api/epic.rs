//! `epic.*`: what the epic's sheet shows besides a ticket's (SHT-07; d0059, d0080, d0083; `s25-11`). `epic.overview`
//! is Overview's TICKETS stage grid and the Attachments column's note; `epic.tickets` the Tickets tab, its children as
//! List rows (LST-01) with Add ticket to epic. Both read the project's board (its tickets and whether it is paused),
//! its flow and the cards' named agents as the board does, and follow `ticket_changed` like the board.

use otaq_core::board::tickets::Ticket;
use otaq_core::protocol::{KeyParams, ProjectParams, Reply, Request};
use otaq_view::view_models::board::BoardInput;
use otaq_view::view_models::epic_sheet::{self as vm, OverviewView, TicketsView};
use serde::Deserialize;
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "epic.overview" => reply(&overview(app, parse(params)?).await?),
        "epic.tickets" => reply(&tickets(app, parse(params)?).await?),
        _ => Err(unknown(method)),
    }
}

/// The page's types (`generated.rs`' one line for them).
#[cfg(test)]
pub fn export_types(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    OverviewView::export_all(cfg)?;
    TicketsView::export_all(cfg)?;
    vm::Head::export_all(cfg)?;
    vm::Gallery::export_all(cfg)?;
    Params::export_all(cfg)?;
    Ok(())
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(format!("bad parameters: {e}")))
}

/// `{"key": "SHOP-40"}`: the open epic.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "EpicSheetParams")]
pub struct Params {
    pub key: String,
}

/// What the epic's views are built from: the epic, its project's tickets, its flow and the cards' named agents.
struct Facts {
    epic: Ticket,
    tickets: Vec<Ticket>,
    flow: otaq_core::flow::Flow,
    agents: Option<otaq_core::agents::facts::BoardAgents>,
    /// The project is paused (PAU-04): children that would start say so.
    paused: bool,
}

async fn facts(app: &App, key: &str) -> Result<Facts, RequestError> {
    let epic = match app
        .service
        .send(Request::TicketGet(KeyParams {
            key: key.to_owned(),
        }))
        .await?
    {
        Reply::Ticket(t) => *t,
        other => {
            return Err(RequestError::plain(format!(
                "unexpected reply to ticket.get: {other:?}"
            )));
        }
    };
    let project = epic.key.project.to_string();
    let board = super::board::snapshot(app, &project).await?;
    let tickets: Vec<Ticket> = board.tickets().cloned().collect();
    let paused = board.project.paused_since.is_some();
    let flow = super::board::flow(app, &project).await;
    // Who works on each child, as the cards and the List say it (T-0345).
    let agents = match app
        .service
        .send(Request::BoardAgents(ProjectParams { project }))
        .await
    {
        Ok(Reply::BoardAgents(agents)) => Some(*agents),
        _ => None,
    };
    Ok(Facts {
        epic,
        tickets,
        flow,
        agents,
        paused,
    })
}

fn input(f: &Facts) -> BoardInput<'_> {
    BoardInput {
        flow: &f.flow,
        tickets: &f.tickets,
        visible: None,
        strong: None,
        project_base: None,
        now_ms: otaq_core::store::now_ms(),
        dev_budget: None,
        folded: &[],
        fold_empty: false,
        agent_names: &[],
        was_validating: &[],
        agents: f.agents.as_ref(),
        agent_list: None,
        starts_held: None,
        paused: f.paused,
    }
}

pub async fn overview(app: &App, params: Params) -> Result<OverviewView, RequestError> {
    let f = facts(app, &params.key).await?;
    Ok(vm::overview(&f.epic, &input(&f)))
}

pub async fn tickets(app: &App, params: Params) -> Result<TicketsView, RequestError> {
    let f = facts(app, &params.key).await?;
    Ok(vm::tickets_view(&f.epic, &input(&f)))
}
