# v0.1 progress

| Ticket | Title | Milestone | Commit | Hand checks pending | Notes |
|---|---|---|---|---|---|
| T-0001 | Initialize the repository and Cargo workspace | M0 | deddef3 | none | gpui not added yet; no rust-toolchain.toml, workspace rust-version = 1.99 |
| T-0005 | Local check script: fmt, clippy, tests | M0 | 6bee9b9 | "unformatted file / clippy warning makes it fail" (agent ran both once; both failed as expected) | no .github/workflows |
| T-0002 | One name constant for product, command, plugin namespace and env prefix | M0 | f151604 | none | literal guard is tests/naming_literals.rs |
| T-0003 | Headless core baseline: data folder and daily logs | M0 | e71c2f0 | none | crewboard.db, crewboard.sock, daily logs keep 14 |
| T-0004 | SQLite schema and migrations (SDD §8) | M0 | def36e2 | none | TEXT UUIDv7 ids, BIGINT ms timestamps, board_column; see commit for §8 refinements |
| T-0006 | Protocol: versioned JSON messages, events and build ID | M1 | 83513ab | none | newline-delimited JSON; build ID = git hash[-dirty].build secs; ping/service.status/service.stop version-independent |
| T-0007 | CLI skeleton and global conventions | M1 | 267912f | none | clap; JSON errors on stdout; app = crewboard-app next to the CLI binary |
| T-0008 | `crewboard daemon start / stop / status` | M1 | 9406532 | none | flock on service.lock (PID inside); start runs hidden `daemon run` detached |
