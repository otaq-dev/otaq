# 0022. A background service owns agent sessions (replaces tmux)

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0002, 0008 (superseded), 0009, 0018, 0021
- **Supersedes:** 0008

## Context
0008 used tmux so that sessions survive app restarts and can be attached from any terminal. tmux does not
run natively on Windows, and the user wants Windows supported at launch (0021).

## Decision
- `factory-core` runs as a **background service** (`factory daemon`). It owns every agent terminal (a PTY
  via a cross-platform crate such as `portable-pty`, which uses ConPTY on Windows), plus the scheduler,
  the store, git, Run processes and Shell sessions (0035).
- The **GPUI app is a client**. It connects to the service over a local socket (a Unix domain socket on
  macOS and Linux, a named pipe or AF_UNIX on Windows). Closing the window **does not stop agents**.
  - The app starts the service if it isn't running.
- Each ticket's session runs the real interactive `claude` CLI in its worktree (unchanged from 0008).
  The embedded terminal and the side panel (**Terminal · Diff · Run · Shell · Ticket · History**, 0035, 0036)
  stay as described in 0008.
- **`factory attach <ticket>`** opens a ticket's live session from any terminal, replacing `tmux attach`.
- The `crewboard` agent commands (0031) and the hooks (0009) talk to the same service socket.

## What 0008 content carries over
- Interactive `claude`, one session per ticket, an embedded terminal, and the ability to talk to the agent at any time.

## Consequences
- One code path on all three platforms.
- The service stores scrollback, so the app can redraw a terminal when it reconnects.
- A reboot or a service crash ends sessions. Those tickets become **Interrupted** and wait for Resume (0018).
  An app restart alone no longer interrupts anything.
- **Updating the service (refined, grilling session):** rebuilding Crewboard means the running service is older than the
  app. On connect, the app compares build IDs. If they differ, it shows one banner: **"Service update ready · Restart
  now (3 agents working, 2 waiting)"**, and the user picks the moment. The old service keeps running until then. If the
  socket protocol changed incompatibly, the app shows the same banner as "Restart required" and only that button works.
  A restart is treated like a reboot: Dev tickets become Interrupted, with Resume all (0045). The service never restarts
  itself.

## Alternatives considered
- Terminals inside the app process: rejected, because quitting the app would stop every agent.
- Separate per-session keeper processes, so the service can restart without killing agents: rejected for now (more
  processes and a protocol to keep stable). The user chose the cheaper restart banner. Revisit if daily updates hurt.
- tmux on macOS and Linux, in-app terminals on Windows: rejected, because it means two code paths.

## Changelog
- 2026-10-09: created
- 2026-10-09: added the "Service update ready · Restart now" banner for rebuilds; per-session keepers rejected for now (grilling session, user decision).
