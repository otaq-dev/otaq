//! The page's event stream: the door subscribes once per page load and every published event goes to each
//! subscriber. The service's events are published here from T-0211.

use std::sync::Mutex;

use serde_json::Value;

/// Where one subscriber's messages go. Returns false once the page behind it is gone, and is then dropped.
pub type Sink = Box<dyn Fn(Value) -> bool + Send + Sync>;

#[derive(Default)]
pub struct EventHub {
    sinks: Mutex<Vec<Sink>>,
}

impl EventHub {
    pub fn subscribe(&self, sink: Sink) {
        self.sinks.lock().expect("event sinks").push(sink);
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "T-0211 publishes the service's events")
    )]
    pub fn publish(&self, event: &Value) {
        self.sinks
            .lock()
            .expect("event sinks")
            .retain(|sink| sink(event.clone()));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use serde_json::json;

    use super::*;

    #[test]
    fn every_subscriber_gets_each_event_and_gone_pages_are_dropped() {
        let hub = EventHub::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let alive = Arc::new(AtomicBool::new(true));
        for name in ["first", "second"] {
            let (seen, alive) = (seen.clone(), alive.clone());
            hub.subscribe(Box::new(move |event| {
                seen.lock().unwrap().push(format!("{name} {event}"));
                name == "first" || alive.load(Ordering::SeqCst)
            }));
        }
        hub.publish(&json!(1));
        alive.store(false, Ordering::SeqCst);
        hub.publish(&json!(2));
        hub.publish(&json!(3));
        assert_eq!(
            *seen.lock().unwrap(),
            ["first 1", "second 1", "first 2", "second 2", "first 3"]
        );
    }
}
