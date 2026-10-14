//! Logs go to `~/.otaq/logs/`, one file per day (d0055): `otaq.2026-10-09.log`.
//!
//! Implementation choices (SDD §26): rotation is daily only, with no size cap; the newest
//! [`KEEP_LOG_FILES`] files are kept and older ones removed. `OTAQ_LOG` sets the filter
//! (for example `debug`); the default is `info`.

use std::io;

use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

use crate::data_dir::DataDir;
use crate::naming;

/// How many daily log files are kept.
pub const KEEP_LOG_FILES: usize = 14;

/// Keeps the background log writer alive; dropping it flushes pending lines.
#[must_use = "logs stop being written when the guard is dropped"]
pub struct LogGuard {
    _worker: WorkerGuard,
}

/// The variable that sets the log filter: `OTAQ_LOG`.
pub fn filter_env_var() -> String {
    naming::env_var("LOG")
}

/// A writer to today's log file. Creates the log folder (user-only) on first use.
pub fn file_writer(data: &DataDir) -> io::Result<(NonBlocking, WorkerGuard)> {
    let dir = data.logs_dir();
    DataDir::ensure_dir(&dir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(naming::COMMAND_NAME)
        .filename_suffix("log")
        .max_log_files(KEEP_LOG_FILES)
        .build(&dir)
        .map_err(io::Error::other)?;
    Ok(tracing_appender::non_blocking(appender))
}

/// Sends this process's `tracing` output to the daily log file. Call once, at start-up.
pub fn init(data: &DataDir) -> io::Result<LogGuard> {
    let (writer, worker) = file_writer(data)?;
    let filter =
        EnvFilter::try_from_env(filter_env_var()).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(filter)
        .try_init()
        .map_err(io::Error::other)?;
    Ok(LogGuard { _worker: worker })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_to_a_dated_file_in_the_log_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let data = DataDir::new(tmp.path().join("home"));

        let (writer, worker) = file_writer(&data).unwrap();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(writer)
            .with_ansi(false)
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("service started");
        });
        drop(worker);

        let files: Vec<_> = std::fs::read_dir(data.logs_dir())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1, "{files:?}");
        let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
        let prefix = format!("{}.", naming::COMMAND_NAME);
        assert!(
            name.starts_with(&prefix) && name.ends_with(".log"),
            "{name}"
        );
        // prefix + YYYY-MM-DD + ".log"
        assert_eq!(name.len(), prefix.len() + 10 + 4, "{name}");
        let text = std::fs::read_to_string(&files[0]).unwrap();
        assert!(text.contains("service started"), "{text}");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(data.logs_dir())
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700);
        }
    }

    #[test]
    fn filter_variable_name() {
        assert_eq!(filter_env_var(), naming::env_var("LOG"));
    }
}
