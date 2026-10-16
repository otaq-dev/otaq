# Existing parallel coding-agent tools: isolation, dependencies, disk, cleanup, complaints

Research date: 2026-10-16. Sources are mostly 2025–2026. Third-party blogs and vendor comparison pages (notably superset.sh, which sells a competing product) are flagged where used.

## Q1. Per tool: isolation unit, dependency/env handling, setup config, cleanup, Windows

### Takeaway
Almost every local tool uses one plain git worktree per task and leaves dependencies to a user-written setup script, plus a `.worktreeinclude`-style list for copying gitignored files such as `.env`. Cloud tools (Codex cloud, Copilot, Jules, Devin) use throwaway containers or VMs and make setup cheap with **cached snapshots of the environment after setup**, not with shared files. Container-based local tools (Sculptor, container-use) exist, but they need Docker and are weak on Windows.

### Cited Findings

**Conductor (conductor.build, macOS)**
- Each workspace is a git worktree, usually at `~/conductor/workspaces/<repo>/<workspace>`. A new worktree contains only tracked files — [Conductor docs: git worktrees](https://conductor.build/docs/concepts/git-worktrees)
- A setup script runs in the new workspace (`$CONDUCTOR_WORKSPACE_PATH`), and `$CONDUCTOR_ROOT_PATH` points at the main repo. The docs' example runs `npm install`, copies `.env` and builds. The recommended way to share `.env` is a symlink: `ln -s "$CONDUCTOR_ROOT_PATH/.env" .env`. Scripts are set in repo settings or committed as `conductor.json` / `.conductor/settings.toml` `[scripts] setup = "pnpm install"` — [Conductor scripts docs](https://docs.conductor.build/core/scripts); [setup reference](https://www.conductor.build/docs/reference/scripts/setup)
- It copies gitignored files that match `.worktreeinclude`-style patterns, and only files git already ignores — [Conductor docs](https://conductor.build/docs/concepts/git-worktrees)
- The docs say isolation "is not a security boundary": agents run on your Mac with your permissions. The docs describe archiving as the final step but don't say what it deletes. The page says nothing about ports or disk — [Conductor docs](https://conductor.build/docs/concepts/git-worktrees)
- The site now advertises "Conductor Cloud" and a $22M Series A. I found no details on how the cloud version isolates work — [conductor.build](https://conductor.build/docs/concepts/git-worktrees)

**Sculptor (Imbue)**
- Each agent runs in its own Docker container holding a full copy of the project, with its own filesystem and git branch — [Imbue blog](https://imbue.com/blog/containers); [Imbue docs: containers](https://docs.imbue.com/features/containers)
- "Pairing Mode" temporarily mirrors one container's state into the user's local IDE, syncing both ways, so the user can test or edit that agent's work — [Imbue docs](https://docs.imbue.com/features/containers); [Show HN thread](https://hn.svelte.dev/item/45427697)
- It supports custom images and the devcontainer spec, snapshots the container after every turn for rollback, and lists Modal remote sandboxes and "possibly a local mode that uses worktrees" as plans. A team member said "Sculptor does not use work trees" — [Show HN thread](https://hn.svelte.dev/item/45427697)

**Vibe Kanban**
- Each workspace is a git worktree in `.vibe-kanban-workspaces` (configurable). The docs warn: "Multiple workspaces for large repositories can consume significant disk space". They name `node_modules`, `target/`, `build/` as the main costs and say archived workspaces still use space — [Vibe Kanban docs: managing workspaces](https://www.vibekanban.com/docs/workspaces/managing-workspaces)
- A setup script runs after the worktree and branch are created. A "cleanup script" runs after **every agent turn**, for linting and formatting, so it is not a teardown step — [creating workspaces](https://vibekanban.com/docs/workspaces/creating-workspaces); [monitoring](https://vibekanban.com/docs/core-features/monitoring-task-execution)
- Only third-party pages mention a "Copy Files" setting for `.env`, automatic worktree cleanup after a PR merges, and a `DISABLE_WORKTREE_CLEANUP` env var. None of these are confirmed in the official docs — [skillselion](https://skillselion.com/skills/itechmeat/llm-code/vibekanban)

**Cursor (Parallel Agents / Best-of-N, Cursor 2.x)**
- It uses a git worktree per agent under `.cursor/worktrees`. Setup is configured in `.cursor/worktrees.json` with `setup-worktree`, `setup-worktree-unix` and `setup-worktree-windows` keys (typical value: `npm install`) — [Cursor forum guide](https://forum.cursor.com/t/cursor-2-0-split-tasks-using-parallel-agents-automatically-in-one-chat-how-to-setup-worktree-json/140218); [teebe guide](https://teebe.io/guides/cursor-worktrees/)
- Cleanup is by count and timer. A Cursor staff member (Dec 14, 2025) gave the defaults as `cursor.worktreeMaxCount` = 20 per workspace and `cursor.worktreeCleanupIntervalHours` = 6, and suggested 5 and 1. Worktrees are used on all operating systems, Windows included. A third-party guide says the max is 25, which conflicts — [Cursor forum](https://forum.cursor.com/t/windows-request-to-disable-automatic-worktree-creation-critical-disk-space-issue/146189); [teebe](https://teebe.io/guides/cursor-worktrees/)
- Worktrees are "not meant to persist across sessions" and are cleaned up automatically when limits are hit — [Cursor forum help](https://forum.cursor.com/t/working-with-worktrees-in-cursor/153472)

**OpenAI Codex — desktop app / CLI (local)**
- It uses managed git worktrees. A "local environment" holds setup and cleanup scripts, with per-platform versions. A `.worktreeinclude` file at the repo root lists ignored files to copy. Codex skips symlinks and never overwrites existing files — [OpenAI docs: worktrees](https://learn.chatgpt.com/docs/environments/git-worktrees); [Vaughan guide](https://codex.danielvaughan.com/2026/04/11/codex-app-worktree-lifecycle-local-environments/)
- By default it keeps the 15 most recent Codex-managed worktrees and deletes older ones. The limit can be changed or the deletion turned off, and it "tries to avoid deleting worktrees that are still important". One third-party guide claims it saves a snapshot before deleting and never deletes pinned worktrees. This is unverified — [OpenAI docs](https://learn.chatgpt.com/docs/environments/git-worktrees); [Vaughan](https://codex.danielvaughan.com/2026/04/11/codex-app-worktree-lifecycle-local-environments/)

**OpenAI Codex — cloud**
- Each task gets a container built from the `codex-universal` image. The **setup script** runs on a fresh container, with internet. The container state is **cached for up to 12 hours**. An optional **maintenance script** runs when a cached container is resumed, to update dependencies that changed since the cache was built. Changing the scripts, env vars or secrets invalidates the cache, and there is a "Reset cache" button. Business and Enterprise workspaces share the cache — [OpenAI docs: cloud environments](https://learn.chatgpt.com/docs/environments/cloud-environment); [Vaughan guide](https://codex.danielvaughan.com/2026/05/31/codex-cloud-environments-setup-scripts-caching-secrets-codex-universal/)

**GitHub Copilot coding agent (cloud agent)**
- It runs on an ephemeral GitHub Actions runner. Setup goes in `.github/workflows/copilot-setup-steps.yml`, which must have a single job named `copilot-setup-steps`. Only `steps`, `permissions`, `runs-on`, `services`, `snapshot` and `timeout-minutes` are honoured — [GitHub docs](https://docs.github.com/enterprise-cloud@latest/copilot/customizing-copilot/customizing-the-development-environment-for-copilot-coding-agent); [changelog 2025-07-30](https://github.blog/changelog/2025-07-30-copilot-coding-agent-custom-setup-steps-are-more-reliable-and-easier-to-debug/)
- One user (Nov 2025) reports that system-level changes made in setup (pip installs, `~/.aws`, Docker images) don't carry over to the agent's environment. This is not confirmed officially — [GitHub community #179978](https://github.com/orgs/community/discussions/179978)

**Google Jules**
- Each task runs in a short-lived VM that clones the repo and installs dependencies. Simple repos need no configuration: Jules infers setup and reads AGENTS.md / README. Complex repos can use a setup script with "Run and Snapshot". Since Aug 5, 2025, a successful setup creates a snapshot that later tasks load automatically — [Jules docs](https://jules.google/docs/environment/); [changelog 2025-08-05](https://jules.google/docs/changelog/2025-08-05)

**Devin (Cognition)**
- Each organisation has exactly one active machine **snapshot**, and every session boots a fresh copy of it. Changes made in a session never write back to the snapshot. Setup is declared in YAML "blueprints" with sections `initialize` (tools), `maintenance` (dependencies) and `knowledge`. Builds run on save and roughly every 24h. The recommended way to set up is to ask Devin to propose the blueprint itself. All organisations were migrated by June 30, 2026 — [Devin docs: blueprints](https://docs.devin.ai/onboard-devin/environment/blueprints.md); [blueprint reference](https://docs.devin.ai/onboard-devin/environment/blueprint-reference); [migration](https://docs.devin.ai/onboard-devin/environment/migration)

**Dagger container-use (MCP server)**
- Each agent gets a fresh container plus its own git branch, backed by git worktrees. Container state history is stored as git notes. `cu` commands open a terminal in an environment and merge it. The project says it is early and still changing — [container-use README](https://cdn.jsdelivr.net/gh/dagger/container-use@main/README.md); [container-use.com](https://container-use.com)
- There is no native Windows support ("Dagger has not released a Windows binary"), so WSL is the workaround — [Techdecline blog](https://techdecline.bearblog.dev/integrating-container-use-in-wsl-for-agentic-development)

**Claude Code (`--worktree`, subagent `isolation: worktree`)**
- `claude --worktree <name>` creates `.claude/worktrees/<name>/` on branch `worktree-<name>`. The docs say: "A worktree is a fresh checkout, so initialize your development environment there: ask Claude to install dependencies" — there is no dependency sharing — [Claude Code docs: worktrees](https://code.claude.com/docs/en/worktrees)
- `.worktreeinclude` (gitignore syntax) copies matching gitignored files. A `WorktreeCreate` hook replaces `git worktree add` entirely, for example to put the worktree elsewhere or use another VCS. With a hook, `.worktreeinclude` is skipped — [docs](https://code.claude.com/docs/en/worktrees); [hooks reference](https://code.claude.com/docs/en/hooks)
- Cleanup on exit: an unnamed clean worktree is removed automatically, and a dirty one prompts the user. Runs with `-p` are never cleaned up. Subagent and background worktrees are swept after `cleanupPeriodDays`, but only if they hold no work. Claude Code takes a `git worktree lock` while an agent runs. Worktrees created by a hook are never swept — [docs](https://code.claude.com/docs/en/worktrees)
- It enforces isolation by blocking Edit/Write calls aimed at the main checkout and git commands redirected there (`git -C`, `GIT_DIR` and similar). The docs admit that `cp` or shell redirects into the main checkout are not caught — [docs](https://code.claude.com/docs/en/worktrees)
- Windows: removal deletes only junctions and symlinks, not their targets (fixed in v2.1.205; before that, nested links could delete the folder they pointed to) — [docs](https://code.claude.com/docs/en/worktrees)

**Superset (superset.sh, desktop; page updated Oct 14, 2026; vendor source)**
- It uses a worktree per workspace. `.superset/config.json` holds setup, teardown and run commands, and a gitignored `config.local.json` holds personal steps. It sets `SUPERSET_WORKSPACE_PATH`. It **detects** listening ports per workspace but does **not** assign them, so users reserve ports in setup scripts themselves. Restoring a deleted workspace recreates the worktree, with committed work only. Each worktree "needs its own node_modules or virtualenv" and nothing is shared. Only macOS is shown — [Superset: isolate parallel agents](https://superset.sh/compare/isolate-parallel-coding-agents)

**Claude Squad / uzi / Crystal / Zed / Warp**
- Claude Squad is a terminal UI using tmux + git worktrees, AGPL-3.0 — [superset compare](https://superset.sh/compare/superset-vs-claude-squad)
- uzi gives each agent a worktree under `~/.local/share/uzi/worktrees/` plus a tmux session. Its only cleanup is `uzi reset`, which wipes everything — [uzi docs](https://www.mintlify.com/devflowinc/uzi/concepts/architecture)
- Crystal (Stravu) was a worktree-based Electron app, reported as deprecated in Feb 2026 in favour of Nimbalyst. Sources disagree on whether Nimbalyst is paid and closed-source — [dev.to (Stravu author)](https://dev.to/stravukarl/best-tools-for-managing-parallel-ai-coding-agents-in-2026-14l8); [abralo](https://abralo.com/alternatives)
- Zed Parallel Agents (April 2026) run threads in linked git worktrees, which is optional — [Zed docs](https://zed.dev/docs/ai/parallel-agents)

### Inferences
- Zero-config isn't solved locally by any tool. Every worktree tool hands dependency setup to a user-written script. Only the cloud agents (Jules infers setup, Devin proposes its own blueprint) try to configure themselves, and both then **snapshot** the result instead of reinstalling.
- The `.worktreeinclude` convention now appears in Claude Code, Codex and Conductor. It is a de facto standard Otaq could adopt for `.env`-style files.
- "Snapshot after setup, boot copies" (Codex cloud, Jules, Devin, and Sculptor per turn) is the cloud answer to install cost. Locally, the closest equivalent is a copy-on-write clone of a warmed checkout (see Q3).

### Gaps
- Terragon: I found nothing (no docs, no confirmation of a shutdown).
- Warp: sources conflict on whether local agents get worktrees.
- Conductor: no official statement on Windows/Linux support, ports, or what archiving deletes. Conductor Cloud's isolation model is unknown.
- Agent Farm and a "git-town-like" tool: not researched.
- Whether Sculptor's "local worktree mode" ever shipped, or whether its isolation changed after the Show HN, is unconfirmed.

## Q2. What users complain about

### Takeaway
The biggest complaint is disk blow-up from full per-worktree copies plus per-worktree dependency and build outputs, made worse by cleanup that silently fails. Next come shared runtime resources that worktrees don't isolate (ports, databases, Docker), Windows file locks blocking removal, and Rust's `target/` (either duplicated per worktree or serialised by Cargo's lock).

### Cited Findings
- **Cursor, Windows, Dec 13, 2025:** in a ~2 GB repo (IsaacLab), two worktrees used 9.82 GB in 20 minutes (8.5 GB + 1.32 GB). About 20 worktrees piled up to an estimated **140 GB in a week**. The reporter rated it "Yes – Cursor is unusable" and asked for a way to turn worktrees off. Staff blamed cleanup failing and pointed to the max-count and interval settings — [Cursor forum](https://forum.cursor.com/t/windows-request-to-disable-automatic-worktree-creation-critical-disk-space-issue/146189)
- One developer found 15 forgotten worktrees "consuming gigabytes" — [Upsun dev center](https://developer.upsun.com/posts/ai/git-worktrees-for-parallel-ai-coding-agents)
- Bazel, Pants and Nx monorepos keep gigabytes of cache that gets multiplied per worktree. For large repos with `node_modules/` or `target/`, worktree creation is "slow and wastes disk space" — [Upsun](https://developer.upsun.com/posts/ai/git-worktrees-for-parallel-ai-coding-agents); [dev.to](https://dev.to/battyterm/git-worktrees-changed-how-i-run-parallel-ai-agents-39pm)
- **Ports and databases:** two dev servers fight over 3000/5432/8080. The second one crashes or silently moves to another port, so a browser tab can end up showing the wrong agent's server. Worktrees share the local DB, the Docker daemon and cache directories — [dev.to: bugs git won](https://dev.to/robat_das_3c6e956212f6408/multiple-ai-coding-agents-same-repository-the-bugs-git-won-43fb); [Upsun](https://developer.upsun.com/posts/ai/git-worktrees-for-parallel-ai-coding-agents)
- Superset itself says worktrees "do not sandbox processes or prevent merge conflicts" — [Superset](https://superset.sh/compare/isolate-parallel-coding-agents)
- **Containers feel heavy:** a Sculptor user said multiple containerised apps, each with its own Postgres, "sounds very heavy for a dev laptop". Another reported the backend using lots of CPU and memory while idle — [Show HN Sculptor](https://hn.svelte.dev/item/45427697)
- **Windows file locks:** Claude Code worktree removal fails with "Device or resource busy" / "Permission denied" because Node processes (dev servers, node_modules) hold locks — [claudeissues #41740](https://claudeissues.com/issue/41740-worktree-removal-fails-file-locks-prevent-cleanup-on-windows)
- **Isolation leaks:** a Claude Code issue (Aug 2026) reports agents isolated in worktrees committing to each other's branches and adding stash entries in the main repo — [claudeissues #83311](https://claudeissues.com/issue/83311-parallel-isolation-worktree-agents-commit-to-each-others-branches-and-mutate-the)
- **Semantic merge conflicts** happen even with perfect file isolation (agent A renames a function that agent B still calls) — [zylos.ai](https://zylos.ai/research/2026-02-22-git-worktree-parallel-ai-development/)
- **Rust:** a shared `CARGO_TARGET_DIR` means "Cargo's directory lock makes builds wait for each other". Separate target dirs recompile everything per worktree — [jdx.dev mr-boxington](https://jdx.dev/posts/2026-09-05-introducing-mr-boxington?a=1); [Rust goals: build-dir layout](https://goals.rust-lang.org/2025h2/cargo-build-dir-layout.html)
- **Copilot:** users find it confusing which parts of the setup environment carry over to the agent — [GitHub community #179978](https://github.com/orgs/community/discussions/179978)

### Inferences
- Otaq's situation (10–15 tickets sitting in Review for days, each kept runnable) is the worst case for the count-and-age cleanup that Cursor and Codex use (keep 15–20, delete older). Those tools assume worktrees are throwaway, while Otaq needs them long-lived.
- Port collisions matter directly for "each must stay runnable". No surveyed tool assigns ports automatically; Superset only detects them.

### Gaps
- I found no HN or Reddit threads directly (search didn't surface them), and no Conductor-specific disk complaints. The Vibe Kanban GitHub tracker was not searched directly.

## Q3. Has anyone moved away from worktrees or built something clever (CoW, shared caches, snapshots)?

### Takeaway
No major tool has dropped worktrees for local work. The innovation sits in a layer of small open-source tools and PRs that **copy-on-write clone** (APFS clonefile / Linux reflink / ReFS block-clone) a warm checkout, including `node_modules` and `target/`, plus content-addressed build caches (pnpm store, sccache, mr-boxington). The cloud vendors use environment snapshots instead. The containers-not-worktrees route (Sculptor, container-use) is the main way to get process, port and DB isolation.

### Cited Findings
- **git-cow** wraps `git worktree add`: it runs `--no-checkout`, then clones tracked files plus ignored build caches in one `clonefile(2)` call on macOS, or `FICLONE` per file on btrfs/XFS/bcachefs/ZFS 2.2+. It falls back to a normal checkout. It carries over `target`, `node_modules`, `dist`, `_build`, `.gradle`, `bin/obj` and similar. It **excludes `.venv` and CMake build dirs because they contain absolute paths**. It waits up to about 1s so git's ctime-based change detection stays correct. Windows is not mentioned — [git-cow](https://github.com/onnimonni/git-cow)
- **t3code PR #12631** (opened Sep 19, closed unmerged Oct 1, 2026) proposed APFS clones for worktrees and an optional `node_modules` seed. The maintainer pushed back: tracked-file cloning would never trigger because of `text=auto` attributes, default-on was the wrong trade-off, and dependency seeding is a speed feature for pnpm, not a disk saver, so it needs measuring. Linux and Windows were split into separate PRs — [t3code PR](https://github.com/pingdotgg/t3code/pull/12631)
- Other projects ship CoW seeding of dependencies. Measured: a `node_modules` seed of about 43 MiB in about 1s. A file-by-file `cp -c -R` of an 87k-file node_modules took 14–20s, versus about 2s for one `clonefileat(2)`. Symlinking the main `node_modules` is rejected because an `npm install` inside the worktree can empty the original — [DashFrame PR #419](https://github.com/youhaowei/DashFrame/pull/419); [rove PR #1217](https://github.com/Sma1lboy/rove/pull/1217); [tars PR #1053](https://github.com/devlikebear/tars/pull/1053); [switchboard PR #107](https://github.com/tejasnafde/switchboard/pull/107); [cow](https://github.com/joeinnes/cow)
- Some projects clone first, then run `bun install --frozen-lockfile` so the result matches the branch's lockfile, or clone only when the lockfiles match — [DashFrame PR #419](https://github.com/youhaowei/DashFrame/pull/419); [sparse-worktrees blog](https://vivecuervo7.github.io/dev-blog/p/sparse-worktrees/)
- **mr-boxington (`mbx`, Sep 2026)** is a shared, self-pruning, content-addressed Rust build cache. A new checkout copies dependency units from another checkout so "the build only compiles or restores what differs". It restores via reflink, falling back to hard link and then copy. Garbage collection runs when a checkout disappears, after 30 days, or at a size budget (`gc.max_total_size`). It runs on Linux, macOS and Windows (ReFS reflink) — [jdx.dev](https://jdx.dev/posts/2026-09-05-introducing-mr-boxington?a=1)
- sccache as `rustc-wrapper` is the usual Rust workaround for sharing across worktrees. The upstream Cargo cross-workspace cache is a 2026 Rust project goal, not yet stable — [workmux caveats](https://workmux.raine.dev/guide/git-worktree-caveats.md); [Rust goals 2026](https://rust-lang.github.io/goals/2026/cargo-cross-workspace-cache.html)
- pnpm documents worktrees for multi-agent work: one content-addressable store shared by all worktrees. Boris Cherny (Anthropic) is quoted as endorsing it — [pnpm: git worktrees](https://pnpm.io/git-worktrees); [Upsun summary](https://developer.upsun.com/posts/ai/git-worktrees-for-parallel-ai-coding-agents)
- **Cloud snapshots:** Codex cloud caches containers for 12h with a maintenance script, Jules uses "Run and Snapshot", Devin keeps one org snapshot rebuilt about every 24h, and Sculptor snapshots per turn — see Q1 sources.
- **Containers instead of worktrees:** Sculptor ("does not use work trees") and container-use. The trade-offs are Docker weight, CPU and memory, and Windows gaps — [Show HN](https://hn.svelte.dev/item/45427697); [Techdecline](https://techdecline.bearblog.dev/integrating-container-use-in-wsl-for-agentic-development)

### Inferences
- A zero-config, stack-agnostic option for Otaq: create the worktree, then copy-on-write clone **all gitignored directories** from the main checkout (APFS clonefile; reflink on btrfs/XFS; ReFS block-clone on Windows Dev Drive), then let the stack's own incremental tool reconcile. Known traps: absolute paths (Python venvs, CMake), mtime/ctime freshness, and a fallback to a full copy on ext4 and NTFS (most Linux and Windows machines), which several projects refuse to do silently.
- Clones share blocks only until written. A Rust `target/` that is rebuilt after edits will diverge, so CoW saves the initial copy and the first build, not steady-state growth over days in Review. Content-addressed caches (mbx, sccache, the pnpm store) handle the long tail better.

### Gaps
- No benchmark of CoW-cloned Rust `target/` reuse (whether Cargo fingerprints stay valid after a clone) was found.
- I found no tool that automatically assigns ports per worktree with zero configuration.
- How common ReFS / Dev Drive is on Windows machines was not researched.
