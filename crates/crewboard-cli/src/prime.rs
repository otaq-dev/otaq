//! `crewboard prime` (PLG-02; d0032, d0040): the current ticket's context for its agent, as Markdown. Every skill
//! runs it first. `--json` prints the context itself (`crewboard_core::prime::PrimeContext`).

use crewboard_core::data_dir::DataDir;
use crewboard_core::prime::render;
use crewboard_core::protocol::{ErrorBody, KeyParams, Reply, Request};

use crate::cli::GlobalArgs;
use crate::output::Output;
use crate::read::unexpected;
use crate::service::{self, client_error, internal};
use crate::target;

pub fn run(key: Option<String>, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, key, global).await?;
        let context = match client
            .request(Request::TicketPrime(KeyParams {
                key: key.to_string(),
            }))
            .await
            .map_err(client_error)?
        {
            Reply::Prime(context) => *context,
            other => return Err(unexpected(&other)),
        };
        out.print(
            render(&context).trim_end(),
            &context,
            Some(&context.ticket.key.to_string()),
        );
        Ok(())
    })
}
