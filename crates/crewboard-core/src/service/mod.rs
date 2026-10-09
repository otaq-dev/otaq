//! The background service, `crewboard daemon` (d0022): owns the store, and later every PTY and process, the
//! scheduler and git. App and CLI talk to it over a Unix domain socket in the data folder (PLT-01) using
//! [`crate::protocol`].
//!
//! Keep this small and boring (SVC-07, d0027): it is the component whose crash interrupts every agent. It never
//! restarts itself and nothing installs it as a login item (d0055); the app or `crewboard daemon start` starts it.
//!
//! Life cycle: [`Service::bind`] takes the lock file (one service per data folder), replaces a stale socket left
//! by a crash, opens the store and listens. [`Service::serve`] answers requests until a [`Stopper`] fires (a
//! `service.stop` request or a signal), then removes the socket, closes the store and removes the lock file.

pub mod access;
mod board_requests;
mod connection;
mod events;
pub mod lock;
mod session_requests;

use std::fmt;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UnixListener;
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::board::Board;
use crate::data_dir::DataDir;
use crate::ports::PortPool;
use crate::protocol::{self, PROTOCOL_VERSION, ServiceStatus};
use crate::session::Sessions;
use crate::session::deliveries::{Deliveries, NeverIdle};
use crate::session::launch::LaunchContext;
use crate::start::Starter;
use crate::store::{Store, StoreError, now_ms};
use crate::tools::CheckEnv;
use access::{OsPeerCredentials, PeerCredentials};
pub use events::EventBus;
use lock::{LockError, ServiceLock};

/// How long a stopping service waits for connections to send what they have queued.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct ServiceConfig {
    pub data: DataDir,
    /// Reported in `service.status`; the app compares it with its own (d0022). Tests override it.
    pub build_id: String,
    /// Only connections from this user are served (SVC-02). Defaults to the current user.
    pub owner_uid: u32,
    pub peer_credentials: Arc<dyn PeerCredentials>,
    /// Events kept for subscribers that are behind; a subscriber further behind is disconnected.
    pub event_capacity: usize,
    /// Messages queued for one client; a subscriber whose queue is full is disconnected.
    pub client_queue: usize,
    /// Where `tools.check` looks (PATH, Claude Code settings). Tests point it at fixtures.
    pub tools: CheckEnv,
    /// How agents are launched. `None` (the default, and in tests) makes the start sequence stop at its launch
    /// step, so no test ever starts a real `claude`; the daemon sets it ([`ServiceConfig::for_daemon`]).
    pub launch: Option<LaunchContext>,
}

impl ServiceConfig {
    pub fn new(data: DataDir) -> Self {
        Self {
            data,
            build_id: protocol::build_id().to_owned(),
            owner_uid: access::current_uid(),
            peer_credentials: Arc::new(OsPeerCredentials),
            event_capacity: 1024,
            client_queue: 256,
            tools: CheckEnv::from_process(),
            launch: None,
        }
    }

    /// The configuration of `crewboard daemon run`: launches the real `claude` with the plugin found next to
    /// the binary (or the repository's in a dev build).
    pub fn for_daemon(data: DataDir) -> Self {
        let launch = LaunchContext::from_process();
        if launch.is_none() {
            tracing::warn!("the Claude Code plugin folder wasn't found; agents can't start");
        }
        Self {
            launch,
            ..Self::new(data)
        }
    }
}

/// A service that holds the lock and listens on the socket, ready to [`serve`](Service::serve).
pub struct Service {
    listener: UnixListener,
    shared: Arc<Shared>,
    lock: ServiceLock,
}

/// State every connection can read.
struct Shared {
    config: ServiceConfig,
    store: Store,
    board: Board,
    sessions: Sessions,
    deliveries: Deliveries,
    starter: Starter,
    started_at: i64,
    started: Instant,
    stop: Arc<watch::Sender<bool>>,
    events: EventBus,
}

impl Shared {
    async fn status(&self) -> ServiceStatus {
        let counts = self.store.agent_counts().await.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "can't count agents");
            Default::default()
        });
        ServiceStatus {
            pid: std::process::id(),
            build_id: self.config.build_id.clone(),
            protocol_version: PROTOCOL_VERSION,
            started_at: self.started_at,
            uptime_secs: self.started.elapsed().as_secs(),
            agents_working: counts.working,
            agents_waiting: counts.waiting,
        }
    }
}

/// Asks a running service to shut down. Cheap to clone.
#[derive(Clone)]
pub struct Stopper(Arc<watch::Sender<bool>>);

impl Stopper {
    pub fn stop(&self) {
        self.0.send_replace(true);
    }
}

impl Service {
    /// Takes the lock, replaces a stale socket, opens the store and starts listening.
    pub async fn bind(config: ServiceConfig) -> Result<Self, ServiceError> {
        let data = &config.data;
        DataDir::ensure_dir(data.root()).map_err(ServiceError::Io)?;
        // The socket's folder: user-only even if it existed with looser permissions.
        access::restrict(data.root(), 0o700).map_err(ServiceError::Io)?;
        let lock = ServiceLock::acquire(&data.service_lock_path()).map_err(|e| match e {
            LockError::Held { pid } => ServiceError::AlreadyRunning { pid },
            LockError::Io(e) => ServiceError::Io(e),
        })?;

        // We hold the lock, so a socket file here was left by a service that crashed.
        let socket = data.socket_path();
        if socket.exists() {
            tracing::warn!(path = %socket.display(), "replacing a stale socket");
            std::fs::remove_file(&socket).map_err(ServiceError::Io)?;
        }

        let store = Store::open_in(data).await.map_err(ServiceError::Store)?;
        // Sessions of a service that is gone ended with it (until M7's reconcile, questions.md Q11.6).
        let leftovers = Sessions::end_leftovers(&store)
            .await
            .map_err(ServiceError::Store)?;
        if leftovers > 0 {
            tracing::warn!(
                leftovers,
                "sessions of the previous service recorded as ended"
            );
        }
        let listener = UnixListener::bind(&socket).map_err(ServiceError::Io)?;
        access::restrict(&socket, 0o600).map_err(ServiceError::Io)?;
        let (stop, _) = watch::channel(false);
        let events = EventBus::new(config.event_capacity);
        let board = Board::with_path_git(store.clone());
        let sessions = Sessions::new(store.clone(), events.clone());
        let starter = Starter::new(
            board.clone(),
            config.data.clone(),
            PortPool::new(store.clone()),
            sessions.clone(),
            config.launch.clone(),
            events.clone(),
        );
        // Hook signals say when an agent is idle from M4; until then automatic deliveries wait for Deliver now.
        let deliveries = Deliveries::new(
            store.clone(),
            sessions.clone(),
            events.clone(),
            Arc::new(NeverIdle),
        );
        let shared = Arc::new(Shared {
            config,
            board,
            sessions,
            deliveries,
            starter,
            store,
            started_at: now_ms(),
            started: Instant::now(),
            stop: Arc::new(stop),
            events,
        });
        tracing::info!(
            pid = std::process::id(),
            build_id = %shared.config.build_id,
            socket = %socket.display(),
            "service listening"
        );
        Ok(Self {
            listener,
            shared,
            lock,
        })
    }

    pub fn stopper(&self) -> Stopper {
        Stopper(self.shared.stop.clone())
    }

    /// Publishes events to every subscribed client.
    pub fn events(&self) -> EventBus {
        self.shared.events.clone()
    }

    /// The service's sessions.
    pub fn sessions(&self) -> Sessions {
        self.shared.sessions.clone()
    }

    /// Types automatic and click deliveries into sessions.
    pub fn deliveries(&self) -> Deliveries {
        self.shared.deliveries.clone()
    }

    /// Runs start sequences.
    pub fn starter(&self) -> Starter {
        self.shared.starter.clone()
    }

    /// The service's board (store and git).
    pub fn board(&self) -> Board {
        self.shared.board.clone()
    }

    /// Answers connections until stopped, then cleans up.
    pub async fn serve(self) {
        let Self {
            listener,
            shared,
            lock,
        } = self;
        let mut stopped = shared.stop.subscribe();
        let mut connections = JoinSet::new();
        loop {
            tokio::select! {
                _ = until_stopped(&mut stopped) => break,
                accepted = listener.accept() => match accepted {
                    Ok((stream, _)) => {
                        let peer = shared.config.peer_credentials.peer_uid(&stream);
                        if access::peer_allowed(shared.config.owner_uid, peer) {
                            connections.spawn(connection::serve(stream, shared.clone()));
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "accept failed");
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                },
                Some(_) = connections.join_next(), if !connections.is_empty() => {}
            }
        }

        tracing::info!("service stopping");
        drop(listener);
        shared.sessions.shutdown().await;
        let _ = std::fs::remove_file(shared.config.data.socket_path());
        let drained = tokio::time::timeout(DRAIN_TIMEOUT, async {
            while connections.join_next().await.is_some() {}
        })
        .await;
        if drained.is_err() {
            connections.shutdown().await;
        }
        shared.store.clone().close().await;
        drop(lock);
        tracing::info!("service stopped");
    }
}

/// Resolves once the stop flag is set (or its sender is gone).
async fn until_stopped(stopped: &mut watch::Receiver<bool>) {
    let _ = stopped.wait_for(|s| *s).await;
}

/// Runs the service in this process until `service.stop`, SIGTERM or SIGINT. SIGHUP is ignored, so closing the
/// terminal that started it doesn't stop it.
pub async fn run_until_signal(config: ServiceConfig) -> Result<(), ServiceError> {
    let service = Service::bind(config).await?;
    let stopper = service.stopper();
    tokio::spawn(async move {
        if shutdown_signal().await.is_ok() {
            stopper.stop();
        }
    });
    service.serve().await;
    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() -> io::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate())?;
    let mut int = signal(SignalKind::interrupt())?;
    // Registering a handler replaces the default action (exit), so SIGHUP is ignored from here on.
    let mut hup = signal(SignalKind::hangup())?;
    loop {
        tokio::select! {
            _ = term.recv() => return Ok(()),
            _ = int.recv() => return Ok(()),
            _ = hup.recv() => tracing::info!("SIGHUP ignored"),
        }
    }
}

#[derive(Debug)]
pub enum ServiceError {
    /// Another service already runs for this data folder.
    AlreadyRunning {
        pid: Option<u32>,
    },
    Io(io::Error),
    Store(StoreError),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning { pid: Some(pid) } => write!(f, "already running (PID {pid})"),
            Self::AlreadyRunning { pid: None } => write!(f, "already running"),
            Self::Io(e) => write!(f, "{e}"),
            Self::Store(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ServiceError {}
