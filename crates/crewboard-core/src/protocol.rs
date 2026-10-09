//! The one protocol spoken by the app, the CLI and the background service (d0055): JSON messages over the
//! user-locked local socket, as requests and replies plus a live event stream.
//!
//! # Messages
//!
//! Every message is one JSON object carrying the [`PROTOCOL_VERSION`], an optional request id and exactly one
//! body: `request`, `reply`, `event` or `error`.
//!
//! ```text
//! {"protocol_version":1,"id":7,"request":{"method":"service.status"}}
//! {"protocol_version":1,"id":7,"reply":{"service_status":{"pid":4120,...}}}
//! {"protocol_version":1,"event":{"kind":"test","seq":3}}
//! {"protocol_version":1,"id":8,"error":{"kind":"bad_request","message":"..."}}
//! ```
//!
//! A reply or error answers the request with the same id. Events have no id.
//!
//! # Framing (implementation choice, SDD §26)
//!
//! Newline-delimited JSON: each message is serialized compactly (JSON never contains a raw newline there)
//! and followed by `\n`. A frame may be at most [`MAX_FRAME_BYTES`] long. Chosen over length prefixes because
//! it is trivial to read in logs and to poke at by hand (`nc -U ~/.crewboard/crewboard.sock`).
//!
//! # Versions
//!
//! A peer with another `protocol_version` is reported as [`DecodeError::IncompatibleVersion`]
//! ([`ErrorKind::IncompatibleProtocol`]), which drives "Restart required" (d0022). The envelope fields and the
//! [`VERSION_INDEPENDENT_METHODS`] (`ping`, `service.status`, `service.stop`) never change incompatibly, so an
//! app can always learn the old service's status and restart it.

use std::fmt;
use std::io;

use crate::tools::ToolsReport;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Bumped whenever a message changes incompatibly. Adding optional fields or new methods doesn't need a bump.
pub const PROTOCOL_VERSION: u32 = 1;

/// The longest frame accepted, in bytes (newline included).
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Requests every service answers whatever protocol version the caller speaks. Their request and reply
/// shapes may only grow optional fields.
pub const VERSION_INDEPENDENT_METHODS: &[&str] = &["ping", "service.status", "service.stop"];

/// This build's ID, generated at compile time by `build.rs`: `<git hash>[-dirty].<build time>`.
/// The app, the CLI and the service all report the same value when built together.
pub fn build_id() -> &'static str {
    env!("BUILD_ID")
}

/// One message on the socket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub protocol_version: u32,
    /// Set on requests, and on the reply or error that answers one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// Who sends a request, for example `agent:SHOP-18` (SDD §11.3). Filled by the CLI from M4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
    #[serde(flatten)]
    pub body: Body,
}

impl Message {
    pub fn request(id: u64, request: Request) -> Self {
        Self::new(Some(id), Body::Request(request))
    }

    pub fn reply(id: u64, reply: Reply) -> Self {
        Self::new(Some(id), Body::Reply(reply))
    }

    pub fn error(id: Option<u64>, error: ErrorBody) -> Self {
        Self::new(id, Body::Error(error))
    }

    pub fn event(event: Event) -> Self {
        Self::new(None, Body::Event(event))
    }

    fn new(id: Option<u64>, body: Body) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            id,
            caller: None,
            body,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Body {
    Request(Request),
    Reply(Reply),
    Event(Event),
    Error(ErrorBody),
}

/// What a client asks the service. Message names are an implementation choice (SDD §5.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Request {
    /// Answered with [`Reply::Pong`].
    #[serde(rename = "ping")]
    Ping,
    /// Answered with [`Reply::ServiceStatus`].
    #[serde(rename = "service.status")]
    ServiceStatus,
    /// Asks the service to shut down; answered with [`Reply::Ok`] before it stops.
    #[serde(rename = "service.stop")]
    ServiceStop,
    /// Starts the event stream on this connection; answered with [`Reply::Ok`], then [`Event`]s follow.
    #[serde(rename = "events.subscribe")]
    Subscribe,
    /// The first-run tools checklist and permission-mode check; answered with [`Reply::ToolsCheck`].
    #[serde(rename = "tools.check")]
    ToolsCheck,
}

impl Request {
    /// The method name as sent on the wire.
    pub fn method(&self) -> &'static str {
        match self {
            Self::Ping => "ping",
            Self::ServiceStatus => "service.status",
            Self::ServiceStop => "service.stop",
            Self::Subscribe => "events.subscribe",
            Self::ToolsCheck => "tools.check",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reply {
    Ok,
    Pong,
    ServiceStatus(ServiceStatus),
    ToolsCheck(ToolsReport),
}

/// The service's answer to `service.status`. Version-independent: fields may be added (with defaults), never
/// renamed or removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub pid: u32,
    pub build_id: String,
    pub protocol_version: u32,
    /// When the service started, in Unix milliseconds.
    pub started_at: i64,
    pub uptime_secs: u64,
    /// Agents working right now, for "Restart now (3 agents working, 2 waiting)" (d0022).
    #[serde(default)]
    pub agents_working: u32,
    /// Agents waiting for the user (the orange family, d0042).
    #[serde(default)]
    pub agents_waiting: u32,
}

/// Something that happened in the service, pushed to subscribers (SDD §5.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// Only used by tests, so the event stream is testable before real events exist.
    Test { seq: u64 },
}

/// The `{"error": {...}}` body, used on the socket and printed by the CLI with `--json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub kind: ErrorKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ErrorBody {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            data: None,
        }
    }
}

impl fmt::Display for ErrorBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// The peer speaks another protocol version ("Restart required", d0022).
    IncompatibleProtocol,
    /// The message couldn't be parsed or isn't a valid request.
    BadRequest,
    /// The request was valid but the service failed to carry it out.
    Internal,
    /// The background service isn't reachable.
    Disconnected,
    /// Wrong command-line usage.
    Usage,
    /// A kind this build doesn't know (sent by a newer peer).
    #[serde(other)]
    Unknown,
}

/// Why a frame couldn't be turned into a [`Message`].
#[derive(Debug)]
pub enum DecodeError {
    /// Not JSON, or not a message this build understands.
    Malformed(serde_json::Error),
    /// A well-formed message from a peer with another protocol version.
    IncompatibleVersion(ForeignMessage),
}

impl DecodeError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Malformed(_) => ErrorKind::BadRequest,
            Self::IncompatibleVersion(_) => ErrorKind::IncompatibleProtocol,
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(e) => write!(f, "malformed message: {e}"),
            Self::IncompatibleVersion(m) => write!(
                f,
                "incompatible protocol version {} (this build speaks {PROTOCOL_VERSION})",
                m.protocol_version
            ),
        }
    }
}

impl std::error::Error for DecodeError {}

/// A message from a peer speaking another protocol version, kept raw. Only its envelope and the
/// [`VERSION_INDEPENDENT_METHODS`] can be relied on.
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignMessage {
    pub protocol_version: u32,
    pub id: Option<u64>,
    pub raw: Value,
}

impl ForeignMessage {
    /// The request method, if this is a request.
    pub fn method(&self) -> Option<&str> {
        self.raw.get("request")?.get("method")?.as_str()
    }

    /// Whether this is a request every service answers regardless of version.
    pub fn is_version_independent_request(&self) -> bool {
        self.method()
            .is_some_and(|m| VERSION_INDEPENDENT_METHODS.contains(&m))
    }

    /// Reads the message with this build's types. Works for the version-independent requests and replies.
    pub fn read_as_current(&self) -> Option<Message> {
        let mut raw = self.raw.clone();
        raw.as_object_mut()?
            .insert("protocol_version".into(), PROTOCOL_VERSION.into());
        serde_json::from_value(raw).ok()
    }
}

/// Serializes a message as one frame: compact JSON followed by `\n`.
pub fn encode(message: &Message) -> Vec<u8> {
    let mut frame = serde_json::to_vec(message).expect("messages always serialize");
    frame.push(b'\n');
    frame
}

/// Parses one frame (with or without its trailing newline). The protocol version is checked before the body,
/// so a peer with another version is reported as such even if its messages no longer parse.
pub fn decode(frame: &[u8]) -> Result<Message, DecodeError> {
    let raw: Value = serde_json::from_slice(frame).map_err(DecodeError::Malformed)?;
    let version = raw
        .get("protocol_version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok());
    match version {
        Some(v) if v != PROTOCOL_VERSION => Err(DecodeError::IncompatibleVersion(ForeignMessage {
            protocol_version: v,
            id: raw.get("id").and_then(Value::as_u64),
            raw,
        })),
        _ => serde_json::from_value(raw).map_err(DecodeError::Malformed),
    }
}

/// Reads frames from a byte stream. Handles partial reads; a frame may arrive in any number of pieces.
pub struct FrameReader<R> {
    inner: R,
    buf: Vec<u8>,
}

impl<R: AsyncBufRead + Unpin> FrameReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            buf: Vec::new(),
        }
    }

    /// The next frame without its newline, or `None` at a clean end of stream.
    pub async fn next_frame(&mut self) -> io::Result<Option<&[u8]>> {
        self.buf.clear();
        let limit = MAX_FRAME_BYTES as u64;
        let n = (&mut self.inner)
            .take(limit)
            .read_until(b'\n', &mut self.buf)
            .await?;
        if n == 0 {
            return Ok(None);
        }
        if self.buf.last() != Some(&b'\n') {
            let error = if self.buf.len() >= MAX_FRAME_BYTES {
                io::Error::new(io::ErrorKind::InvalidData, "frame too large")
            } else {
                io::Error::new(io::ErrorKind::UnexpectedEof, "stream ended mid-frame")
            };
            return Err(error);
        }
        self.buf.pop();
        Ok(Some(&self.buf))
    }
}

/// Writes one message as a frame and flushes it.
pub async fn write_message<W: AsyncWrite + Unpin>(w: &mut W, message: &Message) -> io::Result<()> {
    w.write_all(&encode(message)).await?;
    w.flush().await
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use serde_json::json;
    use tokio::io::{AsyncRead, BufReader, ReadBuf};

    use super::*;

    fn status() -> ServiceStatus {
        ServiceStatus {
            pid: 4120,
            build_id: "abc.1".into(),
            protocol_version: PROTOCOL_VERSION,
            started_at: 1_791_504_000_000,
            uptime_secs: 42,
            agents_working: 3,
            agents_waiting: 2,
        }
    }

    fn samples() -> Vec<Message> {
        vec![
            Message::request(1, Request::Ping),
            Message::request(2, Request::ServiceStatus),
            Message::request(3, Request::ServiceStop),
            Message::request(6, Request::Subscribe),
            Message::request(7, Request::ToolsCheck),
            Message::reply(1, Reply::Pong),
            Message::reply(2, Reply::ServiceStatus(status())),
            Message::reply(3, Reply::Ok),
            Message::event(Event::Test { seq: 9 }),
            Message::error(
                Some(4),
                ErrorBody::new(ErrorKind::BadRequest, "unknown method"),
            ),
            Message {
                caller: Some("agent:SHOP-18".into()),
                ..Message::request(5, Request::Ping)
            },
        ]
    }

    #[test]
    fn round_trips() {
        for message in samples() {
            let frame = encode(&message);
            assert_eq!(frame.last(), Some(&b'\n'));
            assert_eq!(
                frame.iter().filter(|b| **b == b'\n').count(),
                1,
                "one line per message"
            );
            assert_eq!(decode(&frame).unwrap(), message);
        }
    }

    #[test]
    fn wire_shapes() {
        let as_json = |m: &Message| serde_json::to_value(m).unwrap();
        assert_eq!(
            as_json(&Message::request(7, Request::ServiceStatus)),
            json!({"protocol_version": 1, "id": 7, "request": {"method": "service.status"}})
        );
        assert_eq!(
            as_json(&Message::error(
                Some(8),
                ErrorBody::new(ErrorKind::IncompatibleProtocol, "x")
            )),
            json!({"protocol_version": 1, "id": 8, "error": {"kind": "incompatible_protocol", "message": "x"}})
        );
        assert_eq!(
            as_json(&Message::event(Event::Test { seq: 3 })),
            json!({"protocol_version": 1, "event": {"kind": "test", "seq": 3}})
        );
        for m in VERSION_INDEPENDENT_METHODS {
            assert!(
                [Request::Ping, Request::ServiceStatus, Request::ServiceStop]
                    .iter()
                    .any(|r| r.method() == *m)
            );
        }
    }

    #[test]
    fn unknown_error_kinds_are_tolerated() {
        let m = decode(
            br#"{"protocol_version":1,"id":1,"error":{"kind":"from_the_future","message":"m"}}"#,
        )
        .unwrap();
        let Body::Error(e) = m.body else {
            panic!("{m:?}")
        };
        assert_eq!(e.kind, ErrorKind::Unknown);
    }

    #[test]
    fn version_mismatch_is_a_distinct_error() {
        let frame = br#"{"protocol_version":2,"id":5,"request":{"method":"tickets.frobnicate","params":{"x":1}}}"#;
        let err = decode(frame).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::IncompatibleProtocol);
        let DecodeError::IncompatibleVersion(foreign) = err else {
            panic!()
        };
        assert_eq!(foreign.protocol_version, 2);
        assert_eq!(foreign.id, Some(5));
        assert_eq!(foreign.method(), Some("tickets.frobnicate"));
        assert!(!foreign.is_version_independent_request());
    }

    #[test]
    fn version_independent_messages_read_across_versions() {
        let request = br#"{"protocol_version":9,"id":1,"request":{"method":"service.status"}}"#;
        let DecodeError::IncompatibleVersion(foreign) = decode(request).unwrap_err() else {
            panic!()
        };
        assert!(foreign.is_version_independent_request());
        assert_eq!(
            foreign.read_as_current().unwrap().body,
            Body::Request(Request::ServiceStatus)
        );

        // A newer service's status reply, with a field this build doesn't know.
        let mut reply =
            serde_json::to_value(Message::reply(1, Reply::ServiceStatus(status()))).unwrap();
        reply["protocol_version"] = json!(9);
        reply["reply"]["service_status"]["new_field"] = json!(true);
        let DecodeError::IncompatibleVersion(foreign) =
            decode(&serde_json::to_vec(&reply).unwrap()).unwrap_err()
        else {
            panic!()
        };
        assert_eq!(
            foreign.read_as_current().unwrap().body,
            Body::Reply(Reply::ServiceStatus(status()))
        );
    }

    #[test]
    fn malformed_frames() {
        for frame in [
            &b"not json"[..],
            br#"{"protocol_version":1}"#,
            br#"{"id":1,"request":{"method":"ping"}}"#,
        ] {
            let err = decode(frame).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::BadRequest, "{frame:?}");
        }
    }

    #[test]
    fn build_id_has_a_hash_and_a_time() {
        let id = build_id();
        let (hash, secs) = id.rsplit_once('.').expect(id);
        assert!(!hash.is_empty(), "{id}");
        assert!(secs.parse::<u64>().unwrap() > 0, "{id}");
    }

    /// Yields at most `chunk` bytes per read, to exercise partial reads.
    struct Trickle {
        data: Vec<u8>,
        pos: usize,
        chunk: usize,
    }

    impl AsyncRead for Trickle {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            let end = (self.pos + self.chunk)
                .min(self.data.len())
                .min(self.pos + buf.remaining());
            let piece = self.data[self.pos..end].to_vec();
            buf.put_slice(&piece);
            self.pos = end;
            Poll::Ready(Ok(()))
        }
    }

    fn trickle(data: Vec<u8>, chunk: usize) -> FrameReader<BufReader<Trickle>> {
        FrameReader::new(BufReader::with_capacity(
            4,
            Trickle {
                data,
                pos: 0,
                chunk,
            },
        ))
    }

    #[tokio::test]
    async fn framing_with_partial_reads() {
        let messages = samples();
        let stream: Vec<u8> = messages.iter().flat_map(encode).collect();
        for chunk in [1, 3, 7, 1000] {
            let mut reader = trickle(stream.clone(), chunk);
            let mut got = Vec::new();
            while let Some(frame) = reader.next_frame().await.unwrap() {
                got.push(decode(frame).unwrap());
            }
            assert_eq!(got, messages, "chunk size {chunk}");
        }
    }

    #[tokio::test]
    async fn stream_ending_mid_frame_is_an_error() {
        let mut stream = encode(&Message::request(1, Request::Ping));
        stream.extend_from_slice(br#"{"protocol_version":1,"#);
        let mut reader = trickle(stream, 5);
        assert!(reader.next_frame().await.unwrap().is_some());
        let err = reader.next_frame().await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn write_then_read() {
        let (client, server) = tokio::io::duplex(64);
        let (_, mut write_half) = tokio::io::split(client);
        let (read_half, _) = tokio::io::split(server);
        let messages = samples();
        let sent = messages.clone();
        let writer = tokio::spawn(async move {
            for m in &sent {
                write_message(&mut write_half, m).await.unwrap();
            }
        });
        let mut reader = FrameReader::new(BufReader::new(read_half));
        for expected in &messages {
            let frame = reader.next_frame().await.unwrap().unwrap();
            assert_eq!(&decode(frame).unwrap(), expected);
        }
        writer.await.unwrap();
    }
}
