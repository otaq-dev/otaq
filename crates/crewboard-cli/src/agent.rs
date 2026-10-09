//! The commands an agent uses to hand back control: `crewboard ask human --stdin` (ATT-01, d0037) and
//! `crewboard review submit [KEY] --stdin` (REV-07, d0050). Both take their text on stdin (PLG-05) and work on
//! `$CREWBOARD_TICKET` unless a key is given.
//!
//! # `--json` shapes
//!
//! Both print the ticket afterwards (`Ticket`).

use std::io::Read;

use crewboard_core::data_dir::DataDir;
use crewboard_core::naming;
use crewboard_core::protocol::{
    AskHumanParams, ErrorBody, ErrorKind, Reply, Request, ReviewSubmitParams,
};

use crate::cli::GlobalArgs;
use crate::output::Output;
use crate::read::unexpected;
use crate::service::{self, client_error, internal};
use crate::target;

fn usage(message: impl Into<String>) -> ErrorBody {
    ErrorBody::new(ErrorKind::Usage, message)
}

fn read_stdin() -> Result<String, ErrorBody> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| internal(format!("can't read stdin: {e}")))?;
    Ok(text.trim().to_owned())
}

pub fn ask(
    target_name: &str,
    stdin: bool,
    global: &GlobalArgs,
    out: &Output,
) -> Result<(), ErrorBody> {
    let cmd = naming::COMMAND_NAME;
    if !target_name.eq_ignore_ascii_case("human") {
        return Err(usage(format!(
            "asking another ticket isn't available yet; ask the user with `{cmd} ask human --stdin`"
        )));
    }
    if !stdin {
        return Err(usage(format!(
            "give the question on stdin: `{cmd} ask human --stdin`"
        )));
    }
    let question = read_stdin()?;
    if question.is_empty() {
        return Err(usage("the question on stdin is empty"));
    }
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, None, global).await?;
        let ticket = match client
            .request(Request::TicketAskHuman(AskHumanParams {
                key: key.to_string(),
                question,
            }))
            .await
            .map_err(client_error)?
        {
            Reply::Ticket(t) => *t,
            other => return Err(unexpected(&other)),
        };
        out.print(
            &format!(
                "{} asks you. The question is on the board; end your turn and wait for the answer.",
                ticket.key
            ),
            &ticket,
            Some(&ticket.key.to_string()),
        );
        Ok(())
    })
}

pub fn submit(
    key: Option<String>,
    stdin: bool,
    global: &GlobalArgs,
    out: &Output,
) -> Result<(), ErrorBody> {
    let summary = if stdin { read_stdin()? } else { String::new() };
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, key, global).await?;
        let ticket = match client
            .request(Request::TicketReviewSubmit(ReviewSubmitParams {
                key: key.to_string(),
                summary,
            }))
            .await
            .map_err(client_error)?
        {
            Reply::Ticket(t) => *t,
            other => return Err(unexpected(&other)),
        };
        out.print(
            &format!("{} is Ready for Review.", ticket.key),
            &ticket,
            Some(&ticket.key.to_string()),
        );
        Ok(())
    })
}
