//! `otaq create` (alias `new`) and `otaq update` (SDD §11.2; PLG-05, CLI-02, CLI-04; d0019, d0032,
//! d0039).
//!
//! New tickets land in **Backlog**. Long text goes through **stdin** (`--stdin`, PLG-05); `-d` is for a short
//! line. The service records the actor, so tickets an agent creates show "created by agent" and its edits are
//! kept as before/after (T-0023); an agent may only update its own ticket (T-0065).
//!
//! # `--json` shapes
//!
//! - `create "Title"`: the new `Ticket`; `create -f`: `[Ticket]` in file order.
//! - `update`: the `Ticket` as it is afterwards.

use std::io::Read;
use std::path::PathBuf;

use otaq_core::board::batch::{BatchTicket, parse_markdown};
use otaq_core::board::tickets::{Ticket, TicketUpdate};
use otaq_core::client::Client;
use otaq_core::data_dir::DataDir;
use otaq_core::domain::{Kind, Priority};
use otaq_core::protocol::{BatchParams, ErrorBody, ErrorKind, Reply, Request, TicketUpdateParams};

use crate::cli::GlobalArgs;
use crate::output::Output;
use crate::read::unexpected;
use crate::service::{self, client_error, internal};
use crate::target;

pub struct CreateArgs {
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<Priority>,
    pub deps: Vec<String>,
    pub stdin: bool,
    pub file: Option<PathBuf>,
    pub epic: bool,
    pub parent: Option<String>,
}

pub struct UpdateArgs {
    pub key: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<Priority>,
    pub stdin: bool,
}

fn usage(message: impl Into<String>) -> ErrorBody {
    ErrorBody::new(ErrorKind::Usage, message)
}

/// The body from `-d` or `--stdin` (not both).
fn body(description: Option<String>, stdin: bool) -> Result<Option<String>, ErrorBody> {
    match (description, stdin) {
        (Some(_), true) => Err(usage("give the description with -d or --stdin, not both")),
        (Some(d), false) => Ok(Some(d)),
        (None, true) => {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .map_err(|e| internal(format!("can't read stdin: {e}")))?;
            Ok(Some(text.trim_end().to_owned()))
        }
        (None, false) => Ok(None),
    }
}

pub fn create(args: CreateArgs, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    let (tickets, single) = match &args.file {
        Some(file) => {
            if args.title.is_some()
                || args.parent.is_some()
                || !args.deps.is_empty()
                || args.priority.is_some()
                || args.epic
                || args.description.is_some()
                || args.stdin
            {
                return Err(usage(
                    "-f takes everything from the file; give no title or other flags",
                ));
            }
            let text = std::fs::read_to_string(file)
                .map_err(|e| usage(format!("can't read {}: {e}", file.display())))?;
            (parse_markdown(&text).map_err(usage)?, false)
        }
        None => {
            let title = args
                .title
                .clone()
                .filter(|t| !t.trim().is_empty())
                .ok_or_else(|| {
                    usage(format!(
                        "give the new ticket a title: {} create \"Title\"",
                        otaq_core::naming::COMMAND_NAME
                    ))
                })?;
            let body = body(args.description.clone(), args.stdin)?.unwrap_or_default();
            let ticket = BatchTicket {
                id: None,
                title,
                kind: if args.epic { Kind::Epic } else { Kind::Ticket },
                body,
                priority: args.priority,
                parent: args.parent.clone(),
                depends_on: args.deps.clone(),
            };
            (vec![ticket], true)
        }
    };
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let (project, tickets) = resolve_refs(&client, &data, global, tickets).await?;
        let created = match client
            .request(Request::TicketCreateBatch(BatchParams { project, tickets }))
            .await
            .map_err(client_error)?
        {
            Reply::Tickets(t) => t,
            other => return Err(unexpected(&other)),
        };
        let human: Vec<String> = created
            .iter()
            .map(|t| format!("Created {} · {}", t.key, t.title))
            .collect();
        let keys: Vec<String> = created.iter().map(|t| t.key.to_string()).collect();
        if single {
            out.print(&human.join("\n"), &created[0], Some(&keys[0]));
        } else {
            out.print(&human.join("\n"), &created, Some(keys.join("\n").as_str()));
        }
        Ok(())
    })
}

/// The project the batch goes to, and its references with bare numbers made full keys. Batch IDs stay.
async fn resolve_refs(
    client: &Client,
    data: &DataDir,
    global: &GlobalArgs,
    mut tickets: Vec<BatchTicket>,
) -> Result<(String, Vec<BatchTicket>), ErrorBody> {
    let ids: Vec<String> = tickets.iter().filter_map(|t| t.id.clone()).collect();
    // The epic's project, if a ticket names one by key; else the current project.
    let parent_key = tickets
        .iter()
        .filter_map(|t| t.parent.as_ref())
        .find(|p| !ids.contains(p))
        .cloned();
    let project = match parent_key {
        Some(parent) if global.project_key().is_none() => {
            target::resolve_key(client, data, Some(parent), global)
                .await?
                .project
        }
        _ => target::require_project(client, data, global).await?,
    };
    let full = async |r: &String| -> Result<String, ErrorBody> {
        if ids.contains(r) || !r.trim().starts_with(|c: char| c.is_ascii_digit()) {
            return Ok(r.trim().to_owned());
        }
        Ok(target::resolve_key(client, data, Some(r.clone()), global)
            .await?
            .to_string())
    };
    for ticket in &mut tickets {
        if let Some(parent) = &ticket.parent {
            ticket.parent = Some(full(parent).await?);
        }
        let mut deps = Vec::new();
        for dep in &ticket.depends_on {
            deps.push(full(dep).await?);
        }
        ticket.depends_on = deps;
    }
    Ok((project.to_string(), tickets))
}

pub fn update(args: UpdateArgs, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    let body = body(args.description, args.stdin)?;
    let update = TicketUpdate {
        title: args.title,
        body,
        priority: args.priority,
        ..Default::default()
    };
    if update == TicketUpdate::default() {
        return Err(usage("nothing to change: give --title, -d, -p or --stdin"));
    }
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, args.key, global).await?;
        let ticket: Ticket = match client
            .request(Request::TicketUpdate(TicketUpdateParams {
                key: key.to_string(),
                update,
            }))
            .await
            .map_err(client_error)?
        {
            Reply::Ticket(t) => *t,
            other => return Err(unexpected(&other)),
        };
        out.print(
            &format!("Updated {} · {}", ticket.key, ticket.title),
            &ticket,
            Some(&ticket.key.to_string()),
        );
        Ok(())
    })
}
