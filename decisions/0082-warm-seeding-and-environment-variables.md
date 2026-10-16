# 0082. Worktrees stay; a new worktree starts from a warm build; per-project environment variables

- **Status:** Accepted
- **Date:** 2026-10-16
- **Related:** 0007, 0010, 0016, 0023, 0035, 0045

## Context
A worktree per ticket (0010) is cheap in itself: on this repo the source is 31 MB. The cost is build output: `target/` is
36 GB and `node_modules` 192 MB, and each new worktree builds and installs from nothing. With 10–15 tickets waiting in
Review for days, disk and setup time add up. Separately, several agents building at once each take every core, which
can push the machine into swap and make builds and tests slow and flaky.

The user asked for research before deciding (2026-10-16): how other parallel-agent tools do it, GitButler, jj, a shared
checkout, copy-on-write, overlays, containers, shared caches, and measurements on this repo. Every local tool found
(Conductor, Cursor, Codex, Claude Code, Vibe Kanban, herdr-projects…) uses a worktree per task. herdr-projects
(eliasstravik/herdr-projects, MIT) adds, in its own repo's setup, a warm build seed and per-agent CPU sharing.

## Decision
- **A worktree per ticket stays** (0010), following what the other tools do.
- **Warm seeding:** a new worktree starts from an existing build instead of from nothing, so the first build only redoes
  what differs. As in herdr-projects: copy-on-write where the file system can (APFS, btrfs, XFS), and the files that are
  unchanged since the seed's commit are given an old timestamp so build tools take them as built.
  Measured on this repo: cloning the 36 GB `target/` took ~30 s and ~49 MB of real disk; the first `cargo build` after it
  took ~26 s, with all 302 dependencies kept and only the 4 workspace crates rebuilt.
- **The seed is a kept build per project, refreshed at Done.** When a ticket reaches Done, Otaq moves (renames, no copy)
  its worktree's gitignored files into the project's warm-build folder, `<app-data>/warm/<repo-name>` (next to `wt/`), with the commit they
  were built at, before it removes the worktree. Forward only: an older build never replaces a newer one. New worktrees
  clone from it. With no warm build yet, a new worktree seeds from the user's own checkout if it has gitignored files, or
  builds from nothing. All gitignored files are seeded, not chosen folders, since Otaq doesn't know the stack.
  herdr-projects keeps one per repo too, but refreshes it when a fresh worktree's setup has built main (its setup runs
  `cargo build`; Otaq's setup doesn't build, so Done is the moment its build is closest to main), and seeds only Rust's
  `target/quick`.

- **No copy-on-write, no seeding.** On a file system that can't clone (NTFS on most Windows disks, ext4, exFAT), a new
  worktree starts as before (setup, then a build from nothing), and Done deletes as before with no warm build kept. A
  plain copy would trade tens of GB of disk for time. herdr-projects falls back to a plain copy, kept small by its 2 GB
  cap, and runs only on macOS and Linux. Suggesting a Windows Dev Drive is left to the docs.

- **A per-project toggle turns seeding off:** "Start new worktrees from a warm build", on by default, in project
  settings only (not onboarding). Off: new worktrees start as before and Done deletes as before. It is for a clone that
  breaks something (a Python `.venv` holds its old folder's path). A list of folders not to seed was considered and left
  for later, if real cases show the toggle is too coarse. herdr-projects has no switch in the tool; its repo script is
  turned off by `HERDR_PROJECTS_NO_WARM=1` or the user's own `CARGO_TARGET_DIR`.
- **Per-project environment variables, and no CPU sharing built into Otaq.** A project setting lists variables
  (`NAME = value`) that Otaq sets in everything it starts in that project: ticket agents, the validator (0063),
  Project agents (0068), the Project terminal, the size check (0081), the setup command (0007), Run and Shell, and the
  git commands Otaq runs in the project's repo and worktrees (so the user's git hooks see them). It is empty by default, so
  onboarding doesn't change. Otaq sets no tool's variables by itself and names no language or build tool, and it offers
  no placeholder of its own (such as a computed core share): a user who wants parallel agents to share the cores writes
  it for their stack (for Rust, `CARGO_BUILD_JOBS = 3`). The setting's hint uses an example that names no language or
  tool: `CI = true` ("so tools never wait for input or start watch mode"). Considered and rejected: Otaq setting known build-tool
  variables to cores ÷ agents, a line in the agent's instructions about its share, operating-system CPU caps (none on
  macOS), and an `$OTAQ_CPUS` placeholder. herdr-projects shares the cores in its own repo's scripts
  (`CARGO_BUILD_JOBS`, `RUST_TEST_THREADS`), not in the tool.

- **No automatic size limit; the warm build shows in the Cleanup view.** Build tools rarely delete old files (cargo
  never does), so a warm build handed down from Done to Done grows, in disk and in clone time. The Cleanup view (0018)
  lists each project's warm build with its size and a **Clear** button; turning the toggle off clears it too. A fixed
  cap, or a reset by age, needs a number Otaq can't know for an unknown stack; one can be added later from real numbers.
  herdr-projects caps its kept build at 2 GB: a bigger one isn't used, and the build made from nothing replaces it.

## Alternatives considered
- **A small pool of reusable "slots"** (a ticket owns a branch and borrows a warm folder): disk depends on the slot count,
  not the ticket count. Rejected by the user in favour of following the other tools; tickets move between folders, and it
  adds moving parts.
- **GitButler parallel branches / a shared checkout:** one folder can't keep 15 branches each runnable alone; agents
  overwrite each other. GitButler's license also has a non-compete clause.
- **jj workspaces:** same disk cost as worktrees.
- **Overlays, file-system snapshots, containers:** need drivers, admin rights or a VM; containers can't build a desktop app.
- **Symlinked shared folders:** tickets would change each other's dependencies.
- **Freeing build output of Review tickets automatically:** Otaq can't tell build output from user files without knowing
  the stack.

## Consequences
- New tickets start building in seconds instead of minutes on macOS and Linux btrfs/XFS.
- Each worktree still keeps its own build output; 10–15 Review tickets still cost real disk (about 2–11 GB each on this
  repo). The Cleanup view (0018) remains the way to reclaim it.
- Parallel builds still each use every core unless the project's environment variables say otherwise.

## Changelog
- 2026-10-16: created (Proposed); research in `reports/Parallel agent isolation without worktrees.md`.
- 2026-10-16: all details agreed (seed kept at Done, no seeding without copy-on-write, toggle, environment
  variables instead of CPU sharing, no size limit); Accepted.
- 2026-10-16: design audit of update R: the settings hint's example is `CI = true`, not `CARGO_BUILD_JOBS = 3`.
- 2026-10-16: design audit: a variable change applies to sessions, Runs and Shells started or resumed after it; running
  ones keep the old values, and there is no restart offer.
- 2026-10-16: design audit: "every session" means everything Otaq starts in the project, validator, Project agents,
  Project terminal and size check included.
- 2026-10-16: design audit: a duplicate name is marked and not applied (the first row applies; on Windows,
  names differing only in case count as duplicates); an invalid name is marked
  only when it would break the start (empty, or containing `=` or a null character).
- 2026-10-16: design audit: values are stored and shown in plain text (no masking or keychain), like the env files.
- 2026-10-16: design audit: availability is decided by the disk under Otaq's data folder; a first seed from a checkout
  on another disk is skipped, not copied.
- 2026-10-16: design audit: the warm build's folder is `<app-data>/warm/<repo-name>`.
- 2026-10-16: design audit: Clear (or turning the switch off) while a new worktree is being seeded waits for that copy,
  then clears.
- 2026-10-16: design audit: confirmations say "up to <size>" (clones share space), and the toast gives no number.
- 2026-10-16: design audit done: the switch asks only when turning it off would delete a warm build.
- 2026-10-16: ticket writing (M18): the setup command gets the project variables too (it runs in the project, e.g.
  `NODE_OPTIONS` for an install, or a core share for a setup that builds; herdr-projects' setup loads its core share).
- 2026-10-16: ticket writing (M18): the git commands Otaq runs in the project's repo and worktrees get them too (the
  checkpoint commit, rebases and merges run the user's git hooks). Otaq's own variables (`OTAQ_*`, `PORT`) win over a
  row of the same name, and a `PATH` row keeps Otaq's bin folder in front. Block cloning on a Windows Dev Drive (ReFS)
  is built in M18 (T-0299), so the Dev Drive note in the docs works.
