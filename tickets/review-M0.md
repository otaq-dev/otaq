# M0 · Foundations — review

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0001 | Initialize the repository and Cargo workspace | e64ea4b |
| T-0005 | Local check script: fmt, clippy, tests | 570cb68 |
| T-0002 | One name constant for product, command, plugin namespace and env prefix | 6bd822a |
| T-0003 | Headless core baseline: data folder and daily logs | cc27627 |
| T-0004 | SQLite schema and migrations (SDD §8) | 4074099 |

## 2. How to try it

```sh
scripts/check.sh                      # fmt, clippy -D warnings, tests (from any folder in the repo)
cargo build --workspace               # crewboard-core (lib), crewboard-app, crewboard (CLI)
cargo test -p crewboard-core --test store            # schema, migrations, round trip through every table
cargo test -p crewboard-core --test naming_literals  # fails if the name is spelled out outside naming.rs
cargo test -p crewboard-core --test no_ui_dependency # fails if crewboard-core depends on gpui
```

Both binaries still do nothing when run (their work starts in M1). No sample data is needed. Tests use temporary
folders only; `~/.crewboard/` is not created by anything in M0.

## 3. Hand checks

**T-0005**
- [ ] A deliberately unformatted file or a clippy warning makes `scripts/check.sh` fail.
  Steps: append `pub fn   ugly( ) {}` to `crates/crewboard-core/src/lib.rs`, run `scripts/check.sh`, expect
  `check.sh: FAILED at step 'fmt'` and exit code 1. Undo, then append
  `pub fn warn() -> bool { let x = 1; x == x }` (formatted with `cargo fmt`), run again, expect
  `FAILED at step 'clippy'`. Undo.
  The agent ran both cases once; both failed at the right step.

No ticket in M0 has a design frame, so there are no `crewboard.pen` comparisons.

## 4. Implementation choices (SDD §26)

- **Toolchain:** no `rust-toolchain.toml`. Pinning an exact toolchain would make rustup download one, and the runbook
  says not to install anything. The workspace declares `rust-version = "1.99"` (built with stable 1.99.0), edition 2024.
- **Name constant:** `crewboard_core::naming` (product, command, plugin namespace, env prefix, plus `env_var`,
  `ticket_env_var`, `skill`, `branch_prefix`, `branch`, `data_dir_name`). The "no hard-coded name" check is a small
  lexer in a test that looks only inside string literals, so comments and identifiers like `crewboard_core` are fine.
- **Data folder:** database `crewboard.db`, socket `crewboard.sock`, both in `~/.crewboard/` (or `CREWBOARD_HOME`).
  Folders are created with `0700`. Repository names and keys must be a single path component.
- **Logs:** `logs/crewboard.YYYY-MM-DD.log`, rotated daily, no size cap, newest 14 kept; `CREWBOARD_LOG` sets the
  filter (default `info`).
- **No-gpui check:** a test that runs `cargo tree` for `crewboard-core`, not an extra `check.sh` step.
- **Schema (refinements of §8 names and types, same facts):**
  - ids are TEXT UUIDv7 strings; timestamps BIGINT milliseconds; BOOLEAN; DOUBLE PRECISION; JSON as TEXT.
  - `tickets.column` → `board_column` (COLUMN is an SQL keyword).
  - Epic child SHOP-40.1 = epic's number 40 + `child_number` 1 (0 = not a child); `next_child_number` added on
    tickets so child numbers are never reused (d0039).
  - Per-project settings in `projects.settings_json`; global settings as key/value rows.
  - `sessions.project_id` added, because Project terminal sessions have no ticket.
  - `usage` stores cache read and cache write tokens separately; `plan_limits` keeps one row per observation.
  - Postgres deviations: only the connection setup (WAL, `foreign_keys`) and `?` placeholders in query text.
- **New crates:** tokio, tracing, tracing-subscriber, tracing-appender, sqlx, uuid; tempfile (tests only).

## 5. Open questions

None added. One note for you: decision 0003 lists the first tables as `repos, tickets, ticket_deps, sessions,
events, reviews`. The schema follows SDD §8 instead (`projects`, per 0024, plus the other §8 tables). It stores the
same facts, so I treated it as a refinement, not a conflict.
