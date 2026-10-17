//! Named agent requests (AGT-01…AGT-04; d0086, d0088): Settings → Agents (list, add, edit, duplicate, delete), the
//! role defaults (app-wide and a project's) and a ticket's rows, all the user's ([`super::permissions`] refuses
//! agents; T-0338 is their way in). Changes to entries and defaults publish `agents_changed`; a row change
//! `ticket_changed`. A new row after a refused model starts the ticket or its round again (Q-B, T-0337); a new row
//! of a held role has the check before the launch run again (T-0357).

use serde_json::json;

use super::Shared;
use super::board_requests::error_body;
use crate::board::{BoardError, Refusal};
use crate::domain::{Actor, StartStep, Status, ValidationAction};
use crate::named_agents::{DeleteOutcome, Role};
use crate::protocol::{ErrorBody, ErrorKind, Event, Reply, Request};
use crate::validation::refused_its_model;

pub(super) async fn handle(request: Request, shared: &Shared) -> Result<Reply, ErrorBody> {
    let agents = &shared.named_agents;
    let changed = |reply: Reply| {
        shared.events.publish(Event::AgentsChanged);
        reply
    };
    let answer = match request {
        Request::AgentsList => agents.list().await.map(|l| Reply::Agents(Box::new(l))),
        Request::AgentsAdd(fields) => agents
            .add(&fields)
            .await
            .map(|a| changed(Reply::Agent(Box::new(a)))),
        Request::AgentsEdit(p) => agents
            .edit(&p.id, &p.fields)
            .await
            .map(|a| changed(Reply::Agent(Box::new(a)))),
        Request::AgentsDuplicate(p) => agents
            .duplicate(&p.id)
            .await
            .map(|draft| Reply::AgentDraft(Box::new(draft))),
        Request::AgentsDelete(p) => match agents.delete(&p.id).await {
            Ok(DeleteOutcome::Deleted) => Ok(changed(Reply::Ok)),
            Ok(DeleteOutcome::InUse(uses)) => {
                let mut body =
                    error_body(&BoardError::refused(Refusal::AgentInUse, uses.message()));
                body.data = Some(json!({ "refusal": Refusal::AgentInUse, "in_use": uses }));
                return Err(body);
            }
            Err(e) => Err(e),
        },
        Request::AgentsInUse(p) => agents
            .in_use(&p.id)
            .await
            .map(|uses| Reply::AgentInUse(Box::new(uses))),
        Request::ProjectAgentDefaults(p) => agents
            .project_defaults(&p.project)
            .await
            .map(|d| Reply::AgentDefaults(Box::new(d))),
        Request::AgentsSetDefault(p) => agents
            .set_default(p.role, &p.agent)
            .await
            .map(|d| changed(Reply::AgentDefaults(Box::new(d)))),
        Request::ProjectSetAgentDefault(p) => agents
            .set_project_default(&p.project, p.role, p.agent.as_deref())
            .await
            .map(|d| changed(Reply::AgentDefaults(Box::new(d)))),
        Request::TicketAgentRow(p) => match agents
            .set_row_by_user(&p.key, p.role, p.agent.as_deref())
            .await
        {
            Ok(()) => {
                if p.agent.is_some() {
                    start_again(shared, &p.key, p.role).await;
                }
                // A held role on its new row is checked again (Change agent; T-0357).
                if let Err(e) = shared.gate.lift().await {
                    tracing::warn!(key = p.key, error = %e, "can't lift the holds");
                }
                if let Ok(ticket) = shared.board.ticket(&p.key).await {
                    shared.events.publish(Event::TicketChanged {
                        project: ticket.key.project.to_string(),
                        id: ticket.id,
                        key: ticket.key,
                    });
                }
                Ok(Reply::Ok)
            }
            Err(e) => Err(e),
        },
        other => {
            return Err(ErrorBody::new(
                ErrorKind::BadRequest,
                format!("{} isn't a named agent request", other.method()),
            ));
        }
    };
    answer.map_err(|e| error_body(&e))
}

/// A new row picked after a refused model starts again as its retry would (Q-B; the pick is the user's click,
/// d0018): a new Developer row on a Start failed ticket is Retry start (the scheduler then starts it on the row's
/// agent), a new Validator row on a No result of the validator's model is Retry validation. Saving a new model on
/// the named agent alone starts nothing.
async fn start_again(shared: &Shared, key: &str, role: Role) {
    let Ok(ticket) = shared.board.ticket(key).await else {
        return;
    };
    let actor = Actor::current_user();
    let result = match role {
        Role::Developer
            if ticket.status == Status::StartInterrupted
                && ticket
                    .start_problem
                    .as_ref()
                    .is_some_and(|p| p.step == StartStep::Model) =>
        {
            shared.starter.retry(key, &actor).await.map(|_| ())
        }
        Role::Validator
            if ticket.status == Status::ValidationNoResult
                && ticket
                    .signal
                    .as_ref()
                    .and_then(|s| s.text.as_deref())
                    .is_some_and(refused_its_model) =>
        {
            shared
                .validation
                .user_action(key, ValidationAction::Retry, "", &actor)
                .await
                .map(|_| ())
        }
        _ => return,
    };
    if let Err(e) = result {
        tracing::warn!(key, %role, error = %e, "a new row didn't start the ticket again");
    }
}
