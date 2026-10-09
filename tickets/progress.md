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
| T-0009 | Socket locked to the current OS user (macOS) | M1 | a9c7058 | none | folder 0700 + socket 0600 at every start; peer UID check (rustix getuid) |
| T-0010 | Event stream subscription | M1 | c67e1e8 | none | events.subscribe; laggard = full queue (256) or missed bus events (1024) → disconnected |
| T-0011 | Client library: connect, start the service, reconnect | M1 | 57c6f0e | none | backoff 100 ms → 2 s; launcher runs `crewboard daemon start` next to the program |
| T-0012 | Build-ID / protocol check and user-clicked service restart (core) | M1 | d55b058 | none | any other build ID = ServiceOlder; restart waits for the old lock file to vanish |
| T-0013 | Tools checklist and permission-mode check (core) | M1 | 6723bb8 | none | tools.check; defaultMode from managed then user settings; no mode set → unknown |
