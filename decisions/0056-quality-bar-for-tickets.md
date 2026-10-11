# 0056. Quality bar: tests, CI and definition of done for every ticket

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0002, 0018, 0021, 0027, 0053, 0062

## Context
The SDD becomes tickets implemented by agents, outside Crewboard for all of v0.1 (0062). Every ticket needs the same
clear bar.

## Decision
- **Tests:**
  - The core is tested without a UI (0002): unit tests for state rules (readiness, sort order, signal mapping, permission
    scope) and **git tests against temporary repos** (worktrees, Local merge, checkpoint commit, rebase conflicts).
  - **No fake `claude`.** Hook handling is tested by feeding `crewboard hook <event>` the hook JSON directly, and PTY and
    session plumbing is tested with an ordinary command in place of `claude`. Behaviour with the real `claude` is checked by
    hand.
  - The UI keeps its logic in testable view models. Screens are checked by hand against the pen.dev frames, and each UI ticket
    names the frame it implements.
- **Checks ("CI"):** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, run through **one local script**,
  `scripts/check.sh`. The repository has no remote yet, so there are no GitHub Actions for now: "CI is green" means the script
  passes on the developer's machine **before every commit**. macOS first. Windows and Linux are checked at the very end of v0.1
  (M11) by running the same checks in local virtual machines. GitHub Actions are added later, once the repository has a remote.
- **Frontend checks (from M14, 0075):** the same script also checks the Node version, runs `npm ci`, checks that the
  `ts-rs` types are up to date, and runs `tsc --noEmit`, ESLint (`--max-warnings 0`), the Prettier check and Vitest. React
  keeps only interaction state; the screen logic stays in tested Rust view models.
- **Definition of done for every ticket:**
  - The checks are green (`scripts/check.sh`), and new behavior has tests.
  - The ticket's "Done when" items are met.
  - The commit names the decisions it implements (for example "implements 0049").
  - If the work reveals a conflict with a decision, the agent stops and asks; it doesn't decide alone.

## Alternatives considered
- A fake `claude` test program that simulates sessions and hooks: rejected by the user.

## Changelog
- 2026-10-09: created (SDD preparation, user decision)
- 2026-10-09: no GitHub Actions while the repo has no remote: the checks run locally through `scripts/check.sh` before every
  commit; Windows and Linux are checked in local VMs as the last step of v0.1 (M11); GitHub Actions once a remote exists (gap
  review, user decision).
- 2026-10-12: frontend checks added to `scripts/check.sh` for the Tauri UI (0075).
