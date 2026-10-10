//! Crewboard core: domain, SQLite store, scheduler, git and worktrees, sessions, hook handling and the
//! socket server. Runs as `crewboard daemon`. Has no UI dependency, so it is fully testable headless (d0002).

pub mod attention;
pub mod board;
pub mod caller;
pub mod client;
pub mod data_dir;
pub mod domain;
pub mod git;
pub mod logging;
pub mod naming;
pub mod policy;
pub mod ports;
pub mod prime;
pub mod process;
pub mod protocol;
pub mod review;
pub mod scheduler;
pub mod service;
pub mod session;
pub mod signals;
pub mod start;
pub mod store;
pub mod tools;
