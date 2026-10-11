//! Session requests: list, attach (stored output + live output on this connection), detach, resize, input
//! (SES-04, SVC-05, SVC-06) and Deliver now (SES-06, SES-07).

use serde_json::json;

use super::Shared;
use super::board_requests::error_body;
use super::connection::Then;
use crate::attention::ClientId;
use crate::board::Refusal;
use crate::protocol::{
    ErrorBody, ErrorKind, Reply, Request, SessionAttach, SessionTarget, decode_bytes, encode_bytes,
};
use crate::session::{SessionError, SessionKind, SessionTicket};

pub(super) async fn handle(
    request: Request,
    client: ClientId,
    shared: &Shared,
) -> Result<(Reply, Then), ErrorBody> {
    let sessions = &shared.sessions;
    match request {
        Request::SessionList(p) => {
            let list = match p.key {
                Some(key) => {
                    let ticket = ticket(shared, &key).await?;
                    sessions.history(&ticket).await.map_err(session_error)?
                }
                None => sessions.in_memory(),
            };
            Ok((Reply::Sessions(list), Then::Continue))
        }
        Request::SessionAttach(target) => {
            let id = session_id(shared, target).await?;
            let attached = sessions.attach(&id).map_err(session_error)?;
            let reply = Reply::SessionAttached(Box::new(SessionAttach {
                session: attached.info,
                scrollback: encode_bytes(&attached.scrollback),
                start: attached.start,
                end: attached.end,
            }));
            Ok((
                reply,
                Then::Attach {
                    session: id,
                    output: attached.output,
                },
            ))
        }
        Request::SessionDetach(p) => Ok((Reply::Ok, Then::Detach(p.session))),
        Request::SessionResize(p) => {
            sessions.resize(&p.session, p.size).map_err(session_error)?;
            Ok((Reply::Ok, Then::Continue))
        }
        Request::SessionInput(p) => {
            let bytes = decode_bytes(&p.data).map_err(|e| {
                ErrorBody::new(ErrorKind::BadRequest, format!("input isn't base64: {e}"))
            })?;
            if p.reply {
                sessions.write(&p.session, &bytes).map_err(session_error)?;
                return Ok((Reply::Ok, Then::Continue));
            }
            let effect = sessions.input(&p.session, &bytes).map_err(session_error)?;
            // Enter in the ticket's terminal is the user's answer: the orange label clears (ATT-02).
            if effect.enters > 0
                && let Some(key) = sessions.info(&p.session).ok().and_then(|i| i.ticket)
                && let Ok(id) = shared.board.ticket_id(&key.to_string()).await
                && let Err(e) = shared.signals.enter(&id).await
            {
                tracing::warn!(session = p.session, error = %e, "can't clear the signal");
            }
            // Enter in a Project agent's terminal answers its permission prompt (ATT-02, d0068).
            if effect.enters > 0
                && sessions
                    .info(&p.session)
                    .is_ok_and(|i| i.kind == crate::session::SessionKind::ProjectAgent)
                && let Err(e) = shared.project_agents.enter(&p.session).await
            {
                tracing::warn!(session = p.session, error = %e, "can't clear the Project agent's wait");
            }
            // An Enter or a backspace may have emptied the line: waiting messages may go now.
            if !sessions.has_unsent_input(&p.session).unwrap_or(true) {
                let deliveries = shared.deliveries.clone();
                let session = p.session.clone();
                tokio::spawn(async move {
                    if let Err(e) = deliveries.deliver_if_ready(&session).await {
                        tracing::warn!(session, error = %e, "delivery failed");
                    }
                });
            }
            Ok((Reply::Ok, Then::Continue))
        }
        Request::SessionFocus(p) => {
            sessions
                .set_focused(&p.session, client, p.focused)
                .map_err(session_error)?;
            Ok((Reply::Ok, Then::Continue))
        }
        Request::SessionDeliverNow(p) => {
            let id = session_id(shared, p.target).await?;
            let delivery = if p.clear_first {
                shared.deliveries.clear_and_send(&id, None).await
            } else {
                shared.deliveries.deliver_now(&id).await
            }
            .map_err(session_error)?;
            Ok((Reply::Delivery(delivery), Then::Continue))
        }
        other => Err(ErrorBody::new(
            ErrorKind::BadRequest,
            format!("{} isn't a session request", other.method()),
        )),
    }
}

/// A session by id, the newest session of a kind (agent by default) of a ticket by key, or a project's Project
/// terminal.
async fn session_id(shared: &Shared, target: SessionTarget) -> Result<String, ErrorBody> {
    let kind = target.kind.unwrap_or(SessionKind::Agent);
    match (target.session, target.key, target.project) {
        (Some(id), _, _) => Ok(id),
        (None, Some(key), _) => {
            let ticket = ticket(shared, &key).await?;
            shared
                .sessions
                .latest_of(&ticket.id, kind)
                .map(|s| s.id)
                .ok_or_else(|| {
                    ErrorBody::new(
                        ErrorKind::NotFound,
                        match kind {
                            SessionKind::Agent => format!("{} has no live session", ticket.key),
                            other => format!("{} has no live {other} session", ticket.key),
                        },
                    )
                })
        }
        (None, None, Some(project)) => shared
            .shells
            .latest_project_terminal(&project)
            .map(|t| t.session.id)
            .ok_or_else(|| {
                ErrorBody::new(
                    ErrorKind::NotFound,
                    format!("{project} has no Project terminal"),
                )
            }),
        (None, None, None) => Err(ErrorBody::new(
            ErrorKind::BadRequest,
            "name a session, a ticket key or a project",
        )),
    }
}

async fn ticket(shared: &Shared, key: &str) -> Result<SessionTicket, ErrorBody> {
    let ticket = shared.board.ticket(key).await.map_err(|e| error_body(&e))?;
    Ok(SessionTicket {
        id: ticket.id,
        key: ticket.key,
    })
}

pub(super) fn session_error(e: SessionError) -> ErrorBody {
    let refused = |code: Refusal, message: String| {
        let mut body = ErrorBody::new(ErrorKind::Refused, message);
        body.data = Some(json!({ "refusal": code }));
        body
    };
    match e {
        SessionError::NotFound(_) => ErrorBody::new(ErrorKind::NotFound, e.to_string()),
        SessionError::AlreadyRunning { .. } => refused(Refusal::AlreadyRunning, e.to_string()),
        SessionError::Ended(_) => refused(Refusal::SessionEnded, e.to_string()),
        SessionError::Spawn(_) | SessionError::Io(_) | SessionError::Store(_) => {
            tracing::error!(error = %e, "session request failed");
            ErrorBody::new(ErrorKind::Internal, e.to_string())
        }
    }
}
