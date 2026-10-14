//! `otaq dep add | remove | tree | cycles` (SDD §11.2; DEP-01, DEP-02, CLI-02; d0032). The service checks
//! every link: loops ("SHOP-21 → SHOP-20 → SHOP-21"), bases ("Linked tickets must use the same base branch.") and,
//! for agents, the permission scope.
//!
//! # `--json` shapes
//!
//! - `dep add` / `dep remove`: the dependent `Ticket` afterwards.
//! - `dep tree`: `[{"key", "title", "status", "kind", "depth", "current"}]`, the Stack list in the panel's order.
//! - `dep cycles`: `[[key, …, key]]`, each loop starting and ending on the same ticket; `[]` when healthy.

use otaq_core::board::stack::{empty_text, stack_rows};
use otaq_core::data_dir::DataDir;
use otaq_core::protocol::{DepParams, ErrorBody, ProjectParams, Reply, Request};

use crate::cli::{DepCommand, GlobalArgs};
use crate::output::Output;
use crate::read::{get_ticket, list_tickets, unexpected};
use crate::service::{self, client_error, internal};
use crate::target;

pub fn run(command: DepCommand, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = async |k: Option<String>| target::resolve_key(&client, &data, k, global).await;
        let adding = matches!(command, DepCommand::Add { .. });
        match command {
            DepCommand::Add {
                key: child,
                depends_on,
            }
            | DepCommand::Remove {
                key: child,
                depends_on,
            } => {
                let child = key(Some(child)).await?.to_string();
                let parent = key(Some(depends_on)).await?.to_string();
                let params = DepParams { child, parent };
                let request = if adding {
                    Request::DepAdd(params.clone())
                } else {
                    Request::DepRemove(params.clone())
                };
                let ticket = match client.request(request).await.map_err(client_error)? {
                    Reply::Ticket(t) => *t,
                    other => return Err(unexpected(&other)),
                };
                let human = if adding {
                    format!("{} now depends on {}", ticket.key, params.parent)
                } else {
                    format!("{} no longer depends on {}", ticket.key, params.parent)
                };
                out.print(&human, &ticket, Some(&ticket.key.to_string()));
            }
            DepCommand::Tree { key: k } => {
                let current = get_ticket(&client, &key(k).await?.to_string()).await?;
                let tickets = list_tickets(&client, current.key.project.as_str()).await?;
                let rows = stack_rows(&current.key, &tickets);
                let human = if rows.is_empty() {
                    empty_text(&current.key)
                } else {
                    rows.iter()
                        .map(|r| {
                            let mark = if r.current { "  ← this ticket" } else { "" };
                            format!(
                                "{}{:<10} {:<22} {}{mark}",
                                "  ".repeat(r.depth),
                                r.key.to_string(),
                                r.status.label(),
                                r.title
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                out.print(&human, &rows, Some(&current.key.to_string()));
            }
            DepCommand::Cycles => {
                let project = target::require_project(&client, &data, global).await?;
                let cycles = match client
                    .request(Request::DepCycles(ProjectParams {
                        project: project.to_string(),
                    }))
                    .await
                    .map_err(client_error)?
                {
                    Reply::Cycles(c) => c,
                    other => return Err(unexpected(&other)),
                };
                let human = if cycles.is_empty() {
                    format!("No dependency loops in {project}")
                } else {
                    cycles
                        .iter()
                        .map(|c| {
                            c.iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(" → ")
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                out.print(&human, &cycles, None);
            }
        }
        Ok(())
    })
}
