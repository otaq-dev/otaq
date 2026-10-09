# v0.1 progress

| Ticket | Title | Milestone | Commit | Hand checks pending | Notes |
|---|---|---|---|---|---|
| T-0001 | Initialize the repository and Cargo workspace | M0 | deddef3 | none | gpui not added yet; no rust-toolchain.toml, workspace rust-version = 1.99 |
| T-0005 | Local check script: fmt, clippy, tests | M0 | 6bee9b9 | "unformatted file / clippy warning makes it fail" (agent ran both once; both failed as expected) | no .github/workflows |
| T-0002 | One name constant for product, command, plugin namespace and env prefix | M0 | f151604 | none | literal guard is tests/naming_literals.rs |
| T-0003 | Headless core baseline: data folder and daily logs | M0 | e71c2f0 | none | crewboard.db, crewboard.sock, daily logs keep 14 |
| T-0004 | SQLite schema and migrations (SDD §8) | M0 | def36e2 | none | TEXT UUIDv7 ids, BIGINT ms timestamps, board_column; see commit for §8 refinements |
