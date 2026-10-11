# 0055. Technical baseline: protocol, data folder, git, crates, service start, logs

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0001, 0002, 0003, 0012, 0018, 0022, 0023, 0027

## Context
Before writing the SDD, several implementation choices that shape many tickets had no decision yet.

## Decision
| Topic | Decision |
|---|---|
| **App ↔ service messages** | JSON messages over the user-locked local socket (0027): requests and replies, plus a live event stream the app subscribes to. Every message carries a protocol version, which drives the "Restart required" check (0022). The `crewboard` CLI uses the same protocol. |
| **Data folder** | `~/.crewboard/` on every platform: the SQLite database, `wt/<repo-name>/<KEY>` worktrees, `logs/`, and the socket (a named pipe on Windows). |
| **Git** | Crewboard runs the user's own `git` command, not a git library, so the user's git config, hooks and credentials apply (0012). |
| **Crates** | `tauri` 2 for the app (0074; it replaced `gpui`, 0001), `ts-rs` (types shared with the UI, 0075), `portable-pty`, `sqlx` (SQLite, 0003), `tokio`, `syntect` (diff highlighting), `tracing` (logs). |
| **Frontend** | React with TypeScript, built with Vite; xterm.js with its WebGL renderer for terminals; Node 24 with npm (0075). |
| **Starting the service** | The app starts `crewboard daemon` when it isn't running. **No auto-start at login.** After a reboot, nothing runs until the user opens Crewboard, which then shows the Interrupted tickets (0018, 0045). |
| **Logs** | `~/.crewboard/logs/`, rotated daily. Settings shows the path (next to the database path, 0003). |

## Changelog
- 2026-10-09: created (SDD preparation, user decision)
- 2026-10-11: `gpui` replaced by `tauri` 2 (0074); `alacritty_terminal` pending the terminal-library follow-up of 0074.
- 2026-10-12: the frontend row and `ts-rs` added; the terminal is xterm.js, so `alacritty_terminal` goes with the GPUI app (0075).
