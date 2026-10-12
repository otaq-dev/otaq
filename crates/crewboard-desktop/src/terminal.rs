//! Terminal output for the page (UIT-05, SVC-05): the door opens one channel per terminal and hands its sending end
//! here as a [`Sink`]; [`attach`] answers with the attach's number and keeps the channel fed in the background.
//!
//! **Attach with replay**, as the M14 test build did it: the task subscribes to the service's events *before* it asks
//! for the attach, sends the stored scrollback ([`Feed::Replay`]), then each live chunk cut to start exactly where the
//! shown output ends ([`Follow`]), so no byte shows twice or goes missing. While the connection is lost the page hears
//! [`Phase::Reconnecting`] and keeps its scrollback (SVC-04); once the service answers again the task attaches anew
//! and the page redraws from the fresh replay. A gap in the live output (the event queue overflowed) reattaches too.
//!
//! Input, resize and focus are plain requests (`api/terminal.rs`); [`Terminals::detach`] ends an attach.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crewboard_core::client::{Client, ClientError, ConnectionState};
use crewboard_core::protocol::{
    ErrorKind, Event, Reply, Request, SessionIdParams, SessionTarget, decode_bytes, encode_bytes,
};
use crewboard_core::session::{SessionKind, SessionState};
use crewboard_view::terminal::Phase;
use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::watch;
use ts_rs::TS;

use crate::App;
use crate::events::Sink;

/// One message of a terminal's channel. `TerminalFeed` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "TerminalFeed")]
pub enum Feed {
    /// Clear the terminal and write the session's stored output (base64; the bytes need not be UTF-8): the first
    /// attach, and again after a reconnect.
    Replay { data: String },
    /// Live output (base64), continuing exactly where the previous piece ended.
    Live { data: String },
    /// Where the terminal stands; it takes input only while `live`.
    Phase { phase: Phase },
}

/// Where the output shown so far ends, in the session's whole output (the service numbers every byte it ever
/// printed): live chunks are cut to start exactly there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Follow {
    next: u64,
}

/// What became of one live chunk.
#[derive(Debug, PartialEq, Eq)]
pub enum Piece<'a> {
    /// Already shown (it arrived before the attach answer and the replay holds it).
    Old,
    /// The part not shown yet.
    New(&'a [u8]),
    /// Bytes before it are missing: attach again.
    Gap,
}

impl Follow {
    /// After a replay that ended at `end`.
    pub fn after_replay(end: u64) -> Self {
        Self { next: end }
    }

    pub fn live<'a>(&mut self, offset: u64, bytes: &'a [u8]) -> Piece<'a> {
        let end = offset + bytes.len() as u64;
        if end <= self.next {
            return Piece::Old;
        }
        if offset > self.next {
            return Piece::Gap;
        }
        let new = &bytes[(self.next - offset) as usize..];
        self.next = end;
        Piece::New(new)
    }
}

/// The page's attaches, by the number [`attach`] answered.
#[derive(Default)]
pub struct Terminals {
    next: AtomicU64,
    attached: Mutex<HashMap<u64, Attached>>,
}

struct Attached {
    session: String,
    /// The session's kind once the service answered the attach.
    kind: Arc<Mutex<Option<SessionKind>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Attached {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Terminals {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Attached>> {
        self.attached.lock().expect("terminals")
    }

    /// Whether some attach of `session` shows an agent: only the agent's terminal counts for the focused-terminal
    /// rule (ATT-03); a Shell or a Project terminal is the user's own.
    pub fn is_agent(&self, session: &str) -> bool {
        self.lock().values().any(|a| {
            a.session == session && *a.kind.lock().expect("kind") == Some(SessionKind::Agent)
        })
    }

    /// Ends an attach: its channel gets nothing more. The service stops sending the session's output to this app
    /// once no other terminal of the page shows it. The session keeps running.
    pub async fn detach(&self, app: &App, attach: u64) {
        let session = {
            let mut attached = self.lock();
            let Some(gone) = attached.remove(&attach) else {
                return;
            };
            let session = gone.session.clone();
            (!attached.values().any(|a| a.session == session)).then_some(session)
        };
        if let Some(session) = session {
            let _ = app
                .service
                .send(Request::SessionDetach(SessionIdParams { session }))
                .await;
        }
    }
}

/// Attaches the page's terminal to `session`: `sink` gets a [`Feed`] per message, as JSON, from now until
/// [`Terminals::detach`]. Answers the attach's number at once; the scrollback follows on the channel.
pub async fn attach(app: &App, session: &str, sink: Sink) -> Result<u64, String> {
    if session.is_empty() {
        return Err("no terminal session given".into());
    }
    let number = app.terminals.next.fetch_add(1, Ordering::Relaxed) + 1;
    let kind = Arc::new(Mutex::new(None));
    let task = tokio::spawn(follow(
        app.service.clients(),
        session.to_owned(),
        Feeder::new(sink),
        kind.clone(),
    ));
    app.terminals.lock().insert(
        number,
        Attached {
            session: session.to_owned(),
            kind,
            task,
        },
    );
    Ok(number)
}

/// Sends feeds, leaving out a phase the page already has.
struct Feeder {
    sink: Sink,
    phase: Option<Phase>,
}

impl Feeder {
    fn new(sink: Sink) -> Self {
        Self { sink, phase: None }
    }

    /// False once the page behind the channel is gone.
    fn send(&mut self, feed: Feed) -> bool {
        if let Feed::Phase { phase } = &feed {
            if self.phase.as_ref() == Some(phase) {
                return true;
            }
            self.phase = Some(phase.clone());
        }
        (self.sink)(serde_json::to_value(&feed).expect("feeds serialize"))
    }

    fn phase(&mut self, phase: Phase) -> bool {
        self.send(Feed::Phase { phase })
    }
}

/// How long to wait before asking again when the service didn't answer although the link looked connected.
const RETRY: Duration = Duration::from_millis(200);

/// The attach task: attaches whenever the service is connected, streams until the connection drops or output goes
/// missing, and attaches again.
async fn follow(
    mut clients: watch::Receiver<Option<Client>>,
    session: String,
    mut feed: Feeder,
    kind: Arc<Mutex<Option<SessionKind>>>,
) {
    feed.phase(Phase::Attaching);
    let client = loop {
        if let Some(client) = clients.borrow_and_update().clone() {
            break client;
        }
        if clients.changed().await.is_err() {
            return;
        }
    };
    let mut states = client.state();
    loop {
        while !matches!(*states.borrow_and_update(), ConnectionState::Connected(_)) {
            if !feed.phase(Phase::Reconnecting) || states.changed().await.is_err() {
                return;
            }
        }
        // Subscribe first: live output sent while the attach is answered waits in the receiver.
        let Ok(mut events) = client.subscribe().await else {
            tokio::time::sleep(RETRY).await;
            continue;
        };
        let attached = match client
            .request(Request::SessionAttach(SessionTarget {
                session: Some(session.clone()),
                ..SessionTarget::default()
            }))
            .await
        {
            Ok(Reply::SessionAttached(attached)) => attached,
            // Gone (ended and forgotten, or a service restart ended it): nothing more will come.
            Err(ClientError::Service(body)) if body.kind == ErrorKind::NotFound => {
                feed.phase(Phase::NoSession);
                return;
            }
            Ok(other) => {
                eprintln!("terminal {session}: unexpected answer to session.attach: {other:?}");
                feed.phase(Phase::NoSession);
                return;
            }
            Err(_) => {
                tokio::time::sleep(RETRY).await;
                continue;
            }
        };
        *kind.lock().expect("kind") = Some(attached.session.kind);
        let mut shown = Follow::after_replay(attached.end);
        let phase = if attached.session.state == SessionState::Ended {
            Phase::Ended
        } else {
            Phase::Live
        };
        if !feed.send(Feed::Replay {
            data: attached.scrollback,
        }) || !feed.phase(phase)
        {
            return;
        }
        loop {
            tokio::select! {
                event = events.recv() => match event {
                    Ok(Event::SessionOutput { session: s, offset, data }) if s == session => {
                        let Ok(bytes) = decode_bytes(&data) else {
                            continue;
                        };
                        let data = match shown.live(offset, &bytes) {
                            Piece::Old => continue,
                            Piece::New(new) if new.len() == bytes.len() => data,
                            Piece::New(new) => encode_bytes(new),
                            Piece::Gap => break,
                        };
                        if !feed.send(Feed::Live { data }) {
                            return;
                        }
                    }
                    Ok(Event::SessionState { session: info })
                        if info.id == session && info.state == SessionState::Ended =>
                    {
                        if !feed.phase(Phase::Ended) {
                            return;
                        }
                    }
                    Ok(_) => {}
                    // Output went missing: attach again for a fresh replay.
                    Err(RecvError::Lagged(_)) => break,
                    Err(RecvError::Closed) => return,
                },
                changed = states.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    if !matches!(*states.borrow(), ConnectionState::Connected(_)) {
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_chunks_are_cut_where_the_replay_ended() {
        let mut shown = Follow::after_replay(10);
        // Entirely inside the replay: dropped.
        assert_eq!(shown.live(4, b"456789"), Piece::Old);
        assert_eq!(shown.live(10, b""), Piece::Old);
        // Overlapping its end: only the new part.
        assert_eq!(shown.live(8, b"89ab"), Piece::New(b"ab"));
        // Straight after: whole.
        assert_eq!(shown.live(12, b"cd"), Piece::New(b"cd"));
        // A repeat of what was just shown: dropped.
        assert_eq!(shown.live(12, b"cd"), Piece::Old);
        // Something missing in between: attach again.
        assert_eq!(shown.live(20, b"kl"), Piece::Gap);
        assert_eq!(
            shown.live(14, b"ef"),
            Piece::New(b"ef"),
            "a gap changes nothing"
        );
    }

    #[test]
    fn the_page_reads_each_feed_by_its_kind() {
        let json = |f: Feed| serde_json::to_value(f).unwrap();
        assert_eq!(
            json(Feed::Replay {
                data: "aGk=".into()
            }),
            serde_json::json!({ "kind": "replay", "data": "aGk=" })
        );
        assert_eq!(
            json(Feed::Phase {
                phase: Phase::Reconnecting
            }),
            serde_json::json!({ "kind": "phase", "phase": "reconnecting" })
        );
    }

    #[test]
    fn a_phase_the_page_has_is_not_sent_again() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let record = sent.clone();
        let mut feed = Feeder::new(Box::new(move |v| {
            record.lock().unwrap().push(v);
            true
        }));
        feed.phase(Phase::Reconnecting);
        feed.phase(Phase::Reconnecting);
        feed.send(Feed::Live {
            data: "eA==".into(),
        });
        feed.phase(Phase::Live);
        assert_eq!(sent.lock().unwrap().len(), 3);
    }
}
