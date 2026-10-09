//! The window's shared state: the projects (for the rail and ⌘P), the selected project, and its tickets (for
//! the board). It follows the service's events: a changed ticket is fetched and replaced in place, a deleted one
//! removed, and the project list refetched when projects change (UI-01, UI-02). After a reconnect everything
//! is fetched again, since events from the gap are lost.

use std::time::Instant;

use crewboard_core::board::snapshot::ProjectInfo;
use crewboard_core::board::tickets::Ticket;
use crewboard_core::protocol::{Event, KeyParams, ProjectParams, Reply, Request};
use gpui::{Context, Entity, EventEmitter};

use crate::service_link::{RequestError, ServiceEvent, ServiceLink};
use crate::view_models::agent_activity::{BATCH_QUIET, CreationBatch};
use crate::view_models::filter::Filter;
use crate::view_models::switcher::SwitcherModel;

/// Something views may want to react to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    /// The shown project changed.
    ProjectSelected(String),
    /// Agents created tickets: one success toast per burst (title, text; TKT-03).
    AgentCreated(String, String),
}

pub struct Workspace {
    pub link: Entity<ServiceLink>,
    pub switcher: SwitcherModel,
    /// The selected project's tickets and epics.
    pub tickets: Vec<Ticket>,
    /// The global settings (appearance and friends), once loaded.
    pub global_settings: Option<crewboard_core::board::settings::GlobalSettings>,
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
            }
        })
        .detach();
        Self {
            link,
            switcher: SwitcherModel::default(),
            tickets: Vec::new(),
            global_settings: None,
            filter: Filter::default(),
            selected: None,
            loaded: false,
            error: None,
            loading: false,
            created: CreationBatch::default(),
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
            }
            ServiceEvent::Event(Event::ProjectsChanged) => self.refresh_projects(cx),
            ServiceEvent::Event(Event::TicketChanged { project, key, .. }) => {
                // Counts on the rail may change too.
                self.refresh_projects(cx);
                if self.switcher.selected.as_ref() == Some(project) {
                    self.fetch_ticket(key.to_string(), cx);
                }
            }
            // A session started or ended: the rail's and switcher's agent counts change.
            ServiceEvent::Event(Event::SessionState { .. }) => self.refresh_projects(cx),
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
    pub fn upsert(&mut self, ticket: Ticket, cx: &mut Context<Self>) {
        if ticket.key.project.as_str() != self.switcher.selected.as_deref().unwrap_or_default() {
            return;
        }
        match self.tickets.iter_mut().find(|t| t.id == ticket.id) {
            Some(slot) => *slot = ticket,
            None => {
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
