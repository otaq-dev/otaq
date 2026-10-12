//! Crewboard's screen logic as plain Rust, without GPUI or Tauri (d0075, SDD §5.5): the view models with the theme
//! data, the keyboard map, the icon names and the diff highlighting they read. The desktop apps draw what these
//! say; the tests here run without a display (SDD §23.1).

pub mod highlight;
pub mod icons;
pub mod keymap;
pub mod shell;
pub mod terminal;
pub mod theme;
pub mod view_models;
