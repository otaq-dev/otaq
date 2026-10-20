//! Which agent runs (AGM-05; d0108, d0088): a session in a stage runs the ticket's choice for that stage (T-0407),
//! else **the stage's agent** (the agent the flow names); the Project agent and the epic lead run **the agent with
//! that role**. A flow names agents by name: the shipped default flow's names were bound to their agents' ids when it
//! was applied (`agents.flow_names`), so a rename changes nothing; any other name (the test flow's) is looked up
//! among the agents, ignoring case. An unknown name is a check error naming the stage ([`crate::flow::check_agents`]).
//!
//! The model is resolved by the check before every launch ([`super::gate`]): `default` becomes the default model,
//! and the agent, tool, exact model and effort are saved on the session and its stage run.

use super::{Agent, Role, SessionRole};
use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError, Refusal};
use crate::domain::Place;
use crate::flow::{Flow, Stage};
use crate::store::{Store, StoreError};

/// The agent a flow's `name` stands for: the one the shipped default flow bound it to, else the agent with that name.
pub async fn named(store: &Store, name: &str) -> Result<Option<Agent>, StoreError> {
    if let Some(id) = store.flow_names().await?.get(name)
        && let Some(agent) = store.agent(id).await?
    {
        return Ok(Some(agent));
    }
    store.agent_named(name).await
}

/// The refusal for a stage whose agent isn't one of the agents: "Dev names the agent Rust developer, which isn't one
/// of your agents".
fn unknown(stage: &Stage, name: &str) -> BoardError {
    BoardError::refused(
        Refusal::UnknownAgent,
        crate::flow::check::unknown_agent_message(stage, name),
    )
}

/// The stage's agent (AGM-05): the agent `stage` names. Refused for a stage that runs no agent, or names one that
/// isn't there.
pub async fn stage_agent(board: &Board, stage: &Stage) -> Result<Agent, BoardError> {
    let Some(name) = &stage.agent else {
        return Err(BoardError::refused(
            Refusal::UnknownAgent,
            format!("{} runs no agent", stage.name),
        ));
    };
    named(board.store(), name)
        .await?
        .ok_or_else(|| unknown(stage, name))
}

/// The running agent of `ticket` in the stage `stage_id` (AGM-05): the ticket's choice for it (T-0407 adds choices),
/// else the stage's agent.
pub async fn running_agent(
    board: &Board,
    ticket: &Ticket,
    stage_id: &str,
) -> Result<Agent, BoardError> {
    let flow = board.effective_flow(ticket.key.project.as_str()).await?;
    let stage = flow.stage(stage_id).ok_or_else(|| {
        BoardError::NotFound(format!(
            "{} has no stage {stage_id}",
            ticket.key.project.as_str()
        ))
    })?;
    stage_agent(board, stage).await
}

/// The agent with `role` (AGM-01, AGM-05).
pub async fn role_agent(store: &Store, role: Role) -> Result<Agent, BoardError> {
    store
        .role_agent(role)
        .await?
        .ok_or_else(|| BoardError::NotFound(format!("no agent has the {} role", role.label())))
}

/// The Agent stage a ticket's agent session starts or continues in: the one it waits before, the Agent stage it is in,
/// else (from a check, coming back) the nearest earlier Agent stage, else the first one.
pub fn agent_stage_of<'f>(flow: &'f Flow, ticket: &Ticket) -> Option<&'f Stage> {
    let at = match &ticket.place {
        Place::Waiting(Some(id)) | Place::InStage(id) => flow.stage(id),
        _ => None,
    };
    match at {
        Some(stage) if stage.is_agent_stage() => Some(stage),
        Some(stage) => {
            crate::flow::engine::back_to(flow, &stage.id, &[]).or_else(|| flow.first_agent_stage())
        }
        None => flow.first_agent_stage(),
    }
}

/// The agent check a ticket's check session runs in: the one it is in, else the first agent check after its Agent
/// stage, else the flow's first.
pub fn check_stage_of<'f>(flow: &'f Flow, ticket: &Ticket) -> Option<&'f Stage> {
    let at = ticket.place.stage().and_then(|id| flow.stage(id));
    match at {
        Some(stage) if stage.is_agent_check() => Some(stage),
        Some(stage) => {
            let index = flow.position(&stage.id)?;
            flow.stages[index..]
                .iter()
                .find(|s| s.is_agent_check())
                .or_else(|| flow.stages.iter().find(|s| s.is_agent_check()))
        }
        None => flow.stages.iter().find(|s| s.is_agent_check()),
    }
}

/// The agent a ticket's new session of `role` runs (AGM-05): an Agent stage's ([`agent_stage_of`]), an agent check's
/// ([`check_stage_of`]), or the role's agent for the epic lead and the Project agent.
pub async fn session_agent(
    board: &Board,
    ticket: &Ticket,
    role: SessionRole,
) -> Result<Agent, BoardError> {
    if let Some(role) = role.role() {
        return role_agent(board.store(), role).await;
    }
    let flow = board.effective_flow(ticket.key.project.as_str()).await?;
    let stage = match role {
        SessionRole::Validator => check_stage_of(&flow, ticket),
        _ => agent_stage_of(&flow, ticket),
    }
    .ok_or_else(|| {
        BoardError::NotFound(format!(
            "{}'s flow has no stage for its {}",
            ticket.key,
            role.label()
        ))
    })?;
    running_agent(board, ticket, &stage.id.clone()).await
}

/// The Delivery stage's agent of a project (the PR path, AGM-05), else the first Agent stage's.
pub async fn delivery_agent(board: &Board, project: &str) -> Result<Agent, BoardError> {
    let flow = board.effective_flow(project).await?;
    let stage = flow
        .stages
        .iter()
        .find(|s| s.is_delivery() && s.agent.is_some())
        .or_else(|| flow.first_agent_stage())
        .ok_or_else(|| BoardError::NotFound(format!("{project}'s flow has no agent to deliver")))?;
    stage_agent(board, stage).await
}
