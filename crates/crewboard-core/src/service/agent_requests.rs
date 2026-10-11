//! Project agent requests (PAG-01, PAG-04, PAG-06; d0068): open, list, close, resume and rename (the user's), and
//! `crewboard agent name` (the Project agent's own).

use super::Shared;
use super::board_requests::error_body;
use crate::board::{BoardError, Refusal};
use crate::caller::Caller;
use crate::project_agents::ONLY_A_PROJECT_AGENT;
use crate::protocol::{ErrorBody, ErrorKind, Reply, Request};

pub(super) async fn handle(
    request: Request,
    caller: &Caller,
    shared: &Shared,
) -> Result<Reply, ErrorBody> {
    let agents = &shared.project_agents;
    let agent = |r: Result<crate::project_agents::ProjectAgent, BoardError>| {
        r.map(|a| Reply::ProjectAgent(Box::new(a)))
            .map_err(|e| error_body(&e))
    };
    match request {
        Request::ProjectAgentOpen(p) => agent(agents.open(&p.project).await),
        Request::ProjectAgentList(p) => agents
            .list(&p.project)
            .await
            .map(|l| Reply::ProjectAgents(Box::new(l)))
            .map_err(|e| error_body(&e)),
        Request::ProjectAgentClose(p) => agents
            .close(&p.id, p.confirm)
            .await
            .map(|()| Reply::Ok)
            .map_err(|e| error_body(&e)),
        Request::ProjectAgentResume(p) => agent(agents.resume(&p.id).await),
        Request::ProjectAgentRename(p) => agent(agents.rename(&p.id, &p.name).await),
        Request::AgentName(p) => {
            let Caller::ProjectAgent { agent: id, .. } = caller else {
                return Err(error_body(&BoardError::refused(
                    Refusal::NotAllowed,
                    ONLY_A_PROJECT_AGENT,
                )));
            };
            agent(agents.name_by_agent(id, &p.name, p.rename).await)
        }
        other => Err(ErrorBody::new(
            ErrorKind::BadRequest,
            format!("{} isn't a Project agent request", other.method()),
        )),
    }
}
