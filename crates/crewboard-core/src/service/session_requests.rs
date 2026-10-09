//! Session requests: list, attach (stored output + live output on this connection), detach and resize
//! (SES-04, SVC-05).

use serde_json::json;

use super::Shared;
use super::board_requests::error_body;
use super::connection::Then;
use crate::board::Refusal;
use crate::protocol::{
    ErrorBody, ErrorKind, Reply, Request, SessionAttach, decode_bytes, encode_bytes,
};
use crate::session::{SessionError, SessionKind, SessionTicket};

pub(super) async fn handle(request: Request, shared: &Shared) -> Result<(Reply, Then), ErrorBody> {
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
            let id = match (target.session, target.key) {
                (Some(id), _) => id,
                (None, Some(key)) => {
                    let ticket = ticket(shared, &key).await?;
                    sessions
                        .latest_of(&ticket.id, SessionKind::Agent)
                        .ok_or_else(|| {
                            ErrorBody::new(
                                ErrorKind::NotFound,
                                format!("{} has no live session", ticket.key),
                            )
                        })?
                        .id
                }
                (None, None) => {
                    return Err(ErrorBody::new(
                        ErrorKind::BadRequest,
                        "name a session or a ticket key",
                    ));
                }
            };
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
            sessions.input(&p.session, &bytes).map_err(session_error)?;
            Ok((Reply::Ok, Then::Continue))
        }
        other => Err(ErrorBody::new(
            ErrorKind::BadRequest,
            format!("{} isn't a session request", other.method()),
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
