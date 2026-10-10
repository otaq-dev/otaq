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
mod permissions;
mod session_requests;
mod terminal_requests;
pub mod testing;

use std::fmt;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UnixListener;
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::attention::Inbox;
use crate::attention::notify::{NoNotifier, Notifier, OsaScriptNotifier};
use crate::board::Board;
use crate::caller::Caller;
use crate::data_dir::DataDir;
use crate::ports::PortPool;
use crate::protocol::{self, PROTOCOL_VERSION, ServiceStatus};
use crate::recovery::cleanup::Cleanup;
use crate::recovery::{self, Recovery};
use crate::review::Review;
use crate::scheduler::{self, Scheduler};
use crate::session::Sessions;
use crate::session::deliveries::Deliveries;
use crate::session::launch::LaunchContext;
use crate::signals::{Signal, Signals};
use crate::start::Starter;
use crate::store::{Store, StoreError, now_ms};
use crate::terminals::TerminalEnv;
use crate::terminals::run::Runs;
use crate::terminals::shell::Shells;
use crate::terminals::suggest::Suggester;
use crate::tools::CheckEnv;
use crate::usage::limit::PlanLimit;
use crate::usage::{self, Usage};
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
    /// The scheduler's slow tick (SCH-01).
    pub scheduler_tick: Duration,
    /// Posts OS notifications (ATT-11). Posts nothing by default (and in tests); the daemon sets the platform's.
    pub notifier: Arc<dyn Notifier>,
    /// The token usage reader's slow tick (USG-01). It reads Claude Code's folder from [`Self::tools`].
    pub usage_tick: Duration,
    /// What Run, Shell and the Project terminal start with (the `crewboard` folder, PATH, the user's shell).
    pub terminals: TerminalEnv,
    /// The `claude` that Ask Claude runs (RUN-02). `None` (the default, and in tests) refuses it; the daemon sets
    /// `claude`, and tests pass an ordinary command that prints a fixed answer.
    pub ask_claude: Option<Vec<String>>,
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
            scheduler_tick: scheduler::DEFAULT_TICK,
            notifier: Arc::new(NoNotifier),
            usage_tick: usage::DEFAULT_TICK,
            terminals: TerminalEnv::from_process(),
            ask_claude: None,
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
            notifier: platform_notifier(),
            ask_claude: Some(vec!["claude".to_owned()]),
            ..Self::new(data)
        }
    }
}

/// The OS notifier of this platform (macOS: `osascript`; Windows and Linux arrive in M11).
fn platform_notifier() -> Arc<dyn Notifier> {
    if cfg!(target_os = "macos") {
        Arc::new(OsaScriptNotifier)
    } else {
        Arc::new(NoNotifier)
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
    scheduler: Scheduler,
    review: Review,
    signals: Signals,
    recovery: Recovery,
    inbox: Inbox,
    cleanup: Cleanup,
    usage: Usage,
    plan_limit: PlanLimit,
    runs: Runs,
    suggester: Suggester,
    shells: Shells,
    prs: crate::pr::service::Prs,
    messages: crate::messages::Messages,
    /// This run, in `service_runs`; a clean stop records its end.
    run: recovery::Run,
    started_at: i64,
    started: Instant,
    stop: Arc<watch::Sender<bool>>,
    events: EventBus,
}

impl Shared {
    /// What follows a hook signal: a turn that ended makes the agent idle, so waiting messages may go now
    /// (SES-06).
    fn after_signal(&self, caller: &Caller, signal: &Signal) {
        let Caller::Agent {
            session, ticket_id, ..
        } = caller
        else {
            return;
        };
        // The plan limit (USG-03): it becomes active once, for every agent it stops. Waiting messages stay
        // queued: typing them now would only hit the limit again.
        if let Signal::LimitReached { hit } = signal {
            let plan_limit = self.plan_limit.clone();
            let (hit, ticket_id) = (hit.clone(), ticket_id.clone());
            tokio::spawn(async move {
                if let Err(e) = plan_limit.hit(&hit, &ticket_id, now_ms()).await {
                    tracing::warn!(error = %e, "can't record the plan limit");
                }
            });
            return;
        }
        if signal.ends_turn() {
            let deliveries = self.deliveries.clone();
            let session = session.clone();
            tokio::spawn(async move {
                if let Err(e) = deliveries.deliver_if_ready(&session).await {
                    tracing::warn!(session, error = %e, "delivery failed");
                }
            });
        }
    }

    async fn status(&self) -> ServiceStatus {
        let report = self.recovery.startup_report();
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
            unclean_start: report.unclean,
            interrupted_at_start: u32::try_from(report.interrupted.len()).unwrap_or(u32::MAX),
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
        let run = recovery::Run::begin(&store, &config.build_id)
            .await
            .map_err(ServiceError::Store)?;
        // Sessions of a service that is gone ended with it; the reconcile below decides what that means for
        // their tickets (REC-01).
        let leftovers = Sessions::end_leftovers(&store)
            .await
            .map_err(ServiceError::Store)?;
        if leftovers > 0 {
            tracing::warn!(
                leftovers,
                "sessions of the previous service recorded as ended"
            );
        }
        let board = Board::with_path_git(store.clone());
        // Starts cut short by the previous service stopping are shown, never re-run silently (SES-02).
        match Starter::interrupt_cut_short(&board).await {
            Ok(0) => {}
            Ok(cut) => tracing::warn!(
                cut,
                "starts cut short by the previous service: Start interrupted"
            ),
            Err(e) => tracing::warn!(error = %e, "can't check for starts cut short"),
        }
        let listener = UnixListener::bind(&socket).map_err(ServiceError::Io)?;
        access::restrict(&socket, 0o600).map_err(ServiceError::Io)?;
        let (stop, _) = watch::channel(false);
        let events = EventBus::new(config.event_capacity);
        let sessions = Sessions::with_focus(store.clone(), events.clone(), board.focus().clone());
        let ports = PortPool::new(store.clone());
        let starter = Starter::new(
            board.clone(),
            config.data.clone(),
            ports.clone(),
            sessions.clone(),
            config.launch.clone(),
            events.clone(),
        );
        let scheduler = Scheduler::new(
            board.clone(),
            Arc::new(starter.clone()),
            events.clone(),
            config.scheduler_tick,
        );
        // Hook signals say when an agent is idle between turns (T-0067).
        let signals = Signals::new(board.clone(), events.clone());
        let deliveries = Deliveries::new(
            store.clone(),
            sessions.clone(),
            events.clone(),
            Arc::new(signals.clone()),
        );
        starter.set_deliveries(deliveries.clone());
        let cleanup = Cleanup::new(
            board.clone(),
            config.data.clone(),
            sessions.clone(),
            ports.clone(),
        );
        let review = Review::new(
            board.clone(),
            config.data.clone(),
            sessions.clone(),
            deliveries.clone(),
            starter.clone(),
            scheduler.clone(),
            ports,
        );
        // Startup reconcile (REC-01): tickets whose agent was at work are Interrupted; nothing resumes.
        let recovery = Recovery::new(board.clone(), sessions.clone(), events.clone());
        starter.set_recovery(recovery.clone());
        if let Err(e) = recovery.startup(run.previous).await {
            tracing::error!(error = %e, "startup reconcile failed");
        }
        let inbox = Inbox::new(
            board.clone(),
            sessions.clone(),
            deliveries.clone(),
            signals.clone(),
            review.clone(),
            events.clone(),
            config.notifier.clone(),
        );
        let usage = Usage::new(
            store.clone(),
            events.clone(),
            config.tools.claude_config_dir.clone(),
            config.usage_tick,
        );
        let plan_limit = PlanLimit::new(
            board.clone(),
            events.clone(),
            inbox.clone(),
            sessions.clone(),
            deliveries.clone(),
        );
        if let Err(e) = plan_limit.restore().await {
            tracing::warn!(error = %e, "can't read the plan limit");
        }
        // Run (RUN-01): stops at Done and Close; "stopped by restart" after a restart, never started again.
        let runs = Runs::new(
            board.clone(),
            sessions.clone(),
            config.data.clone(),
            events.clone(),
            config.terminals.clone(),
        );
        {
            let runs = runs.clone();
            review.on_ticket_finished(Arc::new(move |ticket| {
                let (runs, ticket) = (runs.clone(), ticket.clone());
                Box::pin(async move { runs.finished(&ticket).await })
            }));
        }
        // The Shell (RUN-05) closes at Done and Close.
        let shells = Shells::new(
            board.clone(),
            sessions.clone(),
            config.data.clone(),
            config.terminals.clone(),
        );
        {
            let shells = shells.clone();
            review.on_ticket_finished(Arc::new(move |ticket| {
                let (shells, ticket) = (shells.clone(), ticket.clone());
                Box::pin(async move { shells.finished(&ticket).await })
            }));
        }
        // Park frees the ports, so the Run stops first (Q30); the Shell holds no port and stays.
        {
            let runs = runs.clone();
            review.on_ticket_parked(Arc::new(move |ticket| {
                let (runs, ticket) = (runs.clone(), ticket.clone());
                Box::pin(async move { runs.parked(&ticket).await })
            }));
        }
        {
            let runs = runs.clone();
            recovery.on_stopped_by_restart(Arc::new(move |info| runs.stopped_by_restart(info)));
        }
        let suggester = Suggester::new(board.clone(), events.clone(), config.ask_claude.clone());
        let prs = crate::pr::service::Prs::new(
            board.clone(),
            config.data.clone(),
            sessions.clone(),
            deliveries.clone(),
            config.launch.clone(),
            events.clone(),
        );
        let messages = crate::messages::Messages::new(
            board.clone(),
            sessions.clone(),
            deliveries.clone(),
            events.clone(),
        );
        let shared = Arc::new(Shared {
            messages,
            prs,
            shells,
            suggester,
            runs,
            plan_limit,
            usage,
            cleanup,
            inbox,
            recovery,
            run,
            review,
            config,
            signals,
            board,
            sessions,
            deliveries,
            starter,
            scheduler,
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

    /// The scheduler.
    pub fn scheduler(&self) -> Scheduler {
        self.shared.scheduler.clone()
    }

    /// Review actions (Send back, Local merge, …).
    pub fn review(&self) -> Review {
        self.shared.review.clone()
    }

    /// Leftover scan and delete (REC-04).
    pub fn cleanup(&self) -> Cleanup {
        self.shared.cleanup.clone()
    }

    /// The token usage reader.
    pub fn usage(&self) -> Usage {
        self.shared.usage.clone()
    }

    /// The plan limit (hit, reset bookkeeping).
    pub fn plan_limit(&self) -> PlanLimit {
        self.shared.plan_limit.clone()
    }

    /// Shells and Project terminals (RUN-05, RUN-06).
    pub fn messages(&self) -> crate::messages::Messages {
        self.shared.messages.clone()
    }

    pub fn prs(&self) -> crate::pr::service::Prs {
        self.shared.prs.clone()
    }

    pub fn shells(&self) -> Shells {
        self.shared.shells.clone()
    }

    /// Ask Claude for run commands (RUN-02).
    pub fn suggester(&self) -> Suggester {
        self.shared.suggester.clone()
    }

    /// Run processes (RUN-01).
    pub fn runs(&self) -> Runs {
        self.shared.runs.clone()
    }

    /// The inbox's actions (answers).
    pub fn inbox(&self) -> Inbox {
        self.shared.inbox.clone()
    }

    /// Startup reconcile and agent exits.
    pub fn recovery(&self) -> Recovery {
        self.shared.recovery.clone()
    }

    /// The agents' signal state.
    pub fn signals(&self) -> Signals {
        self.shared.signals.clone()
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
        let scheduler = tokio::spawn(shared.scheduler.clone().run(shared.stop.subscribe()));
        let recovery = tokio::spawn(shared.recovery.clone().run(shared.stop.subscribe()));
        let inbox = tokio::spawn(shared.inbox.clone().run(shared.stop.subscribe()));
        let usage = tokio::spawn(shared.usage.clone().run(shared.stop.subscribe()));
        let plan_limit = tokio::spawn(shared.plan_limit.clone().run(shared.stop.subscribe()));
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
        let _ = scheduler.await;
        let _ = recovery.await;
        let _ = inbox.await;
        let _ = usage.await;
        let _ = plan_limit.await;
        shared.sessions.shutdown().await;
        let _ = std::fs::remove_file(shared.config.data.socket_path());
        let drained = tokio::time::timeout(DRAIN_TIMEOUT, async {
            while connections.join_next().await.is_some() {}
        })
        .await;
        if drained.is_err() {
            connections.shutdown().await;
        }
        if let Err(e) = shared.run.end(&shared.store).await {
            tracing::warn!(error = %e, "can't record the clean stop");
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
