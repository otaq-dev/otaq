//! The window's shared state: the projects (for the rail and ⌘P), the selected project, and its tickets (for
//! the board). It follows the service's events: a changed ticket is fetched and replaced in place, a deleted one
//! removed, and the project list refetched when projects change (UI-01, UI-02). After a reconnect everything
//! is fetched again, since events from the gap are lost.

use std::time::Instant;

use crewboard_core::board::settings::AgentLimits;
use crewboard_core::board::snapshot::ProjectInfo;
use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Status, TicketKey};
use crewboard_core::protocol::{Event, KeyParams, ProjectParams, Reply, Request};
use crewboard_core::scheduler::SchedulerStatus;
use gpui::{Context, Entity, EventEmitter};

use crate::service_link::{RequestError, ServiceEvent, ServiceLink};
use crate::view_models::agent_activity::{BATCH_QUIET, CreationBatch};
use crate::view_models::filter::Filter;
use crate::view_models::inbox::InboxModel;
use crate::view_models::queue::start_toast;
use crate::view_models::switcher::SwitcherModel;
use crate::view_models::usage;

/// Something views may want to react to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    /// The shown project changed.
    ProjectSelected(String),
    /// Agents created tickets: one success toast per burst (title, text; TKT-03).
    AgentCreated(String, String),
    /// A ticket that waited for its dependencies started (title, text; T-0083).
    TicketStarted(String, String),
    /// A Run stopped because its ticket finished: "Run stopped · SHOP-19 is done" (RUN-01).
    RunStopped(String),
    /// After a push the PR's content differs from the review (REV-06): a warning toast.
    PushedContentDiffers(String),
}

pub struct Workspace {
    pub link: Entity<ServiceLink>,
    pub switcher: SwitcherModel,
    /// The selected project's tickets and epics.
    pub tickets: Vec<Ticket>,
    /// The global settings (appearance and friends), once loaded.
    pub global_settings: Option<crewboard_core::board::settings::GlobalSettings>,
    /// The agent limits as the scheduler sees them (header "3 / 5 agents", SCH-03), once loaded.
    pub scheduler: Option<SchedulerStatus>,
    /// The board's search and filter (⌘K).
    pub filter: Filter,
    /// The selected card, shared by the board and the filter's `dep:` suggestion.
    pub selected: Option<crewboard_core::domain::TicketKey>,
    /// The project list has been fetched at least once.
    pub loaded: bool,
    /// The last failure to load, shown quietly on the board.
    pub error: Option<String>,
    loading: bool,
    /// Agent-created tickets waiting for their toast.
    created: CreationBatch,
    /// The Diff tab's viewed ticks, folded summaries and view choice, kept while the app runs (REV-02).
    pub review: crate::view_models::diff::ReviewMemory,
    /// The global needs-you inbox (UI-06), refetched on `inbox_changed`.
    pub inbox: InboxModel,
    /// A ⌘J run through the inbox, while one is going (ATT-07).
    pub inbox_run: Option<crate::view_models::inbox_run::InboxRun>,
    /// Leftovers in Cleanup (the rail's broom shows a dot when there are any, REC-04).
    pub cleanup_count: usize,
    /// Today's tokens and the selected project's tickets' tokens (USG-01), refetched on `usage_changed`.
    pub usage: Option<crewboard_core::usage::UsageSummary>,
    /// Tickets that appeared live (not from a board load), with when: a short highlight on the board.
    pub fresh: std::collections::HashMap<crewboard_core::domain::TicketKey, Instant>,
    /// Every Run that is up, across projects (RUN-04), refetched on `run_changed`.
    pub runs: Vec<crewboard_core::terminals::run::RunInfo>,
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

impl Workspace {
    pub fn new(link: Entity<ServiceLink>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&link, |ws, _, event: &ServiceEvent, cx| {
            ws.on_service(event, cx)
        })
        .detach();
        // The first connection has no resync event: fetch once connected.
        cx.observe(&link, |ws, link, cx| {
            let connected = matches!(
                link.read(cx).state,
                crate::view_models::connection::LinkState::Connected(_)
            );
            if connected && !ws.loaded && !ws.loading {
                ws.loading = true;
                ws.refresh_projects(cx);
                ws.load_global_settings(cx);
                ws.refresh_scheduler(cx);
                ws.refresh_inbox(cx);
                ws.refresh_cleanup(cx);
                ws.refresh_usage(cx);
                ws.refresh_runs(cx);
            }
        })
        .detach();
        Self {
            link,
            switcher: SwitcherModel::default(),
            tickets: Vec::new(),
            global_settings: None,
            scheduler: None,
            filter: Filter::default(),
            selected: None,
            loaded: false,
            error: None,
            loading: false,
            created: CreationBatch::default(),
            review: Default::default(),
            inbox: InboxModel::default(),
            inbox_run: None,
            cleanup_count: 0,
            usage: None,
            runs: Vec::new(),
            fresh: Default::default(),
        }
    }

    pub fn current(&self) -> Option<&ProjectInfo> {
        self.switcher.current()
    }

    /// Fetches the project list (and the board when the selection changes).
    pub fn refresh_projects(&self, cx: &mut Context<Self>) {
        let request = self.link.read(cx).send(Request::ProjectList);
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |ws, cx| match result {
                Ok(Reply::Projects(projects)) => {
                    let before = ws.switcher.selected.clone();
                    ws.switcher.set_projects(projects, Instant::now());
                    ws.loaded = true;
                    ws.loading = false;
                    if ws.switcher.selected != before || ws.tickets.is_empty() {
                        ws.load_board(cx);
                        if let Some(key) = ws.switcher.selected.clone() {
                            cx.emit(WorkspaceEvent::ProjectSelected(key));
                        }
                    }
                    cx.notify();
                }
                Ok(other) => ws.fail(format!("unexpected answer: {other:?}"), cx),
                Err(e) => ws.fail(e.message, cx),
            });
        })
        .detach();
    }

    fn load_global_settings(&self, cx: &mut Context<Self>) {
        let request = self.link.read(cx).send(Request::SettingsGet);
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::GlobalSettings(reply)) = request.await {
                let _ = this.update(cx, |ws, cx| {
                    ws.global_settings = Some(reply.settings);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Fetches the scheduler's view of the limits.
    pub fn refresh_scheduler(&self, cx: &mut Context<Self>) {
        let request = self.link.read(cx).send(Request::SchedulerStatus);
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::SchedulerStatus(status)) = request.await {
                let _ = this.update(cx, |ws, cx| {
                    if ws.scheduler != Some(status) {
                        ws.scheduler = Some(status);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    /// Fetches the inbox; items that left it fade out (ATT-09).
    pub fn refresh_inbox(&self, cx: &mut Context<Self>) {
        let request = self.link.read(cx).send(Request::InboxList);
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::Inbox(list)) = request.await {
                let _ = this.update(cx, |ws, cx| {
                    ws.inbox.update(*list, Instant::now());
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Fetches today's tokens and the selected project's tickets' tokens, and fills the cards' counts.
    pub fn refresh_usage(&self, cx: &mut Context<Self>) {
        let project = self.switcher.selected.clone();
        let request = self.link.read(cx).send(Request::UsageSummary(
            crewboard_core::protocol::UsageParams {
                project: project.clone(),
            },
        ));
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::Usage(summary)) = request.await {
                let _ = this.update(cx, |ws, cx| {
                    if ws.switcher.selected != project {
                        return;
                    }
                    usage::apply(&mut ws.tickets, &summary);
                    ws.usage = Some(*summary);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Counts the leftovers for the rail's dot (a read-only scan).
    pub fn refresh_cleanup(&self, cx: &mut Context<Self>) {
        let request = self.link.read(cx).send(Request::CleanupScan);
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::CleanupScan(items)) = request.await {
                let _ = this.update(cx, |ws, cx| ws.set_cleanup_count(items.len(), cx));
            }
        })
        .detach();
    }

    pub fn set_cleanup_count(&mut self, n: usize, cx: &mut Context<Self>) {
        if self.cleanup_count != n {
            self.cleanup_count = n;
            cx.notify();
        }
    }

    /// Drops inbox rows that have faded; called by the window's clock.
    pub fn expire_inbox(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        if self.inbox.expire(now) {
            cx.notify();
        }
        if self.inbox_run.as_ref().is_some_and(|r| r.finished(now)) {
            self.inbox_run = None;
            cx.notify();
        }
    }

    /// ⌘J: the next inbox item to open (the run starts at the first press), or `None` with "All clear".
    pub fn next_inbox_item(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<crewboard_core::board::needs_you::NeedsYouItem> {
        use crate::view_models::inbox_run::{InboxRun, NextStep};
        let now = Instant::now();
        let items = self.inbox.items().to_vec();
        let run = self.inbox_run.get_or_insert_with(|| InboxRun::new(now));
        cx.notify();
        match run.next(&items, now) {
            NextStep::Open(item) => Some(*item),
            NextStep::AllClear => None,
        }
    }

    /// The run bar for the panel showing `key`: only while a run is going and that ticket is the run's.
    pub fn run_bar(&mut self, key: &TicketKey) -> Option<crate::view_models::inbox_run::RunBar> {
        let items = self.inbox.items().to_vec();
        let run = self.inbox_run.as_mut()?;
        if run.current() != Some(key) {
            return None;
        }
        Some(run.bar(&items, Instant::now()))
    }

    /// "All clear" over the board, while it shows (ATT-07): items handled, minutes.
    pub fn all_clear(&mut self) -> Option<(usize, u64)> {
        let items = self.inbox.items().to_vec();
        match self.inbox_run.as_mut()?.bar(&items, Instant::now()) {
            crate::view_models::inbox_run::RunBar::AllClear { handled, minutes } => {
                Some((handled, minutes))
            }
            _ => None,
        }
    }

    /// Exit: the run ends; the open ticket stays open.
    pub fn exit_inbox_run(&mut self, cx: &mut Context<Self>) {
        if self.inbox_run.take().is_some() {
            cx.notify();
        }
    }

    /// The closed parent of a Parent closed ticket of the shown project, for the inbox's two ways forward.
    pub fn closed_parent(&self, key: &TicketKey) -> Option<TicketKey> {
        let ticket = self.tickets.iter().find(|t| &t.key == key)?;
        ticket
            .depends_on
            .iter()
            .find(|p| {
                self.tickets
                    .iter()
                    .any(|t| &t.key == *p && t.status == Status::ClosedNotMerged)
            })
            .cloned()
    }

    /// A user's click on an inbox row or dialog: sends the request; a refusal shows on the board quietly.
    pub fn act(&self, request: Request, cx: &mut Context<Self>) {
        let request = self.link.read(cx).send(request);
        cx.spawn(async move |this, cx| {
            if let Err(e) = request.await {
                let _ = this.update(cx, |ws, cx| ws.fail(e.message, cx));
            }
        })
        .detach();
    }

    /// Saves new agent limits (the header popover): they apply at once and never stop a running agent.
    pub fn set_limits(&mut self, limits: AgentLimits, cx: &mut Context<Self>) {
        let Some(mut settings) = self.global_settings.clone() else {
            return;
        };
        settings.agent_limits = limits;
        self.global_settings = Some(settings.clone());
        if let Some(status) = &mut self.scheduler {
            status.limits = limits;
        }
        cx.notify();
        let request = self.link.read(cx).send(Request::SettingsSet(settings));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |ws, cx| {
                match result {
                    Ok(Reply::GlobalSettings(reply)) => ws.global_settings = Some(reply.settings),
                    Ok(_) => {}
                    Err(e) => ws.fail(e.message, cx),
                }
                ws.refresh_scheduler(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn select(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.switcher.selected.as_deref() == Some(key) {
            return;
        }
        if self.switcher.select(key) {
            self.tickets.clear();
            self.filter.clear();
            self.selected = None;
            self.load_board(cx);
            cx.emit(WorkspaceEvent::ProjectSelected(key.to_owned()));
            cx.notify();
        }
    }

    /// A project was just added: show it at once (the service's event refreshes the list too).
    pub fn added(&mut self, project: ProjectInfo, cx: &mut Context<Self>) {
        let mut list = self.switcher.projects().to_vec();
        if !list.iter().any(|p| p.key == project.key) {
            list.push(project.clone());
        }
        self.switcher.set_projects(list, Instant::now());
        self.loaded = true;
        self.select(&project.key, cx);
        cx.notify();
    }

    /// Fetches the selected project's board.
    pub fn load_board(&self, cx: &mut Context<Self>) {
        let Some(project) = self.switcher.selected.clone() else {
            return;
        };
        let request = self
            .link
            .read(cx)
            .send(Request::BoardSnapshot(ProjectParams {
                project: project.clone(),
            }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |ws, cx| {
                if ws.switcher.selected.as_deref() != Some(project.as_str()) {
                    return;
                }
                match result {
                    Ok(Reply::Board(board)) => {
                        ws.tickets = board.tickets().cloned().collect();
                        ws.error = None;
                        // The counts of the shown project's tickets.
                        ws.refresh_usage(cx);
                        cx.notify();
                    }
                    Ok(other) => ws.fail(format!("unexpected answer: {other:?}"), cx),
                    Err(e) => ws.fail(e.message, cx),
                }
            });
        })
        .detach();
    }

    fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        log_failure(&message);
        self.loading = false;
        self.error = Some(message);
        cx.notify();
    }

    fn on_service(&mut self, event: &ServiceEvent, cx: &mut Context<Self>) {
        match event {
            ServiceEvent::Resync => {
                self.refresh_projects(cx);
                self.load_board(cx);
                self.refresh_scheduler(cx);
                self.refresh_inbox(cx);
                self.refresh_usage(cx);
                self.refresh_runs(cx);
            }
            ServiceEvent::Event(Event::InboxChanged { .. }) => self.refresh_inbox(cx),
            ServiceEvent::Event(Event::UsageChanged) => self.refresh_usage(cx),
            ServiceEvent::Event(Event::ProjectsChanged) => self.refresh_projects(cx),
            ServiceEvent::Event(Event::TicketChanged { project, key, .. }) => {
                // Counts on the rail and the header may change too.
                self.refresh_projects(cx);
                self.refresh_scheduler(cx);
                if self.switcher.selected.as_ref() == Some(project) {
                    self.fetch_ticket(key.to_string(), cx);
                }
            }
            // A session started or ended: the rail's and switcher's agent counts change.
            ServiceEvent::Event(Event::SessionState { .. }) => {
                self.refresh_projects(cx);
                self.refresh_scheduler(cx);
            }
            // The message-waiting count lives on the ticket's flags (SES-06).
            ServiceEvent::Event(Event::MessagesWaiting {
                project: Some(project),
                ticket: Some(key),
                ..
            }) => {
                if self.switcher.selected.as_ref() == Some(project) {
                    self.fetch_ticket(key.to_string(), cx);
                }
            }
            ServiceEvent::Event(Event::RunChanged { .. }) => self.refresh_runs(cx),
            ServiceEvent::Event(Event::RunStopped { message, .. }) => {
                cx.emit(WorkspaceEvent::RunStopped(message.clone()))
            }
            // A question or answer reached the other agent: its card's "asked · waiting" may clear (MSG-06).
            ServiceEvent::Event(Event::MessageDelivered { project, key, .. })
                if self.switcher.selected.as_ref() == Some(project) =>
            {
                self.fetch_ticket(key.to_string(), cx)
            }
            ServiceEvent::Event(Event::PushedContentDiffers { message, .. }) => {
                cx.emit(WorkspaceEvent::PushedContentDiffers(message.clone()))
            }
            ServiceEvent::Event(Event::TicketDeleted { project, id, .. }) => {
                if self.switcher.selected.as_ref() == Some(project) {
                    self.tickets.retain(|t| &t.id != id);
                    cx.notify();
                }
                self.refresh_projects(cx);
            }
            ServiceEvent::Event(_) => {}
        }
    }

    /// Every Run that is up, across projects (RUN-04): the header's list and the cards' chip menus.
    fn refresh_runs(&self, cx: &mut Context<Self>) {
        let request = self.link.read(cx).send(Request::RunList);
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::Runs(runs)) = request.await {
                let _ = this.update(cx, |ws, cx| {
                    ws.runs = runs;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Fetches one ticket and replaces it in place (no board reload).
    fn fetch_ticket(&self, key: String, cx: &mut Context<Self>) {
        let request = self
            .link
            .read(cx)
            .send(Request::TicketGet(KeyParams { key }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |ws, cx| match result {
                Ok(Reply::Ticket(ticket)) => ws.upsert(*ticket, cx),
                Ok(_) => {}
                Err(RequestError {
                    not_found: true, ..
                }) => {}
                Err(e) => ws.fail(e.message, cx),
            });
        })
        .detach();
    }

    /// Puts a ticket into the board (new or changed); the board re-sorts on its next draw.
    pub fn upsert(&mut self, mut ticket: Ticket, cx: &mut Context<Self>) {
        if ticket.key.project.as_str() != self.switcher.selected.as_deref().unwrap_or_default() {
            return;
        }
        if let Some(summary) = &self.usage {
            let id = ticket.id.clone();
            usage::apply_one(&mut ticket, usage::of_ticket(summary, &id));
        }
        match self.tickets.iter_mut().find(|t| t.id == ticket.id) {
            Some(slot) => {
                let started = start_toast(slot, &ticket);
                *slot = ticket;
                if let Some((title, text)) = started {
                    cx.emit(WorkspaceEvent::TicketStarted(title, text));
                }
            }
            None => {
                // Live, not from a board load: highlighted for a moment (the Project terminal's new tickets).
                self.fresh.insert(ticket.key.clone(), Instant::now());
                cx.spawn(async move |this, cx| {
                    let fade = std::time::Duration::from_millis(
                        crate::view_models::project_terminal::FRESH_FOR_MS,
                    );
                    cx.background_executor().timer(fade).await;
                    let _ = this.update(cx, |ws, cx| {
                        ws.fresh.retain(|_, at| at.elapsed() < fade);
                        cx.notify();
                    });
                })
                .detach();
                // A new ticket from an agent (live, not from a board load): toast it with its burst.
                if ticket.created_by.is_agent() {
                    self.created.add(&ticket, Instant::now());
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(BATCH_QUIET).await;
                        let _ = this.update(cx, |ws, cx| {
                            if let Some((title, text)) = ws.created.take_if_quiet(Instant::now()) {
                                cx.emit(WorkspaceEvent::AgentCreated(title, text));
                            }
                        });
                    })
                    .detach();
                }
                self.tickets.push(ticket)
            }
        }
        cx.notify();
    }
}

/// Load failures are shown on the board; they are also worth a line in the terminal while developing.
fn log_failure(message: &str) {
    eprintln!("workspace: {message}");
}
