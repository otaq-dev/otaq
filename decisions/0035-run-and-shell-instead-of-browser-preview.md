# 0035. "Run" and "Shell" replace the browser-only Preview

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0010, 0022, 0023, 0026 (preview part superseded)
- **Supersedes:** the browser-preview part of 0026

## Context
0026's Preview assumed every project is a web app ("Open opens `localhost:<port>` in the browser"). Users also
build backends, CLIs, libraries and mobile apps, which can't be checked in a browser. The user asked for something
that works for any kind of project.

## Decision
- **"Preview" is renamed "Run".**
  - Each project has a **run command** (for example `pnpm dev`, `dotnet run`, `cargo run -- serve`, `python -m app`).
  - **Run** starts it in the ticket's worktree with the ticket's own Run port as `$PORT` (0026: separate from the agent's port). It is click-only, has Stop, and
    stops at Done.
  - The **Run tab** shows the live output log, which works for any stack.
  - On first use, the app asks for the run command (with **Ask Claude** to suggest one) and saves it in project settings (0023).
- **"Open URL" is optional per project.** It is shown only if the project sets a URL (for example
  `http://localhost:$PORT` or `http://localhost:$PORT/swagger`). There is no browser assumption otherwise.
- **Shell tab:** a plain terminal (not Claude) in the ticket's worktree, with `$PORT` and the ticket's env set. The user
  can curl an API, run tests, run a CLI with arguments, start a simulator, and so on. It is owned by the background
  service like agent sessions (0022), so it survives closing the window, and it closes when the ticket reaches Done.
- **Not adopted for now:** "Open in editor".
- Because "Run" and "Runs" would clash, the session-history tab is renamed **History**.
- Side panel tabs are now: **Terminal · Diff · Run · Shell · Ticket · History** (Investigation removed by 0036).

## Changelog
- 2026-10-09: created (design review, user decision)
- 2026-10-09: Run and Shell use their own port, separate from the agent's (0026).
