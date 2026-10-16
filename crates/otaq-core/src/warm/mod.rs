//! Warm builds (SDD §9.6, d0082): a project's kept build output, `<app-data>/warm/<repo-name>`, that a new worktree
//! is cloned from so its first build only redoes what changed.
//!
//! - [`clone`]: copy-on-write cloning and whether the data folder's disk can clone (WRM-01). A plain copy is never
//!   made.
//! - [`store`]: the warm build folder, its record, the lock that lets a Clear wait for a copy, replace (forward
//!   only), Clear and the size (WRM-02, WRM-05…WRM-07).
//! - [`keep`]: keeping a clean worktree's gitignored files at Done (WRM-03).
//! - [`seed`]: seeding a new worktree at start, with the timestamp trick (WRM-04).

pub mod clone;
pub(crate) mod keep;
pub mod seed;
pub mod store;
