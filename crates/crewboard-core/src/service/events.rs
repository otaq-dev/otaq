//! The live event stream (d0055, SDD §5.4). Later tickets publish ticket, session, inbox, banner and usage
//! events here; every subscribed connection forwards them to its client in order.
//!
//! Publishing never waits: a subscriber that falls too far behind (its connection's queue is full, or it
//! missed events) is disconnected. The client reconnects and fetches fresh state.

use std::sync::Arc;

use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{broadcast, mpsc, watch};

use crate::protocol::{Event, Message};

/// Publishes events to all subscribers. Cheap to clone.
#[derive(Debug, Clone)]
pub struct EventBus(broadcast::Sender<Event>);

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        Self(broadcast::channel(capacity).0)
    }

    /// Sends an event to every subscriber. Never blocks; with no subscribers the event is dropped.
    pub fn publish(&self, event: Event) {
        let _ = self.0.send(event);
    }

    /// Connections currently subscribed.
    pub fn subscriber_count(&self) -> usize {
        self.0.receiver_count()
    }

    pub(super) fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.0.subscribe()
    }
}

/// Moves events from the bus into one connection's outgoing queue until the connection ends. If the client
/// can't keep up, sets `kick` so the connection closes.
pub(super) async fn forward(
    mut events: broadcast::Receiver<Event>,
    out: mpsc::Sender<Message>,
    kick: Arc<watch::Sender<bool>>,
) {
    loop {
        match events.recv().await {
            Ok(event) => match out.try_send(Message::event(event)) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    tracing::warn!("subscriber fell behind (queue full); disconnecting it");
                    kick.send_replace(true);
                    return;
                }
                Err(TrySendError::Closed(_)) => return,
            },
            Err(RecvError::Lagged(missed)) => {
                tracing::warn!(
                    missed,
                    "subscriber fell behind (missed events); disconnecting it"
                );
                kick.send_replace(true);
                return;
            }
            Err(RecvError::Closed) => return,
        }
    }
}
