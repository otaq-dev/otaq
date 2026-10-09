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

mod connection;
pub mod lock;

use std::fmt;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UnixListener;
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::data_dir::DataDir;
use crate::protocol::{self, PROTOCOL_VERSION, ServiceStatus};
use crate::store::{Store, StoreError, now_ms};
use lock::{LockError, ServiceLock};

/// How long a stopping service waits for connections to send what they have queued.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub data: DataDir,
    /// Reported in `service.status`; the app compares it with its own (d0022). Tests override it.
    pub build_id: String,
}

impl ServiceConfig {
    pub fn new(data: DataDir) -> Self {
        Self {
            data,
            build_id: protocol::build_id().to_owned(),
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
    started_at: i64,
    started: Instant,
    stop: Arc<watch::Sender<bool>>,
}

impl Shared {
    fn status(&self) -> ServiceStatus {
        ServiceStatus {
            pid: std::process::id(),
            build_id: self.config.build_id.clone(),
            protocol_version: PROTOCOL_VERSION,
            started_at: self.started_at,
            uptime_secs: self.started.elapsed().as_secs(),
            agents_working: 0,
            agents_waiting: 0,
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
        let listener = UnixListener::bind(&socket).map_err(ServiceError::Io)?;
        let (stop, _) = watch::channel(false);
        let shared = Arc::new(Shared {
            config,
            store,
            started_at: now_ms(),
            started: Instant::now(),
            stop: Arc::new(stop),
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
                        connections.spawn(connection::serve(stream, shared.clone()));
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
