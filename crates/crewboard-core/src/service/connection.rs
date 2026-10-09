//! One client connection: a reader that answers requests in order, and a writer task that sends everything queued
//! for this client. Replies go through the same queue, so a slow client only ever slows itself down.

use std::sync::Arc;

use serde_json::json;
use tokio::io::BufReader;
use tokio::net::UnixStream;
use tokio::sync::{mpsc, watch};

use super::{Shared, board_requests, events, until_stopped};
use crate::protocol::{
    Body, DecodeError, ErrorBody, ErrorKind, FrameReader, Message, OnboardingState,
    PROTOCOL_VERSION, Reply, Request, decode, write_message,
};
use crate::store::StoreError;
use crate::store::settings::PERMISSION_WARNING_DISMISSED;
use crate::tools;

pub(super) async fn serve(stream: UnixStream, shared: Arc<Shared>) {
    let (read, mut write) = stream.into_split();
    let (out, mut queued) = mpsc::channel::<Message>(shared.config.client_queue);
    // Set when this client must be dropped at once (it fell behind on events).
    let (kick, kicked) = watch::channel(false);
    let kick = Arc::new(kick);

    let mut writer_kicked = kicked.clone();
    let writer = tokio::spawn(async move {
        loop {
            let message = tokio::select! {
                biased;
                _ = until_stopped(&mut writer_kicked) => break,
                message = queued.recv() => match message {
                    Some(message) => message,
                    None => break,
                },
            };
            tokio::select! {
                biased;
                _ = until_stopped(&mut writer_kicked) => break,
                written = write_message(&mut write, &message) => if written.is_err() { break },
            }
        }
    });

    let mut stopped = shared.stop.subscribe();
    let mut reader_kicked = kicked;
    let mut forwarder = None;
    let mut reader = FrameReader::new(BufReader::new(read));
    loop {
        let frame = tokio::select! {
            _ = until_stopped(&mut stopped) => break,
            _ = until_stopped(&mut reader_kicked) => break,
            frame = reader.next_frame() => frame,
        };
        let frame = match frame {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(e) => {
                tracing::debug!(error = %e, "connection read failed");
                break;
            }
        };
        let (reply, then) = handle(frame, &shared).await;
        // Subscribe before the reply goes out, so no event published after the reply is missed.
        let events =
            (then == Then::Subscribe && forwarder.is_none()).then(|| shared.events.subscribe());
        if out.send(reply).await.is_err() {
            break;
        }
        if let Some(events) = events {
            forwarder = Some(tokio::spawn(events::forward(
                events,
                out.clone(),
                kick.clone(),
            )));
        }
        if then == Then::Stop {
            tracing::info!("stop requested");
            shared.stop.send_replace(true);
        }
    }
    if let Some(forwarder) = forwarder {
        forwarder.abort();
        let _ = forwarder.await;
    }
    // Closing the queue lets the writer send what is left (unless kicked), then end.
    drop(out);
    let _ = writer.await;
}

#[derive(Debug, PartialEq, Eq)]
enum Then {
    Continue,
    Subscribe,
    Stop,
}

async fn handle(frame: &[u8], shared: &Shared) -> (Message, Then) {
    match decode(frame) {
        Ok(Message {
            id,
            caller,
            body: Body::Request(request),
            ..
        }) => respond(id, request, caller.as_deref(), shared).await,
        Ok(Message { id, .. }) => (
            Message::error(
                id,
                ErrorBody::new(ErrorKind::BadRequest, "expected a request"),
            ),
            Then::Continue,
        ),
        Err(DecodeError::IncompatibleVersion(foreign)) => {
            let current = foreign
                .is_version_independent_request()
                .then(|| foreign.read_as_current())
                .flatten();
            match current {
                Some(Message {
                    body: Body::Request(request),
                    ..
                }) => respond(foreign.id, request, None, shared).await,
                _ => {
                    let mut error = ErrorBody::new(
                        ErrorKind::IncompatibleProtocol,
                        format!(
                            "the background service speaks protocol {PROTOCOL_VERSION}, not {}; restart it",
                            foreign.protocol_version
                        ),
                    );
                    error.data = Some(json!({
                        "protocol_version": PROTOCOL_VERSION,
                        "build_id": shared.config.build_id,
                    }));
                    (Message::error(foreign.id, error), Then::Continue)
                }
            }
        }
        Err(e @ DecodeError::Malformed(_)) => (
            Message::error(None, ErrorBody::new(e.kind(), e.to_string())),
            Then::Continue,
        ),
    }
}

async fn respond(
    id: Option<u64>,
    request: Request,
    caller: Option<&str>,
    shared: &Shared,
) -> (Message, Then) {
    let (reply, then) = match request {
        Request::Ping => (Reply::Pong, Then::Continue),
        Request::ServiceStatus => (Reply::ServiceStatus(shared.status().await), Then::Continue),
        Request::ServiceStop => (Reply::Ok, Then::Stop),
        Request::Subscribe => (Reply::Ok, Then::Subscribe),
        Request::ToolsCheck => (
            Reply::ToolsCheck(tools::check(&shared.config.tools).await),
            Then::Continue,
        ),
        Request::OnboardingState => match onboarding_state(shared).await {
            Ok(state) => (Reply::OnboardingState(state), Then::Continue),
            Err(e) => return (store_error(id, &e), Then::Continue),
        },
        Request::DismissPermissionWarning => match shared
            .store
            .set_setting(PERMISSION_WARNING_DISMISSED, "true")
            .await
        {
            Ok(()) => (Reply::Ok, Then::Continue),
            Err(e) => return (store_error(id, &e), Then::Continue),
        },
        board => match board_requests::handle(board, caller, shared).await {
            Some(Ok(reply)) => (reply, Then::Continue),
            Some(Err(error)) => return (Message::error(id, error), Then::Continue),
            None => unreachable!("service requests are answered above"),
        },
    };
    let mut message = Message::reply(0, reply);
    message.id = id;
    (message, then)
}

async fn onboarding_state(shared: &Shared) -> Result<OnboardingState, StoreError> {
    Ok(OnboardingState {
        has_projects: shared.store.project_count().await? > 0,
        permission_warning_dismissed: shared
            .store
            .setting(PERMISSION_WARNING_DISMISSED)
            .await?
            .is_some_and(|v| v == "true"),
    })
}

fn store_error(id: Option<u64>, e: &StoreError) -> Message {
    tracing::error!(error = %e, "store request failed");
    Message::error(id, ErrorBody::new(ErrorKind::Internal, e.to_string()))
}
