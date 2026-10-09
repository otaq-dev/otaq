//! `crewboard attach <KEY>`: a ticket's live session in any terminal (SES-05, d0022).
//!
//! The local terminal goes into raw mode; the stored scrollback is printed, then live output follows while
//! keystrokes and window resizes are relayed through the service (the same relay as the app, so the unsent-input
//! flag stays right). **Detach with Ctrl+]**: the terminal is restored and the session keeps running. Ctrl+] was
//! chosen because Claude Code doesn't use it, unlike Esc and Ctrl+C (implementation choice).
//!
//! The key may be `SHOP-18`, a bare number (`18`, `40.1`) in the current project (`-P`, the project of
//! `$CREWBOARD_TICKET`, or the project whose repository or worktree holds the current folder), or omitted to use
//! `$CREWBOARD_TICKET`.

use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::path::PathBuf;

use crewboard_core::client::{Client, ClientConfig, ClientError};
use crewboard_core::data_dir::DataDir;
use crewboard_core::domain::{ProjectKey, TicketKey};
use crewboard_core::naming;
use crewboard_core::protocol::{
    ErrorBody, ErrorKind, Event, InputParams, Reply, Request, ResizeParams, SessionAttach,
    SessionIdParams, SessionTarget, decode_bytes, encode_bytes,
};
use crewboard_core::session::{SessionState, TerminalSize};
use tokio::sync::{broadcast, mpsc};

use crate::cli::GlobalArgs;

/// Ctrl+]: leaves the session running and returns to the shell.
pub const DETACH_KEY: u8 = 0x1d;

pub fn run(key: Option<String>, global: &GlobalArgs) -> Result<(), ErrorBody> {
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| internal(e.to_string()))?;
    runtime.block_on(attach(&data, key, global))
}

async fn attach(data: &DataDir, key: Option<String>, global: &GlobalArgs) -> Result<(), ErrorBody> {
    let client = match Client::connect(ClientConfig::new(data)).await {
        Ok(client) => client,
        Err(ClientError::NotRunning(_)) => {
            return Err(ErrorBody::new(
                ErrorKind::Disconnected,
                format!(
                    "the background service isn't running; start it with `{} daemon start`",
                    naming::COMMAND_NAME
                ),
            ));
        }
        Err(e) => return Err(client_error(e)),
    };
    let key = resolve_key(&client, data, key, global).await?;
    let mut events = client.subscribe().await.map_err(client_error)?;
    let attached = request_attach(&client, &key).await?;
    let session = attached.session.id.clone();

    let terminal = RawTerminal::enter();
    let mut stdout = std::io::stdout();
    let mut end = write_scrollback(&mut stdout, &attached)?;
    if let Some(size) = terminal.as_ref().and_then(|t| t.size()) {
        resize(&client, &session, size).await;
    }

    let (keys, mut typed) = mpsc::channel::<Vec<u8>>(64);
    std::thread::spawn(move || read_stdin(&keys));
    let mut winch = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::window_change())
        .map_err(|e| internal(e.to_string()))?;

    let ended = loop {
        tokio::select! {
            chunk = typed.recv() => {
                let Some(chunk) = chunk else { break None };
                let (before, detach) = split_detach(&chunk);
                if !before.is_empty() {
                    let input = Request::SessionInput(InputParams {
                        session: session.clone(),
                        data: encode_bytes(before),
                        reply: false,
                    });
                    if let Err(e) = client.request(input).await {
                        break Some(Err(client_error(e)));
                    }
                }
                if detach {
                    let _ = client
                        .request(Request::SessionDetach(SessionIdParams { session: session.clone() }))
                        .await;
                    break None;
                }
            }
            event = events.recv() => match event {
                Ok(Event::SessionOutput { session: s, offset, data }) if s == session => {
                    let bytes = decode_bytes(&data).unwrap_or_default();
                    // Skip anything the scrollback already showed.
                    let skip = usize::try_from(end.saturating_sub(offset)).unwrap_or(usize::MAX);
                    if skip < bytes.len() {
                        write_out(&mut stdout, &bytes[skip..])?;
                    }
                    end = end.max(offset + bytes.len() as u64);
                }
                Ok(Event::SessionState { session: info }) if info.id == session && info.state == SessionState::Ended => {
                    break Some(Ok(info.end_reason.unwrap_or_default()));
                }
                Ok(_) => {}
                // Output was dropped: redraw from the service's scrollback.
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let again = request_attach(&client, &key).await?;
                    write_out(&mut stdout, b"\x1b[2J\x1b[H")?;
                    end = write_scrollback(&mut stdout, &again)?;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break Some(Err(ErrorBody::new(ErrorKind::Disconnected, "lost the connection to the background service")));
                }
            },
            _ = winch.recv() => {
                if let Some(size) = terminal.as_ref().and_then(|t| t.size()) {
                    resize(&client, &session, size).await;
                }
            }
        }
    };
    drop(terminal);
    match ended {
        None => {
            eprintln!("\r\n[detached from {key}; its session keeps running]");
            Ok(())
        }
        Some(Ok(reason)) => {
            eprintln!("\r\n[{key}'s session ended: {reason}]");
            Ok(())
        }
        Some(Err(e)) => Err(e),
    }
}

/// The ticket to attach to: the argument (a key or a bare number in the current project) or `$CREWBOARD_TICKET`.
async fn resolve_key(
    client: &Client,
    data: &DataDir,
    key: Option<String>,
    global: &GlobalArgs,
) -> Result<TicketKey, ErrorBody> {
    let from_env = std::env::var(naming::ticket_env_var())
        .ok()
        .filter(|v| !v.trim().is_empty());
    let input = key.or_else(|| from_env.clone()).ok_or_else(|| {
        ErrorBody::new(
            ErrorKind::Usage,
            format!(
                "name a ticket (`{} attach SHOP-18`), or run it where {} is set",
                naming::COMMAND_NAME,
                naming::ticket_env_var()
            ),
        )
    })?;
    let needs_project = input.trim().starts_with(|c: char| c.is_ascii_digit());
    let current = if needs_project {
        current_project(client, data, global, from_env.as_deref()).await?
    } else {
        None
    };
    TicketKey::resolve(&input, current.as_ref())
        .map_err(|e| ErrorBody::new(ErrorKind::Usage, e.to_string()))
}

async fn current_project(
    client: &Client,
    data: &DataDir,
    global: &GlobalArgs,
    ticket_env: Option<&str>,
) -> Result<Option<ProjectKey>, ErrorBody> {
    let parse = |key: &str| ProjectKey::new(key).ok();
    if let Some(key) = global.project.as_deref().and_then(parse) {
        return Ok(Some(key));
    }
    if let Some(key) = ticket_env.and_then(|t| t.parse::<TicketKey>().ok()) {
        return Ok(Some(key.project));
    }
    let Ok(cwd) = std::env::current_dir() else {
        return Ok(None);
    };
    let cwd = cwd.canonicalize().unwrap_or(cwd);
    // Inside a ticket's worktree: wt/<repo-name>/<KEY>/…
    let worktrees = data.worktrees_dir();
    let worktrees = worktrees.canonicalize().unwrap_or(worktrees);
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
        .filter(|p| cwd.starts_with(PathBuf::from(&p.repo_path)))
        .max_by_key(|p| p.repo_path.len())
        .and_then(|p| parse(&p.key)))
}

async fn request_attach(client: &Client, key: &TicketKey) -> Result<SessionAttach, ErrorBody> {
    let no_session = || ErrorBody::new(ErrorKind::NotFound, format!("{key} has no live session"));
    let reply = client
        .request(Request::SessionAttach(SessionTarget {
            session: None,
            key: Some(key.to_string()),
        }))
        .await;
    match reply {
        Ok(Reply::SessionAttached(attached)) if attached.session.is_running() => Ok(*attached),
        Ok(Reply::SessionAttached(_)) => Err(no_session()),
        Ok(other) => Err(internal(format!("unexpected answer {other:?}"))),
        Err(ClientError::Service(e))
            if e.kind == ErrorKind::NotFound && e.message.contains("no live session") =>
        {
            Err(no_session())
        }
        Err(e) => Err(client_error(e)),
    }
}

fn write_scrollback(out: &mut impl Write, attached: &SessionAttach) -> Result<u64, ErrorBody> {
    let bytes = decode_bytes(&attached.scrollback).unwrap_or_default();
    write_out(out, &bytes)?;
    Ok(attached.end)
}

fn write_out(out: &mut impl Write, bytes: &[u8]) -> Result<(), ErrorBody> {
    out.write_all(bytes)
        .and_then(|()| out.flush())
        .map_err(|e| internal(format!("can't write to the terminal: {e}")))
}

async fn resize(client: &Client, session: &str, size: TerminalSize) {
    let _ = client
        .request(Request::SessionResize(ResizeParams {
            session: session.to_owned(),
            size,
        }))
        .await;
}

/// The bytes before the detach key, and whether it was pressed.
pub fn split_detach(chunk: &[u8]) -> (&[u8], bool) {
    match chunk.iter().position(|b| *b == DETACH_KEY) {
        Some(at) => (&chunk[..at], true),
        None => (chunk, false),
    }
}

fn read_stdin(keys: &mpsc::Sender<Vec<u8>>) {
    let mut stdin = std::io::stdin().lock();
    let mut buf = [0u8; 4096];
    loop {
        match stdin.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                if keys.blocking_send(buf[..n].to_vec()).is_err() {
                    return;
                }
            }
        }
    }
}

/// The local terminal in raw mode for as long as this lives; restored on drop (also on errors).
struct RawTerminal {
    saved: rustix::termios::Termios,
}

impl RawTerminal {
    /// `None` when stdin isn't a terminal (keys are then relayed as they come).
    fn enter() -> Option<Self> {
        let stdin = std::io::stdin();
        if !rustix::termios::isatty(stdin.as_fd()) {
            return None;
        }
        let saved = rustix::termios::tcgetattr(stdin.as_fd()).ok()?;
        let mut raw = saved.clone();
        raw.make_raw();
        rustix::termios::tcsetattr(stdin.as_fd(), rustix::termios::OptionalActions::Now, &raw)
            .ok()?;
        Some(Self { saved })
    }

    fn size(&self) -> Option<TerminalSize> {
        let size = rustix::termios::tcgetwinsize(std::io::stdout().as_fd()).ok()?;
        (size.ws_col > 0 && size.ws_row > 0).then_some(TerminalSize {
            cols: size.ws_col,
            rows: size.ws_row,
        })
    }
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        let _ = rustix::termios::tcsetattr(
            std::io::stdin().as_fd(),
            rustix::termios::OptionalActions::Now,
            &self.saved,
        );
    }
}

fn client_error(e: ClientError) -> ErrorBody {
    match e {
        ClientError::Service(body) => body,
        other => ErrorBody::new(ErrorKind::Disconnected, other.to_string()),
    }
}

fn internal(message: String) -> ErrorBody {
    ErrorBody::new(ErrorKind::Internal, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detach_key_splits_the_input() {
        assert_eq!(split_detach(b"abc"), (&b"abc"[..], false));
        assert_eq!(split_detach(b"ab\x1dcd"), (&b"ab"[..], true));
        assert_eq!(split_detach(b"\x1d"), (&b""[..], true));
        // Esc and Ctrl+C go to the agent.
        assert_eq!(split_detach(b"\x1b\x03"), (&b"\x1b\x03"[..], false));
    }
}
