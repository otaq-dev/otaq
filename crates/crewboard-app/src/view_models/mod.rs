//! View models: the UI's logic as plain Rust, without GPUI, so it is unit-tested (SDD §23.1).
//!
//! The pattern: a view model is built from core state (here the link to the service) and exposes exactly what a
//! view draws and which interactions are allowed. Views stay thin: they read a view model and render it.

pub mod add_project;
pub mod agent_activity;
pub mod agent_edits;
pub mod banners;
pub mod board;
pub mod card;
pub mod card_fixtures;
pub mod card_menu;
pub mod chain;
pub mod clock;
pub mod comments;
pub mod connection;
pub mod delete;
pub mod dialog;
pub mod diff;
pub mod drag;
pub mod edit_buffer;
pub mod editor;
pub mod editor_fields;
pub mod epic_actions;
pub mod epic_card;
pub mod fields;
pub mod filter;
pub mod history;
pub mod limits;
pub mod menu;
pub mod onboarding;
pub mod panel;
pub mod panel_actions;
pub mod queue;
pub mod restart;
pub mod send_back;
pub mod settings;
pub mod stack;
pub mod status;
pub mod switcher;
pub mod terminal_strip;
pub mod toasts;
