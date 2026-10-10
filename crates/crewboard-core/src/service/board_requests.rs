//! Board requests (projects, tickets, epics, dependencies, pins, settings) mapped to [`Board`], and the events
//! their changes publish (SDD §5.4). The service enforces every rule; clients stay thin (d0055).
//!
//! Change events come from comparing the project's tickets before and after a mutation: every ticket that
//! differs gets `ticket_changed`, a changed status also `card_status_changed`, and a ticket that is gone
//! `ticket_deleted`. Projects being added, changed or removed publish `projects_changed`.

use std::collections::HashMap;

use serde_json::json;

use super::Shared;
use crate::board::change_note::change_note;
use crate::board::questions::validate_options;
use crate::board::readiness::project_readiness;
use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError, Refusal};
use crate::caller::Caller;
use crate::domain::Actor;
use crate::protocol::{
    ErrorBody, ErrorKind, Event, GlobalSettingsReply, ProjectSettingsReply, Reply, Request,
};
use crate::session::SessionKind;
use crate::session::deliveries::DeliveryKind;
use crate::signals::LIVE;

/// Answers a board request. `None` when the request isn't a board request.
pub(super) async fn handle(
    request: Request,
    caller: &Caller,
    requested_actor: Option<&str>,
    shared: &Shared,
) -> Option<Result<Reply, ErrorBody>> {
    // The actor recorded on changes: an agent caller always acts as itself (CLI-04).
    let actor = caller.actor(requested_actor);
    let board = &shared.board;
    let result = match request {
        Request::ProjectList => board.project_infos().await.map(Reply::Projects),
        Request::ProjectProbe(p) => board.probe_folder(&p.path).await.map(Reply::FolderProbe),
        Request::ProjectAdd(add) => {
            let result = board.add_project(&add).await;
            if result.is_ok() {
                shared.events.publish(Event::ProjectsChanged);
            }
            match result {
                Ok(project) => board
                    .project_info_of(&project.key)
                    .await
                    .map(Reply::Project),
                Err(e) => Err(e),
            }
        }
        Request::ProjectUpdate(p) => {
            let result = board.update_project(&p.project, &p.update).await;
            if result.is_ok() {
                shared.events.publish(Event::ProjectsChanged);
            }
            match result {
                Ok(project) => board
                    .project_info_of(&project.key)
                    .await
                    .map(Reply::Project),
                Err(e) => Err(e),
            }
        }
        Request::ProjectRemove(p) => {
            let before = board.tickets(&p.key).await.unwrap_or_default();
            let result = board.remove_project(&p.key).await;
            if result.is_ok() {
                publish_changes(shared, &p.key, &before, &[]);
                shared.events.publish(Event::ProjectsChanged);
            }
            result.map(|()| Reply::Ok)
        }
        Request::ProjectBranches(p) => board.branches(&p.project).await.map(Reply::Branches),
        Request::BoardSnapshot(p) => board.board_snapshot(&p.project).await.map(Reply::Board),

        Request::TicketGet(p) => board
            .ticket(&p.key)
            .await
            .map(|t| Reply::Ticket(Box::new(t))),
        Request::TicketList(p) => board.tickets(&p.project).await.map(Reply::Tickets),
        Request::DepReadiness(p) => board
            .tickets(&p.project)
            .await
            .map(|tickets| Reply::Readiness(project_readiness(&tickets))),
        Request::TicketCreate(input) => {
            let project = input.project.clone();
            mutate(shared, &project, board.create_ticket(&input, &actor))
                .await
                .map(|t| Reply::Ticket(Box::new(t)))
        }
        Request::TicketCreateBatch(p) => {
            let project = p.project.clone();
            mutate(
                shared,
                &project,
                board.create_batch(&p.project, &p.tickets, &actor),
            )
            .await
            .map(Reply::Tickets)
        }
        Request::TicketUpdate(p) => {
            let project = project_of(&p.key);
            let before = board.ticket(&p.key).await.ok();
            let after = mutate(
                shared,
                &project,
                board.update_ticket(&p.key, &p.update, &actor),
            )
            .await;
            if let (Some(before), Ok(after)) = (&before, &after) {
                queue_change_note(shared, before, after, &actor).await;
            }
            after.map(|t| Reply::Ticket(Box::new(t)))
        }
        Request::TicketMove(p) => {
            let project = project_of(&p.key);
            let moved = mutate(
                shared,
                &project,
                board.move_ticket(&p.key, p.target, p.confirmed, &actor),
            )
            .await;
            // Entering Queued only queues: the scheduler starts it once it's ready (its change event wakes it).
            moved.map(|t| Reply::Ticket(Box::new(t)))
        }
        Request::TicketHistory(p) => board.history(&p.key).await.map(Reply::History),
        Request::TicketAskHuman(p) => ask_human(shared, caller, &p).await,
        Request::TicketReviewSubmit(p) => {
            let project = project_of(&p.key);
            let submitted = mutate(
                shared,
                &project,
                board.submit_for_review(&p.key, &p.summary, &actor),
            )
            .await;
            if submitted.is_ok()
                && let Caller::Agent { session, .. } = caller
            {
                shared.signals.mark_submitted_or_asked(session);
            }
            submitted.map(|t| Reply::Ticket(Box::new(t)))
        }
        Request::TicketPrime(p) => board
            .prime_context(&p.key)
            .await
            .map(|c| Reply::Prime(Box::new(c))),
        Request::TicketRetryStart(p) => {
            let project = project_of(&p.key);
            // Back to Queued; the scheduler's next pass runs the start sequence again.
            let retried = mutate(shared, &project, shared.starter.retry(&p.key, &actor)).await;
            retried.map(|t| Reply::Ticket(Box::new(t)))
        }
        Request::TicketPin(p) => {
            let project = project_of(&p.key);
            mutate(shared, &project, board.pin(&p.key, &actor))
                .await
                .map(|t| Reply::Ticket(Box::new(t)))
        }
        Request::TicketUnpin(p) => {
            let project = project_of(&p.key);
            mutate(shared, &project, board.unpin(&p.key, &actor))
                .await
                .map(|t| Reply::Ticket(Box::new(t)))
        }
        Request::TicketDeletePreview(p) => {
            board.delete_preview(&p.key).await.map(Reply::DeletePreview)
        }
        Request::TicketDelete(p) => {
            let project = project_of(&p.key);
            mutate(shared, &project, board.delete_ticket(&p.key))
                .await
                .map(Reply::DeletePreview)
        }
        Request::TicketAgentEdits(p) => board.agent_edits(&p.key).await.map(Reply::AgentEdits),
        Request::TicketAgentEditsSeen(p) => {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                board.mark_agent_edits_seen(&p.key, &actor),
            )
            .await
            .map(|()| Reply::Ok)
        }

        Request::EpicMoveTo(p) => {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                board.move_to_epic(&p.key, &p.epic, &actor),
            )
            .await
            .map(|t| Reply::Ticket(Box::new(t)))
        }
        Request::EpicClosePlan(p) => board.epic_close_plan(&p.key).await.map(Reply::ClosePlan),
        Request::EpicClose(p) => {
            let project = project_of(&p.key);
            mutate(shared, &project, board.close_epic(&p.key, &actor))
                .await
                .map(Reply::ClosePlan)
        }

        Request::DepAdd(p) => {
            let project = project_of(&p.child);
            let before = board.ticket(&p.child).await.ok();
            let after = mutate(
                shared,
                &project,
                board.add_dependency(&p.child, &p.parent, &actor),
            )
            .await;
            if let (Some(before), Ok(after)) = (&before, &after) {
                queue_change_note(shared, before, after, &actor).await;
            }
            after.map(|t| Reply::Ticket(Box::new(t)))
        }
        Request::DepRemove(p) => {
            let project = project_of(&p.child);
            let before = board.ticket(&p.child).await.ok();
            let after = mutate(
                shared,
                &project,
                board.remove_dependency(&p.child, &p.parent, &actor),
            )
            .await;
            if let (Some(before), Ok(after)) = (&before, &after) {
                queue_change_note(shared, before, after, &actor).await;
            }
            after.map(|t| Reply::Ticket(Box::new(t)))
        }
        Request::DepTree(p) => board.dependency_tree(&p.key).await.map(Reply::DepTree),
        Request::DepCycles(p) => board.dependency_cycles(&p.project).await.map(Reply::Cycles),

        Request::SettingsGet => global_settings(board).await,
        Request::SettingsSet(settings) => match board.set_global_settings(&settings).await {
            Ok(()) => global_settings(board).await,
            Err(e) => Err(e),
        },
        Request::ProjectSettingsGet(p) => project_settings(board, &p.project).await,
        Request::ProjectSettingsSet(p) => {
            match board.set_project_settings(&p.project, &p.settings).await {
                Ok(()) => project_settings(board, &p.project).await,
                Err(e) => Err(e),
            }
        }

        Request::Ping
        | Request::ServiceStatus
        | Request::ServiceStop
        | Request::Subscribe
        | Request::ToolsCheck
        | Request::OnboardingState
        | Request::DismissPermissionWarning
        | Request::SessionList(_)
        | Request::SessionAttach(_)
        | Request::SessionDetach(_)
        | Request::SessionResize(_)
        | Request::SessionInput(_)
        | Request::SessionDeliverNow(_)
        | Request::SessionFocus(_)
        | Request::Hook(_) => return None,
    };
    Some(result.map_err(|e| error_body(&e)))
}

async fn global_settings(board: &Board) -> Result<Reply, BoardError> {
    Ok(Reply::GlobalSettings(Box::new(GlobalSettingsReply {
        settings: board.global_settings().await?,
        problems: board.global_setting_problems().await?,
    })))
}

async fn project_settings(board: &Board, project: &str) -> Result<Reply, BoardError> {
    Ok(Reply::ProjectSettings(Box::new(ProjectSettingsReply {
        settings: board.project_settings(project).await?,
        problems: board.project_setting_problems(project).await?,
    })))
}

/// `crewboard ask human`: the question (with its options) is stored and the ticket shows Asks you (ATT-01,
/// ATT-12).
async fn ask_human(
    shared: &Shared,
    caller: &Caller,
    p: &crate::protocol::AskHumanParams,
) -> Result<Reply, BoardError> {
    if p.question.trim().is_empty() {
        return Err(BoardError::refused(
            Refusal::BadQuestion,
            "Write the question on stdin",
        ));
    }
    validate_options(&p.options, p.recommended)?;
    let board = &shared.board;
    let ticket = board.ticket(&p.key).await?;
    board
        .store_question(&ticket.id, &p.question, &p.options, p.recommended)
        .await?;
    shared
        .signals
        .ask_human(caller, &ticket.id, p.question.trim())
        .await?;
    board
        .ticket_by_id(&ticket.id)
        .await
        .map(|t| Reply::Ticket(Box::new(t)))
}

/// SES-08: a user's change to a ticket whose agent is at work queues a note with what changed. Agent edits and
/// tickets that aren't working (Queued, Parked, Backlog, …) get nothing.
async fn queue_change_note(shared: &Shared, before: &Ticket, after: &Ticket, actor: &Actor) {
    if actor.is_agent() || !LIVE.contains(&after.status) {
        return;
    }
    let Some(session) = shared.sessions.running_of(&after.id, SessionKind::Agent) else {
        return;
    };
    let Some(note) = change_note(before, after) else {
        return;
    };
    if let Err(e) = shared
        .deliveries
        .queue_delivery(&session.id, DeliveryKind::TicketChange, &note)
        .await
    {
        tracing::warn!(ticket = %after.key, error = %e, "can't queue the ticket-change note");
    }
}

/// The project part of a ticket key (`SHOP-18` → `SHOP`). Aliases keep their project, so this holds for them too.
fn project_of(key: &str) -> String {
    key.split_once('-').map_or(key, |(p, _)| p).to_owned()
}

/// Runs a mutation of one project's tickets and publishes the changes it made.
async fn mutate<T>(
    shared: &Shared,
    project: &str,
    change: impl Future<Output = Result<T, BoardError>>,
) -> Result<T, BoardError> {
    let before = shared.board.tickets(project).await.unwrap_or_default();
    let result = change.await?;
    match shared.board.tickets(project).await {
        Ok(after) => publish_changes(shared, project, &before, &after),
        Err(e) => tracing::warn!(error = %e, project, "can't read the board after a change"),
    }
    Ok(result)
}

fn publish_changes(shared: &Shared, project: &str, before: &[Ticket], after: &[Ticket]) {
    let old: HashMap<&str, &Ticket> = before.iter().map(|t| (t.id.as_str(), t)).collect();
    for ticket in after {
        let previous = old.get(ticket.id.as_str());
        if previous.is_some_and(|p| *p == ticket) {
            continue;
        }
        shared.events.publish(Event::TicketChanged {
            project: project.to_owned(),
            id: ticket.id.clone(),
            key: ticket.key.clone(),
        });
        if previous.is_none_or(|p| p.status != ticket.status) {
            shared.events.publish(Event::CardStatusChanged {
                project: project.to_owned(),
                id: ticket.id.clone(),
                key: ticket.key.clone(),
                status: ticket.status,
            });
        }
    }
    for gone in before
        .iter()
        .filter(|b| !after.iter().any(|a| a.id == b.id))
    {
        shared.events.publish(Event::TicketDeleted {
            project: project.to_owned(),
            id: gone.id.clone(),
            key: gone.key.clone(),
        });
    }
}

/// A board error as the protocol's error body: refusals keep their code in `data.refusal`.
pub(super) fn error_body(e: &BoardError) -> ErrorBody {
    match e {
        BoardError::Refused { code, message } => {
            let mut body = ErrorBody::new(ErrorKind::Refused, message.clone());
            body.data = Some(json!({ "refusal": code }));
            body
        }
        BoardError::NotFound(message) => ErrorBody::new(ErrorKind::NotFound, message.clone()),
        BoardError::Git(_) | BoardError::Store(_) => {
            tracing::error!(error = %e, "board request failed");
            ErrorBody::new(ErrorKind::Internal, e.to_string())
        }
    }
}
