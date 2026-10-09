# 0003. SQLite storage; beads only as an import adapter

- **Status:** Accepted
- **Date:** 2026-10-08
- **Related:** 0002

## Context
A local-first native app needs storage that requires no setup. The app has to store tickets,
dependencies, sessions, events and reviews.

## Decision
SQLite through `sqlx`, written so a move to Postgres stays possible for a future multi-user version.
Initial tables: `repos`, `tickets`, `ticket_deps`, `sessions`, `events`, `reviews`.

## Alternatives considered
- **Postgres:** setup overhead that a local single-user app doesn't need.
- **beads:** git-native issue tracking for agents, with dependencies built in. But it stores issues
  per repo and is built as agent memory. It does not model sessions, runs, ports or review states.
  It is kept as a candidate **import/sync adapter**.

## Consequences
- The app ships as a single file with no database server to run.
- **Backup (v0.1):** tickets exist only in this database (0023 means there's no copy in repos). v0.1 relies on the
  user's normal machine backup. The Settings screen shows where the database file lives. Export/Import of a
  project goes on the later list, next to importers.
- **Teams and importers:** nothing to decide now. Teams are out of scope (0014), and importers come later.

## Changelog
- 2026-10-08: created
- 2026-10-09: backup approach and team scope confirmed (Topic 13).
