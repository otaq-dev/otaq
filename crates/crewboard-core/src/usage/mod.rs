//! Token usage (USG-01; d0020): tokens per ticket and today's total, read **read-only** from the conversations
//! Claude Code saves for the Claude session IDs Crewboard stored ([`transcript`]).
//!
//! The [`Usage`] reader runs in the service on a slow tick ([`DEFAULT_TICK`]) and soon after a session starts or
//! ends or a card's status changes. Each read takes only the lines added since the last one, attributes every API
//! response to the Crewboard session that was running when it was written (a resumed conversation keeps its
//! Claude session ID, so its file is shared by several sessions) and to the local day it was written on, and
//! stores the totals in the `usage` table, one row per session and day. Clients get `usage_changed` and ask again.
//!
//! Claude Code's files are internal and may change (P8): a missing, unreadable or unrecognised file logs one
//! warning and makes that conversation's usage **unknown**; nothing else is affected. Errors never reach tickets,
//! sessions or the scheduler. Conversations whose sessions have all ended are read once more and then left
//! alone; after a service restart, those that ended more than a day ago and already have counts aren't read again.

pub mod limit;
pub mod plan;
pub mod pricing;
pub mod transcript;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{Notify, broadcast, watch};

use crate::board::BoardError;
use crate::protocol::Event;
use crate::recovery::transcript::find_transcript;
use crate::service::EventBus;
use crate::store::{Store, StoreError, now_ms};
use transcript::{Conversation, MessageUsage, Unknown};

/// The slow tick: a read at least this often.
pub const DEFAULT_TICK: Duration = Duration::from_secs(60);
/// How long a burst of session and status changes may settle before a read.
const SETTLE: Duration = Duration::from_secs(1);
/// After a restart, finished conversations with counts aren't read again once they ended this long ago.
const SETTLED_AFTER_MS: i64 = 24 * 60 * 60 * 1000;

/// Token counts as Claude Code reports them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    /// Prompt-cache writes (`cache_creation_input_tokens`).
    pub cache_write: u64,
    /// Prompt-cache reads (`cache_read_input_tokens`).
    pub cache_read: u64,
}

impl Tokens {
    /// The one number shown on cards, in the panel and in the header: input, output and cache writes. Cache reads
    /// (the conversation re-read on every request) are stored but not shown (Q26, deferred: built as option 1).
    pub fn shown(&self) -> u64 {
        self.input + self.output + self.cache_write
    }

    fn add(&mut self, other: &Tokens) {
        self.input += other.input;
        self.output += other.output;
        self.cache_write += other.cache_write;
        self.cache_read += other.cache_read;
    }
}

/// A ticket's tokens over all its sessions (agent, resumed, answer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketUsage {
    /// The ticket's id.
    pub id: String,
    /// `None` when no conversation of the ticket could be read ("unknown", shown as "—").
    pub tokens: Option<Tokens>,
}

/// One session's tokens (the History tab).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionUsage {
    pub session: String,
    /// `None` when its conversation couldn't be read.
    pub tokens: Option<Tokens>,
}

/// Tokens and their estimated cost (one `usage` row, or a sum of rows).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counted {
    pub tokens: Tokens,
    /// What they would cost at the API's prices, in millionths of a dollar ([`pricing`]).
    pub cost_micro_usd: u64,
}

impl Counted {
    fn add(&mut self, other: &Counted) {
        self.tokens.add(&other.tokens);
        self.cost_micro_usd += other.cost_micro_usd;
    }
}

/// The answer to `usage.summary`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageSummary {
    /// Today, as `YYYY-MM-DD` in local time.
    pub day: String,
    /// Today's tokens across all projects.
    pub today: Tokens,
    /// Today's dollar estimate across all projects, in millionths of a dollar (shown only when the setting is
    /// on, USG-02).
    #[serde(default)]
    pub today_cost_micro_usd: u64,
    /// The requested project's tickets that ran a conversation.
    pub tickets: Vec<TicketUsage>,
}

/// `YYYY-MM-DD` of a Unix-milliseconds moment in local time (the `usage.day` column).
pub fn local_day(at_ms: i64) -> String {
    let secs: libc::time_t = at_ms.div_euclid(1000);
    // SAFETY: localtime_r writes only into the zeroed struct passed to it.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&secs, &mut tm).is_null() } {
        return format!("unix-day-{}", at_ms.div_euclid(86_400_000));
    }
    format!(
        "{:04}-{:02}-{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday
    )
}

/// A Crewboard session that ran a Claude conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSpan {
    pub id: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
}

/// Totals per (session, day) of a conversation's responses. A response belongs to the last session that had
/// started when it was written (the first session when it is older, the last when its time is unknown).
pub fn attribute<'a>(
    sessions: &[SessionSpan],
    messages: impl IntoIterator<Item = &'a MessageUsage>,
    now: i64,
) -> BTreeMap<(String, String), Counted> {
    let mut sorted: Vec<&SessionSpan> = sessions.iter().collect();
    sorted.sort_by_key(|s| (s.started_at, s.id.clone()));
    let mut rows: BTreeMap<(String, String), Counted> = BTreeMap::new();
    let (Some(first), Some(last)) = (sorted.first(), sorted.last()) else {
        return rows;
    };
    for message in messages {
        let session = match message.at {
            Some(at) => sorted
                .iter()
                .rev()
                .find(|s| s.started_at <= at)
                .unwrap_or(first),
            None => last,
        };
        let day = local_day(message.at.unwrap_or(now));
        let cost = pricing::cost_micro_usd(
            message.model.as_deref(),
            &message.tokens,
            message.cache_write_1h,
        );
        rows.entry((session.id.clone(), day))
            .or_default()
            .add(&Counted {
                tokens: message.tokens,
                cost_micro_usd: cost.unwrap_or(0),
            });
    }
    rows
}

/// The usage reader of one service. Cheap to clone.
#[derive(Clone)]
pub struct Usage {
    inner: Arc<Inner>,
}

struct Inner {
    store: Store,
    events: EventBus,
    /// Claude Code's folder (`$CLAUDE_CONFIG_DIR` or `~/.claude`); `None` makes every conversation unknown.
    claude_dir: Option<PathBuf>,
    tick: Duration,
    /// One read at a time.
    state: tokio::sync::Mutex<HashMap<String, Tracked>>,
    /// Claude session IDs whose usage is unknown.
    unknown: Mutex<HashSet<String>>,
    wake: Notify,
}

/// What the reader knows about one Claude session ID.
#[derive(Debug, Default)]
struct Tracked {
    conversation: Conversation,
    path: Option<PathBuf>,
    unknown: Option<Unknown>,
    /// The one warning for this conversation was logged.
    warned: bool,
    /// Every session ended and was read after: nothing more will come.
    settled: bool,
    /// The rows last written.
    written: BTreeMap<(String, String), Counted>,
}

/// One row of the session query.
#[derive(Debug, Clone)]
struct StoredSession {
    span: SessionSpan,
    claude_id: String,
    /// The worktree, else the repository: where `claude` ran.
    cwd: Option<String>,
}

impl Usage {
    pub fn new(
        store: Store,
        events: EventBus,
        claude_dir: Option<PathBuf>,
        tick: Duration,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                store,
                events,
                claude_dir,
                tick,
                state: tokio::sync::Mutex::new(HashMap::new()),
                unknown: Mutex::new(HashSet::new()),
                wake: Notify::new(),
            }),
        }
    }

    /// Asks the loop for a read soon.
    pub fn wake(&self) {
        self.inner.wake.notify_one();
    }

    /// Reads what was added to every conversation that may still change and stores the totals. Returns whether
    /// anything changed (counts or unknown conversations); `usage_changed` is published then. Errors are logged
    /// by the loop and never reach anything else.
    pub async fn read_now(&self) -> Result<bool, StoreError> {
        let inner = &self.inner;
        let mut state = inner.state.lock().await;
        let started = now_ms();
        let sessions = self.sessions().await?;
        let mut by_claude: BTreeMap<String, Vec<StoredSession>> = BTreeMap::new();
        for session in sessions {
            by_claude
                .entry(session.claude_id.clone())
                .or_default()
                .push(session);
        }
        let counted = self.counted_conversations().await?;

        let mut work = Vec::new();
        for (claude_id, sessions) in &by_claude {
            let ended_at = sessions
                .iter()
                .map(|s| s.span.ended_at)
                .collect::<Option<Vec<_>>>()
                .and_then(|ends| ends.into_iter().max());
            let tracked = state.entry(claude_id.clone()).or_insert_with(|| Tracked {
                // Finished long ago and counted by an earlier run: nothing new will come.
                settled: ended_at.is_some_and(|end| started - end > SETTLED_AFTER_MS)
                    && counted.contains(claude_id),
                ..Default::default()
            });
            // A session started again on this conversation (a resume).
            if ended_at.is_none() {
                tracked.settled = false;
            }
            if tracked.settled {
                continue;
            }
            work.push((claude_id.clone(), std::mem::take(tracked), sessions.clone()));
        }

        let claude_dir = inner.claude_dir.clone();
        let read = tokio::task::spawn_blocking(move || {
            work.into_iter()
                .map(|(claude_id, mut tracked, sessions)| {
                    let result = read_conversation(
                        claude_dir.as_deref(),
                        &claude_id,
                        &sessions,
                        &mut tracked,
                    );
                    (claude_id, tracked, sessions, result)
                })
                .collect::<Vec<_>>()
        })
        .await
        .unwrap_or_default();

        let mut changed = false;
        for (claude_id, mut tracked, sessions, result) in read {
            match result {
                Ok(()) => {
                    if tracked.unknown.take().is_some() {
                        changed = true;
                    }
                    let spans: Vec<SessionSpan> = sessions.iter().map(|s| s.span.clone()).collect();
                    let rows = attribute(&spans, tracked.conversation.messages(), started);
                    for (key, counted) in &rows {
                        if tracked.written.get(key) != Some(counted) {
                            self.write_row(&key.0, &key.1, counted).await?;
                            changed = true;
                        }
                    }
                    tracked.written = rows;
                }
                Err(why) => {
                    if !tracked.warned {
                        tracing::warn!(claude_session = %claude_id, reason = ?why, "token usage unknown");
                        tracked.warned = true;
                    }
                    if tracked.unknown.replace(why).is_none() {
                        changed = true;
                    }
                }
            }
            let all_ended = sessions
                .iter()
                .all(|s| s.span.ended_at.is_some_and(|end| end < started));
            tracked.settled = all_ended;
            state.insert(claude_id, tracked);
        }

        let unknown: HashSet<String> = state
            .iter()
            .filter(|(_, t)| t.unknown.is_some())
            .map(|(id, _)| id.clone())
            .collect();
        *inner.unknown.lock().expect("unknown usage") = unknown;
        if changed {
            inner.events.publish(Event::UsageChanged);
        }
        Ok(changed)
    }

    /// Runs reads on the tick and soon after session and status changes until `stop` is set.
    pub async fn run(self, mut stop: watch::Receiver<bool>) {
        let mut listener = self.inner.events.listen();
        let mut tick = tokio::time::interval(self.inner.tick);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            let settle = tokio::select! {
                _ = stop.wait_for(|s| *s) => return,
                _ = tick.tick() => false,
                () = self.inner.wake.notified() => false,
                event = listener.receiver.recv() => match event {
                    Ok(Event::SessionState { .. } | Event::CardStatusChanged { .. })
                    | Err(broadcast::error::RecvError::Lagged(_)) => true,
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Closed) => return,
                },
            };
            if settle {
                // Let the burst settle (a session start brings several changes), then read once.
                tokio::time::sleep(SETTLE).await;
                while listener.receiver.try_recv().is_ok() {}
            }
            if let Err(e) = self.read_now().await {
                tracing::warn!(error = %e, "can't record token usage");
            }
        }
    }

    /// Today's tokens across all projects, and the tokens of `project`'s tickets (`usage.summary`).
    pub async fn summary(&self, project: Option<&str>) -> Result<UsageSummary, BoardError> {
        let day = local_day(now_ms());
        let today = self.day_total(&day).await?;
        let tickets = match project {
            Some(project) => self.tickets(project).await?,
            None => Vec::new(),
        };
        Ok(UsageSummary {
            day,
            today: today.tokens,
            today_cost_micro_usd: today.cost_micro_usd,
            tickets,
        })
    }

    /// The tokens of one day (`YYYY-MM-DD`) across all projects, with their estimated cost.
    pub async fn day_total(&self, day: &str) -> Result<Counted, StoreError> {
        let (a, b, c, d, cost): DayRow = sqlx::query_as(
            "SELECT SUM(tokens_in), SUM(tokens_out), SUM(cache_write_tokens), SUM(cache_read_tokens),
                    SUM(cost_micro_usd)
             FROM usage WHERE day = ?",
        )
        .bind(day)
        .fetch_one(self.inner.store.pool())
        .await?;
        Ok(Counted {
            tokens: tokens((a, b, c, d)),
            cost_micro_usd: u64::try_from(cost.unwrap_or(0)).unwrap_or(0),
        })
    }

    /// The tokens of every ticket of `project` (by key) that ran a conversation.
    pub async fn tickets(&self, project: &str) -> Result<Vec<TicketUsage>, StoreError> {
        let rows: Vec<ConversationRow> = sqlx::query_as(
            "SELECT s.ticket_id, s.claude_session_id, SUM(u.tokens_in), SUM(u.tokens_out),
                    SUM(u.cache_write_tokens), SUM(u.cache_read_tokens)
             FROM sessions s
             JOIN projects p ON p.id = s.project_id
             LEFT JOIN usage u ON u.session_id = s.id
             WHERE p.key = ? AND s.ticket_id IS NOT NULL AND s.claude_session_id IS NOT NULL
             GROUP BY s.ticket_id, s.claude_session_id
             ORDER BY s.ticket_id",
        )
        .bind(project)
        .fetch_all(self.inner.store.pool())
        .await?;
        let unknown = self.inner.unknown.lock().expect("unknown usage").clone();
        let mut out: Vec<TicketUsage> = Vec::new();
        for (ticket, claude_id, a, b, c, d) in rows {
            let known = !unknown.contains(&claude_id);
            if out.last().is_none_or(|t| t.id != ticket) {
                out.push(TicketUsage {
                    id: ticket,
                    tokens: None,
                });
            }
            let entry = out.last_mut().expect("just pushed");
            if known {
                entry
                    .tokens
                    .get_or_insert_with(Tokens::default)
                    .add(&tokens((a, b, c, d)));
            }
        }
        Ok(out)
    }

    /// The tokens of each session of a ticket (by id) that ran a conversation, oldest first.
    pub async fn sessions_of(&self, ticket_id: &str) -> Result<Vec<SessionUsage>, StoreError> {
        let rows: Vec<ConversationRow> = sqlx::query_as(
            "SELECT s.id, s.claude_session_id, SUM(u.tokens_in), SUM(u.tokens_out),
                    SUM(u.cache_write_tokens), SUM(u.cache_read_tokens)
             FROM sessions s LEFT JOIN usage u ON u.session_id = s.id
             WHERE s.ticket_id = ? AND s.claude_session_id IS NOT NULL
             GROUP BY s.id, s.claude_session_id
             ORDER BY MIN(s.started_at), s.id",
        )
        .bind(ticket_id)
        .fetch_all(self.inner.store.pool())
        .await?;
        let unknown = self.inner.unknown.lock().expect("unknown usage").clone();
        Ok(rows
            .into_iter()
            .map(|(session, claude_id, a, b, c, d)| SessionUsage {
                session,
                tokens: (!unknown.contains(&claude_id)).then(|| tokens((a, b, c, d))),
            })
            .collect())
    }

    /// Every session that stored a Claude session ID, with where it ran.
    async fn sessions(&self) -> Result<Vec<StoredSession>, StoreError> {
        let rows: Vec<SessionRow> = sqlx::query_as(
            "SELECT s.id, s.claude_session_id, s.started_at, s.ended_at, t.worktree_path, p.repo_path
             FROM sessions s
             JOIN projects p ON p.id = s.project_id
             LEFT JOIN tickets t ON t.id = s.ticket_id
             WHERE s.claude_session_id IS NOT NULL AND s.claude_session_id <> ''",
        )
        .fetch_all(self.inner.store.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(id, claude_id, started_at, ended_at, worktree, repo)| StoredSession {
                    span: SessionSpan {
                        id,
                        started_at,
                        ended_at,
                    },
                    claude_id,
                    cwd: worktree.or(Some(repo)),
                },
            )
            .collect())
    }

    /// The Claude session IDs that already have counts stored.
    async fn counted_conversations(&self) -> Result<HashSet<String>, StoreError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT s.claude_session_id FROM usage u JOIN sessions s ON s.id = u.session_id
             WHERE s.claude_session_id IS NOT NULL",
        )
        .fetch_all(self.inner.store.pool())
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    async fn write_row(&self, session: &str, day: &str, c: &Counted) -> Result<(), StoreError> {
        let n = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        let t = &c.tokens;
        sqlx::query(
            "INSERT INTO usage (session_id, day, tokens_in, tokens_out, cache_read_tokens, cache_write_tokens,
                                cost_micro_usd)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (session_id, day) DO UPDATE SET tokens_in = excluded.tokens_in,
                 tokens_out = excluded.tokens_out, cache_read_tokens = excluded.cache_read_tokens,
                 cache_write_tokens = excluded.cache_write_tokens, cost_micro_usd = excluded.cost_micro_usd",
        )
        .bind(session)
        .bind(day)
        .bind(n(t.input))
        .bind(n(t.output))
        .bind(n(t.cache_read))
        .bind(n(t.cache_write))
        .bind(n(c.cost_micro_usd))
        .execute(self.inner.store.pool())
        .await?;
        Ok(())
    }
}

/// `(SUM(in), SUM(out), SUM(cache write), SUM(cache read), SUM(cost))` of a day.
type DayRow = (
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

/// `(id, claude_session_id, started_at, ended_at, worktree_path, repo_path)`.
type SessionRow = (String, String, i64, Option<i64>, Option<String>, String);

/// `(id, claude_session_id, SUM(in), SUM(out), SUM(cache write), SUM(cache read))`.
type ConversationRow = (
    String,
    String,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

fn tokens(
    (input, output, cache_write, cache_read): (Option<i64>, Option<i64>, Option<i64>, Option<i64>),
) -> Tokens {
    let n = |v: Option<i64>| u64::try_from(v.unwrap_or(0)).unwrap_or(0);
    Tokens {
        input: n(input),
        output: n(output),
        cache_write: n(cache_write),
        cache_read: n(cache_read),
    }
}

/// Finds (once) and reads a conversation's files. Only ever opens files for reading.
fn read_conversation(
    claude_dir: Option<&Path>,
    claude_id: &str,
    sessions: &[StoredSession],
    tracked: &mut Tracked,
) -> Result<(), Unknown> {
    let Some(claude_dir) = claude_dir else {
        return Err(Unknown::Missing);
    };
    if tracked.path.as_ref().is_none_or(|p| !p.is_file()) {
        tracked.path = sessions
            .iter()
            .rev()
            .filter_map(|s| s.cwd.as_deref())
            .find_map(|cwd| find_transcript(claude_dir, Path::new(cwd), claude_id));
    }
    let Some(path) = tracked.path.clone() else {
        return Err(Unknown::Missing);
    };
    tracked.conversation.read(&path).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(id: &str, started_at: i64) -> SessionSpan {
        SessionSpan {
            id: id.into(),
            started_at,
            ended_at: None,
        }
    }

    fn message(id: &str, at: Option<i64>, output: u64) -> MessageUsage {
        MessageUsage {
            id: id.into(),
            at,
            model: None,
            cache_write_1h: 0,
            tokens: Tokens {
                output,
                ..Default::default()
            },
        }
    }

    #[test]
    fn responses_go_to_the_session_running_when_they_were_written() {
        // Noon local time on two days, so the day split doesn't depend on the time zone.
        let day1 = 1_791_547_200_000 - 12 * 3_600_000;
        let noon = |d: i64| day1 + d * 86_400_000 + 12 * 3_600_000;
        let sessions = [span("first", noon(0)), span("resumed", noon(1))];
        let messages = [
            message("m0", Some(noon(0) - 1000), 1),
            message("m1", Some(noon(0) + 1000), 2),
            message("m2", Some(noon(1) + 1000), 4),
            message("m3", None, 8),
        ];
        let rows = attribute(&sessions, &messages, noon(1) + 5000);
        let get = |s: &str, d: i64| {
            rows.get(&(s.to_owned(), local_day(noon(d))))
                .map(|c| c.tokens.output)
        };
        assert_eq!(get("first", 0), Some(3));
        assert_eq!(get("resumed", 1), Some(12));
        assert_eq!(rows.len(), 2);
        assert!(attribute(&[], &messages, 0).is_empty());
    }

    #[test]
    fn days_are_local_dates() {
        let day = local_day(1_791_547_200_000);
        assert_eq!(day.len(), 10);
        assert!(day.starts_with("2026-10-"));
    }

    #[test]
    fn the_shown_number_leaves_cache_reads_out() {
        let t = Tokens {
            input: 1,
            output: 2,
            cache_write: 4,
            cache_read: 1000,
        };
        assert_eq!(t.shown(), 7);
    }
}
