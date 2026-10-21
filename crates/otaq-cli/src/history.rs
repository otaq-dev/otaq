//! `otaq history` (ONB-01, ONB-02; d0108): what the user typed into Claude Code in every project, slash commands
//! included and pasted text left out, for the Project agent that proposes a first flow. The service reads it (it
//! knows the caller): the user's terminal and a Project agent may; a ticket's agent, a check and an epic lead are
//! refused. Nothing is written to disk.
//!
//! # `--json` shape
//!
//! `{tools, found, since, project?, command?, messages, projects, page, pages, skipped, entries: [{at, time,
//! project, text}]}`: the same fields as the text; `at` in Unix milliseconds.

use otaq_core::agent_tools::history::{self, HistoryParams, Since};
use otaq_core::data_dir::DataDir;
use otaq_core::protocol::{ErrorBody, Reply, Request};

use crate::cli::GlobalArgs;
use crate::output::Output;
use crate::read::unexpected;
use crate::service::{self, client_error, internal};

pub fn run(
    since: Since,
    all: bool,
    command: Option<String>,
    page: u32,
    global: &GlobalArgs,
    out: &Output,
) -> Result<(), ErrorBody> {
    let params = HistoryParams {
        since: if all { Since::All } else { since },
        // The global `--project`: here a project's name as the lines print it (or an Otaq project's key).
        project: global.project_key().map(str::to_owned),
        command,
        page,
    };
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let page = match client
            .request(Request::HistoryRead(params))
            .await
            .map_err(client_error)?
        {
            Reply::UserHistory(page) => *page,
            other => return Err(unexpected(&other)),
        };
        out.print(&history::text(&page), &page, None);
        Ok(())
    })
}
