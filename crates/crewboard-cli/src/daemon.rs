//! `crewboard daemon start | stop | status` (SVC-01). The service itself lives in `crewboard_core::service`;
//! `start` launches this same binary as `daemon run` in the background, detached from the terminal.
//! Nothing here installs a launch agent or login item: the service runs only when started (d0055).

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crewboard_core::client::{ClientError, request_once};
use crewboard_core::data_dir::DataDir;
use crewboard_core::protocol::{ErrorBody, ErrorKind, Reply, Request, ServiceStatus};
use crewboard_core::service::{self, ServiceConfig, ServiceError};
use crewboard_core::{logging, naming};
use serde_json::json;

use crate::cli::DaemonCommand;
use crate::output::Output;

/// How long `start` and `stop` wait for the service to come up or go away.
const WAIT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(50);

pub fn run(command: DaemonCommand, out: &Output) -> Result<(), ErrorBody> {
    let data =
        DataDir::from_env().map_err(|e| ErrorBody::new(ErrorKind::Internal, e.to_string()))?;
    match command {
        DaemonCommand::Start => start(&data, out),
        DaemonCommand::Stop => stop(&data, out),
        DaemonCommand::Status => status(&data, out),
        DaemonCommand::Run => foreground(data),
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a tokio runtime")
}

/// The running service's status, or `None` when nothing listens on the socket.
fn query_status(data: &DataDir) -> Result<Option<ServiceStatus>, ErrorBody> {
    match runtime().block_on(request_once(&data.socket_path(), Request::ServiceStatus)) {
        Ok(Reply::ServiceStatus(status)) => Ok(Some(status)),
        Ok(other) => Err(unexpected(&other)),
        Err(ClientError::NotRunning(_)) => Ok(None),
        Err(e) => Err(client_error(&e)),
    }
}

fn start(data: &DataDir, out: &Output) -> Result<(), ErrorBody> {
    if let Some(status) = query_status(data)? {
        print_started(out, &status, true);
        return Ok(());
    }
    let exe =
        std::env::current_exe().map_err(|e| internal(format!("can't find this program: {e}")))?;
    let mut command = Command::new(exe);
    command
        .args(["daemon", "run"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        // Its own process group: Ctrl+C in the terminal that ran `start` doesn't reach the service.
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|e| internal(format!("can't start the background service: {e}")))?;

    let deadline = Instant::now() + WAIT;
    loop {
        std::thread::sleep(POLL);
        if let Some(status) = query_status(data)? {
            // Another `start` may have won the race; either way one service runs.
            let ours = status.pid == child.id();
            print_started(out, &status, !ours);
            return Ok(());
        }
        let exited = child.try_wait().ok().flatten();
        if exited.is_some() || Instant::now() > deadline {
            if let Some(status) = query_status(data)? {
                print_started(out, &status, true);
                return Ok(());
            }
            return Err(internal(format!(
                "the background service didn't start; see the logs in {}",
                data.logs_dir().display()
            )));
        }
    }
}

fn print_started(out: &Output, status: &ServiceStatus, already: bool) {
    let human = if already {
        format!("Background service already running (PID {})", status.pid)
    } else {
        format!("Background service started (PID {})", status.pid)
    };
    let mut value = status_json(Some(status));
    value["already_running"] = json!(already);
    out.print(&human, &value, Some(&status.pid.to_string()));
}

fn stop(data: &DataDir, out: &Output) -> Result<(), ErrorBody> {
    let pid = match runtime().block_on(request_once(&data.socket_path(), Request::ServiceStop)) {
        Ok(Reply::Ok) => service::lock::read_pid(&data.service_lock_path()),
        Ok(other) => return Err(unexpected(&other)),
        Err(ClientError::NotRunning(_)) => {
            out.print(
                "Background service isn't running",
                &json!({"stopped": false, "running": false}),
                None,
            );
            return Ok(());
        }
        Err(e) => return Err(client_error(&e)),
    };
    // The service removes its lock file as the very last step.
    if !wait_until_gone(&data.service_lock_path()) {
        return Err(internal("the background service didn't stop in time"));
    }
    out.print(
        "Background service stopped",
        &json!({"stopped": true, "running": false, "pid": pid}),
        pid.map(|p| p.to_string()).as_deref(),
    );
    Ok(())
}

fn wait_until_gone(path: &Path) -> bool {
    let deadline = Instant::now() + WAIT;
    while path.exists() {
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(POLL);
    }
    true
}

fn status(data: &DataDir, out: &Output) -> Result<(), ErrorBody> {
    let status = query_status(data)?;
    let human = match &status {
        Some(s) => format!(
            "Background service: running\nPID: {}\nBuild: {}\nProtocol: {}\nUptime: {}",
            s.pid,
            s.build_id,
            s.protocol_version,
            format_uptime(s.uptime_secs)
        ),
        None => "Background service: stopped".to_owned(),
    };
    let pid = status.as_ref().map(|s| s.pid.to_string());
    out.print(&human, &status_json(status.as_ref()), pid.as_deref());
    Ok(())
}

fn status_json(status: Option<&ServiceStatus>) -> serde_json::Value {
    match status {
        Some(s) => {
            let mut value = serde_json::to_value(s).expect("status serializes");
            value["running"] = json!(true);
            value
        }
        None => json!({"running": false}),
    }
}

/// `3d 4h`, `2h 5m`, `7m 12s`, `40s`.
pub fn format_uptime(secs: u64) -> String {
    let (d, h, m, s) = (secs / 86_400, secs / 3_600 % 24, secs / 60 % 60, secs % 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

/// `daemon run`: the service in the foreground, logging to the daily log file.
fn foreground(data: DataDir) -> Result<(), ErrorBody> {
    let _log = logging::init(&data).map_err(|e| internal(format!("can't open the log: {e}")))?;
    tracing::info!(
        command = naming::COMMAND_NAME,
        "starting the background service"
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| internal(e.to_string()))?;
    match runtime.block_on(service::run_until_signal(ServiceConfig::for_daemon(data))) {
        Ok(()) => Ok(()),
        Err(ServiceError::AlreadyRunning { pid }) => {
            tracing::info!(?pid, "another service is already running");
            Ok(())
        }
        Err(e) => {
            tracing::error!(error = %e, "service failed");
            Err(internal(e.to_string()))
        }
    }
}

fn internal(message: impl Into<String>) -> ErrorBody {
    ErrorBody::new(ErrorKind::Internal, message)
}

fn client_error(e: &ClientError) -> ErrorBody {
    match e {
        ClientError::Service(body) => body.clone(),
        ClientError::NotRunning(_) => ErrorBody::new(ErrorKind::Disconnected, e.to_string()),
        _ => internal(e.to_string()),
    }
}

fn unexpected(reply: &Reply) -> ErrorBody {
    internal(format!(
        "unexpected answer from the background service: {reply:?}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptime_format() {
        assert_eq!(format_uptime(40), "40s");
        assert_eq!(format_uptime(7 * 60 + 12), "7m 12s");
        assert_eq!(format_uptime(2 * 3600 + 5 * 60 + 9), "2h 5m");
        assert_eq!(format_uptime(3 * 86_400 + 4 * 3600), "3d 4h");
    }
}
