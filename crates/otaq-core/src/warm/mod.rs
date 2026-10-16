//! Warm builds (SDD §9.6, d0082): a project's kept build output, `<app-data>/warm/<repo-name>`, that a new worktree
//! is cloned from so its first build only redoes what changed.
//!
//! - [`clone`]: copy-on-write cloning and whether the data folder's disk can clone (WRM-01). A plain copy is never
//!   made.

pub mod clone;
