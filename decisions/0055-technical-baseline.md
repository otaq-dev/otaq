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
| **Crates** | `gpui` pinned to one Zed commit (0001), `alacritty_terminal`, `portable-pty`, `sqlx` (SQLite, 0003), `tokio`, `syntect` (diff highlighting), `tracing` (logs). |
| **Starting the service** | The app starts `crewboard daemon` when it isn't running. **No auto-start at login.** After a reboot, nothing runs until the user opens Crewboard, which then shows the Interrupted tickets (0018, 0045). |
| **Logs** | `~/.crewboard/logs/`, rotated daily. Settings shows the path (next to the database path, 0003). |

## Changelog
- 2026-10-09: created (SDD preparation, user decision)
