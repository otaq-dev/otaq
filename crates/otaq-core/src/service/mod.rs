//! The background service, `otaq daemon` (d0022): owns the store, and later every PTY and process, the
//! scheduler and git. App and CLI talk to it over a Unix domain socket in the data folder (a named pipe on
//! Windows; PLT-01, PLT-02, [`crate::ipc`]) using [`crate::protocol`].
//!
//! Keep this small and boring (SVC-07, d0027): it is the component whose crash interrupts every agent. It never
//! restarts itself and nothing installs it as a login item (d0055); the app or `otaq daemon start` starts it.
//!
//! Life cycle: [`Service::bind`] takes the lock file (one service per data folder), replaces a stale socket left
//! by a crash, opens the store and listens. [`Service::serve`] answers requests until a [`Stopper`] fires (a
//! `service.stop` request or a signal), then removes the socket, closes the store and removes the lock file.

pub mod access;
mod agent_requests;
mod board_requests;
mod connection;
mod events;
pub mod lock;
mod named_agent_requests;
mod permissions;
mod session_requests;
mod terminal_requests;
pub mod testing;

use std::fmt;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::agent_tools::claude::trust::ClaudeConfig;
use crate::agent_tools::claude::{ClaudeCode, ClaudeContext};
use crate::agent_tools::{Adapters, Tool};
use crate::attention::Inbox;
use crate::attention::notify::{NoNotifier, Notifier, Opener};
use crate::board::Board;
use crate::caller::Caller;
use crate::data_dir::DataDir;
use crate::ipc::{self, UserId};
use crate::named_agents::{NamedAgents, starter};
use crate::ports::PortPool;
use crate::protocol::{self, PROTOCOL_VERSION, ServiceStatus};
use crate::recovery::cleanup::Cleanup;
use crate::recovery::{self, Recovery};
use crate::review::Review;
use crate::scheduler::{self, Scheduler};
use crate::session::Sessions;
use crate::session::deliveries::Deliveries;
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
    pub owner: UserId,
    pub peer_credentials: Arc<dyn PeerCredentials>,
    /// Events kept for subscribers that are behind; a subscriber further behind is disconnected.
    pub event_capacity: usize,
    /// Messages queued for one client; a subscriber whose queue is full is disconnected.
    pub client_queue: usize,
    /// Where `tools.check` looks (PATH, Claude Code settings). Tests point it at fixtures.
    pub tools: CheckEnv,
    /// How agents are launched. `None` (the default, and in tests) makes the start sequence stop at its launch
    /// step, so no test ever starts a real agent tool; the daemon sets it ([`ServiceConfig::for_daemon`]).
    pub launch: Option<crate::agent_tools::LaunchContext>,
    /// The scheduler's slow tick (SCH-01).
    pub scheduler_tick: Duration,
    /// Posts OS notifications (ATT-11). Posts nothing by default (and in tests); the daemon sets the platform's.
    pub notifier: Arc<dyn Notifier>,
    /// The app a click on a notification starts when no app is connected (T-0159). `None` (the default, and in
    /// tests) starts nothing; the daemon sets the app next to its binary.
    pub app: Option<std::path::PathBuf>,
    /// The token usage reader's slow tick (USG-01). It reads Claude Code's folder from [`Self::tools`].
    pub usage_tick: Duration,
    /// What Run, Shell and the Project terminal start with (the `otaq` folder, PATH, the user's shell).
    pub terminals: TerminalEnv,
    /// Whether the data folder's disk can clone (WRM-01): probed once, at service start. Tests force an answer
    /// ([`crate::warm::clone::CloneProbe::forced`]).
    pub clone_probe: Arc<crate::warm::clone::CloneProbe>,
    /// Claude Code's config file, where the service records the folder trust of added projects (PRJ-06, d0085).
    /// Agents get the service's environment, so it is the service's file. `None` (the default, and in tests unless
    /// they pass a temporary one) records nothing; the daemon locates the real one ([`ServiceConfig::for_daemon`]).
    pub claude: Option<ClaudeConfig>,
    /// Whether the service reads the agent tools' model lists (AGT-02): at start, on `agents.models_refresh` and
    /// once a day. Off by default (and in tests: no test starts a tool for its list); the daemon turns it on.
    pub read_model_lists: bool,
}

impl ServiceConfig {
    pub fn new(data: DataDir) -> Self {
        Self {
            data,
            build_id: protocol::build_id().to_owned(),
            owner: access::current_user(),
            peer_credentials: Arc::new(OsPeerCredentials),
            event_capacity: 1024,
            client_queue: 256,
            tools: CheckEnv::from_process(),
            launch: None,
            scheduler_tick: scheduler::DEFAULT_TICK,
            notifier: Arc::new(NoNotifier),
            app: None,
            usage_tick: usage::DEFAULT_TICK,
            terminals: TerminalEnv::from_process(),
            clone_probe: Arc::new(crate::warm::clone::CloneProbe::new()),
            claude: None,
            read_model_lists: false,
        }
    }

    /// The configuration of `otaq daemon run`: launches the real agent tools with the plugin found next to
    /// the binary (or the repository's in a dev build).
    pub fn for_daemon(data: DataDir) -> Self {
        let launch = crate::agent_tools::LaunchContext::from_process();
        if launch.is_none() {
            tracing::warn!("the Claude Code plugin folder wasn't found; agents can't start");
        }
        // Located once, here: the only place the service reads Claude Code's config location (PRJ-06).
        let claude = ClaudeConfig::for_service();
        Self {
            launch,
            notifier: crate::attention::notify::platform_notifier(),
            app: std::env::current_exe()
                .ok()
                .and_then(|exe| Some(exe.parent()?.join(crate::naming::app_binary_name())))
                .filter(|app| app.is_file()),
            claude,
            read_model_lists: true,
            ..Self::new(data)
        }
    }
}

/// A service that holds the lock and listens on the socket, ready to [`serve`](Service::serve).
pub struct Service {
    listener: ipc::Listener,
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
    validation: crate::validation::Validation,
    project_agents: crate::project_agents::ProjectAgents,
    warm: crate::warm::store::WarmBuilds,
    /// Every agent tool's adapter, built once.
    adapters: Adapters,
    /// The copies of the tools' model lists (AGT-02).
    models: crate::agent_tools::models::ModelLists,
    /// The named agents' rules (AGT-01…AGT-04).
    named_agents: NamedAgents,
    ticket_env: crate::ticket_env::TicketEnv,
    /// This run, in `service_runs`; a clean stop records its end.
    run: recovery::Run,
    started_at: i64,
    started: Instant,
    stop: Arc<watch::Sender<bool>>,
    events: EventBus,
    /// Counts the clients subscribed to events, for notification clicks.
    opener: Opener,
}

impl Shared {
    /// What follows a hook signal of `tool`'s: a turn that ended makes the agent idle, so waiting messages may go
    /// now (SES-06); a limit is recorded as `tool`'s.
    fn after_signal(&self, caller: &Caller, tool: Tool, signal: &Signal) {
        // A Project agent's turn ended: what waits for it (answers, "continue") may go now; the plan limit is
        // recorded as for agents (d0068).
        if let Some((_, session)) = caller.project_agent() {
            if let Signal::LimitReached { hit } = signal {
                let plan_limit = self.plan_limit.clone();
                let hit = hit.clone();
                tokio::spawn(async move {
                    if let Err(e) = plan_limit.record_hit(tool, &hit, now_ms()).await {
                        tracing::warn!(error = %e, "can't record the plan limit");
                    }
                });
            } else if signal.ends_turn() {
                let deliveries = self.deliveries.clone();
                let session = session.to_owned();
                tokio::spawn(async move {
                    if let Err(e) = deliveries.deliver_if_ready(&session).await {
                        tracing::warn!(session, error = %e, "delivery failed");
                    }
                });
            }
            return;
        }
        // A validator's hooks are its validation's (§11.4 M12 rows); the usage limit is the plan's as for agents.
        if let Caller::Validator {
            session, ticket_id, ..
        } = caller
        {
            let validation = self.validation.clone();
            let plan_limit = self.plan_limit.clone();
            let (session, ticket_id, signal) = (session.clone(), ticket_id.clone(), signal.clone());
            tokio::spawn(async move {
                if let Err(e) = validation
                    .validator_signal(&ticket_id, &session, &signal)
                    .await
                {
                    tracing::warn!(session, error = %e, "can't apply the validator's hook");
                }
                if let Signal::LimitReached { hit } = &signal
                    && let Err(e) = plan_limit.hit(tool, hit, &ticket_id, now_ms()).await
                {
                    tracing::warn!(error = %e, "can't record the plan limit");
                }
            });
            return;
        }
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
                if let Err(e) = plan_limit.hit(tool, &hit, &ticket_id, now_ms()).await {
                    tracing::warn!(error = %e, "can't record the plan limit");
                }
            });
            return;
        }
        // While its ticket validates, a turn of the agent may change the code (VAL-08).
        if matches!(signal, Signal::Working { .. }) {
            let validation = self.validation.clone();
            let ticket_id = ticket_id.clone();
            tokio::spawn(async move { validation.agent_working(&ticket_id).await });
        }
        if signal.ends_turn() {
            let deliveries = self.deliveries.clone();
            let validation = self.validation.clone();
            let (session, ticket_id) = (session.clone(), ticket_id.clone());
            tokio::spawn(async move {
                if let Err(e) = validation.agent_turn_ended(&ticket_id).await {
                    tracing::warn!(session, error = %e, "can't check the code under validation");
                }
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

/// Creates the starter entry and the role defaults if the list is empty and a tool is connected (AGT-01), in the
/// background: at service start and when onboarding adds a project. `can_launch` is false without a launch context
/// (tests): then no tool is detected, so nothing runs a tool's program.
fn spawn_starter(adapters: &Adapters, store: &Store, can_launch: bool) {
    let (adapters, store) = (adapters.clone(), store.clone());
    tokio::spawn(async move {
        if store.named_agent_count().await.unwrap_or(0) > 0 {
            return;
        }
        let tools = starter::connected(&adapters, can_launch).await;
        if let Err(e) = starter::ensure(&store, &tools).await {
            tracing::warn!(error = %e, "can't create the starter named agents");
        }
    });
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

        let socket = data.socket_path();

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
        let adapters = Adapters::new(ClaudeCode::new(ClaudeContext {
            launch: config.launch.clone(),
            check: config.tools.clone(),
            data: config.data.clone(),
            trust: config.claude.clone(),
        }));
        board.trust().set_adapters(adapters.clone());
        // Starts cut short by the previous service stopping are shown, never re-run silently (SES-02).
        match Starter::interrupt_cut_short(&board).await {
            Ok(0) => {}
            Ok(cut) => tracing::warn!(
                cut,
                "starts cut short by the previous service: Start interrupted"
            ),
            Err(e) => tracing::warn!(error = %e, "can't check for starts cut short"),
        }
        // We hold the lock, so a socket file already there was left by a service that crashed: replaced.
        let listener = ipc::Listener::bind(&socket).map_err(ServiceError::Io)?;
        let (stop, _) = watch::channel(false);
        let events = EventBus::new(config.event_capacity);
        let sessions = Sessions::with_focus(store.clone(), events.clone(), board.focus().clone());
        // Every session gets its project's variables, read at each spawn (ENV-02).
        {
            let board = board.clone();
            sessions.set_env_provider(Arc::new(move |project_id| {
                let board = board.clone();
                Box::pin(async move {
                    board.project_env(&project_id).await.unwrap_or_else(|e| {
                        tracing::warn!(project = %project_id, error = %e, "can't read the project's variables");
                        Vec::new()
                    })
                })
            }));
        }
        // Every ticket session gets the ticket's resource variables, read at each spawn (d0087).
        {
            let board = board.clone();
            let data = config.data.clone();
            sessions.set_ticket_env_provider(Arc::new(move |ticket_id| {
                let board = board.clone();
                let data = data.clone();
                Box::pin(async move {
                    crate::ticket_env::resources::session_vars(&board, &data, &ticket_id).await
                })
            }));
        }
        // Warm builds (d0082): what a crash left half-done goes first, then the disk is probed once.
        let trash = recovery::remove_interrupted_warm_work(data).await;
        let warm = crate::warm::store::WarmBuilds::new(
            config.data.clone(),
            config.clone_probe.clone(),
            board.clone(),
            events.clone(),
        );
        // The trash a crash left is emptied in the background: start doesn't wait for it.
        for repo in &trash {
            warm.empty_trash_later(repo);
        }
        {
            let warm = warm.clone();
            let support = tokio::task::spawn_blocking(move || warm.support().clone())
                .await
                .map_err(|e| ServiceError::Io(io::Error::other(e)))?;
            tracing::info!(
                available = support.available,
                file_system = %support.file_system,
                volume = %support.volume,
                "copy-on-write cloning for warm builds"
            );
        }
        let ports = PortPool::new(store.clone());
        let starter = Starter::new(
            board.clone(),
            config.data.clone(),
            ports.clone(),
            sessions.clone(),
            adapters.clone(),
            events.clone(),
        );
        let scheduler = Scheduler::new(
            board.clone(),
            Arc::new(starter.clone()),
            events.clone(),
            config.scheduler_tick,
        );
        // Hook signals say when an agent is idle between turns (T-0067).
        let signals = Signals::new(
            board.clone(),
            events.clone(),
            adapters.clone(),
            sessions.clone(),
        );
        let deliveries = Deliveries::new(
            store.clone(),
            sessions.clone(),
            events.clone(),
            Arc::new(signals.clone()),
            adapters.clone(),
        );
        starter.set_deliveries(deliveries.clone());
        let seeder = crate::warm::seed::Seeder::new(
            warm.clone(),
            crate::ticket_env::stock::Stock::new(warm.clone(), board.clone()),
            Arc::new(crate::warm::clone::FsCloner),
            Arc::new(crate::ticket_env::handoff::FsMover),
        );
        starter.set_seeder(seeder.clone());
        // A child going idle with the lead's unread note gets it typed in instead (LEAD-10); else its first Idle
        // stop goes to its live lead once (LEAD-07).
        {
            let (board, sessions, deliveries, events) = (
                board.clone(),
                sessions.clone(),
                deliveries.clone(),
                events.clone(),
            );
            signals.on_idle(Arc::new(move |ticket_id, last_line| {
                let (board, sessions, deliveries, events) = (
                    board.clone(),
                    sessions.clone(),
                    deliveries.clone(),
                    events.clone(),
                );
                Box::pin(async move {
                    crate::epic_lead::notes::type_in_at_stop(
                        &board,
                        &sessions,
                        &deliveries,
                        &events,
                        &ticket_id,
                    )
                    .await
                        || crate::epic_lead::stuck::hold_at_idle(
                            &board,
                            &ticket_id,
                            last_line.as_deref(),
                        )
                        .await
                })
            }));
        }
        // The ticket environment (M19): Free at Review, Bring back for more work, Release at Done.
        let ticket_env = crate::ticket_env::TicketEnv::new(
            board.clone(),
            config.data.clone(),
            warm.clone(),
            seeder,
            sessions.clone(),
            events.clone(),
        );
        starter.set_ticket_env(ticket_env.clone());
        let cleanup = Cleanup::new(
            board.clone(),
            config.data.clone(),
            sessions.clone(),
            ports.clone(),
            ticket_env.clone(),
        );
        let review = Review::new(
            board.clone(),
            config.data.clone(),
            sessions.clone(),
            deliveries.clone(),
            starter.clone(),
            scheduler.clone(),
            ports,
            ticket_env.clone(),
        );
        // No epic lead is live after a restart: the questions that waited for one go to the user, before the
        // reconcile keeps the asking child's label (LEAD-06, d0045).
        match crate::epic_lead::questions::hand_back_after_restart(&board, &signals).await {
            Ok(0) => {}
            Ok(handed) => tracing::info!(
                handed,
                "questions waiting for a lead handed back to the user"
            ),
            Err(e) => {
                tracing::warn!(error = %e, "can't hand back the questions waiting for a lead")
            }
        }
        // Nor are the stops held for a lead: they go to the user too, before the reconcile (LEAD-07).
        match crate::epic_lead::stuck::hand_back_after_restart(&board, &signals, &sessions).await {
            Ok(0) => {}
            Ok(handed) => tracing::info!(handed, "stops held for a lead handed back to the user"),
            Err(e) => tracing::warn!(error = %e, "can't hand back the stops held for a lead"),
        }
        // Startup reconcile (REC-01): tickets whose agent was at work are Interrupted; nothing resumes.
        let recovery = Recovery::new(
            board.clone(),
            sessions.clone(),
            adapters.clone(),
            events.clone(),
        );
        starter.set_recovery(recovery.clone());
        if let Err(e) = recovery.startup(run.previous).await {
            tracing::error!(error = %e, "startup reconcile failed");
        }
        // The agent tools' trust for the projects added earlier (PRJ-08): in the background, so a slow or broken
        // config file never holds up the start; every problem is an outcome or a log line.
        {
            let (trust, store, events) = (board.trust().clone(), store.clone(), events.clone());
            tokio::spawn(async move {
                if trust.startup_pass(&store).await {
                    events.publish(protocol::Event::TrustChanged);
                }
            });
        }
        let opener = Opener::new(events.clone(), config.app.clone());
        let inbox = Inbox::new(
            board.clone(),
            sessions.clone(),
            deliveries.clone(),
            signals.clone(),
            review.clone(),
            events.clone(),
            crate::attention::notify::Notifications {
                notifier: config.notifier.clone(),
                opener: opener.clone(),
            },
        );
        let usage = Usage::new(
            store.clone(),
            events.clone(),
            adapters.clone(),
            config.usage_tick,
        );
        let plan_limit = PlanLimit::new(
            board.clone(),
            events.clone(),
            inbox.clone(),
            sessions.clone(),
            deliveries.clone(),
            adapters.clone(),
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
            ticket_env.clone(),
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
            ticket_env.clone(),
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
        let suggester = Suggester::new(board.clone(), events.clone(), adapters.clone());
        let prs = crate::pr::service::Prs::new(
            board.clone(),
            config.data.clone(),
            sessions.clone(),
            deliveries.clone(),
            adapters.clone(),
            events.clone(),
        );
        let messages = crate::messages::Messages::new(
            board.clone(),
            sessions.clone(),
            deliveries.clone(),
            events.clone(),
            config.data.clone(),
            adapters.clone(),
        );
        let validation = crate::validation::Validation::new(
            board.clone(),
            sessions.clone(),
            deliveries.clone(),
            adapters.clone(),
            events.clone(),
        );
        // A model the tool refused (AGT-05): back to not started, No result, or a failed turn.
        signals.set_refusals(crate::signals::Refusals {
            starter: starter.clone(),
            validation: validation.clone(),
            sessions: sessions.clone(),
        });
        {
            let review = review.clone();
            validation.on_resume_agent(Arc::new(move |id, prompt| {
                let review = review.clone();
                Box::pin(async move {
                    let ticket = review.board().ticket_by_id(&id).await?;
                    review
                        .resume_with_prompt(
                            &ticket.key.to_string(),
                            &prompt,
                            &crate::domain::Actor::current_user(),
                        )
                        .await
                        .map(|_| ())
                })
            }));
        }
        {
            let review = review.clone();
            let events = events.clone();
            validation.on_auto_merge(Arc::new(move |key| {
                let (review, events) = (review.clone(), events.clone());
                Box::pin(async move {
                    // The toast (UI-07); never a notification (d0064).
                    if let crate::review::auto_merge::AutoMerged::Merged { toast } =
                        review.auto_merge(&key).await?
                    {
                        events.publish(*toast);
                    }
                    Ok(())
                })
            }));
        }
        {
            let validation = validation.clone();
            review.on_resume_validation(Arc::new(move |id| {
                let validation = validation.clone();
                Box::pin(async move { validation.resume_interrupted(&id).await })
            }));
        }
        // Park and Close stop the validator too; the validation is recorded as stopped (CLS-01, CLS-06).
        for parked in [false, true] {
            let validation = validation.clone();
            let hook: crate::review::finish::FinishedHook = Arc::new(move |ticket| {
                let (validation, id) = (validation.clone(), ticket.id.clone());
                Box::pin(async move {
                    if let Err(e) = validation.abandon(&id).await {
                        tracing::warn!(ticket = id, error = %e, "can't stop the validation");
                    }
                })
            });
            if parked {
                review.on_ticket_parked(hook);
            } else {
                review.on_ticket_finished(hook);
            }
        }
        let project_agents = crate::project_agents::ProjectAgents::new(
            board.clone(),
            sessions.clone(),
            adapters.clone(),
            events.clone(),
            Arc::new(signals.clone()),
            deliveries.clone(),
        );
        match project_agents.startup().await {
            Ok(agents) => recovery.record_project_agents(&agents),
            Err(e) => tracing::warn!(error = %e, "can't reconcile the Project agents"),
        }
        // A child's question the lead didn't act on goes to the user (LEAD-06).
        crate::epic_lead::questions::LeadQuestions::new(
            board.clone(),
            signals.clone(),
            project_agents.leads(),
            deliveries.clone(),
            sessions.clone(),
            events.clone(),
        )
        .register_hand_back();
        // A child's first stop the lead didn't answer goes to the user (LEAD-07).
        crate::epic_lead::stuck::Stuck::new(crate::epic_lead::stuck::StuckParts {
            board: board.clone(),
            leads: project_agents.leads(),
            signals: signals.clone(),
            validation: validation.clone(),
            review: review.clone(),
            deliveries: deliveries.clone(),
            sessions: sessions.clone(),
            events: events.clone(),
        })
        .register_hand_back();
        plan_limit.set_project_agents(project_agents.clone());
        inbox.set_project_agents(project_agents.clone());
        // The copies read earlier; `serve` reads every list again at once, then daily.
        let models = crate::agent_tools::models::ModelLists::load(
            store.clone(),
            adapters.clone(),
            config.read_model_lists,
        )
        .await;
        // The named agents check models against the copies of the tools' lists (AGT-02).
        let named_agents = NamedAgents::new(board.clone(), Arc::new(models.clone()));
        // The starter entry when the list is empty (AGT-01): in the background, as detecting a tool runs it.
        spawn_starter(&adapters, &store, config.launch.is_some());
        let shared = Arc::new(Shared {
            named_agents,
            adapters,
            models,
            warm,
            ticket_env,
            project_agents,
            validation,
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
            opener,
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

    /// Validators (d0063).
    pub fn validation(&self) -> crate::validation::Validation {
        self.shared.validation.clone()
    }

    /// Project agents (d0068).
    pub fn project_agents(&self) -> crate::project_agents::ProjectAgents {
        self.shared.project_agents.clone()
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

    /// The warm builds (d0082).
    pub fn warm(&self) -> crate::warm::store::WarmBuilds {
        self.shared.warm.clone()
    }

    /// Every agent tool's adapter (ADP-01).
    pub fn adapters(&self) -> Adapters {
        self.shared.adapters.clone()
    }

    /// The ticket environment (M19).
    pub fn ticket_env(&self) -> crate::ticket_env::TicketEnv {
        self.shared.ticket_env.clone()
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
            mut listener,
            shared,
            lock,
        } = self;
        let mut stopped = shared.stop.subscribe();
        let scheduler = tokio::spawn(shared.scheduler.clone().run(shared.stop.subscribe()));
        let recovery = tokio::spawn(shared.recovery.clone().run(shared.stop.subscribe()));
        let inbox = tokio::spawn(shared.inbox.clone().run(shared.stop.subscribe()));
        let usage = tokio::spawn(shared.usage.clone().run(shared.stop.subscribe()));
        let plan_limit = tokio::spawn(shared.plan_limit.clone().run(shared.stop.subscribe()));
        let messages = tokio::spawn(shared.messages.clone().run(shared.stop.subscribe()));
        let ticket_env = tokio::spawn(shared.ticket_env.clone().run(shared.stop.subscribe()));
        let models = tokio::spawn(shared.models.clone().run(shared.stop.subscribe()));
        let mut connections = JoinSet::new();
        loop {
            tokio::select! {
                _ = until_stopped(&mut stopped) => break,
                accepted = listener.accept() => match accepted {
                    Ok(stream) => {
                        let peer = shared.config.peer_credentials.peer_user(&stream);
                        if access::peer_allowed(&shared.config.owner, peer) {
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
        let _ = messages.await;
        let _ = ticket_env.await;
        let _ = models.await;
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

/// Windows has no SIGTERM: Ctrl+C, Ctrl+Break, closing the console and logging off stop the service.
#[cfg(windows)]
async fn shutdown_signal() -> io::Result<()> {
    use tokio::signal::windows::{ctrl_break, ctrl_c, ctrl_close, ctrl_logoff, ctrl_shutdown};
    let mut c = ctrl_c()?;
    let mut brk = ctrl_break()?;
    let mut close = ctrl_close()?;
    let mut logoff = ctrl_logoff()?;
    let mut shutdown = ctrl_shutdown()?;
    tokio::select! {
        _ = c.recv() => {}
        _ = brk.recv() => {}
        _ = close.recv() => {}
        _ = logoff.recv() => {}
        _ = shutdown.recv() => {}
    }
    Ok(())
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
