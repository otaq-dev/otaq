//! Starting the background service when it isn't running (d0055: the app starts it; nothing starts it at login).

use std::ffi::OsString;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;

use crate::naming;

/// Starts the service. [`super::Client::connect_or_start`] calls it at most once, when no service answers.
pub trait Launcher: Send + Sync {
    fn launch(&self) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send + '_>>;
}

/// Runs `otaq daemon start` and waits for it to finish. That command returns once the service answers,
/// and reports "already running" instead of starting a second one.
#[derive(Debug, Clone)]
pub struct CommandLauncher {
    pub program: PathBuf,
    /// Extra environment for the command, for example the data-folder override in tests.
    pub env: Vec<(OsString, OsString)>,
}

impl CommandLauncher {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            env: Vec::new(),
        }
    }

    pub fn with_env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// The `otaq` command installed next to the running program (as `cargo build` and installs lay it out).
    pub fn next_to_current_exe() -> io::Result<Self> {
        let exe = std::env::current_exe()?;
        let dir = exe
            .parent()
            .ok_or_else(|| io::Error::other("program has no folder"))?;
        Ok(Self::new(dir.join(format!(
            "{}{}",
            naming::COMMAND_NAME,
            std::env::consts::EXE_SUFFIX
        ))))
    }
}

impl Launcher for CommandLauncher {
    fn launch(&self) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send + '_>> {
        Box::pin(async move {
            let mut command = tokio::process::Command::new(&self.program);
            // The app has no console on Windows: without this the command would open a console window.
            #[cfg(windows)]
            command.creation_flags(crate::process::CREATE_NO_WINDOW);
            let status = command
                .args(["daemon", "start", "--quiet"])
                .envs(self.env.iter().map(|(k, v)| (k, v)))
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await?;
            if status.success() {
                Ok(())
            } else {
                Err(io::Error::other(format!(
                    "{} daemon start failed ({status})",
                    self.program.display()
                )))
            }
        })
    }
}
