//! The connection the agent-facing commands use: never starts the service, and asks for the right actor.
//!
//! **Actor (CLI-04, d0032):** `--actor` if given, else `agent:<KEY>` when `$CREWBOARD_TICKET` is set, else none
//! (the service records the OS user). The service has the last word: a caller inside an agent session always
//! acts as that agent (CLI-01), whatever is asked here.

use crewboard_core::client::{Client, ClientConfig, ClientError};
use crewboard_core::data_dir::DataDir;
use crewboard_core::naming;
use crewboard_core::protocol::{ErrorBody, ErrorKind};

use crate::cli::GlobalArgs;

/// The actor to ask for, from `--actor` and `$CREWBOARD_TICKET`.
pub fn requested_actor(explicit: Option<&str>, ticket_env: Option<&str>) -> Option<String> {
    let non_empty = |v: &&str| !v.trim().is_empty();
    explicit.filter(non_empty).map(str::to_owned).or_else(|| {
        ticket_env
            .filter(non_empty)
            .map(|key| format!("agent:{}", key.trim()))
    })
}

/// `$CREWBOARD_TICKET`, if set and not blank.
pub fn ticket_env() -> Option<String> {
    std::env::var(naming::ticket_env_var())
        .ok()
        .filter(|v| !v.trim().is_empty())
}

/// Connects to the running service. A service that isn't running is an error that says how to start it.
pub async fn connect(data: &DataDir, global: &GlobalArgs) -> Result<Client, ErrorBody> {
    let mut config = ClientConfig::new(data);
    config.actor = requested_actor(global.actor.as_deref(), ticket_env().as_deref());
    match Client::connect(config).await {
        Ok(client) => Ok(client),
        Err(ClientError::NotRunning(_)) => Err(ErrorBody::new(
            ErrorKind::Disconnected,
            format!(
                "the background service isn't running; start it with `{} daemon start`",
                naming::COMMAND_NAME
            ),
        )),
        Err(e) => Err(client_error(e)),
    }
}

pub fn client_error(e: ClientError) -> ErrorBody {
    match e {
        ClientError::Service(body) => body,
        other => ErrorBody::new(ErrorKind::Disconnected, other.to_string()),
    }
}

pub fn internal(message: impl Into<String>) -> ErrorBody {
    ErrorBody::new(ErrorKind::Internal, message)
}

/// A runtime for one command.
pub fn runtime() -> Result<tokio::runtime::Runtime, ErrorBody> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_defaults() {
        assert_eq!(requested_actor(None, None), None);
        assert_eq!(
            requested_actor(None, Some("SHOP-18")).as_deref(),
            Some("agent:SHOP-18")
        );
        assert_eq!(
            requested_actor(Some("nevzat"), Some("SHOP-18")).as_deref(),
            Some("nevzat"),
            "an explicit --actor wins here; the service still decides for agent callers"
        );
        assert_eq!(requested_actor(Some(" "), Some(" ")), None);
    }
}
