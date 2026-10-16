# Sharing dependency installs and build caches across checkouts, and lazy environments

Research date: 2026-10-16. About 16 tool calls. Lines marked "(background knowledge, unverified)" were not confirmed by a fetched source in this session.

## Rust: shared CARGO_TARGET_DIR, sccache, cargo build-dir and the cross-workspace cache, cleanup

### Takeaway
Rust has no good zero-config sharing story today (Oct 2026). One shared target dir makes parallel builds wait on each other. sccache cannot cache incremental builds or any crate that is linked (bin, proc-macro, cdylib, dylib). Cargo's own cross-workspace cache is an accepted 2026 Project Goal, nightly-only at best. Its first version leaves out build scripts and proc macros, and stabilization is expected after 2026.

### Cited Findings
- Concurrent cargo invocations that share one target dir "block on cargo's per-build-dir lock and run serially". A practitioner's fix was "Every concurrent cargo needs its OWN `--target-dir`" — [claudetools commit log](https://git.azcomputerguru.com/azcomputerguru/claudetools/commit/9c9efdd34bf8a286c17026298d744a69e90bd61f)
- Cargo prints a status message ("Blocking waiting for file lock …") whenever a file lock is contended, then blocks until the lock is free — [cargo flock docs](https://doc.rust-lang.org/nightly/nightly-rustc/cargo/util/flock/fn.acquire.html). Background rust-analyzer or watcher processes are a common cause — [users.rust-lang.org](https://users.rust-lang.org/t/use-cargo-check-in-its-own-directory-avoiding-blocking-cargo-run/136213)
- "Blocking waiting for file lock on package cache" is a different lock, on the shared CARGO_HOME (the registry download cache), not on the build directory — [DEV Community](https://dev.to/chandrapenugonda/fix-cargo-build-stuck-at-blocking-waiting-for-file-lock-on-package-cache-8lm)
- sccache limits for Rust: "rustc's incremental compilation needs to be disabled". "Crates that invoke the system linker cannot be cached", which covers `bin`, `dylib`, `cdylib` and `proc-macro` crates. Procedural macros that read files from the filesystem may not be cached properly — [sccache docs/Rust.md](https://github.com/mozilla/sccache/blob/main/docs/Rust.md)
- Cargo cross-workspace cache, a 2026 Rust Project Goal with status Accepted. It aims for "a build cache that is shared across workspaces to [improve] build times and reduce disk usage". The plan for 2026 is to "design and implement this cache in Cargo, making it available on nightly". The first version would "only support basic crates (no build scripts, proc macros, etc)". Stabilization "will likely not be completed during the goal period". Owner Ross Sullivan, champion Ed Page, funded part-time by AWS. Tracking: rust-lang/cargo#5931, #15010 (build-dir layout), #16807 — [Rust Project Goals 2026](https://goals.rust-lang.org/2026/cargo-cross-workspace-cache.html)
- The same page contrasts this with sccache, which "tr[ies] to infer inputs for hashing a cache key from command-line arguments", while Cargo has "much more knowledge about the dependency graph" — [Rust Project Goals 2026](https://goals.rust-lang.org/2026/cargo-cross-workspace-cache.html)
- 2025 groundwork: target-dir was split into `artifact-dir` and `build-dir`, and the layout was regrouped by build unit. Stabilizing the layout rework is a prerequisite for the cache — [Rust Project Goals 2025h2](https://goals.rust-lang.org/2025h2/cargo-build-dir-layout.html). A March 2026 issue still describes the new layout as behind `-Zbuild-dir-new-layout` — [cargo issue 16519 mirror](https://hub.harborframework.com/tasks/abundant/rust-lang__cargo-16519)
- cargo-sweep removes stale artifacts from `target/` without wiping it: `cargo sweep -t 30` removes files older than 30 days, and `-i` removes files from toolchains that are no longer installed — [cargo-sweep](https://github.com/quake/cargo-sweep). One user reported reclaiming about 20 GiB — [users.rust-lang.org](https://users.rust-lang.org/t/cleaning-target-folders-with-cargo-sweep/84229)
- Cargo's automatic gc (once a day, items unused about a month) cleans the CARGO_HOME cache, not `target/`. Target-dir cleaning "was not yet included" in Dec 2023 — [Rust blog 2023-12-11](https://blog.rust-lang.org/2023/12/11/cargo-cache-cleaning/)

### Inferences
- Sharing one CARGO_TARGET_DIR across Otaq tickets would serialize the builds of agents running at the same time, so it defeats the point of parallel agents. Sharing CARGO_HOME (the registry and git download caches) is already the default and safe.
- sccache helps most with the dependency crates (rlibs) of each worktree. It does not help with the workspace's own crates under incremental dev builds, with proc-macros, or with the final link. For a Tauri or GPUI-sized app, a cold build in a new worktree still recompiles proc-macro crates and relinks. Turning off incremental to get sccache hits makes the agent's edit-and-rebuild loop slower.
- A practical Rust middle ground for Otaq: keep a separate target dir per ticket, cap it with cargo-sweep or by deleting it while the ticket sits idle in Review, and rely on sccache plus the shared CARGO_HOME to make the rebuild on wake cheaper.
- Cargo's native cache is the long-term zero-config answer, but it will not be stable during 2026.

### Gaps
- No measured rebuild times for a cold target dir with a warm sccache versus with no cache on a large project. I found no 2025–2026 benchmark.
- No confirmation of whether `build-dir` (the config key, separate from the new layout) is stable in Oct 2026.
- APFS clonefile-based copying of a target dir (`cp -c`) to seed a new worktree was not researched with sources.

## JavaScript: pnpm, npm/yarn, Bun, Yarn PnP, Turborepo/Nx

### Takeaway
JS has the strongest sharing story. pnpm hardlinks package files from one content-addressed store, so the same package costs disk once. pnpm's global virtual store goes further: node_modules becomes symlinks only, a new worktree costs "almost no extra disk space", and pnpm documents this setup specifically for parallel AI agents in worktrees. Bun hardlinks (Linux/Windows) or clonefiles (macOS) from a global cache by default.

### Cited Findings
- pnpm keeps one copy of each package file in a global store and hardlinks it into each project. "A 1MB package appears as 1MB in each project but takes 1MB on disk." On Windows it uses junctions for directory links — [pnpm FAQ](https://pnpm.io/faq)
- pnpm's git-worktrees guide: set `virtualStoreType: global` in `pnpm-workspace.yaml` (named `enableGlobalVirtualStore: true` before v11.23.0). Each worktree's node_modules then holds only symlinks into `<store>/links`, and "no files are hardlinked or copied into the worktree". Installs in later worktrees are "nearly instant". A new agent "costs almost no extra disk space". Worktrees can use different dependency versions without conflict. Caveat: don't share a writable store between mutually untrusted agents. The page is written for one worktree per AI agent — [pnpm git worktrees](https://pnpm.io/git-worktrees)
- The global virtual store is experimental and off by default, because some tools mishandle symlinked node_modules. pnpm v11 enables it by default only for `pnpm dlx` and global installs — [pnpm global virtual store](https://pnpm.io/zh/next/global-virtual-store). It is turned off automatically in CI (pnpm 10.12 release notes) — [newreleases.io](https://newreleases.io/project/github/pnpm/pnpm/release/v10.12.0)
- An npm RFC proposes a similar shared store. It requires `install-strategy=linked` and cites a 2.5 GB monorepo whose node_modules shrank to 1.4 MB — [npm/rfcs#911](https://github.com/npm/rfcs/pull/911)
- The aube package manager enables a global virtual store by default and lists multiple worktrees of one repo as a primary use case — [aube docs](https://aube.jdx.dev/package-manager/global-virtual-store.html)
- Bun keeps a global cache at `~/.bun/install/cache`. Its default backend copies into node_modules with hardlinks on Linux and Windows and clonefile on macOS; `copyfile` is the fallback. Bun also has an opt-in global virtual store for its isolated linker, which reduces a warm install to one `access()` plus one `symlink()` per package. Before that, 95.4% of warm-install time on macOS was spent in `clonefileat` — [Bun global cache](https://bun.com/docs/pm/global-cache), [Bun isolated installs](https://bun.com/docs/pm/isolated-installs), [Bun global store](https://bun.sh/docs/pm/global-store.md)
- Practitioner guides for worktrees and agents: each worktree "needs npm install or npm ci, eating up time and disk space". The workz tool symlinks node_modules, target, .venv and similar folders across worktrees, but only "if your dependencies are identical across branches" — [mindstudio](https://www.mindstudio.ai/blog/git-worktrees-parallel-ai-coding-agents), [workz](https://docs.rs/crate/workz/0.5.0)

### Inferences
- If the project already uses pnpm, Otaq (or a setup agent) can set the global virtual store. That brings the extra install cost per ticket close to zero, and pnpm officially endorses the setup for this use case. With npm or yarn classic, per-worktree node_modules are full copies. Only the download cache (`~/.npm`) is shared.
- Bun's default already shares disk through hardlinks or clones with no configuration.
- Symlinking one worktree's whole node_modules into another (the workz approach) is fragile when branches change dependencies, so it is not safe as a zero-config default.

### Gaps
- Yarn PnP, Turborepo and Nx local caches were not researched with sources this session. (Background knowledge, unverified: Yarn Berry's PnP with a global cache avoids node_modules entirely. Turborepo and Nx keep per-repo task-output caches under `.turbo` / `.nx/cache`, which a shared cache directory can point at.)
- No measured disk numbers for hardlinked pnpm node_modules per extra worktree (hardlinks still cost inodes and directory entries).

## Python: uv

### Takeaway
uv installs into each venv from a global cache. It uses clone (copy-on-write) by default on macOS and hardlinks by default on Linux and Windows, so a venv per worktree costs little extra disk as long as the cache and the worktree are on the same filesystem.

### Cited Findings
- Default link mode: `clone` on macOS, `hardlink` on Linux and Windows. It is configurable with `--link-mode` / `UV_LINK_MODE` — [uv commit (mirror)](https://git.joshthomas.dev/mirrors/uv/commit/a3b61a264473183305b2f3489a602be1af6b5721). A tessl.io doc instead calls hardlink the universal default, so the sources conflict; the commit is more authoritative — [tessl](https://tessl.io/registry/tessl/pypi-uv/files/docs/cache-management.md)
- Clone and hardlink both need the cache and the venv on the same filesystem. Otherwise uv falls back to full copies, which "doubles the disk usage". Clearing the cache breaks venvs only in symlink mode — [pydevtools handbook](https://pydevtools.com/handbook/how-to/how-to-manage-uv-cache-size/)
- Across filesystems, explicitly setting `UV_LINK_MODE=hardlink` is silently downgraded to a copy — [ROCm rocm-cli PR #170](https://github.com/ROCm/rocm-cli/pull/170)

### Inferences
- uv is zero-config friendly. Otaq's main concern is keeping worktrees on the same volume as the user's cache (for example, not on an external drive). pip with plain venvs copies files and gets no such sharing.

### Gaps
- No benchmark of uv venv creation time and size per extra worktree found this session.

## Other ecosystems: ccache, Gradle, Bazel, Nix

### Takeaway
Not researched with sources this session. Background knowledge (unverified): these are all content-addressed caches keyed on inputs, so they are inherently shareable across checkouts.

### Cited Findings
- None fetched this session.

### Inferences
- (Background knowledge, unverified) ccache shares C/C++ object files across checkouts by default via `~/.ccache` or `~/.cache/ccache`. It may need `base_dir` set so absolute worktree paths don't break cache hits. The Gradle local build cache (`~/.gradle/caches/build-cache-1`) is opt-in with `org.gradle.caching=true`. Bazel's `--disk_cache` and its per-user output base share action outputs. The Nix store is fully shared by design. All of these are content-addressed caches with input-hash keys, the same model Cargo plans to adopt.

### Gaps
- Exact defaults and whether worktree path differences break cache keys (ccache `base_dir`, Gradle relocatability) need verification.

## Stack-agnostic file-level dedupe

### Takeaway
On macOS, tools exist that find identical files and replace them with APFS clones. This is stack-agnostic and safe, because clones are copy-on-write. Coverage for Windows (ReFS block cloning, Dev Drive) and Linux (btrfs bees, jdupes --dedupe) was not confirmed with sources this session.

### Cited Findings
- `dedup` (ttkb-oss) is a free macOS CLI that finds files with identical content and replaces the duplicates with APFS clones. No files are deleted. It is APFS-only — [Gigazine 2025-03-02](https://www.gigazine.net/gsc_news/en/20250302-dedup-macos-duplicate-file)
- diskDedupe is a paid macOS app that replaces duplicates with APFS clones and preserves metadata — [App Store](https://apps.apple.com/app/diskdedupe/id1457961323)
- APFS clones share data blocks, and later writes are copy-on-write and private to the modified file — [Eclectic Light](https://eclecticlight.co/2020/04/14/copy-move-and-clone-files-in-apfs-a-primer/). Apple's `clonefile(2)` man page "strongly discourage[s]" cloning whole directory hierarchies and recommends copyfile(3) for directories — [Gigazine](https://www.gigazine.net/gsc_news/en/20250302-dedup-macos-duplicate-file)
- There is no standard tool for detecting whether a file is a clone on macOS or Windows, so space accounting tools over-report — [Eclectic Light 2021](https://eclecticlight.co/2021/04/02/how-can-you-tell-whether-a-file-has-been-cloned-in-apfs/)

### Inferences
- Post-hoc dedupe of build outputs (for example, identical dependency rlibs in two target dirs) works on macOS APFS and is stack-agnostic. Its value is limited, though: Rust outputs differ by path and features, so files are often not byte-identical. Seeding a new worktree by cloning (`cp -c` on macOS, reflink on btrfs/XFS) from a sibling's target dir or node_modules is a cheaper stack-agnostic move than dedupe after the fact. It avoids rebuilding as long as the build tool accepts the copied fingerprints. Cargo fingerprints include absolute paths for workspace crates (background knowledge, unverified).
- Disk tools such as Finder and `du` will over-count cloned or hardlinked space, so Otaq would need to be careful if it shows "space used per ticket".

### Gaps
- jdupes `--dedupe`, btrfs bees, ReFS/Dev Drive block cloning and Windows Data Deduplication were not covered by sources.

## Lazy and ephemeral environments: dropping build output while idle, hibernate and wake

### Takeaway
The cloud agent products all use one pattern: a setup step runs once, its result is snapshotted or cached, and a lighter "maintenance" or "install" step refreshes it on resume. Codex caches container state for up to 12 hours. Coder stops idle workspaces and can mark them dormant, then delete them. Locally, the equivalent for Otaq is to keep the code but delete regenerable outputs for idle Review tickets, and to rebuild on demand with warm shared caches.

### Cited Findings
- Codex cloud: the setup script runs first and the resulting container state is cached "for up to 12 hours". On resume, Codex checks out the task's branch and runs an optional maintenance script, for when the setup ran on an older commit. The cache is invalidated when the scripts, env vars or secrets change, and there is a manual "Reset cache" — [OpenAI Codex cloud environments](https://learn.chatgpt.com/docs/environments/cloud-environment.md); secondary summary in [danielvaughan.com 2026-05](https://codex.danielvaughan.com/2026/05/31/codex-cloud-environments-setup-scripts-caching-secrets-codex-universal/)
- Coder: autostop stops a workspace after N hours unless sessions are active (default 1-hour activity bump). With dormancy (Premium), unused workspaces are stopped, marked dormant, and auto-deleted after a further period — [Coder workspace scheduling](https://coder.com/docs/user-guides/workspace-scheduling)
- Practitioner advice: recreating worktrees per task loses "any build cache, compiled artifacts". Keep persistent per-agent worktrees and rotate branches instead — [developer.upsun.com](https://developer.upsun.com/posts/ai/git-worktrees-for-parallel-ai-coding-agents)

### Inferences
- For Otaq: a ticket in Review keeps its worktree (the code, a few MB) but its build outputs (target/, node_modules, .venv) can be dropped after idle time. "Wake" re-runs install and build when the user clicks Run or Open shell. With pnpm's global store, uv and Bun, re-install is seconds. Rust is the expensive case: a full rebuild of the workspace crates is needed even with sccache, because of the bin, proc-macro and incremental limits above.
- An alternative to deletion for Rust is `cargo-sweep`-style trimming or keeping only the most recent N Review tickets "warm" (an LRU).

### Gaps
- Gitpod/Ona, Devin snapshots and Codespaces prebuilds were not covered by fetched sources this session.
- No measured rebuild-on-wake timings with warm shared caches.

## Can an AI agent infer and configure caches automatically (setup agents)?

### Takeaway
Every vendor relies on a per-repo setup file: Codex setup and maintenance scripts, Copilot's `copilot-setup-steps.yml`, Cursor's `environment.json` with `install` and `start`. Copilot explicitly falls back to the agent "discover[ing] and install[ing] these dependencies itself via a process of trial and error", which it calls slow and unreliable. None of the products I found generates cache-sharing configuration automatically.

### Cited Findings
- Copilot: `.github/workflows/copilot-setup-steps.yml` with a single `copilot-setup-steps` job runs before Copilot works. It must be on the default branch. Honored keys are `steps`, `permissions`, `runs-on`, `services`, `snapshot` and `timeout-minutes` (max 59). Without it, "Copilot can discover and install these dependencies itself via a process of trial and error", which can be slow and unreliable. The only caching example is `actions/setup-node` with `cache: "npm"` — [GitHub Docs](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/cloud-agent/customize-the-agent-environment)
- Cursor cloud/background agents: `.cursor/environment.json` has an `install` command, run on each machine boot after the base snapshot. It must be idempotent because it "can run more than once and on partially cached state". A `start` command handles long-running processes — [Cursor docs](https://cursor.com/docs/cloud-agent/setup.md), [Cursor forum](https://forum.cursor.com/t/cloud-agent-start-script-not-blocking/158583)
- Codex: separate setup (cached) and maintenance (run on resume) scripts — [OpenAI docs](https://learn.chatgpt.com/docs/environments/cloud-environment.md)
- The pnpm repo's own `worktree:new` script symlinks `.claude` settings into each worktree. This is an example of a per-project worktree bootstrap script — [pnpm git worktrees](https://pnpm.io/git-worktrees)

### Inferences
- Otaq could copy the Codex and Cursor split: a one-time "environment setup" agent run per project writes an idempotent install script (and, where it detects pnpm, uv, Bun or Cargo, applies the sharing settings above: pnpm global virtual store, uv on the same volume, `RUSTC_WRAPPER=sccache`). Each worktree then runs that script on creation or wake. Detection is easy from lockfiles (pnpm-lock.yaml, bun.lock, uv.lock, Cargo.lock), so a large part could be deterministic rather than agent-driven.

### Gaps
- No source found for any product that auto-generates its setup file with an agent (as opposed to trial and error at task time). Codex may offer auto-generated setup scripts, but that was not verified this session.
