# 0023. Zero-config start; the factory never adds files to user repos

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0009, 0010, 0012, 0021

## Context
A new repo needs some setup (install dependencies, copy `.env.local`), or the agent wastes its first
minutes on it. The earlier plan put this in a committed `.factory.toml`. The user does not want the factory
adding any file to their repos.

## Decision
- **The factory never adds files to a user's repo.** There is no `.factory.toml` or any other config file.
- **Zero-config start:** add a repo and drag a ticket to Dev, and you get a worktree plus `claude`, nothing more.
  The docs show example settings.
- **All per-repo settings live in the app** (a per-repo settings screen, stored in the factory's SQLite):
  setup command, dev command, env files to copy into worktrees, services command, and similar.
- **Worktrees are created outside the repo folder**, in the factory's own data directory (for example
  `<app-data>/wt/<repo-name>/<KEY-N>`, for example `~/.crewboard/wt/shop-web/SHOP-18` (0029)), so nothing new appears in the repo directory.
- The plugin and hooks are loaded with `--plugin-dir` at launch, never installed into the repo (0009).
- **No auto-detection** of project commands. It would mean maintaining rules for every ecosystem forever.

## Alternatives considered
- A committed `.factory.toml` with in-app settings as a fallback: rejected by the user.
- Auto-detecting commands from lockfiles: rejected for maintenance cost.

## Changelog
- 2026-10-09: created
