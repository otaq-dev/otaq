//! The ticket environment (M19, d0085…d0087): a ticket's build and resources across Prepare, Free, Bring back and
//! Release.
//!
//! - [`handoff`]: how a stock entry is handed to a worktree on this disk ([`handoff::HandoffMode`]).
//! - [`stock`]: the stock of builds per repository: its limit, the nearest build for a ticket, and eviction.

pub mod handoff;
pub mod stock;
