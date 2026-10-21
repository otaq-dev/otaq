//! **Bring back** (M19-c, d0086): a ticket whose build was freed at Review gets a build back in its worktree before
//! it is worked on again: Send back, a Delivery's conflict going to the agent (FLW-17), a Run or a new Shell, or a
//! start that resumes it.
//!
//! Under the ticket's lock it reads the ticket again. A build still in the worktree (`present`) is left alone, so
//! bringing it back twice does nothing. Otherwise the ticket is marked `fetching`, the nearest build in the stock is
//! handed over (its own first, [`Seeder::seed`] with `own`), and the project's setup command runs in the worktree
//! (it redoes what the build lacks; with no build in the stock it builds from nothing). Then the build is `present`.
//!
//! A failed setup leaves `fetching` and returns the error: the next fetch takes and sets up again (a start that
//! resumes the ticket fails at its setup step). Where the work goes on anyway (an agent, a Run, a Shell), what came
//! back is taken as `present`, so it is freed again at Review, or when the Run or Shell closes, and kept at Done. The
//! env files are not copied again: a Free never moves them, and the agent may have changed them.
//!
//! [`Seeder::seed`]: crate::warm::seed::Seeder::seed

use std::path::PathBuf;

use serde_json::json;
use tokio::sync::OwnedMutexGuard;

use crate::board::tickets::{EventKind, Ticket, record_event};
use crate::domain::{Actor, BuildState};
use crate::protocol::Event;
use crate::session::env::applied;
use crate::start::{StartError, begin_setup_log, run_setup};
use crate::ticket_env::TicketEnv;
use crate::ticket_env::resources::command_vars;
use crate::tools::Os;
use crate::warm::seed::Seeded;

/// Why a build is brought back (for the log).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchWhy {
    SendBack,
    Fix,
    Run,
    Shell,
    Start,
}

impl FetchWhy {
    fn as_str(self) -> &'static str {
        match self {
            Self::SendBack => "send back",
            Self::Fix => "fix",
            Self::Run => "run",
            Self::Shell => "shell",
            Self::Start => "start",
        }
    }
}

/// What bringing a build back did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    /// The build was in the worktree already: nothing was done.
    AlreadyPresent,
    /// A build was taken from the stock (the ticket's own or the nearest) and set up.
    Took(Seeded),
    /// No build in the stock (or warm builds off): the setup command built it from nothing.
    Built,
}

/// Brings `ticket`'s build back under its lock; returns the lock still held, so the caller can change the ticket
/// (or open its Run or Shell) before a Free could take the build away again.
pub(crate) async fn fetch_held(
    env: &TicketEnv,
    ticket: &Ticket,
    why: FetchWhy,
) -> (Result<Fetched, StartError>, OwnedMutexGuard<()>) {
    let held = env.lock(&ticket.id).await;
    (fetch_locked(env, ticket, why).await, held)
}

async fn fetch_locked(
    env: &TicketEnv,
    ticket: &Ticket,
    why: FetchWhy,
) -> Result<Fetched, StartError> {
    let board = env.board();
    let now = board.ticket_by_id(&ticket.id).await?;
    let work = now.work.clone().unwrap_or_default();
    if work.build_state == BuildState::Present {
        return Ok(Fetched::AlreadyPresent);
    }
    let key = now.key.to_string();
    let Some(worktree) = work
        .worktree
        .as_deref()
        .map(PathBuf::from)
        .filter(|w| w.is_dir())
    else {
        return Err(StartError::Other(format!(
            "{key} has no worktree to bring its build back into"
        )));
    };
    set_state(env, &now, BuildState::Fetching).await?;
    let project = board.project(now.key.project.as_str()).await?;
    let git = board.project_git(&project.key).await?;
    let seeded = env
        .seeder()
        .seed(board, &git, &project, &key, &worktree, true)
        .await;
    let settings = board.project_settings(&project.key).await?;
    let log = env.data().setup_log_path(&key)?;
    let first_line = seeded.as_ref().map(Seeded::log_line);
    if let Err(e) = begin_setup_log(&log, first_line.as_deref()) {
        tracing::warn!(ticket = %key, error = %e, "can't begin the setup log");
    }
    let project_env = applied(&settings.env_vars, Os::current());
    let recorded = now.work.as_ref().and_then(|w| w.resource_name.clone());
    let own = command_vars(
        env.data(),
        &project,
        &now.key,
        recorded.as_deref(),
        &settings,
    );
    run_setup(&settings.setup_command, &worktree, &log, &project_env, &own).await?;
    set_state(env, &now, BuildState::Present).await?;
    tracing::info!(ticket = %key, why = why.as_str(), seeded = ?first_line, "build brought back");
    Ok(match seeded {
        Some(seeded) => Fetched::Took(seeded),
        None => Fetched::Built,
    })
}

async fn set_state(env: &TicketEnv, ticket: &Ticket, state: BuildState) -> Result<(), StartError> {
    env.board()
        .store()
        .set_build_state(&ticket.id, state, None)
        .await?;
    env.events().publish(Event::TicketChanged {
        project: ticket.key.project.to_string(),
        id: ticket.id.clone(),
        key: ticket.key.clone(),
    });
    Ok(())
}

/// For a Run or Shell about to open: brings the build back; a failure never stops it. A ticket left `fetching` (its
/// setup failed) is then taken as `present`: what came back is in the worktree, and is freed again when the Run or
/// Shell closes (a `fetching` ticket is never freed, so it would stay there). Returns the ticket's lock, still held.
pub(crate) async fn for_terminal(
    env: &TicketEnv,
    ticket: &Ticket,
    why: FetchWhy,
) -> (Result<Fetched, StartError>, OwnedMutexGuard<()>) {
    let (result, held) = fetch_held(env, ticket, why).await;
    if result.is_err() {
        let state = env
            .board()
            .ticket_by_id(&ticket.id)
            .await
            .ok()
            .and_then(|t| t.work)
            .map(|w| w.build_state);
        if state == Some(BuildState::Fetching)
            && let Err(e) = set_state(env, ticket, BuildState::Present).await
        {
            tracing::warn!(ticket = %ticket.key, error = %e, "can't record where the build is");
        }
    }
    (result, held)
}

/// For an agent about to work on the ticket again (Send back, Fix): brings the build back; a failure never stops
/// the agent. The build is then taken as present, so the agent runs the setup itself, and a start note records why
/// (with the setup's last lines). Returns the ticket's lock, still held.
pub(crate) async fn for_agent(
    env: &TicketEnv,
    ticket: &Ticket,
    why: FetchWhy,
    actor: &Actor,
) -> OwnedMutexGuard<()> {
    let (result, held) = fetch_held(env, ticket, why).await;
    let Err(e) = result else {
        return held;
    };
    let message = e.to_string();
    tracing::warn!(ticket = %ticket.key, error = %message, "can't bring the build back; the agent goes on");
    if let Err(e) = set_state(env, ticket, BuildState::Present).await {
        tracing::warn!(ticket = %ticket.key, error = %e, "can't record where the build is");
    }
    let noted = async {
        let mut tx = env.board().store().begin_write().await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::StartNote,
            actor,
            json!({ "note": "build_not_restored", "message": message }),
        )
        .await?;
        tx.commit().await?;
        Ok::<_, crate::board::BoardError>(())
    }
    .await;
    if let Err(e) = noted {
        tracing::warn!(ticket = %ticket.key, error = %e, "can't record a start note");
    }
    held
}
