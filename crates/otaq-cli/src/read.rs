//! The read commands (SDD §11.2, d0032, d0039): `show` (alias `view`), `list`, `ready`, `blocked`,
//! `project list | show`. Agents may read anything (CLI-02).
//!
//! # `--json` shapes (stable; fields may be added, never renamed or removed)
//!
//! - `show`: `{"ticket": Ticket, "decisions": [Decision], "children": [Ticket]?, "history": [HistoryEvent]?,
//!   "messages": [Message]?}`: `decisions` are the ticket's recorded decisions (DEC-02, `otaq_core::decisions`),
//!   `children` with `--children` (epics), `history` and `messages` (questions between linked tickets) with
//!   `--long`.
//! - `show --terminal` (SHL-10, d0087): `{"key", "session", "text"}`: the ticket's terminal as the user sees it,
//!   as plain text, and the session it came from.
//! - `list`: `[Ticket]` in key order.
//! - `ready` / `blocked`: `[{"key", "title", "status", "ready", "waiting_for": [{"key", "status", "kind"?,
//!   "pr_number"?}]?, "parent_closed"?, "reason"?}]`; `reason` is the card's waiting text (T-0077).
//! - `project list`: `[ProjectInfo]`; `project show`: `ProjectInfo`.
//!
//! `Ticket`, `HistoryEvent` and `ProjectInfo` are the protocol's types (`otaq_core::board`).

use otaq_core::board::readiness::Readiness;
use otaq_core::board::snapshot::ProjectInfo;
use otaq_core::board::tickets::{Base, HistoryEvent, Ticket};
use otaq_core::client::Client;
use otaq_core::data_dir::DataDir;
use otaq_core::domain::{Column, Priority, Status, TicketKey};
use otaq_core::protocol::{
    ErrorBody, ErrorKind, KeyParams, ProjectParams, Reply, Request, TerminalParams,
};
use otaq_core::store::now_ms;
use serde_json::json;

use crate::cli::{GlobalArgs, ProjectCommand};
use crate::output::Output;
use crate::service::{self, client_error, internal};
use crate::target;

pub struct ShowArgs {
    pub key: Option<String>,
    pub long: bool,
    pub children: bool,
}

pub struct ListArgs {
    pub status: Option<String>,
    pub tree: bool,
    pub parent: Option<String>,
    pub priority: Option<Priority>,
}

/// Runs one read command on a fresh connection.
async fn with_client<F, T>(global: &GlobalArgs, run: F) -> Result<T, ErrorBody>
where
    F: AsyncFnOnce(&Client, &DataDir) -> Result<T, ErrorBody>,
{
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    let client = service::connect(&data, global).await?;
    run(&client, &data).await
}

pub fn show(args: ShowArgs, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    service::runtime()?.block_on(with_client(global, async |client, data| {
        let key = target::resolve_key(client, data, args.key, global).await?;
        let ticket = get_ticket(client, &key.to_string()).await?;
        let tickets = list_tickets(client, ticket.key.project.as_str()).await?;
        let children: Vec<Ticket> = tickets
            .iter()
            .filter(|t| t.epic.as_ref() == Some(&ticket.key))
            .cloned()
            .collect();
        let history = if args.long {
            match request(
                client,
                Request::TicketHistory(KeyParams {
                    key: ticket.key.to_string(),
                }),
            )
            .await?
            {
                Reply::History(h) => Some(h),
                other => return Err(unexpected(&other)),
            }
        } else {
            None
        };
        let messages = if args.long {
            match request(
                client,
                Request::TicketMessages(KeyParams {
                    key: ticket.key.to_string(),
                }),
            )
            .await?
            {
                Reply::Messages(m) => Some(m),
                other => return Err(unexpected(&other)),
            }
        } else {
            None
        };
        let decisions = match request(
            client,
            Request::DecisionList(KeyParams {
                key: ticket.key.to_string(),
            }),
        )
        .await?
        {
            Reply::Decisions(d) => d,
            other => return Err(unexpected(&other)),
        };
        let mut human = show_text(&ticket, &tickets);
        if !decisions.is_empty() {
            // Only a lead marks: its mark is named after the ticket's epic ("Marked by Wishlist lead").
            let lead = ticket
                .epic
                .as_ref()
                .and_then(|e| tickets.iter().find(|t| &t.key == e))
                .map(|e| otaq_core::project_agents::lead::lead_name(&e.title));
            human.push_str(&decisions_text(&decisions, lead.as_deref()));
        }
        if args.children {
            human.push_str(&children_text(&ticket, &children));
        }
        if let Some(messages) = messages.as_ref().filter(|m| !m.is_empty()) {
            human.push_str(&messages_text(messages));
        }
        if let Some(history) = &history {
            human.push_str(&history_text(history, now_ms()));
        }
        let mut value = json!({ "ticket": ticket, "decisions": decisions });
        if args.children {
            value["children"] = json!(children);
        }
        if let Some(history) = history {
            value["history"] = json!(history);
        }
        if let Some(messages) = messages {
            value["messages"] = json!(messages);
        }
        out.print(human.trim_end(), &value, Some(&ticket.key.to_string()));
        Ok(())
    }))
}

/// `otaq show <KEY> --terminal` (SHL-10, CLI-09): the ticket's agent terminal (or with `--validator` its running
/// validator's) as the user sees it, as plain text. Read-only: the service decides who may (CLI-03).
pub fn terminal(
    key: Option<String>,
    validator: bool,
    global: &GlobalArgs,
    out: &Output,
) -> Result<(), ErrorBody> {
    service::runtime()?.block_on(with_client(global, async |client, data| {
        let key = target::resolve_key(client, data, key, global).await?;
        let reply = request(
            client,
            Request::TicketTerminal(TerminalParams {
                key: key.to_string(),
                validator,
            }),
        )
        .await?;
        let Reply::TicketTerminal(terminal) = reply else {
            return Err(unexpected(&reply));
        };
        let value =
            json!({ "key": terminal.key, "session": terminal.session, "text": terminal.text });
        out.print(&terminal.text, &value, Some(&terminal.key.to_string()));
        Ok(())
    }))
}

/// The ticket's recorded decisions (DEC-02): ID and question, then Built, the other options, Why, the lead's mark
/// and the outcome.
fn decisions_text(decisions: &[otaq_core::decisions::Decision], lead: Option<&str>) -> String {
    let mut out = String::from("\n\nDecisions\n");
    for d in decisions {
        out.push_str(&format!(
            "  {}",
            otaq_core::decisions::render(d, "    ", lead)
        ));
    }
    out
}

/// The questions between this ticket and its linked tickets (MSG-01), oldest first, each with its answer.
fn messages_text(messages: &[otaq_core::messages::Message]) -> String {
    let mut out = String::from("\n\nMessages\n");
    for m in messages {
        out.push_str(&format!(
            "  {} {} \u{2192} {}: {}\n",
            m.id,
            m.asker(),
            m.to,
            m.question
        ));
        match &m.answer {
            Some(answer) => out.push_str(&format!("    answered: {answer}\n")),
            None => out.push_str("    waiting for the answer\n"),
        }
    }
    out
}

pub fn list(args: ListArgs, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    service::runtime()?.block_on(with_client(global, async |client, data| {
        let project = match &args.parent {
            Some(parent) => {
                let key = target::resolve_key(client, data, Some(parent.clone()), global).await?;
                key.project
            }
            None => target::require_project(client, data, global).await?,
        };
        let tickets = list_tickets(client, project.as_str()).await?;
        let parent = match &args.parent {
            Some(parent) => {
                let key = target::resolve_key(client, data, Some(parent.clone()), global).await?;
                Some(get_ticket(client, &key.to_string()).await?.key)
            }
            None => None,
        };
        let status = args
            .status
            .as_deref()
            .map(StatusFilter::parse)
            .transpose()?;
        let shown: Vec<Ticket> = tickets
            .into_iter()
            .filter(|t| parent.as_ref().is_none_or(|p| t.epic.as_ref() == Some(p)))
            .filter(|t| status.as_ref().is_none_or(|s| s.matches(t)))
            .filter(|t| args.priority.is_none_or(|p| t.priority == p))
            .collect();
        let human = if args.tree {
            tree_text(&shown)
        } else {
            shown.iter().map(line).collect::<Vec<_>>().join("\n")
        };
        let keys: Vec<String> = shown.iter().map(|t| t.key.to_string()).collect();
        out.print(
            &human,
            &shown,
            Some(keys.join("\n").as_str()).filter(|k| !k.is_empty()),
        );
        Ok(())
    }))
}

/// `ready` (`blocked == false`) or `blocked`.
pub fn readiness(blocked: bool, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    service::runtime()?.block_on(with_client(global, async |client, data| {
        let project = target::require_project(client, data, global).await?;
        let states = match request(
            client,
            Request::DepReadiness(ProjectParams {
                project: project.to_string(),
            }),
        )
        .await?
        {
            Reply::Readiness(states) => states,
            other => return Err(unexpected(&other)),
        };
        let shown: Vec<Readiness> = states.into_iter().filter(|s| s.ready != blocked).collect();
        let human = shown
            .iter()
            .map(readiness_line)
            .collect::<Vec<_>>()
            .join("\n");
        let keys: Vec<String> = shown.iter().map(|s| s.key.to_string()).collect();
        out.print(
            &human,
            &shown,
            Some(keys.join("\n").as_str()).filter(|k| !k.is_empty()),
        );
        Ok(())
    }))
}

pub fn project(
    command: ProjectCommand,
    global: &GlobalArgs,
    out: &Output,
) -> Result<(), ErrorBody> {
    service::runtime()?.block_on(with_client(global, async |client, data| {
        let projects = match request(client, Request::ProjectList).await? {
            Reply::Projects(p) => p,
            other => return Err(unexpected(&other)),
        };
        match command {
            ProjectCommand::List => {
                let human = projects
                    .iter()
                    .map(project_line)
                    .collect::<Vec<_>>()
                    .join("\n");
                let keys: Vec<&str> = projects.iter().map(|p| p.key.as_str()).collect();
                out.print(
                    &human,
                    &projects,
                    Some(keys.join("\n").as_str()).filter(|k| !k.is_empty()),
                );
            }
            ProjectCommand::Show { key } => {
                let key = match key {
                    Some(key) => key.trim().to_ascii_uppercase(),
                    None => target::require_project(client, data, global)
                        .await?
                        .to_string(),
                };
                let project = projects.into_iter().find(|p| p.key == key).ok_or_else(|| {
                    ErrorBody::new(ErrorKind::NotFound, format!("unknown project {key}"))
                })?;
                out.print(&project_text(&project), &project, Some(&project.key));
            }
        }
        Ok(())
    }))
}

async fn request(client: &Client, request: Request) -> Result<Reply, ErrorBody> {
    client.request(request).await.map_err(client_error)
}

pub async fn get_ticket(client: &Client, key: &str) -> Result<Ticket, ErrorBody> {
    match request(
        client,
        Request::TicketGet(KeyParams {
            key: key.to_owned(),
        }),
    )
    .await?
    {
        Reply::Ticket(t) => Ok(*t),
        other => Err(unexpected(&other)),
    }
}

pub async fn list_tickets(client: &Client, project: &str) -> Result<Vec<Ticket>, ErrorBody> {
    match request(
        client,
        Request::TicketList(ProjectParams {
            project: project.to_owned(),
        }),
    )
    .await?
    {
        Reply::Tickets(t) => Ok(t),
        other => Err(unexpected(&other)),
    }
}

pub fn unexpected(reply: &Reply) -> ErrorBody {
    internal(format!(
        "unexpected answer from the background service: {reply:?}"
    ))
}

/// `--status`: a status (`working`, `asks_you`, "Ready for Review") or a column (`dev`, `done`).
enum StatusFilter {
    Status(Status),
    Column(Column),
}

impl StatusFilter {
    fn parse(text: &str) -> Result<Self, ErrorBody> {
        let normal = text.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        if let Ok(status) = normal.parse::<Status>() {
            return Ok(Self::Status(status));
        }
        if let Ok(column) = normal.parse::<Column>() {
            return Ok(Self::Column(column));
        }
        Err(ErrorBody::new(
            ErrorKind::Usage,
            format!(
                "unknown status {text:?}; use a status such as working or a column such as dev"
            ),
        ))
    }

    fn matches(&self, ticket: &Ticket) -> bool {
        match self {
            Self::Status(s) => ticket.status == *s,
            Self::Column(c) => ticket.column == *c,
        }
    }
}

/// `SHOP-18  Working               Split user full name`
pub fn line(ticket: &Ticket) -> String {
    let epic = if ticket.is_epic() { "epic · " } else { "" };
    format!(
        "{:<10} P{} {:<22} {epic}{}",
        ticket.key.to_string(),
        ticket.priority.number(),
        ticket.status.label(),
        ticket.title
    )
}

/// Epics with their children indented under them; tickets of no epic as they are.
pub fn tree_text(tickets: &[Ticket]) -> String {
    let mut lines = Vec::new();
    for ticket in tickets.iter().filter(|t| t.epic.is_none()) {
        lines.push(line(ticket));
        for child in tickets
            .iter()
            .filter(|t| t.epic.as_ref() == Some(&ticket.key))
        {
            lines.push(format!("  {}", line(child)));
        }
    }
    // Children whose epic isn't in the list (filtered out) still show.
    for orphan in tickets.iter().filter(|t| {
        t.epic
            .as_ref()
            .is_some_and(|e| !tickets.iter().any(|p| &p.key == e))
    }) {
        lines.push(format!("  {}", line(orphan)));
    }
    lines.join("\n")
}

fn status_of(key: &TicketKey, tickets: &[Ticket]) -> String {
    tickets
        .iter()
        .find(|t| &t.key == key)
        .map_or_else(|| "?".to_owned(), |t| t.status.label().to_owned())
}

/// The full human view of one ticket.
pub fn show_text(ticket: &Ticket, tickets: &[Ticket]) -> String {
    let mut text = format!("{} · {}\n", ticket.key, ticket.title);
    let kind = if ticket.is_epic() { "Epic · " } else { "" };
    text.push_str(&format!(
        "Status:      {kind}{} ({})\n",
        ticket.status.label(),
        ticket.column.label()
    ));
    if let Some(signal) = ticket.signal.as_ref().and_then(|s| s.text.as_deref()) {
        text.push_str(&format!("Waiting on:  {signal}\n"));
    }
    let inherited = if ticket.priority_inherited {
        " (from the epic)"
    } else {
        ""
    };
    text.push_str(&format!("Priority:    {}{inherited}\n", ticket.priority));
    match &ticket.base {
        Base::Branch(branch) => text.push_str(&format!("Base:        {branch}\n")),
        Base::Epic(epic) => {
            text.push_str(&format!("Base:        from the epic's branch ({epic})\n"))
        }
    }
    if let Some(epic) = &ticket.epic {
        text.push_str(&format!("Epic:        {epic}\n"));
    }
    if !ticket.depends_on.is_empty() {
        let parents: Vec<String> = ticket
            .depends_on
            .iter()
            .map(|p| format!("{p} ({})", status_of(p, tickets)))
            .collect();
        text.push_str(&format!("Depends on:  {}\n", parents.join(", ")));
    }
    if let Some(id) = &ticket.external_id {
        text.push_str(&format!("External ID: {id}\n"));
    }
    if let Some(branch) = ticket.work.as_ref().and_then(|w| w.branch.as_deref()) {
        text.push_str(&format!("Branch:      {branch}\n"));
    }
    if !ticket.aliases.is_empty() {
        let aliases: Vec<String> = ticket.aliases.iter().map(ToString::to_string).collect();
        text.push_str(&format!("Also known:  {}\n", aliases.join(", ")));
    }
    let origin = if ticket.created_by.is_agent() {
        " (created by agent)"
    } else {
        ""
    };
    text.push_str(&format!("Created by:  {}{origin}\n", ticket.created_by));
    if !ticket.body.trim().is_empty() {
        text.push_str(&format!("\nWhat and why\n{}\n", ticket.body.trim_end()));
    }
    text
}

fn children_text(epic: &Ticket, children: &[Ticket]) -> String {
    if children.is_empty() {
        return format!("\n{} has no tickets yet\n", epic.key);
    }
    let mut text = String::from("\nChildren\n");
    for child in children {
        text.push_str(&line(child));
        text.push('\n');
    }
    text
}

fn history_text(history: &[HistoryEvent], now: i64) -> String {
    let mut text = String::from("\nHistory (newest first)\n");
    for event in history {
        text.push_str(&format!(
            "{:<20} {:<16} {}\n",
            event.kind.as_str(),
            event.actor.to_string(),
            ago(now - event.at)
        ));
    }
    text
}

/// "just now", "4 min ago", "3 h ago", "2 d ago".
pub fn ago(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    match s {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{} min ago", s / 60),
        3600..86_400 => format!("{} h ago", s / 3600),
        _ => format!("{} d ago", s / 86_400),
    }
}

fn readiness_line(state: &Readiness) -> String {
    let base = format!(
        "{:<10} {:<22} {}",
        state.key.to_string(),
        state.status.label(),
        state.title
    );
    match &state.reason {
        Some(reason) => format!("{base} · {reason}"),
        None => base,
    }
}

fn project_line(project: &ProjectInfo) -> String {
    format!(
        "{:<6} {:<20} {:<12} {}",
        project.key, project.name, project.base_branch, project.repo_path
    )
}

fn project_text(project: &ProjectInfo) -> String {
    format!(
        "{} · {}\nRepository:  {}\nBase branch: {}\nAgents:      {} working, {} need you",
        project.key,
        project.name,
        project.repo_path,
        project.base_branch,
        project.agents_working,
        project.needs_you
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ages() {
        assert_eq!(ago(10_000), "just now");
        assert_eq!(ago(4 * 60_000), "4 min ago");
        assert_eq!(ago(3 * 3_600_000), "3 h ago");
        assert_eq!(ago(50 * 3_600_000), "2 d ago");
    }

    #[test]
    fn status_filters() {
        assert!(matches!(
            StatusFilter::parse("working"),
            Ok(StatusFilter::Status(Status::Working))
        ));
        assert!(matches!(
            StatusFilter::parse("Ready for Review"),
            Ok(StatusFilter::Status(Status::ReadyForReview))
        ));
        assert!(matches!(
            StatusFilter::parse("dev"),
            Ok(StatusFilter::Column(Column::Dev))
        ));
        assert!(matches!(
            StatusFilter::parse("ready_for_dev"),
            Ok(StatusFilter::Column(Column::ReadyForDev))
        ));
        assert!(StatusFilter::parse("paused").is_err());
    }
}
