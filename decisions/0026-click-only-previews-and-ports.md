# 0026. Click-only previews; every ticket gets its own port

- **Status:** Accepted. The browser-preview part is superseded by 0035 (Run + Shell). Per-ticket ports still apply.
- **Date:** 2026-10-09
- **Related:** 0010, 0018, 0022, 0023

## Context
0010 planned a built-in reverse proxy (`ticket-N.localhost:7000`) and previews that start automatically at
Review. Both add moving parts and have the factory act on its own. The real underlying problem is port
collisions: agents running dev servers in parallel all grab the same port (for example 3000) and can end up
testing each other's code.

## Decision
- **A unique port per ticket, in v0.1:** each ticket's agent session gets `$PORT` from a pool, so parallel
  agents never collide.
- **Two ports per ticket (grilling session):** the agent's session gets one `$PORT` (for example 4019), and Run and Shell
  get a second one (for example 4020); inside Run and Shell, `$PORT` means that second port, and Open URL uses it. So a dev
  server the agent left running never blocks the user's Run, and the other way round. Both come from the same pool, both are
  freed at Done, and leaked ones appear in the Cleanup view.
- **Previews run only on click:** Start preview runs the repo's dev command in the ticket's worktree, Open
  opens `localhost:<port>` in the system browser, and Stop stops it. Reaching Done also stops it (bookkeeping, 0018).
- **No reverse proxy, and no automatic start.**

## Consequences
- Browsers share cookies across `localhost` ports, so logging into one ticket's preview can affect another's.
  This is accepted for a single user.

- **Finding the dev command:** the first time the user clicks Start preview in a project, the app asks for
  the command and saves it in that project's settings (0023). The dialog has an **Ask Claude** button: Claude
  looks at the repo and suggests a command, and the user confirms or edits it. There are no detection rules
  to maintain.
- **Per-ticket services** (for example a separate database per ticket): **left out.** The user saw no need
  for them. Previews use whatever the project already runs. This can be revisited if a project ever needs it.

## Changelog
- 2026-10-09: created
- 2026-10-09: Preview generalized to Run (any stack), optional Open URL, and a Shell tab (0035).
- 2026-10-09: dev command is asked on first use, with Ask Claude; services left out.
- 2026-10-09: two ports per ticket, one for the agent and one for Run and Shell (grilling session, user decision).
