//! Named agent requests (AGT-01…AGT-04; d0086, d0088): Settings → Agents (list, add, edit, duplicate, delete), the
//! role defaults (app-wide and a project's) and a ticket's rows, all the user's ([`super::permissions`] refuses
//! agents; T-0338 is their way in). Changes to entries and defaults publish `agents_changed`; a row change
//! `ticket_changed`.

use serde_json::json;

use super::Shared;
use super::board_requests::error_body;
use crate::board::{BoardError, Refusal};
use crate::named_agents::DeleteOutcome;
use crate::protocol::{ErrorBody, ErrorKind, Event, Reply, Request};

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
