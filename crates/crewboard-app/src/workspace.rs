//! The window's shared state: the projects (for the rail and ⌘P), the selected project, and its tickets (for
//! the board). It follows the service's events: a changed ticket is fetched and replaced in place, a deleted one
//! removed, and the project list refetched when projects change (UI-01, UI-02). After a reconnect everything
//! is fetched again, since events from the gap are lost.

use std::time::Instant;

use crewboard_core::board::snapshot::{ColumnSnapshot, ProjectInfo, columns};
use crewboard_core::board::tickets::Ticket;
use crewboard_core::protocol::{Event, KeyParams, ProjectParams, Reply, Request};
use gpui::{Context, Entity, EventEmitter};

use crate::service_link::{RequestError, ServiceEvent, ServiceLink};
use crate::view_models::switcher::SwitcherModel;

/// Something views may want to react to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    /// The shown project changed.
    ProjectSelected(String),
}

pub struct Workspace {
    pub link: Entity<ServiceLink>,
    pub switcher: SwitcherModel,
    /// The selected project's tickets and epics.
    pub tickets: Vec<Ticket>,
    /// The board's columns, sorted (rebuilt from `tickets` after every change, SDD §7.5).
    pub columns: Vec<ColumnSnapshot>,
    /// The project list has been fetched at least once.
    pub loaded: bool,
    /// The last failure to load, shown quietly on the board.
    pub error: Option<String>,
    loading: bool,
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
            }
        })
        .detach();
        Self {
            link,
            switcher: SwitcherModel::default(),
            tickets: Vec::new(),
            columns: columns(&[]),
            loaded: false,
            error: None,
            loading: false,
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

    pub fn select(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.switcher.selected.as_deref() == Some(key) {
            return;
        }
        if self.switcher.select(key) {
            self.tickets.clear();
            self.columns = columns(&[]);
            self.load_board(cx);
            cx.emit(WorkspaceEvent::ProjectSelected(key.to_owned()));
            cx.notify();
        }
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
                        ws.columns = board.columns;
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
            ServiceEvent::Event(Event::TicketDeleted { project, id, .. }) => {
                if self.switcher.selected.as_ref() == Some(project) {
                    self.tickets.retain(|t| &t.id != id);
                    self.columns = columns(&self.tickets);
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

    /// Puts a ticket into the board (new or changed) and re-sorts the columns.
    pub fn upsert(&mut self, ticket: Ticket, cx: &mut Context<Self>) {
        if ticket.key.project.as_str() != self.switcher.selected.as_deref().unwrap_or_default() {
            return;
        }
        match self.tickets.iter_mut().find(|t| t.id == ticket.id) {
            Some(slot) => *slot = ticket,
            None => self.tickets.push(ticket),
        }
        self.columns = columns(&self.tickets);
        cx.notify();
    }
}

/// Load failures are shown on the board; they are also worth a line in the terminal while developing.
fn log_failure(message: &str) {
    eprintln!("workspace: {message}");
}
