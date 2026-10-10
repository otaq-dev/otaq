//! The focused-terminal rule (ATT-03, d0042): while a ticket's terminal has focus, in the app or through
//! `crewboard attach`, the user is looking at it. Its agent's Asks you / Needs you / Idle are still recorded on the
//! ticket, but they raise **no orange, no inbox item and no OS notification**; the ticket shows a quiet "your
//! turn" instead ([`crate::domain::Flags::your_turn`]). When the last focused terminal lets go and the agent still
//! waits, the orange state and its inbox item appear at that moment.
//!
//! Focus is tracked per **client connection** and session: the app reports focus gained and lost for its
//! terminal view, `crewboard attach` counts as focused while attached, and a connection that closes loses every
//! focus it held. A ticket is focused while any client holds focus on one of its running sessions. Kept in the
//! service's memory only: after a restart no terminal has focus.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// One client connection to the service, numbered by the service.
pub type ClientId = u64;

/// Who holds focus on which ticket. Cheap to clone; clones share the same state.
#[derive(Debug, Clone, Default)]
pub struct Focus {
    inner: Arc<Mutex<HashMap<String, HashSet<Holder>>>>,
}

/// A client's terminal for one session of a ticket.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Holder {
    session: String,
    client: ClientId,
}

/// What a focus report changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FocusChange {
    /// Whether any client now holds focus on the session, and whether that changed.
    pub session_focused: bool,
    pub session_changed: bool,
    /// Whether the ticket's focus (any of its sessions) changed.
    pub ticket_changed: bool,
}

impl Focus {
    /// A client's terminal for `session` (of `ticket_id`) gained or lost focus.
    pub fn set(
        &self,
        ticket_id: &str,
        session: &str,
        client: ClientId,
        focused: bool,
    ) -> FocusChange {
        let mut all = self.inner.lock().expect("focus");
        let ticket_before = all.get(ticket_id).is_some_and(|h| !h.is_empty());
        let session_before = holds_session(&all, session);
        let holder = Holder {
            session: session.to_owned(),
            client,
        };
        if focused {
            all.entry(ticket_id.to_owned()).or_default().insert(holder);
        } else if let Some(holders) = all.get_mut(ticket_id) {
            holders.remove(&holder);
            if holders.is_empty() {
                all.remove(ticket_id);
            }
        }
        let ticket_after = all.get(ticket_id).is_some_and(|h| !h.is_empty());
        let session_after = holds_session(&all, session);
        FocusChange {
            session_focused: session_after,
            session_changed: session_before != session_after,
            ticket_changed: ticket_before != ticket_after,
        }
    }

    /// A client disconnected: it loses every focus it held. Returns the sessions that lost their last holder,
    /// with their ticket, and whether that ticket lost its focus too.
    pub fn client_gone(&self, client: ClientId) -> Vec<Released> {
        self.release(|h| h.client == client)
    }

    /// A session ended: nobody can be looking at it any more.
    pub fn session_gone(&self, session: &str) -> Vec<Released> {
        self.release(|h| h.session == session)
    }

    /// Whether any client holds focus on one of the ticket's sessions.
    pub fn is_focused(&self, ticket_id: &str) -> bool {
        self.inner
            .lock()
            .expect("focus")
            .get(ticket_id)
            .is_some_and(|h| !h.is_empty())
    }

    /// Whether any client holds focus on this session.
    pub fn session_focused(&self, session: &str) -> bool {
        holds_session(&self.inner.lock().expect("focus"), session)
    }

    fn release(&self, drop_it: impl Fn(&Holder) -> bool) -> Vec<Released> {
        let mut all = self.inner.lock().expect("focus");
        let mut released: Vec<Released> = Vec::new();
        for (ticket_id, holders) in all.iter_mut() {
            let dropped: Vec<Holder> = holders.iter().filter(|h| drop_it(h)).cloned().collect();
            if dropped.is_empty() {
                continue;
            }
            for holder in &dropped {
                holders.remove(holder);
            }
            let mut sessions: Vec<String> = dropped.into_iter().map(|h| h.session).collect();
            sessions.sort();
            sessions.dedup();
            for session in sessions {
                if !holders.iter().any(|h| h.session == session) {
                    released.push(Released {
                        ticket_id: ticket_id.clone(),
                        session,
                        ticket_lost_focus: holders.is_empty(),
                    });
                }
            }
        }
        all.retain(|_, holders| !holders.is_empty());
        released
    }
}

/// A session that lost its last focus holder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Released {
    pub ticket_id: String,
    pub session: String,
    /// No terminal of the ticket has focus any more.
    pub ticket_lost_focus: bool,
}

fn holds_session(all: &HashMap<String, HashSet<Holder>>, session: &str) -> bool {
    all.values().flatten().any(|h| h.session == session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ticket_stays_focused_while_any_client_holds_it() {
        let focus = Focus::default();
        let app = focus.set("t1", "s1", 1, true);
        assert!(app.ticket_changed && app.session_changed && app.session_focused);
        let attach = focus.set("t1", "s1", 2, true);
        assert!(!attach.ticket_changed && !attach.session_changed);
        // The app leaves; attach still looks at it.
        let left = focus.set("t1", "s1", 1, false);
        assert!(!left.ticket_changed && left.session_focused);
        assert!(focus.is_focused("t1"));
        let gone = focus.client_gone(2);
        assert_eq!(
            gone,
            vec![Released {
                ticket_id: "t1".into(),
                session: "s1".into(),
                ticket_lost_focus: true,
            }]
        );
        assert!(!focus.is_focused("t1"));
        assert!(!focus.session_focused("s1"));
    }

    #[test]
    fn repeated_reports_change_nothing() {
        let focus = Focus::default();
        focus.set("t1", "s1", 1, true);
        assert_eq!(
            focus.set("t1", "s1", 1, true),
            FocusChange {
                session_focused: true,
                session_changed: false,
                ticket_changed: false,
            }
        );
        focus.set("t1", "s1", 1, false);
        assert!(!focus.set("t1", "s1", 1, false).ticket_changed);
        assert!(focus.client_gone(1).is_empty());
    }

    #[test]
    fn an_ended_session_releases_its_ticket() {
        let focus = Focus::default();
        focus.set("t1", "s1", 1, true);
        focus.set("t2", "s2", 1, true);
        let gone = focus.session_gone("s1");
        assert_eq!(gone.len(), 1);
        assert!(gone[0].ticket_lost_focus);
        assert!(!focus.is_focused("t1"));
        assert!(focus.is_focused("t2"));
    }
}
