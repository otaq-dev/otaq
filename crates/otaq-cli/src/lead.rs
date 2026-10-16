//! An epic lead's own commands (LEAD-08, CLI-07; d0083), run from inside the lead's session. The lead works only
//! through the `otaq` CLI, so any agent provider can be a lead:
//! - `otaq suggest close|delete <KEY> --stdin`: suggests closing or deleting a never-started child of its epic, the
//!   reason on stdin. The user decides from the inbox; nothing waits for it.
//! - `otaq note <KEY> --stdin`: leaves an unread note for a running child of its epic (LEAD-10). Its agent reads it
//!   before its next commit or submit. To a child whose first stop woke the lead, the note answers that stop as
//!   "Answer the agent" would (LEAD-07); `--view "<text>"` adds the lead's view of the cause, shown to the user only
//!   if the child gets stuck again.
//!
//! The service refuses these for anyone but an epic's lead, and for tickets outside its epic.
//!
//! A child's agent reads its notes with `otaq notes` (the Commit skill runs it before each commit): it prints the
//! unread notes and marks them read.
//!
//! # `--json` shape
//!
//! `suggest` prints the suggestion (`otaq_core::epic_lead::suggest::Suggestion`): `{"id", "epic", "child",
//! "child_title", "kind": "close", "reason", "state": "open", …}`. `--silent` prints its id.
//! `note` prints the note (`otaq_core::epic_lead::notes::LeadNote`): `{"id", "epic", "child", "lead", "text",
//! "view", "created_at", …}`; `notes` the list of notes it read. `--silent` prints the note's id, or the number read.

use std::io::Read;

use clap::Subcommand;
use otaq_core::data_dir::DataDir;
use otaq_core::epic_lead::notes::{reply_message, unread_message};
use otaq_core::epic_lead::suggest::{SuggestionKind, suggested_message};
use otaq_core::naming;
use otaq_core::protocol::{
    ErrorBody, ErrorKind, KeyParams, LeadNoteParams, Reply, Request, SuggestParams,
};

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
    /// Epic leads: leave a note for a running child of your epic; the note on stdin
    Note {
        /// The child: SHOP-40.4, or 40.4 in the current project
        key: String,
        /// To a stuck child: your view of the cause, shown to the user only if it gets stuck again
        #[arg(long, value_name = "TEXT")]
        view: Option<String>,
        /// Read the note from stdin
        #[arg(long)]
        stdin: bool,
    },
    /// Print your ticket's unread notes from its epic's lead, and mark them read
    Notes {
        /// The ticket; defaults to $OTAQ_TICKET
        key: Option<String>,
    },
}

pub fn run(command: LeadCommand, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    match command {
        LeadCommand::Suggest { kind, key, stdin } => suggest(&kind, key, stdin, global, out),
        LeadCommand::Note { key, view, stdin } => note(key, view, stdin, global, out),
        LeadCommand::Notes { key } => notes(key, global, out),
    }
}

fn note(
    key: String,
    view: Option<String>,
    stdin: bool,
    global: &GlobalArgs,
    out: &Output,
) -> Result<(), ErrorBody> {
    let usage = |message: String| ErrorBody::new(ErrorKind::Usage, message);
    if !stdin {
        return Err(usage(format!(
            "give the note on stdin: `{} note {key} --stdin`",
            naming::COMMAND_NAME
        )));
    }
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| internal(format!("can't read stdin: {e}")))?;
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Err(usage("the note on stdin is empty".to_owned()));
    }
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, Some(key), global).await?;
        let note = match client
            .request(Request::LeadNote(LeadNoteParams {
                key: key.to_string(),
                text,
                view: view.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()),
            }))
            .await
            .map_err(client_error)?
        {
            Reply::LeadNote(n) => *n,
            other => return Err(unexpected(&other)),
        };
        out.print(&reply_message(&note), &note, Some(&note.id));
        Ok(())
    })
}

fn notes(key: Option<String>, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, key, global).await?;
        let notes = match client
            .request(Request::TicketNotes(KeyParams {
                key: key.to_string(),
            }))
            .await
            .map_err(client_error)?
        {
            Reply::LeadNotes(n) => n,
            other => return Err(unexpected(&other)),
        };
        out.print(
            &unread_message(&notes),
            &notes,
            Some(&notes.len().to_string()),
        );
        Ok(())
    })
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
