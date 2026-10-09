//! Crewboard core: domain, SQLite store, scheduler, git and worktrees, sessions, hook handling and the
//! socket server. Runs as `crewboard daemon`. Has no UI dependency, so it is fully testable headless (d0002).

pub mod data_dir;
pub mod logging;
pub mod naming;
pub mod protocol;
pub mod store;
