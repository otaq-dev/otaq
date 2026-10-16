# 0010. A worktree per ticket, with per-ticket ports (previews became Run and Shell, 0035)

- **Status:** Accepted
- **Date:** 2026-10-08
- **Related:** 0005, 0007

## Decision
- Each ticket gets its own **git worktree and branch**. Worktrees live outside the repo folder, in the app's data directory (0023).
- Each ticket gets a `$PORT` from a pool. ~~Reverse proxy~~ and ~~auto-start at Review~~ were removed in 0026:
  previews run only on click, at `localhost:<port>`, and stop on Stop or at Done.
- **Per-repo settings live in the app** (0023), not in a repo file. They hold `setup`, `dev`, env files to copy, and environment variables (0082). A per-ticket `services` command was considered and dropped (0026).

## Consequences
- Previews share the project's existing databases and services (0026).
- Each worktree runs its own dependency install (a shared cache such as the pnpm store helps), starting from a warm build (0082).

## Changelog
- 2026-10-08: created
- 2026-10-09: proxy and auto-start removed; previews are click-only (0026).
- 2026-10-09: settings moved from `.factory.toml` into the app, and worktrees moved outside the repo (0023).
- 2026-10-09: removed the `test` key, since there is no factory test gate (0018).
- 2026-10-16: kept after research into alternatives; new worktrees start from a warm build, and projects get environment variables (0082).
