//! Talking to the background service, shared by the app and the CLI (SVC-04, ARC-03).
//!
//! [`Client`] keeps one connection open and repairs it: when the service goes away the client reports
//! [`ConnectionState::Reconnecting`], retries with backoff, and on success reports
//! [`ConnectionState::Connected`] again and re-subscribes to events. Requests made meanwhile fail at once with
//! [`ClientError::Disconnected`], so the UI can disable input instead of waiting. Reconnecting never starts a
//! service; only [`Client::connect_or_start`] does, once, at launch (d0055). The app is only a client: dropping
//! a `Client` never stops the service (d0022).
//!
//! [`request_once`] is the one-shot form the CLI uses for single commands.

mod connection;
mod launcher;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::BufReader;
use tokio::net::UnixStream;
use tokio::sync::{broadcast, watch};

use crate::protocol::{
    Body, DecodeError, ErrorBody, Event, FrameReader, Message, Reply, Request, ServiceStatus,
    decode, write_message,
};
use connection::Connection;
pub use launcher::{CommandLauncher, Launcher};

/// Connection settings. [`ClientConfig::new`] has the defaults.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub socket: PathBuf,
    /// First wait before reconnecting; doubled after every failed attempt up to `backoff_max`.
    pub backoff_min: Duration,
    pub backoff_max: Duration,
    /// How long [`Client::connect_or_start`] waits for a service it launched to answer.
    pub start_timeout: Duration,
}

impl ClientConfig {
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self {
            socket: socket.into(),
            backoff_min: Duration::from_millis(100),
            backoff_max: Duration::from_secs(2),
            start_timeout: Duration::from_secs(10),
        }
    }
}

/// What the app shows about its link to the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connected; the service's status as read when the connection opened.
    Connected(ServiceStatus),
    /// The connection dropped; `attempt` counts the tries so far ("Reconnecting… agents keep running").
    Reconnecting { attempt: u32 },
}

/// A self-repairing connection to the service. Cheap to clone; the connection closes when the last clone is
/// dropped.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    config: ClientConfig,
    state: watch::Sender<ConnectionState>,
    connection: Mutex<Option<Arc<Connection>>>,
    events: broadcast::Sender<Event>,
    subscribed: AtomicBool,
    supervisor: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(task) = self.supervisor.lock().expect("supervisor").take() {
            task.abort();
        }
    }
}

/// Events buffered for the app's subscribers before the oldest are dropped.
const EVENT_BUFFER: usize = 1024;

impl Client {
    /// Connects to a running service. Fails with [`ClientError::NotRunning`] if none answers.
    pub async fn connect(config: ClientConfig) -> Result<Self, ClientError> {
        let (events, _) = broadcast::channel(EVENT_BUFFER);
        let (connection, status) = establish(&config.socket, &events, false).await?;
        let inner = Arc::new(Inner {
            config,
            state: watch::Sender::new(ConnectionState::Connected(status)),
            connection: Mutex::new(Some(Arc::new(connection))),
            events,
            subscribed: AtomicBool::new(false),
            supervisor: Mutex::new(None),
        });
        let supervisor = tokio::spawn(supervise(Arc::downgrade(&inner)));
        *inner.supervisor.lock().expect("supervisor") = Some(supervisor);
        Ok(Self { inner })
    }

    /// Connects, starting the service with `launcher` first if none answers (d0055). The launcher runs at most
    /// once; the service's lock file makes sure only one service ever runs.
    pub async fn connect_or_start(
        config: ClientConfig,
        launcher: &dyn Launcher,
    ) -> Result<Self, ClientError> {
        match Self::connect(config.clone()).await {
            Err(ClientError::NotRunning(_)) => {}
            other => return other,
        }
        launcher.launch().await.map_err(ClientError::Start)?;
        let deadline = tokio::time::Instant::now() + config.start_timeout;
        loop {
            match Self::connect(config.clone()).await {
                Err(ClientError::NotRunning(e)) if tokio::time::Instant::now() >= deadline => {
                    return Err(ClientError::NotRunning(e));
                }
                Err(ClientError::NotRunning(_)) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                other => return other,
            }
        }
    }

    /// The connection state now and as it changes.
    pub fn state(&self) -> watch::Receiver<ConnectionState> {
        self.inner.state.subscribe()
    }

    pub fn is_connected(&self) -> bool {
        matches!(*self.inner.state.borrow(), ConnectionState::Connected(_))
    }

    /// Sends a request. Fails at once with [`ClientError::Disconnected`] while reconnecting.
    pub async fn request(&self, request: Request) -> Result<Reply, ClientError> {
        let connection = self
            .inner
            .connection
            .lock()
            .expect("connection")
            .clone()
            .filter(|c| !c.is_closed())
            .ok_or(ClientError::Disconnected)?;
        connection.request(request).await
    }

    /// Receives the service's events from now on, also across reconnects (the subscription is renewed on
    /// every new connection). After a reconnect, events from the gap are lost: refetch what is shown.
    pub async fn subscribe(&self) -> Result<broadcast::Receiver<Event>, ClientError> {
        let receiver = self.inner.events.subscribe();
        if !self.inner.subscribed.swap(true, Ordering::SeqCst) {
            match self.request(Request::Subscribe).await {
                Ok(_) | Err(ClientError::Disconnected) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(receiver)
    }
}

/// Opens a connection, reads the service's status, and subscribes to events if asked.
async fn establish(
    socket: &Path,
    events: &broadcast::Sender<Event>,
    subscribe: bool,
) -> Result<(Connection, ServiceStatus), ClientError> {
    let connection = Connection::open(socket, events.clone()).await?;
    let status = match connection.request(Request::ServiceStatus).await? {
        Reply::ServiceStatus(status) => status,
        other => return Err(ClientError::Unexpected(other)),
    };
    if subscribe {
        connection.request(Request::Subscribe).await?;
    }
    Ok((connection, status))
}

/// Waits for the connection to drop, then reconnects with backoff. Ends when the client is dropped.
async fn supervise(inner: std::sync::Weak<Inner>) {
    loop {
        let Some(current) = inner
            .upgrade()
            .and_then(|i| i.connection.lock().expect("connection").clone())
        else {
            return;
        };
        current.closed().await;
        drop(current);

        let mut attempt = 0;
        let mut backoff = match inner.upgrade() {
            Some(i) => i.config.backoff_min,
            None => return,
        };
        loop {
            attempt += 1;
            let Some(client) = inner.upgrade() else {
                return;
            };
            *client.connection.lock().expect("connection") = None;
            client
                .state
                .send_replace(ConnectionState::Reconnecting { attempt });
            let (socket, max) = (client.config.socket.clone(), client.config.backoff_max);
            drop(client);

            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(max);

            let Some(client) = inner.upgrade() else {
                return;
            };
            let subscribe = client.subscribed.load(Ordering::SeqCst);
            match establish(&socket, &client.events, subscribe).await {
                Ok((connection, status)) => {
                    tracing::info!(attempt, "reconnected to the background service");
                    let connection = Arc::new(connection);
                    *client.connection.lock().expect("connection") = Some(connection.clone());
                    // `subscribe()` may have been called while this connection was being set up.
                    if !subscribe && client.subscribed.load(Ordering::SeqCst) {
                        let _ = connection.request(Request::Subscribe).await;
                    }
                    client
                        .state
                        .send_replace(ConnectionState::Connected(status));
                    break;
                }
                Err(e) => tracing::debug!(attempt, error = %e, "reconnect failed"),
            }
        }
    }
}

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
    /// The client is reconnecting; the request was not sent.
    Disconnected,
    /// The service couldn't be started.
    Start(io::Error),
    /// The service answered with a reply that doesn't fit the request.
    Unexpected(Reply),
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
            Self::Disconnected => write!(f, "not connected to the background service"),
            Self::Start(e) => write!(f, "can't start the background service: {e}"),
            Self::Unexpected(reply) => write!(
                f,
                "unexpected answer from the background service: {reply:?}"
            ),
            Self::Closed => write!(f, "the background service closed the connection"),
            Self::Io(e) => write!(f, "connection to the background service failed: {e}"),
            Self::Protocol(e) => write!(f, "{e}"),
            Self::Service(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ClientError {}
