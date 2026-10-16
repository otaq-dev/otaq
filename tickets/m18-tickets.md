# M18 · Warm builds and environment variables — tickets

M18 keeps a worktree per ticket and makes a new one start warm. At Done, a ticket's build output (its gitignored files)
is kept as the project's **warm build**. The next new worktree clones it copy-on-write, so its first build only redoes
what changed. A per-project switch turns this off. M18 also adds per-project **environment variables**, set in everything
Otaq starts in the project. The sources are:
- `decisions/0082` (with its changelog it overrides anything older that disagrees) and `decisions/0079` (how the
  milestone is built: an orchestrator, teammates, one worktree and one squashed commit per ticket);
- `SDD.md` §9.6 WRM-01…WRM-07 and §9.7 ENV-01…ENV-03, with §9.1 (SES-01, SES-02), §17 (REC-04, REC-05), §20 (SET-01)
  and §23;
- the design, update R in `crewboard.pen`. The frame map and the audit are in `design-briefs/CHANGES-2026-10-16-r.md`.
- Background: the research and measurements in `reports/Parallel agent isolation without worktrees.md`, and
  herdr-projects' `scripts/warm-start.sh` (eliasstravik/herdr-projects, MIT), which does the same by hand for one repo.

**10 tickets, T-0294–T-0303, one epic: M18 · Warm builds and environment variables.**

- **Format:** the same as `tickets/m17-tickets.md` (the shape for `otaq create -f`, 0032). The IDs continue that file's.
- **Definition of done** (0056): `scripts/check.sh` is green, new behaviour has tests, the "Done when" items are met, and
  the commit names the ticket, its requirement IDs and decisions 0082 / 0079. **A conflict with a decision → defer or stop
  as `prompts/implementer.md` says.**
- **Hand checks** compare the running app with the named frames. Agents never tick them. Run the app with
  `OTAQ_HOME=/tmp/m18 npm run tauri dev` in `crates/otaq-desktop`, in Otaq Light and Otaq Dark, at 1440 × 900 and
  1280 × 800. Use the test projects in § "The example" below. `/tmp` is on the system disk (APFS on macOS), so cloning
  works there.
- **Build only what 0082, WRM-01…07 and ENV-01…03 say.** Out of scope:
  - a list of folders to leave out of seeding;
  - a size limit, an automatic reset or an age limit for warm builds;
  - CPU sharing of any kind, `$OTAQ_CPUS` or any other placeholder or computed variable;
  - any variable Otaq sets for a tool or language by itself, and any name of a language or build tool in the UI;
  - a plain-copy fallback where the disk can't clone;
  - masking values or keeping them in a keychain;
  - a "started from a warm build" message on cards, panels or History (not designed);
  - warm builds in onboarding.
- **Stack-agnostic.** Nothing in the code may know about cargo, npm, Python or any other tool. Seeding treats every
  gitignored file the same. Tests may use a fake "build" (a script that writes files), never a real build tool.
- **Build from the frame map only.** The older Settings and Cleanup frames (without "(0082)") are background. Where a
  frame and 0082 differ, 0082 wins. Sizes, SHAs and ages in the frames are samples.

## Words used in these tickets
- **Warm build:** a project's kept build, the folder `<app-data>/warm/<repo-name>` (e.g. `~/.otaq/warm/shop-web`). It
  holds a worktree's gitignored files, laid out as in the worktree, and a small record (T-0296):
  - **built at:** the worktree's `HEAD` when it was kept;
  - **base:** `git merge-base HEAD <the project's base branch>` at that moment, used for "forward only";
  - **from:** the ticket key (`SHOP-21`), and **kept at:** the time.
- **Seed:** the gitignored files a new worktree is cloned from: the warm build, or, when there is none, the user's own
  checkout.
- **Clone:** a copy-on-write copy (APFS `clonefile`, Linux `FICLONE`, Windows block cloning). It shares the disk blocks
  until one side writes. It keeps each file's modified time. **A plain copy is never made** (WRM-01).
- **Clone-capable:** the disk under Otaq's data folder can clone (T-0295). This alone decides whether warm builds are
  available. A checkout on another disk simply can't be cloned, so the first seed from it is skipped.
- **The switch:** the per-project setting "Start new worktrees from a warm build" (T-0296), on by default.
- **Warm builds on** for a project: the switch is on **and** the data folder is clone-capable.
- **The timestamp trick:** after cloning, every tracked file of the new worktree gets the modified time it had where the
  build was made (the kept worktree's times, recorded at Done; or the checkout's), then the files that differ from the
  seed's commit get the current time. Build tools compare source times with output times, so they rebuild only what
  differs, and a file committed after the last build still looks newer than its output. (Refined by Q71: the first
  version used a fixed old time, 2000-01-01, as herdr-projects does with `touch`; that hands on a stale build.)
- **Project variables:** the per-project `NAME = value` rows (ENV-01).
- **Otaq's own variables:** what Otaq sets for itself today: the data folder variable, `OTAQ_TICKET`, the role and epic
  variables, `PORT`, `TERM` and `COLORTERM`, and the Otaq bin folder in `PATH` (`naming.rs`, `terminals/mod.rs`,
  `session/launch.rs`).

## Where the code goes
This list is what makes parallel work possible, so keep to it. If a ticket really needs another place, name it in its
commit and in `tickets/review-M18.md`.
- **Core** (`crates/otaq-core`), tested without a UI (SDD §23.1):
  - `src/board/settings.rs`: the two new fields of `ProjectSettings` and their problems (T-0294 adds `env_vars`, T-0296
    adds `warm_builds`). No migration: project settings are `projects.settings_json`, and a missing field reads as the
    default.
  - `src/session/env.rs` (new): building a session's environment from the project variables and Otaq's own (T-0294).
    `src/session/mod.rs` gets one call in `spawn`.
  - `src/warm/` (new):
    - `clone.rs`: cloning and the clone-capable probe (T-0295; T-0299 adds the Windows part);
    - `store.rs`: the warm build folder, its record, the lock, replace, clear and size (T-0296);
    - `keep.rs`: keeping at Done (T-0297);
    - `seed.rs`: seeding at start and the timestamp trick (T-0298);
    - `mod.rs`: T-0295 creates it; later tickets add one `mod` line each.
  - `src/data_dir.rs`: `warm_dir()` and `warm_path(repo_name)` (T-0296).
  - `src/review/finish.rs`: one call before `remove_worktree` (T-0297).
  - `src/start/sequence.rs`: one call after step 3 (T-0298). `src/start/setup.rs` only if T-0298 needs the env-file list.
  - `src/recovery/cleanup.rs`: the warm-build section of the scan (T-0302). `src/recovery/mod.rs`: removing interrupted
    `.incoming` / `.old` folders at service start (T-0296).
- **View models** (`crates/otaq-view/src/view_models/`): every wording and every rule the screen shows (SDD §5.5).
  - `settings_page.rs`, `settings.rs`: the Environment variables row (T-0300) and the switch row (T-0301).
  - `cleanup.rs`: the warm-build section (T-0302).
- **Desktop** (`crates/otaq-desktop`):
  - `src/api/settings.rs`, `ui/src/screens/settings/` (`Rows.tsx`, `Dialogs.tsx`, `settings.css`): T-0300, then T-0301.
  - `src/api/recovery.rs`, `ui/src/screens/recovery/` (`Cleanup.tsx`, `logic.ts`, `recovery.css`): T-0302.
- **Docs:** `scripts/README.md` or the user docs, wherever setup is described today: the Windows Dev Drive note (T-0303).
- **No new npm package or crate.** Cloning uses `libc` (macOS, Linux) and `windows-sys` (Windows), which the core already
  has. T-0299 may add **one `windows-sys` feature** (`Win32_System_Ioctl`), so it changes `Cargo.toml` and runs alone.

## Shared files
A ticket may add **one entry** to each of these registration files. Conflicts there are one-line merges that keep both
sides:
- `crates/otaq-core/src/board/settings.rs` `ProjectSettings`: one field and its default per ticket (T-0294, T-0296);
- `crates/otaq-core/src/protocol.rs`: one `Request` variant and its name per request, its arm in
  `service/permissions.rs` (an exhaustive match), its dispatch, and its row in `tests/every_request.rs`;
- `crates/otaq-core/src/lib.rs` (`pub mod warm;`, T-0295 only);
- `crates/otaq-desktop/src/generated.rs` (`export_all`, one line per new type) and the files it writes in
  `ui/src/generated/`;
- `crates/otaq-view/src/icons.rs`, its SVG assets and the generated `IconName.ts`. New icon: the flame of the warm-build
  section (T-0302). Check what exists first;
- `crates/otaq-desktop/src/api/mod.rs` (one area or command line);
- `crates/otaq-desktop/ui/src/screens/components/Gallery.tsx` (one section per ticket).

Bookkeeping (`tickets/progress.md`, the ticked boxes here, `tickets/questions.md`) is added on the rebased branch just before
the check (0079). **Question numbers are taken from `main` at landing time**, not from the branch.

## The example
Two test projects. Use them for every hand check.

| Project | What it is | Use |
|---|---|---|
| `shop-web` (key SHOP) | a small Node app with `node_modules/` and a `dist/` build, `.env.local` as its env file, base branch `main` | Settings frames, Cleanup frames (shop-web 4.2 GB in the frames; any size is fine), the env rows of *Row · Environment variables (0082)* |
| `otaq` (key OTQ) | this repository, cloned into a test folder | the real measurement: a ticket reaching Done keeps `target/`; the next ticket's first `cargo build` redoes only the workspace crates (0082 measured ~26 s, with 302 dependencies kept) |

- Ticket keys follow the frames: SHOP-21 is the ticket whose copy is running in the "Clearing… (waiting for SHOP-21's
  copy)" state; make it by starting SHOP-21 right after a Done in `shop-web` with a big enough warm build.
- **Unavailable:** the Windows VM's `C:` (NTFS) and the Ubuntu VM's `/home` (ext4) show the switch disabled with the reason.
  On macOS, an exFAT disk image mounted as `OTAQ_HOME` shows it too.

| Epic | Tickets |
|---|---|
| M18 · Warm builds and environment variables | T-0294 – T-0303 (10) |

## M18 · Warm builds and environment variables
Goal: a new ticket starts building in seconds instead of minutes where the disk can clone, and a project can give every
agent, Run and Shell the variables it needs, without Otaq knowing the project's stack.

### T-0294 · Core: project environment variables, set in everything Otaq starts in the project
- **Epic:** M18 · Warm builds and environment variables
- **Requirements:** ENV-01, ENV-02, ENV-03 (core), SET-01
- **Decisions:** 0082, 0023, 0026, 0063, 0068, 0083
- **Design:** none (no UI). The rules match *15 · Settings → Row · Environment variables · edit, remove, errors (0082)*.
- **Depends on:** none
- **Size:** L
- **Shared files:** `ProjectSettings` (one field), creates `src/session/env.rs`, one call in `session/mod.rs` `spawn`,
  `start/setup.rs` (`run_setup` takes the rows), one argument in `start/sequence.rs` step 5, the hook-running git
  callers (`git/checkpoint.rs`, `git/merge.rs`, `git/sync.rs`, `start/worktree.rs`, `review/finish.rs`): one argument each

**What and why**
- **The setting:** `ProjectSettings.env_vars: Vec<EnvVar>` with `EnvVar { name, value }`, in row order, empty by default.
  Stored in `settings_json` as written (plain text, no masking, ENV-01). An empty list means onboarding and every existing
  project behave exactly as before.
- **Problems (ENV-03)**, as `SettingProblem`s with the row's index, so the UI can mark the row:
  - empty name: "Add a name, or remove the row" (use the frame's wording if it differs);
  - a name containing `=` or a null character: the frame's wording for `MAX=JOBS`;
  - a duplicate: a row whose name repeats an earlier row's is marked; **the first row applies**. On Windows names are
    compared ignoring case (`node_options` repeats `NODE_OPTIONS`); on macOS and Linux they aren't.
  - Nothing else is a problem: a space, a leading digit, an empty value and any value are allowed. Saving never refuses
    and never fixes a row silently (SET-01).
- **Applied rows:** the rows with no problem, in order. A pure function `applied(env_vars, os) -> Vec<(name, value)>`.
- **Where they are set (ENV-02):** every session Otaq starts for the project, in one place: `Sessions::spawn` asks a
  provider (set once at service start, like `set_deliveries`) for the project's applied rows by `spec.project_id`. That
  covers every `SpawnSpec` today:
  - the ticket agent (`start/sequence.rs`), answer sessions (`messages/answering.rs`), the delivery session
    (`pr/service.rs`);
  - the validator (`validation/mod.rs`);
  - Project agents and epic leads (`project_agents/mod.rs`);
  - the Project terminal, Shell and Run (`terminals/`);
  - every Resume, Retry and restart resume (they spawn too);
  - the size check of 0081, when it is built, through the same path;
  - **the setup command** (SES-01 step 5, 0082 changelog). It isn't a session, so `run_setup` gets the applied rows as
    an argument and sets them with the same order rule. "Retry start" reads them again.

  - **the git commands Otaq runs in the project's repo and worktrees** (0082 changelog): the checkpoint commit (GIT-02),
    rebases, merges in the scratch worktree, worktree add and remove, so the user's git hooks see the variables. `Git`
    runs commands for every project, so the callers that know the project pass the applied rows (`run_with_env`
    already exists). Read-only git calls may skip them; any command that can run a hook must not.

  Otaq's other tools (`gh`, `az`) don't get them.
- **Order:** the service's environment, then the project variables, then Otaq's own variables. **Otaq's own win** over a
  row of the same name, because Otaq needs them to work (`OTAQ_TICKET`, the role, `PORT`). A `PATH` row is the
  exception: Otaq puts its bin folder in front of the row's value, so `otaq` still resolves. Write this in the module doc
  and in `tickets/review-M18.md`.
- **When a change applies:** the rows are read at each spawn, never cached. A running session keeps the values it
  started with. Nothing restarts and nothing is offered.

**Done when**
- [x] tests: defaults (empty, old `settings_json` without the field), each problem (empty, `=`, null, duplicate; case on
  Windows vs not), `applied` order and first-wins, a spawned session (the ordinary stand-in command printing its
  environment) for each session kind, a setup command and a git hook (a temporary repo whose `pre-commit` prints its
  environment, run by the checkpoint commit) get the rows, Otaq's own variables win, the `PATH` row keeps Otaq's bin folder
  first, a change reaches the next spawn and not a running one
- [x] checks green; the commit names ENV-01, ENV-02, ENV-03 and decisions 0082, 0079

### T-0295 · Core: copy-on-write cloning, and whether the data folder's disk can clone
- **Epic:** M18 · Warm builds and environment variables
- **Requirements:** WRM-01
- **Decisions:** 0082
- **Design:** none (no UI). The unavailable reasons shown later come from *15 · Settings → Row · Start new worktrees from a
  warm build (0082)* (unavailable) and its dark row.
- **Depends on:** none
- **Size:** M
- **Shared files:** creates `src/warm/` (`mod.rs`, `clone.rs`), `lib.rs` (one line)

**What and why**
- **`clone_tree(from, to, only: Option<&[PathBuf]>)`**: clones a folder (or the listed paths inside it) into `to`,
  keeping the layout, modified times, permissions and symlinks as symlinks.
  - macOS: `clonefile(2)` (a directory clone is one call on APFS; `only` clones each listed path). Clone each listed
    folder with **one call, never file by file**: measured on this repo's `node_modules` (231 MB, 18,000 files), one
    `clonefile` of the folder took 0.19 s, while `cp -c` (a clone per file) took 5.8 s.
  - Linux: walk the tree, `ioctl(FICLONE)` each file, recreate folders and symlinks, set the times (btrfs, XFS).
  - Windows: `Unsupported` until T-0299.
  - **No plain copy**, ever (WRM-01). The first file that can't be cloned stops the clone: the partial `to` is removed
    and `CloneError::Unsupported` (or the I/O error) comes back. `EXDEV` (another disk) is `Unsupported`.
  - Hard links are never made (build tools write into their outputs).
- **`probe(data_dir) -> CloneSupport`**: clones a small file inside `<app-data>/warm/.probe` and removes it.
  `CloneSupport { available: bool, file_system: String, volume: String }`, e.g. `apfs` on `/`, `ntfs` on `C:`,
  `ext4` on `/home`. The file system name comes from `statfs` (macOS `f_fstypename`, Linux `f_type`), and on Windows from
  `GetVolumeInformationW`. Probed once at service start and kept; tests can force a value.
- **`move_tree(from, to, only)`**: renames the listed paths from one folder into another on the same disk, creating parent
  folders (used by T-0297). A rename that fails with `EXDEV` is an error, never a copy.
- Pure Rust on `libc` / `windows-sys`; no new crate. Nothing here knows any build tool.

**Done when**
- [x] tests (skipped with a printed note where the temporary folder can't clone, so they pass on ext4 CI): a tree with
  nested folders, a symlink, an executable bit and old times clones with all of them kept; `only` clones just the listed
  paths; a clone that fails halfway leaves nothing; `move_tree`; the probe on the test disk
- [x] the probe answers `available: false` with the file system name on an exFAT or ext4 folder (tested in a VM or by a
  forced value)
- [x] checks green; the commit names WRM-01 and decisions 0082, 0079

### T-0296 · Core: the warm build store, the switch, Clear and the wait
- **Epic:** M18 · Warm builds and environment variables
- **Requirements:** WRM-02, WRM-05, WRM-06, WRM-07 (core)
- **Decisions:** 0082, 0018, 0023
- **Design:** none (no UI). The states the UI shows (cleared, "Clearing… (waiting for SHOP-21's copy)") are drawn in
  *15 · Settings → Row · Start new worktrees from a warm build (0082)* and *16 · Recovery & cleanup → Row · Cleanup with
  warm builds (0082)*.
- **Depends on:** T-0295
- **Size:** L
- **Shared files:** `ProjectSettings` (one field), `data_dir.rs`, `src/warm/store.rs`, `recovery/mod.rs` (one call), two
  requests in `protocol.rs` and their permission arms

**What and why**
- **The switch (WRM-05):** `ProjectSettings.warm_builds: bool`, `true` by default (an old `settings_json` without it reads
  as on). Not in onboarding.
- **The folder (WRM-02):** `DataDir::warm_path(repo_name)` = `<app-data>/warm/<repo-name>`, next to `wt/`. Inside, the
  gitignored files as laid out in the worktree, plus `.otaq-warm.json` with the record (built at, base, from, kept at).
  The record file name must not clash with a project's files; seeding skips it.
- **Warm builds on:** `warm_builds && probe.available` for the project. Off for either reason means: no seeding, no
  keeping, and Done deletes as before.
- **Size:** the sum of the files' sizes (apparent size, as the Cleanup view shows for worktrees), computed off the main
  path and cached until the warm build changes.
- **The lock:** one per project. A seed holds a **reader** slot while it clones, named by its ticket (`SHOP-21`).
  Replace and Clear take the **writer** slot, and wait for the readers to finish. The store can tell who is copying, so
  the UI can show "Clearing… (waiting for SHOP-21's copy)" (WRM-06).
- **Replace, forward only (T-0297 calls it):** the new files are first moved into `<repo-name>.incoming/` (no lock). Then,
  under the writer slot:
  - if a warm build exists and its **base** is not an ancestor of (or equal to) the new one's base, the new one is
    dropped (deleted in the background): an older build never replaces a newer one;
  - otherwise the old folder is renamed to `<repo-name>.old/`, `.incoming` is renamed into place, and `.old` is deleted
    in the background.
- **Clear (WRM-06, WRM-07):** `warm.clear { project }`. It waits for the writer slot (the UI shows "Clearing…" with the
  ticket being copied), renames the folder to `.old`, answers, and deletes `.old` in the background. Clearing never
  turns the switch off.
- **Turning the switch off** (`ProjectSettingsSet` with `warm_builds: false`) clears the warm build the same way, after the
  setting is saved. The confirmation is the UI's (T-0301).
- **`warm.list`:** every warm build in `<app-data>/warm/`: repo name, the project it belongs to (or none), size, built at
  (short SHA), from, kept at, and "clearing (waiting for SHOP-21's copy)" when a Clear waits. A folder without a record
  still lists (size and folder only).
- **Interrupted work:** at service start, `<repo-name>.incoming/` and `<repo-name>.old/` left by a crash are deleted.
  They are Otaq's own temporary folders, not leftovers the user must confirm (REC-04 is about worktrees, branches and
  sessions).
- **Events:** a change to a project's warm build (kept, replaced, cleared) publishes one board event, so Settings and
  Cleanup refresh.

**Done when**
- [x] tests (with a test cloner where the disk can't clone): the default switch, `warm_path`, the record round-trip,
  replace (first, forward, an older one dropped), Clear while a seed holds a reader (waits, names the ticket, then
  clears), switch off clears, `warm.list` with and without a record, `.incoming` / `.old` removed at start
- [x] checks green; the commit names WRM-02, WRM-05, WRM-06, WRM-07 and decisions 0082, 0079

### T-0297 · Core: keep the build at Done
- **Epic:** M18 · Warm builds and environment variables
- **Requirements:** WRM-03, REC-05
- **Decisions:** 0082, 0018, 0035
- **Design:** none (no UI)
- **Depends on:** T-0296
- **Size:** M
- **Shared files:** `src/warm/keep.rs`, one call in `review/finish.rs`

**What and why**
- In `Review::finish`, **after** the Run / Shell hooks and the ports, and **only when the worktree is clean and about to be
  removed**:
  1. Skip unless warm builds are on for the project.
  2. List the worktree's gitignored paths: `git ls-files --others --ignored --exclude-standard --directory` (folders as
     one entry, so a 108,000-file `target/` is one rename).
  3. Leave out the project's env files (they are copied fresh at each start, SES-01 step 4, and may have changed in the
     settings).
  4. Record built at (`HEAD`), base (`merge-base HEAD <the project's base branch>`), from (the ticket) and now.
  5. `move_tree` them into `<repo-name>.incoming/` and hand it to the store's replace (T-0296).
  6. Then `remove_worktree` as today. The branch stays.
- **Any failure** in 1–5 is logged and the worktree is removed as before. Keeping is a speed-up, never a reason for Done
  to fail. A half-moved `.incoming` is deleted.
- A worktree kept for uncommitted changes keeps everything, as today. Its build isn't kept later either, when the user
  deletes it from Cleanup.
- Done covers Local merge, Close ticket, Mark merged and Auto-merge: every path that calls `finish`.
- An epic child's Done is the same as any ticket's.

**Done when**
- [x] tests against temporary repositories (a fake "build" writes ignored files and folders): Done keeps them and removes
  the worktree; env files aren't kept; a dirty worktree keeps everything; switch off or a disk that can't clone deletes
  as before; a newer base replaces, an older one doesn't; a failure halfway still removes the worktree
- [x] checks green; the commit names WRM-03, REC-05 and decisions 0082, 0079

### T-0298 · Core: seed a new worktree, with the timestamp trick
- **Epic:** M18 · Warm builds and environment variables
- **Requirements:** WRM-04, SES-01 (step 3), SES-02
- **Decisions:** 0082, 0010, 0018, 0023
- **Design:** none (no UI). No new start step label: seeding is part of "Worktree".
- **Depends on:** T-0296, T-0294 (both change `run_setup`)
- **Size:** M
- **Shared files:** `src/warm/seed.rs`, one call in `start/sequence.rs` (after `prepare_worktree`, before env files),
  `start/setup.rs` (the log appends)

**What and why**
- **When:** only for a **new** worktree: not when a returning ticket keeps its worktree (`keep_worktree`), and not when
  `prepare_worktree` reused an existing folder (SES-02: a re-run start doesn't seed twice). Only when warm builds are on.
- **From what:**
  1. The warm build, under a reader slot named by the ticket (T-0296).
  2. With none: the user's own checkout (`project.repo_path`), if it has gitignored files. Its env files are left out.
     It is cloned only if it sits on a clone-capable disk with the data folder. Another disk fails as `Unsupported` and is
     skipped (no plain copy).
  3. Else nothing.
- **The clone:** `clone_tree` of the seed's gitignored paths into the new worktree. A path that exists in the worktree
  already (git checked it out) is skipped. The record file is skipped.
- **The timestamp trick:**
  - The **seed's commit** is the warm build's "built at", or the checkout's `HEAD`.
  - The changed list is `git diff --name-only <seed commit> HEAD` in the new worktree. For a checkout seed, the
    checkout's own uncommitted changes to tracked files are added, because its build may reflect them.
  - Set every tracked file to 2000-01-01 00:00 UTC, then every file in the changed list to now. Symlinks are touched
    without following them. **Refined by Q71:** instead of the fixed old time, every tracked file gets the modified time
    it had where the build was made (recorded at Done by T-0297's keep, next to the record; the checkout's own times for
    a checkout seed); a warm build without recorded times skips the trick.
  - If the seed's commit can't be resolved (the branch was deleted and pruned), skip the trick. The files keep their
    checkout times, so build tools rebuild more, which is slower but correct.
- **A failure** (clone, trick) removes what was cloned so far, logs it, and the start goes on as before: the ticket builds
  from nothing. Seeding never makes a start fail and never adds a start error.
- **The setup log** of the ticket (`setup_log_path`) gets one line first. `run_setup` today creates the log anew
  (`File::create`), which would erase that line: make it append, and keep the "$ <command>" line after the seed line.
  Example line: "Started from shop-web's warm build of
  3e1f9a2 (SHOP-21, 2 h ago) in 4 s." or "Started from your checkout of shop-web (9c41e7b).". The log is not new UI.
- The setup command still runs after seeding, as before (step 5).

**Done when**
- [x] tests against temporary repositories (test cloner where the disk can't clone): seeded from a warm build, from the
  checkout (env files left out, uncommitted changes marked changed), nothing to seed, a returning ticket and a re-run
  start not seeded, the trick (unchanged old, changed now, unresolvable commit skipped), a failed clone leaves the
  worktree clean and the start goes on, Clear waiting for a running seed
- [ ] by hand, `otaq` project on macOS: Done on one ticket, then start another; its first `cargo build` rebuilds only the
  workspace crates. Record the times in `tickets/review-M18.md`
- [x] checks green; the commit names WRM-04, SES-01, SES-02 and decisions 0082, 0079

### T-0299 · Core: block cloning on Windows (ReFS and Dev Drive)
- **Epic:** M18 · Warm builds and environment variables
- **Requirements:** WRM-01 (Windows)
- **Decisions:** 0082, 0021
- **Design:** none (no UI). NTFS keeps the unavailable state of *Row · Start new worktrees from a warm build (0082)*.
- **Depends on:** T-0295
- **Size:** M
- **Shared files:** `crates/otaq-core/Cargo.toml` (one `windows-sys` feature, `Win32_System_Ioctl`) — **runs alone**;
  `src/warm/clone.rs` (the Windows part)

**What and why**
- 0082 says Windows can't clone on NTFS and leaves suggesting a Dev Drive to the docs. A Dev Drive is ReFS, which supports
  block cloning, so a Windows user who follows that note should get warm builds.
- `clone_tree` on Windows: per file, `FSCTL_DUPLICATE_EXTENTS_TO_FILE` (create the target with the same size, clone the
  ranges, set times and attributes), recreating folders and symlinks. Anything else is `Unsupported`.
- `probe` on Windows: the file system name from `GetVolumeInformationW` (`NTFS`, `ReFS`), then a real clone of the probe
  file.
- No plain copy. If block cloning turns out to need more than one feature or a new crate, stop and record it as a blocker
  in `tickets/questions.md`. Windows then stays unavailable everywhere, which is still within 0082.

**Done when**
- [x] in the Windows VM: `C:` (NTFS) probes unavailable with "NTFS"; a Dev Drive (create a ReFS VHD in the VM) probes
  available, and the T-0295 tests pass there
- [x] `check.ps1` green in the Windows VM
- [x] checks green; the commit names WRM-01 and decisions 0082, 0079

### T-0300 · Settings: the Environment variables row
- **Epic:** M18 · Warm builds and environment variables
- **Requirements:** ENV-01, ENV-03 (UI), SET-01
- **Decisions:** 0082, 0023
- **Design:** *15 · Settings*:
  - *Row · Environment variables (0082)*: empty, a few rows (shop-web), adding a row;
  - *Row · Environment variables · edit, remove, errors (0082)*: editing a value, removing a row, invalid name
    (`MAX=JOBS`), duplicate name;
  - *Row · Environment variables & warm build · Dark (0082)*: the otaq sample and the invalid name;
  - *Row · In context & notes (0082)*: where the row sits (after "Env files to copy into worktrees", before PR
    provider).
- **Depends on:** T-0294
- **Size:** M
- **Shared files:** `settings_page.rs`, `settings.rs` (view), `src/api/settings.rs`, `ui/src/screens/settings/Rows.tsx`,
  `settings.css`, `Gallery.tsx` (one section)

**What and why**
- One per-project row with a full-width list: **NAME** (mono, 240 px) `=` **value** (fills the rest) and **×**, then
  **Add variable**. Empty: the box "No variables. Sessions get your normal environment."
- **The hint is always visible:** `CI = true`, "e.g. so tools never wait for input or start watch mode". It is an example
  only: no built-in, read-only or computed rows, and no language or tool named anywhere.
- **Under the list, one line:** "Changes apply to sessions, Runs and Shells started after this."
- **Editing:** settings save on their own (no Save button). Enter or blur saves; Esc reverts an edit or drops a new empty
  row. Add variable adds a row with NAME focused.
- **Removing** a row (hover shows ×, with its tooltip) has no confirmation: it's a setting, nothing on disk is deleted.
- **Problems** come from the core (T-0294): an invalid or duplicate row stays, marked red with the reason, and isn't set
  until fixed. The other rows still apply. Wordings are the view model's, taken from the frames.
- Values are shown in full, plain (0082).
- Keyboard: the list is reachable by Tab, and × has an accessible name ("Remove NODE_OPTIONS").

**Done when**
- [x] view-model tests: empty, rows, each problem wording, the duplicate rule per platform, the hint and the "applies
  after" line
- [ ] matches every frame named above, light and dark
  - *Hand check:* in `shop-web`, add `CI = true`, start a Shell, run `echo $CI` (`true`); change it to `false`; the open
    Shell still says `true`, a new Shell says `false`. Add `MAX=JOBS = 3` and a second `NODE_OPTIONS`: both marked, not
    set.
- [x] checks green; the commit names ENV-01, ENV-03, SET-01 and decisions 0082, 0079

### T-0301 · Settings: the "Start new worktrees from a warm build" switch
- **Epic:** M18 · Warm builds and environment variables
- **Requirements:** WRM-05, WRM-06 (UI)
- **Decisions:** 0082, 0018
- **Design:** *15 · Settings*:
  - *Row · Start new worktrees from a warm build (0082)*: on (the default); the confirmation when it's turned off ("Clear
    shop-web's warm build (up to 4.2 GB)?", button "Turn off and clear up to 4.2 GB", and the line "Tickets copied from it
    share part of this space, so it may free less."); off right after it, with the toast "Cleared shop-web's warm build";
    "Clearing… (waiting for SHOP-21's copy)"; unavailable (Windows, `C:` NTFS);
  - *Row · Environment variables & warm build · Dark (0082)*: the confirmation, and unavailable (Linux, `/home` ext4);
  - *Row · In context & notes (0082)*: its place, right after Environment variables.
- **Depends on:** T-0296, T-0300 (same files)
- **Size:** M
- **Shared files:** `settings_page.rs`, `src/api/settings.rs`, `ui/src/screens/settings/Rows.tsx`, `Dialogs.tsx`,
  `settings.css`, `Gallery.tsx` (one section)

**What and why**
- The switch row, on by default, in project settings only (not in onboarding).
- **Turning it off asks only when it would delete a warm build** (Q9): the confirmation with "up to <size>" and the
  shared-space line. With no warm build, it turns off silently. Turning it on never asks.
- After the confirmation, the setting is saved and the warm build cleared (T-0296). While a seed is copying, the row shows
  "Clearing… (waiting for SHOP-21's copy)" until it's done. Then the toast "Cleared shop-web's warm build" (no number).
- **Unavailable** (the data folder can't clone): the switch is disabled and off, with the reason under it and in its
  tooltip, naming the file system and the disk as drawn (e.g. `C:` NTFS, `/home` ext4). Never hidden. The stored setting
  isn't changed, so moving the data folder to a disk that can clone brings it back on.
- The row says nothing about any build tool.

**Done when**
- [x] view-model tests: on, off, the confirmation only with a warm build, "up to" with the size, clearing with the ticket
  name, unavailable with each file system name
- [ ] matches every frame named above, light and dark
  - *Hand check:* in `shop-web`, Done one ticket (a warm build appears in Cleanup), turn the switch off (confirmation,
    toast, Cleanup's section gone), turn it on (no question). Turn it off again while SHOP-21 is being seeded: "Clearing…
    (waiting for SHOP-21's copy)", then cleared.
- [x] checks green; the commit names WRM-05, WRM-06 and decisions 0082, 0079

### T-0302 · Cleanup: the warm builds section
- **Epic:** M18 · Warm builds and environment variables
- **Requirements:** WRM-07, REC-04
- **Decisions:** 0082, 0018
- **Design:** *16 · Recovery & cleanup*:
  - *Row · Cleanup with warm builds (0082)*: 3 leftovers plus 3 warm builds (otaq 36.1 GB, shop-web 4.2 GB, billing-api
    1.3 GB); the Clear… confirmation ("Clear otaq's warm build (up to 36.1 GB)?", button "Clear up to 36.1 GB", the
    shared-space line, and that Clear doesn't turn the switch off); the row while "Clearing… (waiting for SHOP-21's copy)"
    (spinner, Clear… disabled); right after Clear (row gone, total updated, focus on the next Clear…, toast "Cleared
    otaq's warm build");
  - *Row · Cleanup · no leftovers / no warm builds (0082)*;
  - *Row · Cleanup with warm builds · Dark (0082)*: hover, and the confirmation.
- **Depends on:** T-0296
- **Size:** M
- **Shared files:** `recovery/cleanup.rs` (the scan), `cleanup.rs` (view), `src/api/recovery.rs`,
  `ui/src/screens/recovery/Cleanup.tsx`, `logic.ts`, `recovery.css`, `icons.rs` (the flame, if missing), `Gallery.tsx`
  (one section)

**What and why**
- **Its own section below the leftovers' action bar**: a different background, a flame icon, the total in the section
  header, and the note "Kept to speed up new tickets. It grows over time; clear it to reclaim disk."
- **Rows**, largest first: the project name (the folder name when no project owns it, e.g. after Remove project), size,
  built at (short SHA), age, and **Clear…**. No checkboxes.
- **Not leftovers:** warm builds aren't counted in "N items", aren't part of "Delete selected…", and never light the
  rail's Cleanup dot.
- **Hidden when there are none**, so Cleanup looks exactly as before 0082. With no leftovers but some warm builds, the
  shorter "Nothing to clean up" state stays, with the section under it.
- **Clear…:** the confirmation as drawn, "up to <size>". Clear doesn't turn the switch off; the next Done keeps a new
  warm build. While a seed copies: the row's spinner and "Clearing… (waiting for SHOP-21's copy)", Clear… disabled.
  After: the row goes, the total updates, focus moves to the next Clear…, and the toast gives no number.
- The size and total keep the full size (the "up to" is only in the confirmation).

**Done when**
- [x] view-model tests: sorting, the total, the hidden section, the empty state with the section, the count and the dot
  ignoring warm builds, an orphan row, the clearing state, the focus after Clear
- [ ] matches every frame named above, light and dark
  - *Hand check:* `shop-web` and `otaq` each with a warm build plus one leftover worktree; Clear… on `otaq` while SHOP-21
    seeds from `shop-web` (no wait: different project), then on `shop-web` while it seeds (waits).
- [x] checks green; the commit names WRM-07, REC-04 and decisions 0082, 0079

### T-0303 · Docs, hand-check pass and the VM checks
- **Epic:** M18 · Warm builds and environment variables
- **Requirements:** WRM-01 … WRM-07, ENV-01 … ENV-03
- **Decisions:** 0082, 0079, 0062
- **Design:** every frame named in T-0300 – T-0302
- **Depends on:** T-0294 – T-0302
- **Size:** M
- **Shared files:** `tickets/review-M18.md`, the user docs or `scripts/README.md`

**What and why**
- **Docs:** one short section on warm builds: what is kept and where, the switch, Clear in Cleanup, that it needs a disk
  that can clone (APFS, btrfs, XFS, a Windows Dev Drive), and that a Windows user can put Otaq's data folder on a Dev
  Drive to get it (0082 leaves this to the docs). One paragraph on project variables, with the `CI = true` example and,
  as a user's own example, a core share for parallel builds written for their stack. Also, in general terms: a setup
  command that deletes its output before installing throws the warm build away. **The docs name no language or tool
  either**, like the app.
- Prepare every hand check of M18 in `tickets/review-M18.md`, with steps and frame names, grouped by screen (Settings,
  Cleanup, starting and Done). Include the measured times from T-0298. Agents don't tick hand checks.
- **Sweep** for what 0082 rules out: no tool or language names in the UI strings, no computed or placeholder variables,
  no plain-copy path, no size cap.
- Run `scripts/check.ps1` in the **Windows VM** (VMPal, `Z:\software-factory`): the switch unavailable on `C:`, available
  on a Dev Drive if T-0299 landed; a project variable reaching a Shell. Do the same in the **Ubuntu VM** (`/home` ext4:
  unavailable; a btrfs or XFS loop disk if easy).
- A VM that can't be reached is **skipped** and recorded as "not checked", not a blocker (the M14 rule).
- Fix what fails in new commits that name the ticket they fix.

**Done when**
- [ ] the docs section exists and names no tool inside the app's wording
- [ ] `tickets/review-M18.md` lists every hand check, with "How it was run" (0079) and "Platforms checked"
- [ ] the sweep finds nothing 0082 rules out
- [ ] `check.ps1` green in the Windows VM, or the VM is recorded as skipped
- [ ] checks green on the final tree; the commit names WRM-01…WRM-07, ENV-01…ENV-03 and decisions 0082, 0079

## Parallel lanes (M18)
**The rules are the Depends on lists and the shared files.** This table is only the earliest grouping. The orchestrator
decides the actual order (0079):
- at most 3 teammates at once;
- never two tickets in the same folder or file at once, except the one-line registration files;
- tickets that change `package.json` or `Cargo.toml` run alone (T-0299).

| Phase | Tickets (side by side within a phase) | Watch out for |
|---|---|---|
| 0 | T-0294 ∥ T-0295 | `session/`, `start/setup.rs`, the git callers + one `ProjectSettings` field vs the new `warm/clone.rs`. Different files. T-0294 touches `review/finish.rs` and `start/worktree.rs` (one argument each) before T-0297 / T-0298 add their calls |
| 1 | T-0296 (after T-0295) ∥ T-0300 (after T-0294) | T-0296 adds the second `ProjectSettings` field (a one-line merge with T-0294's); T-0300 is the settings UI |
| 2 | T-0297 ∥ T-0298 ∥ T-0302 (all after T-0296; T-0298 also after T-0294, both touch `start/setup.rs` and step 5) | `review/finish.rs` + `warm/keep.rs` vs `start/sequence.rs` + `warm/seed.rs` vs `recovery/cleanup.rs` and the Cleanup screen. Each adds one `mod` line to `warm/mod.rs` |
| 3 | T-0301 (after T-0296, T-0300) | the same settings files as T-0300 |
| 4 | T-0299 (alone) | `Cargo.toml`; the Windows VM |
| 5 | T-0303 (alone) | the VMs |

The long chain is T-0295 → T-0296 → T-0298 → T-0303. Start T-0295 first in phase 0 and T-0296 first in phase 1.
