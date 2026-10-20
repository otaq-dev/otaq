//! Agent requests (AGM-01…AGM-04; d0108, d0088): Settings → Agents (list, add, edit, duplicate, delete, in use, a
//! role moved, the default model, the fallbacks, imported skills), all the user's ([`super::permissions`] refuses
//! agents). Every change publishes `agents_changed`, which also has the holds checked again.

use super::Shared;
use super::board_requests::error_body;
use crate::protocol::{ErrorBody, ErrorKind, Event, Reply, Request};

pub(super) async fn handle(request: Request, shared: &Shared) -> Result<Reply, ErrorBody> {
    let agents = &shared.agents;
    let board = &shared.board;
    let changed = |reply: Reply| {
        shared.events.publish(Event::AgentsChanged);
        reply
    };
    let list = || async { agents.list().await.map(|l| Reply::Agents(Box::new(l))) };
    let answer = match request {
        Request::AgentsList => list().await,
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
        Request::AgentsDelete(p) => agents.delete(&p.id).await.map(|()| changed(Reply::Ok)),
        Request::AgentsInUse(p) => agents
            .in_use(&p.id)
            .await
            .map(|uses| Reply::AgentInUse(Box::new(uses))),
        Request::AgentsMoveRole(p) => agents
            .move_role(p.role, &p.agent)
            .await
            .map(|a| changed(Reply::Agent(Box::new(a)))),
        Request::AgentsDefaultModel(p) => match agents.set_default_model(&p.model).await {
            Ok(_) => list().await.map(changed),
            Err(e) => Err(e),
        },
        Request::AgentsFallback(p) => {
            match agents.set_fallback(p.tool, p.fallback.as_ref()).await {
                Ok(_) => list().await.map(changed),
                Err(e) => Err(e),
            }
        }
        Request::AgentsImportSkill(p) => crate::agents::skills::import(board, &p.id, &p.folder)
            .await
            .map(|a| changed(Reply::Agent(Box::new(a)))),
        Request::AgentsReimportSkill(p) => crate::agents::skills::reimport(board, &p.id, &p.skill)
            .await
            .map(|a| changed(Reply::Agent(Box::new(a)))),
        other => {
            return Err(ErrorBody::new(
                ErrorKind::BadRequest,
                format!("{} isn't an agent request", other.method()),
            ));
        }
    };
    answer.map_err(|e| error_body(&e))
}
