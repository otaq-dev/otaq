//! One client connection: a reader that answers requests in order, and a writer task that sends everything queued
//! for this client. Replies go through the same queue, so a slow client only ever slows itself down.

use std::sync::Arc;

use serde_json::json;
use tokio::io::BufReader;
use tokio::net::UnixStream;
use tokio::sync::mpsc;

use super::{Shared, until_stopped};
use crate::protocol::{
    Body, DecodeError, ErrorBody, ErrorKind, FrameReader, Message, PROTOCOL_VERSION, Reply,
    Request, decode, write_message,
};

/// Messages waiting to be written to one client.
const OUT_QUEUE: usize = 256;

pub(super) async fn serve(stream: UnixStream, shared: Arc<Shared>) {
    let (read, mut write) = stream.into_split();
    let (out, mut queued) = mpsc::channel::<Message>(OUT_QUEUE);
    let writer = tokio::spawn(async move {
        while let Some(message) = queued.recv().await {
            if write_message(&mut write, &message).await.is_err() {
                break;
            }
        }
    });

    let mut stopped = shared.stop.subscribe();
    let mut reader = FrameReader::new(BufReader::new(read));
    loop {
        let frame = tokio::select! {
            _ = until_stopped(&mut stopped) => break,
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
        let (reply, then) = handle(frame, &shared);
        if out.send(reply).await.is_err() {
            break;
        }
        if then == Then::Stop {
            tracing::info!("stop requested");
            shared.stop.send_replace(true);
        }
    }
    // Closing the queue lets the writer send what is left, then end.
    drop(out);
    let _ = writer.await;
}

#[derive(Debug, PartialEq, Eq)]
enum Then {
    Continue,
    Stop,
}

fn handle(frame: &[u8], shared: &Shared) -> (Message, Then) {
    match decode(frame) {
        Ok(Message {
            id,
            body: Body::Request(request),
            ..
        }) => respond(id, request, shared),
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
                }) => respond(foreign.id, request, shared),
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

fn respond(id: Option<u64>, request: Request, shared: &Shared) -> (Message, Then) {
    let (reply, then) = match request {
        Request::Ping => (Reply::Pong, Then::Continue),
        Request::ServiceStatus => (Reply::ServiceStatus(shared.status()), Then::Continue),
        Request::ServiceStop => (Reply::Ok, Then::Stop),
    };
    let mut message = Message::reply(0, reply);
    message.id = id;
    (message, then)
}
