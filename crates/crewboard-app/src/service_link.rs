//! The app's link to the background service: connects at launch (starting the service if needed, d0055) and
//! follows the client's connection state. The app is only a client: closing it never stops the service (ARC-03).
//!
//! The core client runs on a small tokio runtime owned by the app; GPUI tasks await its results. tokio's sync
//! types work from GPUI's executor, so the state watch is followed directly.

use crewboard_core::client::{Client, ClientConfig, CommandLauncher};
use crewboard_core::data_dir::DataDir;
use gpui::{AsyncApp, Context, WeakEntity};

use crate::view_models::connection::LinkState;

pub struct ServiceLink {
    pub state: LinkState,
    pub client: Option<Client>,
}

impl ServiceLink {
    pub fn new(runtime: tokio::runtime::Handle, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let connected = runtime
                .spawn(async move {
                    let data = DataDir::from_env().map_err(|e| e.to_string())?;
                    let launcher =
                        CommandLauncher::next_to_current_exe().map_err(|e| e.to_string())?;
                    Client::connect_or_start(ClientConfig::new(&data), &launcher)
                        .await
                        .map_err(|e| e.to_string())
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            let client = match connected {
                Ok(client) => client,
                Err(reason) => {
                    let _ = this.update(cx, |link, cx| {
                        link.state = LinkState::Failed(reason);
                        cx.notify();
                    });
                    return;
                }
            };
            let mut states = client.state();
            let first = states.borrow_and_update().clone();
            let updated = this.update(cx, |link, cx| {
                link.state = first.into();
                link.client = Some(client);
                cx.notify();
            });
            if updated.is_err() {
                return;
            }
            while states.changed().await.is_ok() {
                let state = states.borrow_and_update().clone();
                let updated = this.update(cx, |link, cx| {
                    link.state = state.into();
                    cx.notify();
                });
                if updated.is_err() {
                    return;
                }
            }
        })
        .detach();
        Self {
            state: LinkState::Connecting,
            client: None,
        }
    }
}
