//! The user's own terminals and processes (SDD §19; d0019, d0026, d0035): **Run** (the project's run command in a
//! ticket's worktree), the **Shell** tab and the **Project terminal**. All three are PTY sessions owned by the
//! background service (d0022), so they survive closing the window; all three start only on the user's click and
//! are never restarted on their own (d0045). Their callers are the user, with full `crewboard` access (CLI-03):
//! none of them runs `claude` for a ticket.

pub mod run;
pub mod shell;
pub mod suggest;

use std::ffi::OsString;
use std::path::PathBuf;

use crate::data_dir::DataDir;
use crate::domain::TicketKey;
use crate::naming;

/// What the user's terminals start with on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalEnv {
    /// The folder of the `crewboard` command, put first on PATH so the terminals can call it (CLI-03).
    pub bin_dir: Option<PathBuf>,
    /// The PATH the terminals extend (the service's).
    pub path: OsString,
    /// The program Shell and the Project terminal run: the user's login shell (`$SHELL`, else `/bin/sh`).
    pub shell: String,
    /// Its arguments: `-l` for a login shell.
    pub shell_args: Vec<String>,
}

impl TerminalEnv {
    /// The running service's: its binary's folder, its PATH and the user's `$SHELL`.
    pub fn from_process() -> Self {
        Self {
            bin_dir: std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(PathBuf::from)),
            path: std::env::var_os("PATH").unwrap_or_default(),
            shell: std::env::var("SHELL")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "/bin/sh".to_owned()),
            shell_args: vec!["-l".to_owned()],
        }
    }

    /// The environment a terminal adds to the service's: the data folder (so `crewboard` there talks to this
    /// service), PATH with the `crewboard` folder first, and for a ticket's Run and Shell its key and `$PORT` =
    /// the Run port (d0026).
    pub fn env(
        &self,
        data: &DataDir,
        ticket: Option<&TicketKey>,
        port: Option<u16>,
    ) -> Vec<(String, String)> {
        let mut path = OsString::new();
        if let Some(bin) = &self.bin_dir {
            path.push(bin.as_os_str());
        }
        if !self.path.is_empty() {
            if !path.is_empty() {
                path.push(":");
            }
            path.push(&self.path);
        }
        let mut env = vec![
            (DataDir::env_var(), data.root().display().to_string()),
            ("PATH".to_owned(), path.to_string_lossy().into_owned()),
        ];
        if let Some(ticket) = ticket {
            env.push((naming::ticket_env_var(), ticket.to_string()));
        }
        if let Some(port) = port {
            env.push(("PORT".to_owned(), port.to_string()));
        }
        env
    }
}

/// `$PORT` (and `${PORT}`) in a command or URL replaced by the port, for showing: "pnpm dev --port 4020".
pub fn with_port(text: &str, port: u16) -> String {
    let port = port.to_string();
    text.replace("${PORT}", &port).replace("$PORT", &port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_substitution() {
        assert_eq!(
            with_port("pnpm dev --port $PORT", 4020),
            "pnpm dev --port 4020"
        );
        assert_eq!(
            with_port("http://localhost:${PORT}/swagger", 4004),
            "http://localhost:4004/swagger"
        );
        assert_eq!(with_port("cargo run", 4004), "cargo run");
    }

    #[test]
    fn env_has_the_data_folder_path_ticket_and_port() {
        let ctx = TerminalEnv {
            bin_dir: Some(PathBuf::from("/opt/cb/bin")),
            path: OsString::from("/usr/bin:/bin"),
            shell: "/bin/sh".into(),
            shell_args: vec![],
        };
        let data = DataDir::new("/tmp/h");
        let env = ctx.env(&data, Some(&"SHOP-19".parse().unwrap()), Some(4020));
        let get = |k: &str| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("PATH"), Some("/opt/cb/bin:/usr/bin:/bin"));
        assert_eq!(get("PORT"), Some("4020"));
        assert_eq!(get(&naming::ticket_env_var()), Some("SHOP-19"));
        assert_eq!(get(&DataDir::env_var()), Some("/tmp/h"));
        let plain = ctx.env(&data, None, None);
        assert!(plain.iter().all(|(n, _)| n != "PORT"));
        assert!(plain.iter().all(|(n, _)| n != &naming::ticket_env_var()));
    }
}
