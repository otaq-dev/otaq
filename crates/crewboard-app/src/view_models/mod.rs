//! View models: the UI's logic as plain Rust, without GPUI, so it is unit-tested (SDD §23.1).
//!
//! The pattern: a view model is built from core state (here the link to the service) and exposes exactly what a
//! view draws and which interactions are allowed. Views stay thin: they read a view model and render it.

pub mod banners;
pub mod connection;
pub mod dialog;
pub mod edit_buffer;
pub mod fields;
pub mod menu;
pub mod onboarding;
pub mod restart;
pub mod status;
pub mod toasts;
