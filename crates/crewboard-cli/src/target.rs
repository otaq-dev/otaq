//! Which ticket or project a command is about (d0032): an explicit key, a bare number (`18`, `40.1`) in the
//! current project, or `$CREWBOARD_TICKET` when the key is omitted.
//!
//! The current project is `-P`, else the project of `$CREWBOARD_TICKET`, else the project whose worktree
//! (`wt/<repo-name>/<KEY>/…`) or repository holds the current folder.

use std::path::PathBuf;

use crewboard_core::client::Client;
use crewboard_core::data_dir::DataDir;
use crewboard_core::domain::{ProjectKey, TicketKey};
use crewboard_core::naming;
use crewboard_core::protocol::{ErrorBody, ErrorKind, Reply, Request};

use crate::cli::GlobalArgs;
use crate::service::{client_error, ticket_env};

/// The ticket a command names: the argument (a key or a bare number in the current project) or
/// `$CREWBOARD_TICKET`.
pub async fn resolve_key(
    client: &Client,
    data: &DataDir,
    key: Option<String>,
    global: &GlobalArgs,
) -> Result<TicketKey, ErrorBody> {
    let from_env = ticket_env();
    let input = key.or_else(|| from_env.clone()).ok_or_else(|| {
        ErrorBody::new(
            ErrorKind::Usage,
            format!(
                "name a ticket (SHOP-18), or run it where {} is set",
                naming::ticket_env_var()
            ),
        )
    })?;
    let needs_project = input.trim().starts_with(|c: char| c.is_ascii_digit());
    let current = if needs_project {
        current_project(client, data, global).await?
    } else {
        None
    };
    TicketKey::resolve(&input, current.as_ref())
        .map_err(|e| ErrorBody::new(ErrorKind::Usage, e.to_string()))
}

/// The project a project-wide command works in; an error when none can be found.
pub async fn require_project(
    client: &Client,
    data: &DataDir,
    global: &GlobalArgs,
) -> Result<ProjectKey, ErrorBody> {
    current_project(client, data, global).await?.ok_or_else(|| {
        ErrorBody::new(
            ErrorKind::Usage,
            "name a project with -P SHOP, or run it inside a project's folder",
        )
    })
}

pub async fn current_project(
    client: &Client,
    data: &DataDir,
    global: &GlobalArgs,
) -> Result<Option<ProjectKey>, ErrorBody> {
    if let Some(key) = global.project.as_deref() {
        return ProjectKey::new(key.trim())
            .map(Some)
            .map_err(|e| ErrorBody::new(ErrorKind::Usage, e.to_string()));
    }
    if let Some(key) = ticket_env().and_then(|t| t.trim().parse::<TicketKey>().ok()) {
        return Ok(Some(key.project));
    }
    let Ok(cwd) = std::env::current_dir() else {
        return Ok(None);
    };
    let cwd = crewboard_core::paths::canonical(&cwd).unwrap_or(cwd);
    // Inside a ticket's worktree: wt/<repo-name>/<KEY>/…
    let worktrees = data.worktrees_dir();
    let worktrees = crewboard_core::paths::canonical(&worktrees).unwrap_or(worktrees);
    if let Ok(rest) = cwd.strip_prefix(&worktrees)
        && let Some(key) = rest
            .components()
            .nth(1)
            .and_then(|c| c.as_os_str().to_str())
            .and_then(|k| k.parse::<TicketKey>().ok())
    {
        return Ok(Some(key.project));
    }
    let Reply::Projects(projects) = client
        .request(Request::ProjectList)
        .await
        .map_err(client_error)?
    else {
        return Ok(None);
    };
    Ok(projects
        .iter()
        .filter(|p| {
            let repo = PathBuf::from(&p.repo_path);
            let repo = crewboard_core::paths::canonical(&repo).unwrap_or(repo);
            cwd.starts_with(repo)
        })
        .max_by_key(|p| p.repo_path.len())
        .and_then(|p| ProjectKey::new(&p.key).ok()))
}
