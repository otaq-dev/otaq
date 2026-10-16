//! An epic lead's own commands (LEAD-08, CLI-07; d0083), run from inside the lead's session. The lead works only
//! through the `otaq` CLI, so any agent provider can be a lead:
//! - `otaq suggest close|delete <KEY> --stdin`: suggests closing or deleting a never-started child of its epic, the
//!   reason on stdin. The user decides from the inbox; nothing waits for it.
//!
//! The service refuses these for anyone but an epic's lead, and for tickets outside its epic.
//!
//! # `--json` shape
//!
//! `suggest` prints the suggestion (`otaq_core::epic_lead::suggest::Suggestion`): `{"id", "epic", "child",
//! "child_title", "kind": "close", "reason", "state": "open", …}`. `--silent` prints its id.

use std::io::Read;

use clap::Subcommand;
use otaq_core::data_dir::DataDir;
use otaq_core::epic_lead::suggest::{SuggestionKind, suggested_message};
use otaq_core::naming;
use otaq_core::protocol::{ErrorBody, ErrorKind, Reply, Request, SuggestParams};

use crate::cli::GlobalArgs;
use crate::output::Output;
use crate::read::unexpected;
use crate::service::{self, client_error, internal};
use crate::target;

/// The lead's commands, flattened into the top level (`otaq suggest …`). Later lead commands add one entry here
/// and one arm in [`run`].
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum LeadCommand {
    /// Epic leads: suggest closing or deleting a child of your epic that hasn't started; the reason on stdin
    Suggest {
        /// close or delete
        #[arg(value_name = "close|delete", value_parser = ["close", "delete"])]
        kind: String,
        /// The child: SHOP-40.6, or 40.6 in the current project
        key: String,
        /// Read the reason from stdin
        #[arg(long)]
        stdin: bool,
    },
}

pub fn run(command: LeadCommand, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    match command {
        LeadCommand::Suggest { kind, key, stdin } => suggest(&kind, key, stdin, global, out),
    }
}

fn suggest(
    kind: &str,
    key: String,
    stdin: bool,
    global: &GlobalArgs,
    out: &Output,
) -> Result<(), ErrorBody> {
    let usage = |message: String| ErrorBody::new(ErrorKind::Usage, message);
    if !stdin {
        return Err(usage(format!(
            "give the reason on stdin: `{} suggest {kind} {key} --stdin`",
            naming::COMMAND_NAME
        )));
    }
    let kind: SuggestionKind = kind.parse().map_err(|e| usage(format!("{e}")))?;
    let mut reason = String::new();
    std::io::stdin()
        .read_to_string(&mut reason)
        .map_err(|e| internal(format!("can't read stdin: {e}")))?;
    let reason = reason.trim().to_owned();
    if reason.is_empty() {
        return Err(usage("the reason on stdin is empty".to_owned()));
    }
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, Some(key), global).await?;
        let suggestion = match client
            .request(Request::LeadSuggest(SuggestParams {
                key: key.to_string(),
                kind,
                reason,
            }))
            .await
            .map_err(client_error)?
        {
            Reply::Suggestion(s) => *s,
            other => return Err(unexpected(&other)),
        };
        out.print(
            &suggested_message(&suggestion),
            &suggestion,
            Some(&suggestion.id),
        );
        Ok(())
    })
}
