# 0002. UI-agnostic Rust core, Cargo workspace layout

- **Status:** Accepted
- **Date:** 2026-10-08
- **Related:** 0001, 0009

## Context
The engine (scheduler, git, sessions) should not be tied to a particular UI framework.

## Decision
A Cargo workspace:
- `crates/factory-core`: domain, storage, scheduler, git/worktrees, PTY sessions, port pool, Run and Shell processes (0035; there is no preview proxy),
  and a local socket server. Runs as a **background service** (0022). Uses tokio. No UI dependencies.
- `crates/factory-app`: the GPUI app (board, Stack list, terminal view, diff view; graph view maybe later, 0006).
- `crates/factory-cli`: the `factory` binary. It launches the app and provides the `factory daemon`, `factory attach <ticket>`,
  `factory hook <event>` subcommands, plus the agent-facing commands from 0031 (no MCP server).
- `plugin/`: the Claude Code plugin (see 0009).

## Alternatives considered
- One crate with UI and engine mixed together. Rejected because the UI could not be swapped and the
  engine would be hard to test.

## Consequences
- The core can be tested headless, and the UI could be replaced later.

## Changelog
- 2026-10-08: created
- 2026-10-09: the core runs as a background service; added the daemon and attach subcommands (0022).
