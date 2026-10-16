# Prompt: build M18 · Warm builds and environment variables (orchestrator, teammates)

Start **one fresh Claude Code session** in this repository and paste everything below the line. The tickets are already
written and approved (`tickets/m18-tickets.md`, T-0294–T-0303), so there is no planning phase: the session builds them
with teammates, the way M15–M17 were built (decision 0079).

It needs:
- the **Agent** tool (and SendMessage);
- the **pen.dev MCP connection** with `crewboard.pen` open (the UI tickets and hand checks name its frames);
- the **VMPal MCP connection** for T-0299 (Windows Dev Drive) and T-0303 (Windows and Ubuntu checks).

It must be the only session building code in this repository.

**Before you paste it (the user):** commit the M18 planning documents, so `git status` is clean:
`decisions/0082-…`, `SDD.md`, `DESIGN.md`, `design-briefs/15-settings.md`, `tickets/m18-tickets.md`,
`tickets/coverage.md` and this prompt. `~/.claude/settings.json` keeps `{ "subagentPromptCacheTtl": "1h" }`; leave it.

---

You are the **orchestrator for milestone M18 · Warm builds and environment variables** of **Otaq**, a Rust kanban
desktop app (Tauri 2, React, TypeScript) that drives Claude Code agents, one git worktree per ticket. M18 keeps that
worktree per ticket and makes a new one start warm: at Done, a ticket's gitignored files (its build output) are kept as
the project's **warm build**, and the next new worktree clones them copy-on-write, so its first build only redoes what
changed. A per-project switch turns this off, and the Cleanup view lists warm builds with Clear. M18 also adds
per-project **environment variables**, set in everything Otaq starts in the project. The tickets are T-0294–T-0303 in
`tickets/m18-tickets.md`.

**You don't write product code yourself.** You decide the order and what runs in parallel, prepare a worktree for each
teammate, brief it and answer its questions, land finished tickets on `main` one at a time, check each one, relay
blockers to the user, and keep a written record of every choice.

## 1. Read first
1. `CLAUDE.md` and `decisions/README.md`. Then `decisions/0082-warm-seeding-and-environment-variables.md` **with its
   whole changelog** (it overrides anything older that disagrees), and 0007, 0010, 0018, 0021, 0023, 0026, 0035, 0055,
   0056, 0063, 0068, 0079, 0083.
2. `SDD.md` §9.6 (WRM-01…WRM-07) and §9.7 (ENV-01…ENV-03), then §9.1 (SES-01, SES-02), §10.2 (GIT-02), §17 (REC-04,
   REC-05), §20 (SET-01), §23, the M18 row in §24, and §5.5 (the Tauri UI rules: screen logic and wording come from Rust
   view models, ts-rs types).
3. `tickets/m18-tickets.md`, all of it: the intro ("Words used", "Where the code goes", "Shared files", "The example"),
   every ticket, and "Parallel lanes". **This is your plan.**
4. `design-briefs/CHANGES-2026-10-16-r.md`: the frame map (what to build from) and the audit (what was decided in it).
5. The design, read-only: load the tools with ToolSearch (`select:mcp__pencil__get_app_state,mcp__pencil__execute,
   mcp__pencil__read_skill`) and read the "(0082)" rows on "15 · Settings" and "16 · Recovery & cleanup". Never modify
   `crewboard.pen`.
6. How earlier milestones were run: `prompts/m15-markdown-editor.md` (§2–§7 are your procedure), `prompts/m17-epic-lead.md`
   §4 (the lessons), `tickets/review-M16.md` and `tickets/review-M17.md` ("How it was run"), `prompts/implementer.md`.
7. Background for the teammates of T-0295–T-0298: `reports/Parallel agent isolation without worktrees.md` (the
   measurements), if it exists on this machine.
8. The code this builds on: `crates/otaq-core/src/start/` (the start sequence, `worktree.rs`, `setup.rs`),
   `review/finish.rs` (Done), `recovery/cleanup.rs`, `board/settings.rs`, `session/mod.rs` (`Sessions::spawn`,
   `SpawnSpec`), `session/launch.rs`, `terminals/`, `git/mod.rs` (`run_with_env`), `data_dir.rs`; the Settings and Cleanup
   screens in `crates/otaq-desktop/ui/src/screens/settings/` and `recovery/`.
9. `tickets/progress.md` and `tickets/questions.md`. Question numbers are taken from `main` at landing time.

## 2. Before anything else
- **Repository:** exactly as `prompts/m15-markdown-editor.md` §2, for M18 (`m18/*` branches and worktrees, M18 rows in
  `progress.md`). If anything is uncommitted, ask the user whose it is; don't commit, stash or discard it.
- **Machine:** the same (cargo, Node 24 `PATH` prefix passed to every teammate, `check.sh` green on `main`, 15–25 min).
- **Disk:** `df -h /`. Below **60 GB free**, one Rust-building teammate at a time; below **30 GB**, ask the user first.
  Never share one `CARGO_TARGET_DIR` between worktrees. (M18 itself will make this cheaper later; it doesn't help while
  it's being built.)
- **pen.dev:** confirm `crewboard.pen` is open; if not, ask the user.
- **VMs:** check them when T-0299 starts, not now.
- Then tell the user, in three lines, what you'll do first (phase 0: T-0294 and T-0295 side by side).

## 3. Building
Follow `prompts/m15-markdown-editor.md` §3–§7 exactly, replacing M15 with M18, `m15/` with `m18/`, the ticket file with
`tickets/m18-tickets.md`, the review file with `tickets/review-M18.md`, and decisions 0078/0079 with 0082/0079. In short:
- Each ticket gets its own worktree on branch `m18/T-0xxx` in your scratchpad and a fresh teammate (general-purpose, name
  "m18-T-0xxx") with the M15 teammate brief, plus what this section adds.
- A ticket starts when its Depends on tickets have landed on `main`. Parallel only when Shared files don't overlap, at most
  3 teammates. **T-0299 runs alone** (it changes `Cargo.toml`).
- Landing: rebase onto `main`, one squashed commit per ticket (naming the ticket, its requirement IDs and decisions
  0082 / 0079), `check.sh` green on exactly that tree (you run it in the foreground and **read its exit code**), then
  `git merge --ff-only`. If `main` moved, rebase and re-check. One landing at a time.
- Write every parallel/sequence choice, landing, rebase and re-check down for "How it was run".
- Only blockers stop the work. Everything else is deferred to `tickets/questions.md` as "Deferred: built as option N".
  Ping the user only for blockers; no progress summaries.

**Put these in every teammate's brief** (on top of the M15 brief):
- **Stack-agnostic, always.** Otaq must not know any language or build tool. No code path, test, UI string or doc in M18
  may name cargo, npm, Python or any other tool, except the test projects and measurements the tickets already name.
  Seeding treats every gitignored file the same. Tests use a fake "build" (a script that writes ignored files), never a
  real build tool. No placeholder or computed variables, no size cap, no plain-copy fallback.
- **Tests and the real data folder:** `OTAQ_HOME` in a temporary folder, never `~/.otaq/`. Clone tests skip with a printed
  note where the temporary folder's disk can't clone, so `check.sh` passes on ext4 and NTFS.
- **The shared registration files** of the ticket file's "Shared files" section, named in the brief, including the two
  `ProjectSettings` fields (T-0294, T-0296) and the `warm/mod.rs` lines.
- The ticket's "Done when" boxes are the checklist; hand checks are prepared, never ticked.

**Points to watch in M18:**
- **T-0294 is wide** (sessions, the setup command, and the hook-running git callers). Land it before T-0297 and T-0298
  start: they add calls to `review/finish.rs`, `start/worktree.rs`, `start/sequence.rs` and `start/setup.rs`, which
  T-0294 touches first.
- **The T-0298 measurement** (a real `cargo build` of this repository after a seeded start) is the one place a real build
  tool runs, by hand, never in tests. It needs a lot of disk: do it only with **60 GB free or more**, after the other
  Rust-building teammates are done; otherwise record it in `review-M18.md` as "not measured" with the reason. Also time a
  seeded vs a cold start of the desktop app's `node_modules`, since a different kind of project shows whether the result
  holds beyond one stack (reference: one folder clone took 0.19 s for 231 MB / 18,000 files).
- **T-0299** needs a Dev Drive (ReFS) in the Windows VM. Teammates may create one inside the VM. If block cloning needs
  more than one `windows-sys` feature or a new crate, it's a blocker for the user, not a workaround.
- **Run a Windows `check.ps1` part-way**, after phase 2 lands, not only in T-0303. In M16 this caught a stack overflow that
  macOS didn't. New request arms in `board_requests::handle` use `boxed(move || async move {..})` (the 2 MB debug
  stack).
- Lessons from M14–M17: put everything a teammate needs in its **brief** (a message sent while it works may only reach it
  when its turn ends); trust state, not words (check worktrees, processes and exit codes yourself); ask for reports under
  600 words.

## 4. Rules that always apply
- Decisions are the source of truth. A conflict is raised to the user, quoted (decision number and text), with options,
  a concrete `shop-web` example and your recommendation, one topic at a time, in plain language. Never resolve one
  silently.
- Every commit on `main` lands by fast-forward after a green `check.sh` on exactly that tree (0056, 0079).
- Never push, open a PR, rewrite `main`'s history, change git config or Claude Code settings, or modify `crewboard.pen`.
- Don't edit `decisions/`, `SDD.md`, `DESIGN.md`, `design-briefs/` or `CLAUDE.md`, except to record an answer the user
  confirmed (`decisions/README.md` rules), in its own checked commit.
- Never start a real `claude` in tests. Never touch the user's `~/.otaq/` or their warm builds.

## 5. When M18 is done
Everything in `prompts/m15-markdown-editor.md` §7, for M18: every ticket landed with a progress row, `review-M18.md` with
"How it was run", "Platforms checked" and the measured times, `check.sh` green on `main`, clean status, no `m18/*`
worktrees or branches, no test-started `otaq` process, no test warm builds left outside temporary folders. Then give the
user the ticket → commit table, how it was run in a few lines, the measured times, the unticked hand checks by screen,
the deferred questions, and the platforms not checked.
