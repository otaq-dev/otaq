//! The global "Needs you" inbox in the service (ATT-06…ATT-10; d0016, d0024, d0045, d0048).
//!
//! **Listing**: the items are [`crate::board::needs_you`]'s, across all projects, in inbox order, with counts in
//! total and per project (the header and the project switcher, ATT-10). They are derived from the tickets every
//! time, so an item answered elsewhere (`otaq attach`, the terminal) simply isn't there at the next
//! derivation (ATT-09). The service publishes `inbox_changed` whenever the derived set changes, and only then.
//!
//! **Answering** an Asks you or Idle item types the answer into the agent's session as the user's click (d0048): it
//! is never held, but unsent text in the input line asks *Keep it and cancel / Clear it and send* first. For an
//! **Interrupted** ticket that was waiting for an answer (d0045) the same click resumes the conversation and
//! delivers the answer in one step: the answer becomes the resumed conversation's first prompt, so it arrives once
//! the CLI is ready. The answer clears the needs-you state (ATT-02) and so the inbox item; an answered question is
//! recorded with its answer.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{Notify, broadcast, watch};

use super::notify::{self, ItemId, Notifier};
use crate::agent_tools::Tool;
use crate::agent_tools::connected::ConnectedTools;
use crate::board::needs_you::{NeedsYouGroup, NeedsYouItem};
use crate::board::questions::{Choice, format_answer};
use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError, Refusal};
use crate::domain::{Actor, Status, TicketKey};
use crate::protocol::Event;
use crate::review::Review;
use crate::service::EventBus;
use crate::session::deliveries::{Deliveries, Delivery};
use crate::session::{SessionKind, Sessions};
use crate::signals::Signals;
use crate::store::now_ms;

/// How long the inbox waits for a burst of changes to settle before it derives the items again.
const SETTLE: Duration = Duration::from_millis(50);

/// The inbox: every needs-you item with the counts (`inbox.list`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxList {
    pub items: Vec<NeedsYouItem>,
    /// Project agents waiting for the user (group 1, PAG-07), labelled with the project and the agent's name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_agents: Vec<crate::project_agents::AgentNeedsYou>,
    /// Tickets' and Project agents' items together.
    pub counts: InboxCounts,
}

/// The inbox count in total and per project (ATT-10). Every project is listed, with 0 when nothing waits.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxCounts {
    pub total: u32,
    pub projects: Vec<ProjectCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCount {
    /// The project's key.
    pub project: String,
    pub count: u32,
}

impl InboxCounts {
    /// The counts of `items`, with every project of `projects` (keys) listed.
    pub fn of(items: &[NeedsYouItem], projects: &[String]) -> Self {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Self {
            total: count(items.len()),
            projects: projects
                .iter()
                .map(|p| ProjectCount {
                    project: p.clone(),
                    count: count(items.iter().filter(|i| &i.project == p).count()),
                })
                .collect(),
        }
    }

    /// Adds the waiting Project agents to the counts (ATT-10: the header and the project switcher count them).
    pub fn add_agents(&mut self, agents: &[crate::project_agents::AgentNeedsYou]) {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        self.total = self.total.saturating_add(count(agents.len()));
        for p in &mut self.projects {
            p.count = p.count.saturating_add(count(
                agents.iter().filter(|a| a.project == p.project).count(),
            ));
        }
    }

    /// One project's count.
    pub fn of_project(&self, project: &str) -> u32 {
        self.projects
            .iter()
            .find(|p| p.project == project)
            .map_or(0, |p| p.count)
    }
}

/// What makes two derivations the same set: everything but the ages, which grow every second.
type Signature = (
    Vec<(String, NeedsYouGroup, Status, String, u32, TicketKey)>,
    Vec<(String, Status, String, String)>,
);

fn signature(list: &InboxList) -> Signature {
    let agents = list
        .project_agents
        .iter()
        .map(|a| (a.agent.clone(), a.status, a.reason.clone(), a.name.clone()))
        .collect();
    let items = list
        .items
        .iter()
        .map(|i| {
            (
                i.id.clone(),
                i.group,
                i.status,
                i.reason.clone(),
                i.blocked,
                i.key.clone(),
            )
        })
        .collect();
    (items, agents)
}

/// The inbox's actions. Cheap to clone.
#[derive(Clone)]
pub struct Inbox {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    sessions: Sessions,
    deliveries: Deliveries,
    signals: Signals,
    review: Review,
    events: EventBus,
    /// The set last published (or found at start).
    last: Mutex<Option<Signature>>,
    /// Something outside the event stream changed the set (the plan-limit switch).
    changed: Notify,
    /// OS notifications for new blocking items (ATT-11), and the items already seen.
    notifier: Arc<dyn Notifier>,
    seen: Mutex<HashSet<ItemId>>,
    /// What a click on a notification opens.
    opener: notify::Opener,
    /// The Project agents that wait for the user (set by the service).
    project_agents: Mutex<Option<crate::project_agents::ProjectAgents>>,
    /// The tools that aren't available while tickets wait on them or moved off them, since when (USG-10):
    /// notified once each.
    unavailable: Mutex<HashMap<Tool, i64>>,
    /// When each tool was last seen available; moves off a tool count from then (USG-10).
    available_at: Mutex<HashMap<Tool, i64>>,
    /// When the inbox started: the floor for a tool never seen available.
    started_at: i64,
    /// The check before every launch (which tools aren't available) and the tools' states (set by the service).
    tools: std::sync::OnceLock<(crate::named_agents::gate::LaunchGate, ConnectedTools)>,
}

impl Inbox {
    pub fn new(
        board: Board,
        sessions: Sessions,
        deliveries: Deliveries,
        signals: Signals,
        review: Review,
        events: EventBus,
        notifications: notify::Notifications,
    ) -> Self {
        let notify::Notifications { notifier, opener } = notifications;
        Self {
            inner: Arc::new(Inner {
                board,
                sessions,
                deliveries,
                signals,
                review,
                events,
                last: Mutex::new(None),
                changed: Notify::new(),
                notifier,
                seen: Mutex::new(HashSet::new()),
                opener,
                project_agents: Mutex::new(None),
                unavailable: Mutex::new(HashMap::new()),
                available_at: Mutex::new(HashMap::new()),
                started_at: now_ms(),
                tools: std::sync::OnceLock::new(),
            }),
        }
    }

    /// The Project agents whose waits are inbox items (d0068).
    pub fn set_project_agents(&self, agents: crate::project_agents::ProjectAgents) {
        *self.inner.project_agents.lock().expect("project agents") = Some(agents);
    }

    /// Every needs-you item with the counts (`inbox.list`).
    pub async fn list(&self) -> Result<InboxList, BoardError> {
        let board = &self.inner.board;
        let items = board.needs_you_items(now_ms()).await?;
        let projects: Vec<String> = board.projects().await?.into_iter().map(|p| p.key).collect();
        let agents = self
            .inner
            .project_agents
            .lock()
            .expect("project agents")
            .clone();
        let project_agents = match agents {
            Some(agents) => agents.needs_you().await?,
            None => Vec::new(),
        };
        let mut counts = InboxCounts::of(&items, &projects);
        counts.add_agents(&project_agents);
        Ok(InboxList {
            counts,
            items,
            project_agents,
        })
    }

    /// The counts in total and per project (`inbox.counts`).
    pub async fn counts(&self) -> Result<InboxCounts, BoardError> {
        Ok(self.list().await?.counts)
    }

    /// `tool`'s limit switch (ATT-09, per tool from M23; turned on and off with its limit line): while on, the
    /// agents its limit stopped raise no items. Other tools' agents keep theirs.
    pub fn set_suppress_limit_stopped(&self, tool: Tool, on: bool) {
        if self.inner.board.limit_stops().set_suppress(tool, on) {
            self.inner.changed.notify_one();
        }
    }

    /// Marks a ticket's agent as stopped by `tool`'s limit, or (`None`) not any more.
    pub fn mark_limit_stopped(&self, ticket_id: &str, by: Option<Tool>) {
        self.inner.board.limit_stops().mark(ticket_id, by);
        self.inner.changed.notify_one();
    }

    /// Gives the inbox the tools' availability, for a tool that isn't available (its one notification). Set before
    /// [`Self::run`].
    pub fn set_tools(
        &self,
        gate: crate::named_agents::gate::LaunchGate,
        connected: ConnectedTools,
    ) {
        let _ = self.inner.tools.set((gate, connected));
    }

    /// When `tool` was last seen available (or the inbox started): moves off it count from then.
    pub fn available_floor(&self, tool: Tool) -> i64 {
        self.inner
            .available_at
            .lock()
            .expect("available tools")
            .get(&tool)
            .copied()
            .unwrap_or(self.inner.started_at)
    }

    /// Since when tickets wait on `tool` (or moved off it) while it isn't available, as last derived; `None` when
    /// none do.
    pub fn unavailable_since(&self, tool: Tool) -> Option<i64> {
        self.inner
            .unavailable
            .lock()
            .expect("unavailable tools")
            .get(&tool)
            .copied()
    }

    /// Derives the items again, publishes `inbox_changed` if the set differs from the last one and posts OS
    /// notifications for new blocking items. Returns whether the set changed.
    pub async fn refresh(&self) -> Result<bool, BoardError> {
        let list = self.list().await?;
        self.notify(&list.items, &list.project_agents).await?;
        let now = signature(&list);
        let changed = {
            let mut last = self.inner.last.lock().expect("inbox");
            let changed = last.as_ref().is_some_and(|l| *l != now);
            *last = Some(now);
            changed
        };
        if changed {
            self.inner.events.publish(Event::InboxChanged {
                counts: list.counts,
            });
        }
        Ok(changed)
    }

    /// OS notifications for the blocking items not seen before (ATT-11); the setting is read every time.
    async fn notify(
        &self,
        items: &[NeedsYouItem],
        agents: &[crate::project_agents::AgentNeedsYou],
    ) -> Result<(), BoardError> {
        let board = &self.inner.board;
        let enabled = board.global_settings().await?.notifications;
        let mut texts = HashMap::new();
        for item in items.iter().filter(|i| notify::should_notify(i)) {
            let ticket = board.ticket_by_id(&item.id).await?;
            let text = if ticket.status == Status::ValidationFailed {
                ticket.flags.findings.first().cloned()
            } else {
                ticket
                    .question
                    .map(|q| q.text)
                    .or_else(|| ticket.signal.and_then(|s| s.text))
            };
            texts.insert(
                item.id.clone(),
                notify::ItemText {
                    text,
                    failed_times: ticket.validation_round,
                },
            );
        }
        let out = {
            let mut seen = self.inner.seen.lock().expect("notified items");
            let mut out = notify::plan(items, &texts, &mut seen, enabled);
            out.extend(notify::plan_agents(agents, &mut seen, enabled));
            out
        };
        // A tool that isn't available while tickets wait on it or moved off it: one notification when it first
        // happens (the first hold or the first move), none per ticket, and none again until it clears (USG-10).
        let unavailable = match self.inner.tools.get() {
            Some((gate, _)) => gate.facts().await?.unavailable,
            None => HashMap::new(),
        };
        {
            let now = now_ms();
            let mut seen = self.inner.available_at.lock().expect("available tools");
            for &tool in Tool::ALL {
                if !unavailable.contains_key(&tool) {
                    seen.insert(tool, now);
                }
            }
        }
        let waiting =
            crate::named_agents::moves::unavailable_situations(board, &unavailable, |tool| {
                self.available_floor(tool)
            })
            .await?;
        let fresh: Vec<&crate::named_agents::moves::UnavailableSituation> = {
            let mut known = self.inner.unavailable.lock().expect("unavailable tools");
            known.retain(|tool, _| waiting.iter().any(|w| w.tool == *tool));
            waiting
                .iter()
                .filter(|w| match known.entry(w.tool) {
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(w.since);
                        true
                    }
                    std::collections::hash_map::Entry::Occupied(_) => false,
                })
                .collect()
        };
        let mut out = out;
        if enabled {
            out.extend(
                fresh
                    .into_iter()
                    .map(|w| notify::tool_unavailable(w.tool, w.tickets, &w.moved)),
            );
        }
        for notification in &out {
            tracing::info!(title = %notification.title, "notification");
            let (opener, key) = (self.inner.opener.clone(), notification.open.clone());
            self.inner
                .notifier
                .post(notification, Box::new(move || opener.open(key)));
        }
        Ok(())
    }

    /// Watches the changes that can change the set (tickets, projects, terminal focus) until the service stops.
    pub async fn run(self, mut stop: watch::Receiver<bool>) {
        let mut listener = self.inner.events.listen();
        let mut tools = self.inner.tools.get().map(|(_, c)| c.subscribe());
        if let Err(e) = self.refresh().await {
            tracing::warn!(error = %e, "can't derive the inbox");
        }
        loop {
            let relevant = tokio::select! {
                _ = stop.wait_for(|s| *s) => return,
                () = self.inner.changed.notified() => true,
                // A tool's state changed: a tool that isn't available any more, or again.
                changed = async {
                    match tools.as_mut() {
                        Some(rx) => rx.changed().await,
                        None => std::future::pending().await,
                    }
                } => {
                    if changed.is_err() {
                        tools = None;
                    }
                    true
                }
                event = listener.receiver.recv() => match event {
                    Ok(event) => touches_inbox(&event),
                    Err(broadcast::error::RecvError::Lagged(_)) => true,
                    Err(broadcast::error::RecvError::Closed) => return,
                },
            };
            if !relevant {
                continue;
            }
            // Let the burst settle, then derive once.
            tokio::time::sleep(SETTLE).await;
            while let Ok(event) = listener.receiver.try_recv() {
                let _ = event;
            }
            if let Err(e) = self.refresh().await {
                tracing::warn!(error = %e, "can't derive the inbox");
            }
        }
    }

    /// **Answer** an Asks you or Idle item (or an Interrupted one that was waiting for an answer). `clear` is the
    /// user's "Clear it and send" after [`Delivery::NeedsConfirmation`].
    pub async fn answer(
        &self,
        key: &str,
        choice: &Choice,
        clear: bool,
        actor: &Actor,
    ) -> Result<Delivery, BoardError> {
        let inner = &self.inner;
        let ticket = inner.board.ticket(key).await?;
        let waiting = answerable(&ticket).ok_or_else(|| {
            BoardError::refused(
                Refusal::MoveRefused,
                format!("{} isn't waiting for an answer", ticket.key),
            )
        })?;
        let question = inner.board.open_question(&ticket.id).await?;
        let text = answer_text(question.as_ref(), choice)?;
        let session = inner.sessions.running_of(&ticket.id, SessionKind::Agent);
        let delivery = match (waiting, session) {
            (Waiting::Live, Some(session)) => {
                let typed = if clear {
                    self.inner
                        .deliveries
                        .clear_and_send(&session.id, Some(&text))
                        .await
                } else {
                    self.inner
                        .deliveries
                        .deliver_from_click(&session.id, &text)
                        .await
                };
                let delivery =
                    typed.map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
                if matches!(delivery, Delivery::Sent { .. }) {
                    inner.signals.answered(&ticket.id).await?;
                }
                delivery
            }
            (Waiting::Live, None) => {
                return Err(BoardError::refused(
                    Refusal::SessionEnded,
                    format!("{}'s session has ended", ticket.key),
                ));
            }
            (Waiting::Interrupted, _) => {
                self.inner
                    .review
                    .resume_with_answer(key, &text, actor)
                    .await?;
                Delivery::Sent { count: 1 }
            }
        };
        if matches!(delivery, Delivery::Sent { .. })
            && let Some(question) = question
        {
            inner.board.record_answer(&question.id, &text).await?;
            // A question that went through its epic's lead is logged on the epic too (LEAD-06): its Messages refresh.
            if let Some(epic) = &ticket.epic
                && let Ok(epic) = inner.board.ticket(&epic.to_string()).await
            {
                inner.events.publish(Event::TicketChanged {
                    project: epic.key.project.to_string(),
                    id: epic.id,
                    key: epic.key,
                });
            }
        }
        tracing::info!(ticket = %ticket.key, ?delivery, "inbox answer");
        Ok(delivery)
    }
}

/// Whether an event can change the needs-you set.
fn touches_inbox(event: &Event) -> bool {
    matches!(
        event,
        Event::TicketChanged { .. }
            | Event::CardStatusChanged { .. }
            | Event::TicketDeleted { .. }
            | Event::ProjectsChanged
            | Event::SessionFocus { .. }
            | Event::ProjectAgentChanged { .. }
    )
}

/// How an item waits for an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Waiting {
    /// Asks you or Idle, with its session.
    Live,
    /// Interrupted while it asked or was idle (d0045).
    Interrupted,
}

/// Whether the ticket takes an answer: Asks you and Idle (a permission prompt is answered in the terminal, and
/// simply asks again after a resume).
fn answerable(ticket: &Ticket) -> Option<Waiting> {
    let takes_answer = |s: Status| matches!(s, Status::AsksYou | Status::Idle);
    if takes_answer(ticket.status) {
        Some(Waiting::Live)
    } else if ticket.status == Status::Interrupted
        && ticket.interrupted_from.is_some_and(takes_answer)
    {
        Some(Waiting::Interrupted)
    } else {
        None
    }
}

/// The text typed for an answer: a question with options gets [`format_answer`]'s wording ("Answer to your
/// question: Postmark (option 1)."); otherwise the free text as written.
pub fn answer_text(
    question: Option<&crate::board::questions::HumanQuestion>,
    choice: &Choice,
) -> Result<String, BoardError> {
    let text = match (question.filter(|q| !q.options.is_empty()), choice) {
        (Some(q), Choice::Option(n)) if *n >= 1 && (*n as usize) <= q.options.len() => {
            format_answer(q, choice)
        }
        (_, Choice::Option(n)) => {
            return Err(BoardError::refused(
                Refusal::BadQuestion,
                format!("There is no option {n} to choose"),
            ));
        }
        (Some(q), Choice::Other(_)) => format_answer(q, choice),
        (None, Choice::Other(text)) => text.trim().to_owned(),
    };
    if text.trim().is_empty() || matches!(choice, Choice::Other(t) if t.trim().is_empty()) {
        return Err(BoardError::refused(
            Refusal::EmptyField,
            "Write the answer first",
        ));
    }
    Ok(text)
}
