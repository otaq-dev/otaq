//! Talking to the background service from the app and the CLI.

use std::fmt;
use std::io;
use std::path::Path;

use tokio::io::BufReader;
use tokio::net::UnixStream;

use crate::protocol::{
    Body, DecodeError, ErrorBody, FrameReader, Message, Reply, Request, decode, write_message,
};

/// Sends one request on a fresh connection and waits for its answer. Version-independent requests
/// (`ping`, `service.status`, `service.stop`) also work against a service with another protocol version.
pub async fn request_once(socket: &Path, request: Request) -> Result<Reply, ClientError> {
    let stream = UnixStream::connect(socket)
        .await
        .map_err(ClientError::NotRunning)?;
    let (read, mut write) = stream.into_split();
    const ID: u64 = 1;
    write_message(&mut write, &Message::request(ID, request))
        .await
        .map_err(ClientError::Io)?;
    let mut reader = FrameReader::new(BufReader::new(read));
    loop {
        let frame = reader
            .next_frame()
            .await
            .map_err(ClientError::Io)?
            .ok_or(ClientError::Closed)?;
        let message = match decode(frame) {
            Ok(message) => message,
            Err(DecodeError::IncompatibleVersion(foreign)) => foreign.read_as_current().ok_or(
                ClientError::Protocol(DecodeError::IncompatibleVersion(foreign)),
            )?,
            Err(e) => return Err(ClientError::Protocol(e)),
        };
        if message.id != Some(ID) {
            continue;
        }
        return match message.body {
            Body::Reply(reply) => Ok(reply),
            Body::Error(error) => Err(ClientError::Service(error)),
            _ => Err(ClientError::Closed),
        };
    }
}

#[derive(Debug)]
pub enum ClientError {
    /// No service is listening on the socket.
    NotRunning(io::Error),
    /// The connection ended before the answer arrived.
    Closed,
    Io(io::Error),
    /// The service sent something this build can't read.
    Protocol(DecodeError),
    /// The service answered with an error.
    Service(ErrorBody),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRunning(_) => write!(f, "the background service isn't running"),
            Self::Closed => write!(f, "the background service closed the connection"),
            Self::Io(e) => write!(f, "connection to the background service failed: {e}"),
            Self::Protocol(e) => write!(f, "{e}"),
            Self::Service(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ClientError {}
