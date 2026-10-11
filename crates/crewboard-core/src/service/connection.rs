//! One client connection: a reader that answers requests in order, and a writer task that sends everything queued
//! for this client. Replies go through the same queue, so a slow client only ever slows itself down.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use std::collections::HashMap;
use tokio::io::BufReader;

use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;

use super::{
    Shared, board_requests, events, permissions, session_requests, terminal_requests, until_stopped,
};
use crate::attention::ClientId;
use crate::caller::{self, Caller};
use crate::protocol::{
    Body, DecodeError, ErrorBody, ErrorKind, FrameReader, Message, OnboardingState,
    PROTOCOL_VERSION, Reply, Request, decode, write_message,
};
use crate::session::OutputChunk;
use crate::store::StoreError;
use crate::store::settings::PERMISSION_WARNING_DISMISSED;
use crate::tools;

/// Numbers the connections, so focus can be kept per client (ATT-03).
static NEXT_CLIENT: AtomicU64 = AtomicU64::new(1);

pub(super) async fn serve(stream: crate::ipc::ServerStream, shared: Arc<Shared>) {
    let client: ClientId = NEXT_CLIENT.fetch_add(1, Ordering::Relaxed);
    // The kernel's word on who connected; caller identity is resolved from it (CLI-01).
    let peer_pid = crate::ipc::peer_pid(&stream);
    let (read, mut write) = crate::ipc::split(stream);
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
    // While this client receives events, a notification click goes to it rather than starting the app.
    let mut subscribed = None;
    // Live output of the sessions this client attached to.
    let mut attached: HashMap<String, JoinHandle<()>> = HashMap::new();
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
        // In a task of its own: a handler that panics closes this connection (the client reconnects) instead of
        // ending the reader while the socket stays open, which would leave every later request unanswered.
        let request = tokio::spawn({
            let frame = frame.to_vec();
            let shared = shared.clone();
            async move { handle(&frame, peer_pid, client, &shared).await }
        });
        let (reply, then) = match request.await {
            Ok(answer) => answer,
            Err(e) => {
                tracing::error!(error = %e, "a request handler failed; closing the connection");
                break;
            }
        };
        // Subscribe before the reply goes out, so no event published after the reply is missed.
        let events = (matches!(then, Then::Subscribe) && forwarder.is_none())
            .then(|| shared.events.subscribe());
        if out.send(reply).await.is_err() {
            break;
        }
        if let Some(events) = events {
            subscribed = Some(shared.opener.subscribed());
            forwarder = Some(tokio::spawn(events::forward(
                events,
                out.clone(),
                kick.clone(),
            )));
        }
        match then {
            Then::Stop => {
                tracing::info!("stop requested");
                shared.stop.send_replace(true);
            }
            // The output receiver was subscribed with the scrollback in the reply, so nothing is lost or
            // repeated between the two.
            Then::Attach { session, output } => {
                let task = tokio::spawn(forward_output(
                    session.clone(),
                    output,
                    out.clone(),
                    kick.clone(),
                ));
                if let Some(old) = attached.insert(session, task) {
                    old.abort();
                }
            }
            Then::Detach(session) => {
                if let Some(task) = attached.remove(&session) {
                    task.abort();
                }
            }
            Then::Continue | Then::Subscribe => {}
        }
    }
    drop(subscribed);
    // A client that went away isn't looking at any terminal any more (ATT-03).
    shared.sessions.client_gone(client);
    for task in attached.into_values().chain(forwarder) {
        task.abort();
        let _ = task.await;
    }
    // Closing the queue lets the writer send what is left (unless kicked), then end.
    drop(out);
    let _ = writer.await;
}

pub(super) enum Then {
    Continue,
    Subscribe,
    Stop,
    /// Forward this session's live output to the client.
    Attach {
        session: String,
        output: broadcast::Receiver<OutputChunk>,
    },
    Detach(String),
}

/// Moves a session's live output into this connection's queue as `session_output` events until the session is
/// gone or the client detaches. A client that can't keep up is disconnected; it re-attaches and redraws from
/// the scrollback.
async fn forward_output(
    session: String,
    mut output: broadcast::Receiver<OutputChunk>,
    out: mpsc::Sender<Message>,
    kick: Arc<watch::Sender<bool>>,
) {
    use crate::protocol::{Event, encode_bytes};
    use tokio::sync::broadcast::error::RecvError;
    use tokio::sync::mpsc::error::TrySendError;
    loop {
        match output.recv().await {
            Ok(chunk) => {
                let event = Event::SessionOutput {
                    session: session.clone(),
                    offset: chunk.offset,
                    data: encode_bytes(&chunk.data),
                };
                match out.try_send(Message::event(event)) {
                    Ok(()) => {}
                    Err(TrySendError::Full(_)) => {
                        tracing::warn!(
                            session,
                            "attached client fell behind (queue full); disconnecting it"
                        );
                        kick.send_replace(true);
                        return;
                    }
                    Err(TrySendError::Closed(_)) => return,
                }
            }
            Err(RecvError::Lagged(missed)) => {
                tracing::warn!(
                    session,
                    missed,
                    "attached client fell behind; disconnecting it"
                );
                kick.send_replace(true);
                return;
            }
            Err(RecvError::Closed) => return,
        }
    }
}

async fn handle(
    frame: &[u8],
    peer_pid: Option<u32>,
    client: ClientId,
    shared: &Shared,
) -> (Message, Then) {
    match decode(frame) {
        Ok(Message {
            id,
            caller,
            pid,
            body: Body::Request(request),
            ..
        }) => {
            if let (Some(claimed), Some(peer)) = (pid, peer_pid)
                && claimed != peer
            {
                tracing::warn!(claimed, peer, "a request claimed another process's PID");
                let error = ErrorBody::new(
                    ErrorKind::BadRequest,
                    "the request's PID isn't the PID of the process on this connection",
                );
                return (Message::error(id, error), Then::Continue);
            }
            let who = caller::resolve(peer_pid, &shared.sessions);
            // Tickets created from a Project terminal are listed in its summary (T-0167).
            let origin = (!who.is_agent()
                && matches!(
                    request,
                    Request::TicketCreate(_) | Request::TicketCreateBatch(_)
                ))
            .then(|| caller::project_terminal_of(peer_pid, &shared.sessions))
            .flatten();
            respond(
                id,
                request,
                &who,
                caller.as_deref(),
                origin.as_deref(),
                client,
                shared,
            )
            .await
        }
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
                }) => {
                    respond(
                        foreign.id,
                        request,
                        &Caller::User,
                        None,
                        None,
                        client,
                        shared,
                    )
                    .await
                }
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
    who: &Caller,
    requested_actor: Option<&str>,
    origin: Option<&str>,
    client: ClientId,
    shared: &Shared,
) -> (Message, Then) {
    // The agent permission scope (CLI-02): checked once here, for every request.
    if let Err(e) = permissions::check(who, &request, &shared.board).await {
        return (
            Message::error(id, board_requests::error_body(&e)),
            Then::Continue,
        );
    }
    // Any `crewboard` command of an agent is a signal: it clears orange and moves the last signal (ATT-02, ATT-05).
    if who.is_agent()
        && !matches!(request, Request::Hook(_))
        && let Err(e) = shared.signals.command(who).await
    {
        tracing::warn!(error = %e, "can't record an agent command");
    }
    let method = request.method();
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
        Request::Hook(p) => match shared.signals.hook(who, p.event, &p.payload).await {
            Ok(signal) => {
                shared.after_signal(who, &signal);
                (Reply::Ok, Then::Continue)
            }
            Err(e) => {
                return (
                    Message::error(id, board_requests::error_body(&e)),
                    Then::Continue,
                );
            }
        },
        request @ (Request::SessionList(_)
        | Request::SessionAttach(_)
        | Request::SessionDetach(_)
        | Request::SessionResize(_)
        | Request::SessionInput(_)
        | Request::SessionDeliverNow(_)
        | Request::SessionFocus(_)) => {
            match session_requests::handle(request, client, shared).await {
                Ok(answer) => answer,
                Err(error) => return (Message::error(id, error), Then::Continue),
            }
        }
        request @ (Request::RunStart(_)
        | Request::RunStop(_)
        | Request::RunStatus(_)
        | Request::RunList
        | Request::RunSuggest(_)
        | Request::RunSuggestCancel(_)
        | Request::ShellOpen(_)
        | Request::ProjectTerminalOpen(_)
        | Request::ProjectTerminalCreated(_)
        | Request::ProjectTerminalDismiss(_)) => {
            match terminal_requests::handle(request, shared).await {
                Ok(reply) => (reply, Then::Continue),
                Err(error) => return (Message::error(id, error), Then::Continue),
            }
        }
        // Boxed: the board handler's future is large, and a debug build's worker stack is small.
        board => match Box::pin(board_requests::handle(
            board,
            who,
            requested_actor,
            origin,
            shared,
        ))
        .await
        {
            Some(Ok(reply)) => (reply, Then::Continue),
            Some(Err(error)) => return (Message::error(id, error), Then::Continue),
            // A request routed to neither list: answer it rather than end this connection's reader, which
            // would leave the client waiting forever on every later request.
            None => {
                tracing::error!(method, "request not routed to any handler");
                let error = ErrorBody::new(
                    ErrorKind::Internal,
                    format!("{method} isn't handled by the background service"),
                );
                return (Message::error(id, error), Then::Continue);
            }
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
