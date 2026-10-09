//! The app's link to the background service: connects at launch (starting the service if needed, d0055),
//! follows the client's connection state, tells "reconnecting" from "stopped", keeps the agent counts for the
//! service banners fresh, and restarts or starts the service when the user clicks. The app is only a client:
//! closing it never stops the service (ARC-03), and nothing here restarts the service on its own (d0022).
//!
//! The core client runs on a small tokio runtime owned by the app; GPUI tasks await its results. tokio's sync
//! types work from GPUI's executor, so the state watch is followed directly.

use std::time::Duration;

use std::future::Future;

use crewboard_core::client::{Client, ClientConfig, ClientError, CommandLauncher, Compatibility};
use crewboard_core::data_dir::DataDir;
use crewboard_core::protocol::{ErrorKind, Event, Reply, Request};
use crewboard_core::service::lock::service_alive;
use gpui::{AsyncApp, Context, EventEmitter, WeakEntity};
use tokio::sync::broadcast::error::RecvError;

use crate::view_models::connection::LinkState;

/// How often the link checks whether a lost service still runs, and refreshes the banner counts.
const POLL: Duration = Duration::from_secs(1);
const COUNTS_POLL: Duration = Duration::from_secs(5);

/// An event from the service's stream (ticket changed, projects changed, …), re-emitted for views.
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceEvent {
    Event(Event),
    /// Events may have been missed (connection renewed, or too far behind): refetch what is shown.
    Resync,
}

/// Why a request failed, as the views show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestError {
    pub message: String,
    /// A board rule's code (`cycle`, `cross_base`, `empty_done_when`…), when a rule refused the request.
    pub refusal: Option<String>,
    pub not_found: bool,
}

impl RequestError {
    fn from_client(e: ClientError) -> Self {
        match e {
            ClientError::Service(body) => Self {
                refusal: body
                    .data
                    .as_ref()
                    .and_then(|d| d.get("refusal"))
                    .and_then(|r| r.as_str())
                    .map(str::to_owned),
                not_found: body.kind == ErrorKind::NotFound,
                message: body.message,
            },
            other => Self {
                message: other.to_string(),
                refusal: None,
                not_found: false,
            },
        }
    }

    fn plain(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            refusal: None,
            not_found: false,
        }
    }
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl EventEmitter<ServiceEvent> for ServiceLink {}

pub struct ServiceLink {
    pub state: LinkState,
    pub client: Option<Client>,
    /// A start or restart is running (the banner button shows loading).
    pub restarting: bool,
    /// The outcome of the last start/restart, for a toast: `Ok(())` or the error text. Taken by the view.
    pub restart_outcome: Option<Result<(), String>>,
    runtime: tokio::runtime::Handle,
    data: Option<DataDir>,
}

impl ServiceLink {
    pub fn new(runtime: tokio::runtime::Handle, cx: &mut Context<Self>) -> Self {
        let data = DataDir::from_env().ok();
        let link = Self {
            state: LinkState::Connecting,
            client: None,
            restarting: false,
            restart_outcome: None,
            runtime,
            data,
        };
        link.connect(cx);
        cx.spawn(async move |this, cx| poll(this, cx).await)
            .detach();
        link
    }

    /// Sends a request on the tokio runtime; `None` while there is no client yet. Await the handle from a GPUI
    /// task.
    pub fn spawn_request(
        &self,
        request: Request,
    ) -> Option<tokio::task::JoinHandle<Result<Reply, ClientError>>> {
        let client = self.client.clone()?;
        Some(
            self.runtime
                .spawn(async move { client.request(request).await }),
        )
    }

    /// Sends a request; await the result from a GPUI task. Fails at once while there is no client.
    pub fn send(
        &self,
        request: Request,
    ) -> impl Future<Output = Result<Reply, RequestError>> + 'static {
        let task = self.spawn_request(request);
        async move {
            match task {
                None => Err(RequestError::plain(
                    "not connected to the background service",
                )),
                Some(task) => match task.await {
                    Ok(Ok(reply)) => Ok(reply),
                    Ok(Err(e)) => Err(RequestError::from_client(e)),
                    Err(e) => Err(RequestError::plain(e.to_string())),
                },
            }
        }
    }

    /// A queue that sends requests one after another, in order: keystrokes must never overtake each other. The
    /// sending task ends when the sender is dropped. `None` while there is no client.
    #[allow(dead_code)] // used by the terminal view, hosted from T-0059
    pub fn ordered_sender(&self) -> Option<tokio::sync::mpsc::UnboundedSender<Request>> {
        let client = self.client.clone()?;
        let (sender, mut queue) = tokio::sync::mpsc::unbounded_channel::<Request>();
        self.runtime.spawn(async move {
            while let Some(request) = queue.recv().await {
                if let Err(e) = client.request(request).await {
                    eprintln!("terminal: request failed: {e}");
                }
            }
        });
        Some(sender)
    }

    fn launcher() -> Result<CommandLauncher, String> {
        CommandLauncher::next_to_current_exe().map_err(|e| e.to_string())
    }

    /// Connects, starting the service if none answers, then follows the connection state.
    fn connect(&self, cx: &mut Context<Self>) {
        let runtime = self.runtime.clone();
        let data = self.data.clone();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let connected = runtime
                .spawn(async move {
                    let data = data.ok_or("no home folder; set the data folder variable")?;
                    let launcher = Self::launcher()?;
                    Client::connect_or_start(ClientConfig::new(&data), &launcher)
                        .await
                        .map_err(|e| e.to_string())
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            match connected {
                Ok(client) => follow(this, client, cx).await,
                Err(reason) => {
                    let _ = this.update(cx, |link, cx| {
                        link.state = LinkState::Failed(reason);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    /// Restarts the service with this build (or starts it when none runs). Only called from the user's click
    /// (confirmed first when agents would be interrupted).
    pub fn restart(&mut self, cx: &mut Context<Self>) {
        if self.restarting {
            return;
        }
        self.restarting = true;
        cx.notify();
        let Some(client) = self.client.clone() else {
            // Never connected (the start at launch failed): try the launch-time connect again.
            self.state = LinkState::Connecting;
            self.restarting = false;
            self.connect(cx);
            return;
        };
        let runtime = self.runtime.clone();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let outcome = runtime
                .spawn(async move {
                    let launcher = Self::launcher()?;
                    client
                        .restart(&launcher)
                        .await
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            let _ = this.update(cx, |link, cx| {
                link.restarting = false;
                link.restart_outcome = Some(outcome);
                cx.notify();
            });
        })
        .detach();
    }
}

/// Mirrors the client's state into the link until the link is dropped.
async fn follow(this: WeakEntity<ServiceLink>, client: Client, cx: &mut AsyncApp) {
    let mut states = client.state();
    let first = states.borrow_and_update().clone();
    let stored = this.update(cx, |link, cx| {
        link.state = first.into();
        link.client = Some(client.clone());
        cx.notify();
        link.runtime.clone()
    });
    let Ok(runtime) = stored else {
        return;
    };
    let events_client = client.clone();
    let events = runtime
        .spawn(async move { events_client.subscribe().await })
        .await;
    if let Ok(Ok(receiver)) = events {
        let forward = this.clone();
        cx.spawn(async move |cx| forward_events(forward, receiver, cx).await)
            .detach();
    }
    while states.changed().await.is_ok() {
        let state = states.borrow_and_update().clone();
        let reconnected = matches!(state, crewboard_core::client::ConnectionState::Connected(_));
        let updated = this.update(cx, |link, cx| {
            link.state = state.into();
            link.check_alive();
            if reconnected {
                // Events from the gap are lost (the client resubscribes): views refetch.
                cx.emit(ServiceEvent::Resync);
            }
            cx.notify();
        });
        if updated.is_err() {
            return;
        }
    }
}

/// Re-emits the service's events until the link is dropped.
async fn forward_events(
    this: WeakEntity<ServiceLink>,
    mut receiver: tokio::sync::broadcast::Receiver<Event>,
    cx: &mut AsyncApp,
) {
    loop {
        let event = match receiver.recv().await {
            Ok(event) => ServiceEvent::Event(event),
            Err(RecvError::Lagged(_)) => ServiceEvent::Resync,
            Err(RecvError::Closed) => return,
        };
        if this.update(cx, |_, cx| cx.emit(event)).is_err() {
            return;
        }
    }
}

impl ServiceLink {
    /// While disconnected: "stopped" if no service process is alive any more, else "reconnecting".
    fn check_alive(&mut self) {
        let alive = self
            .data
            .as_ref()
            .is_some_and(|d| service_alive(&d.service_lock_path()));
        self.state = match std::mem::replace(&mut self.state, LinkState::Connecting) {
            LinkState::Reconnecting { .. } if !alive => LinkState::Stopped,
            LinkState::Stopped if alive => LinkState::Reconnecting { attempt: 0 },
            other => other,
        };
    }
}

/// Re-checks a lost service every second and refreshes the banner counts every few seconds while a service
/// banner shows.
async fn poll(this: WeakEntity<ServiceLink>, cx: &mut AsyncApp) {
    let mut since_counts = Duration::ZERO;
    loop {
        cx.background_executor().timer(POLL).await;
        since_counts += POLL;
        let refresh = this.update(cx, |link, cx| {
            if matches!(
                link.state,
                LinkState::Reconnecting { .. } | LinkState::Stopped
            ) {
                let before = link.state.clone();
                link.check_alive();
                if link.state != before {
                    cx.notify();
                }
            }
            match (&link.state, &link.client) {
                (LinkState::Connected(info), Some(client))
                    if info.compatibility != Compatibility::Same && since_counts >= COUNTS_POLL =>
                {
                    Some((client.clone(), link.runtime.clone()))
                }
                _ => None,
            }
        });
        let Ok(refresh) = refresh else {
            return;
        };
        let Some((client, runtime)) = refresh else {
            continue;
        };
        since_counts = Duration::ZERO;
        let status = runtime
            .spawn(async move { client.request(Request::ServiceStatus).await })
            .await;
        if let Ok(Ok(Reply::ServiceStatus(status))) = status {
            let _ = this.update(cx, |link, cx| {
                if let LinkState::Connected(info) = &mut link.state
                    && info.status != status
                {
                    info.status = status;
                    cx.notify();
                }
            });
        }
    }
}
