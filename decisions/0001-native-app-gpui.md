# 0001. Native desktop app, UI built with GPUI

- **Status:** Superseded by 0074
- **Date:** 2026-10-08
- **Related:** 0002, 0008

## Context
The user wants a native app, not a web app, and puts performance first.

## Decision
Software Factory is a native desktop app written fully in Rust. The UI uses **GPUI**, the GPU-rendered
framework behind the Zed editor.

## Alternatives considered
- **Web app (React in a browser):** rejected by the user, who wants a native app.
- **Tauri 2:** a native shell with a Rust backend, but the UI is HTML in a system webview. It is the
  fastest to build, but it is not truly native.
- **SwiftUI with a Rust core over FFI:** the most Mac-like option, but it means two languages and macOS only.

## Consequences
- The kanban drag and drop (and the graph view, if it is ever built; it's "maybe later", 0006) must be built ourselves.
- The terminal uses the `alacritty_terminal` crate for emulation, rendered in GPUI, as Zed does.
- GPUI's API changes often and its documentation is thin, so pin the version and use Zed's source as
  the reference.
- There's no embedded browser. Run's optional Open URL opens in the system browser (0035).

## Changelog
- 2026-10-08: created
- 2026-10-11: superseded by 0074: the UI moves to Tauri before v0.1 (user decision).
