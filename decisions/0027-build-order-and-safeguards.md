# 0027. Build order, "last signal" indicator, socket locked to the user

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0013, 0015, 0016, 0021, 0022

## Context
Product-owner wrap-up after the full design review. v0.1 is large, there are no claude version checks
(0021), and the background service's socket can move tickets and type into agent sessions (0022).

## Decision
1. **Build order (scope unchanged):** build **macOS first** and **Local merge first**, so daily use starts as
   early as possible. Windows, Linux and the PR providers (GitHub, Azure DevOps) follow, all still part of
   v0.1 and all supported before any public release.
2. **"Last signal" line on every working card**, for example "agent last active 3 min ago", based on the most
   recent hook event or `crewboard` agent command. A Claude CLI change that silently breaks the signals shows up as an
   obviously stale card rather than a card that looks busy.
3. **The service socket is restricted to the current OS user** (file permissions on the Unix socket, and an
   owner-only ACL on the Windows named pipe). No other account or process running as another user can drive
   agents.

## Advice noted (not decisions)
- Keep the background service small and boring, because it is now the component whose crash interrupts every agent.
- With no factory test step (0018), the user's review is the only quality check. Consider adding "run the
  tests" to the user's own work-ticket skill early.

## Changelog
- 2026-10-09: created
- 2026-10-09: "last signal" counts background activity (0042).
- 2026-10-09: build order detailed as milestones M0–M11 in 0053.
