//! Crewboard core: domain, SQLite store, scheduler, git and worktrees, sessions, hook handling and the
//! socket server. Runs as `crewboard daemon`. Has no UI dependency, so it is fully testable headless (d0002).

pub mod board;
pub mod client;
pub mod data_dir;
pub mod domain;
pub mod git;
pub mod logging;
pub mod naming;
pub mod ports;
pub mod protocol;
pub mod service;
pub mod store;
pub mod tools;
