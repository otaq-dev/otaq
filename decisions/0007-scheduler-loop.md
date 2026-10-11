# 0007. Scheduler loop with a concurrency limit

- **Status:** Accepted
- **Date:** 2026-10-08
- **Related:** 0004, 0005, 0008

## Decision
A scheduler loop runs on every state change, plus a slow tick:
1. Select the tickets in `Queued` whose dependencies are ready (0005).
2. If the optional limits are enabled (0016), respect `max_concurrency` and back-pressure. Otherwise start every
   ready ticket immediately. Ready tickets start in the **start order** of 0065: pinned → priority 1–4 → ready the longest.
3. Starting a ticket means: create the worktree and branch, run the repo's `setup` command, start the
   agent session with `claude` in the background service (0022), and set the ticket to `Agent Working`.

## Defaults (see 0016)
- No limit by default. `max_concurrency` (suggested 5) and back-pressure (suggested 3) are optional settings, off by default.
- The plan-limit hold (0020) always applies.

## Changelog
- 2026-10-08: created
- 2026-10-09: tmux replaced by the background service (0022).
- 2026-10-09: limits made optional and off by default (0016).
- 2026-10-09: added the concurrency defaults and the back-pressure rule (0016).
- 2026-10-09: ready tickets start oldest first; priorities don't exist (SDD review, user decision).
- 2026-10-11: start order is pinned → priority → ready the longest; priorities exist now (0065). Queued is the Ready for Dev column (0066).
