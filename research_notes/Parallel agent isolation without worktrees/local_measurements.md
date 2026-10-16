# Local measurements: CoW-cloned build output and reusable slots

Measured 2026-10-16 ~00:45–00:50 on this Mac (16 cores, 128 GB RAM, APFS, Data volume 54% full),
cargo/rustc 1.99.0, node v20.20.0 (note: `.nvmrc` asks for 24). Everything ran in a scratchpad worktree on the
same APFS volume as the repo; the main checkout and its `target/` were only read. The scratch worktree and all
files were removed afterwards.

**Caveat on noise:** another agent worktree (`m16/T-0272`) was active at the same time (load average 4.5–6),
so wall times carry contention, and `df` deltas include that agent's writes. Where possible, bytes were also
counted directly (files in `target/` written after a marker file).

## The starting point

- Main `target/`: 36 GB (`debug` 35 GB = `incremental` 18 GB + `deps` 16 GB + `build` 557 MB; `release` 1.3 GB),
  **108,500 files**. Last written Oct 15 02:00, which matches commit `960887b` (committed 02:03).
- `crates/otaq-desktop/node_modules`: 192 MB, 11,805 files; the package manager is **npm** (`package-lock.json`).
- The dependency graph: **306 units** in a `cargo build -v` (302 "Fresh" registry/third-party units + 4 workspace
  crates: `otaq-core`, `otaq-view`, `otaq-cli`, `otaq-desktop`). No full cold build was run (it would exceed the
  15-minute budget); `Cargo.lock` did not change across any commit tested, so no dependency ever recompiled.

## A. Fresh worktree + APFS clone

| Step | Command | Wall time | Real disk used |
|---|---|---|---|
| Create worktree | `git worktree add --detach <wt> 960887b` | **0.17 s** | ~14 MB (checkout) |
| Clone `target/` (108,500 files, 36 GB logical) | `cp -Rc target <wt>/target` (one call) | **29.9 s** (sys 18.3 s) | **~49 MB** (df delta; metadata only) |
| Clone `node_modules` (11,805 files) | `cp -Rc .../node_modules <wt>/...` | **3.1 s** | **~3 MB** |

- `cp -Rc` (clonefile) **preserves mtimes**, so cargo's fingerprints see the cloned outputs at their original times.
- First `cargo build -v` in the clone, same commit as the build output: **25.7 s**, recompiled **only the 4
  workspace crates**; all 302 deps "Fresh"; 2 build scripts re-ran (`otaq-core`, `otaq-desktop`). Workspace crates
  rebuild because the freshly checked-out sources are newer than the cloned outputs (and the absolute path
  changed, so rustc's incremental cache gives little help).
  - Wrote ~2.1 GB of new files (incremental session dirs, rlibs, two ~60–70 MB debug binaries, unpacked debuginfo);
    df delta ~2.3 GB (includes noise).
- Immediate second `cargo build`: **0.44 s** (no-op).
- Reference: `touch otaq-core/src/lib.rs` + rebuild at the same path: **5.1 s** (incremental cache hot). So the
  clone saves the whole dependency graph, but the first workspace build after cloning is ~20–25 s, not ~5 s.

## B. Slot reuse (`git switch --detach` in the same worktree, then `cargo build -v`)

| Hop | Commits / `.rs` diff | `git switch` | Build wall | Recompiled | New bytes in target | df delta |
|---|---|---|---|---|---|---|
| `960887b` → HEAD `ac066a2` | 32 commits, 19 files +2084/−152 | 0.08 s | **21.2 s** | 4 workspace crates | 2.08 GB | +1.3 GB |
| HEAD → HEAD~10 `0d13622` | 10 commits, 38 files +4268/−183 | 0.04 s | **20.0 s** | 4 workspace crates | 2.04 GB | +2.0 GB |
| HEAD~10 → HEAD | 10 commits back | 0.05 s | **18.9 s** | 4 workspace crates | 2.08 GB | +0.8 GB |
| HEAD → HEAD~1 `8870b56` | 1 commit | 0.02 s | **8.7 s** | 4 workspace crates | 1.59 GB | +0.14 GB |

- In every hop, 302 deps stayed "Fresh"; only the 4 workspace crates recompiled (git rewrites changed files, which
  bumps their mtimes; the whole workspace crate above a changed file rebuilds).
- "New bytes" counts overwritten binaries too; the df delta is the better guide to growth. Growth per hop is
  mostly new incremental session dirs and fresh `deps/` hashes (27 `otaq*` incremental dirs at the end of the run).
  A long-lived slot's `target/` will **grow** and needs a periodic `cargo clean -p` or sweep.

What the agents actually run, also warm in the slot (at `8870b56`):

- `cargo test --workspace --no-run`: **29.0 s** (user 180 s), only the 4 workspace crates compiled; **+6.8 GB** disk
  (test binaries with debuginfo).
- `cargo clippy --workspace --all-targets -- -D warnings`: **18.0 s**, only workspace crates checked; +0.8 GB.

## npm in the cloned `node_modules`

- The main checkout's `node_modules` is stale relative to HEAD: `npm install --no-audit --no-fund` in the clone
  **added 68 packages in 1.1 s**.
- `npm ci --no-audit --no-fund` (what `check.sh` runs; it deletes `node_modules` first): **2.6 s**, 406 packages,
  ~190 MB real disk (warm `~/.npm` cache). `npm ci` throws the clone away, so cloning `node_modules` only pays off
  if the agent runs `npm install`, not `npm ci`. Either way it's a few seconds; disk is the real difference (3 MB vs
  ~190 MB per worktree).

## Clean-up

- `git worktree remove --force`: **18.0 s** (deleting ~108k+ files), then `git worktree prune` and `rm -rf` of
  the scratch dir.
- **Disk freed: ~11.1 GB** (df before vs after). That is everything the builds wrote into the slot during the run
  (the cloned 36 GB itself only ever cost ~49 MB).

## Takeaways

1. A CoW clone of a warm `target/` into a new worktree is cheap: **~30 s, ~50 MB**, and it keeps all 302
   dependency units warm. The first build is then ~20–25 s (workspace crates only), against a cold build of 306
   units (not measured; many minutes).
2. A reusable slot behaves the same: a `git switch` is instant, and each later build takes **~9–21 s** depending on
   how far the hop is, with deps always fresh as long as `Cargo.lock` doesn't change.
3. Cloning and slots give near-identical build times here; the clone's extra cost is ~30 s and an 18 s delete. Slots
   avoid both but accumulate disk (~1–2 GB per hop for `cargo build`, ~7 GB once tests are built) and need sweeping.
4. Not measured: a `Cargo.lock` change (it would recompile the changed deps and their dependents in either
   approach); cross-volume clones (clonefile only works on the same APFS volume; otherwise `cp -c` falls back or fails).
