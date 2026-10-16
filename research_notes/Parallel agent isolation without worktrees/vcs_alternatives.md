# Version-control-level alternatives to one git worktree per task

Researched 2026-10-16. Dates are noted where sources gave them. Many sources are vendor blogs or community skill pages; this is flagged inline.

## GitButler virtual / parallel branches

### Takeaway
GitButler puts several branches in ONE working directory. That directory holds the union (merge) of all "applied" branches, and changes are attributed to branches by file, hunk, or line range. This removes the per-task install/build cost entirely (one node_modules, one target/). The price: no filesystem isolation (agents can clobber each other's files), a branch cannot be run on its own without unapplying the others (which changes the shared directory for everyone), and the license is FSL (not OSI open source). It fits "agents write in parallel" well. It fits badly with Otaq's need for "10–15 Review branches each runnable on its own for days".

### Cited Findings
- **Mechanism:** multiple branches are applied to the working directory at once. Each branch is a "lane" with its own staging area, and you drag changes between lanes to commit them independently. Plain git has "one HEAD reference and one index", which is the constraint this works around. The working directory shows the combined work of all applied branches — [GitButler docs: virtual branches](https://docs.gitbutler.com/features/branch-management/virtual-branches)
- A generated `gitbutler/workspace` merge commit combines all applied branches, so the dev server, DB and standard tools see one state — [Trigger.dev blog, 2026-04-16](https://trigger.dev/blog/parallel-agents-gitbutler); [Wavect blog, 2026-10-08](https://wavect.io/blog/gitbutler-parallel-ai-agents-one-workspace/)
- **Claude Code hooks integration (July 2025, inferred from the image path):** Claude Code lifecycle hooks tell GitButler when a file is about to be edited and when editing is done, along with a session ID. GitButler creates one branch per session and assigns that session's changes to it. When a chat ends, a hook commits the work (a quick commit storing the prompt, then an AI rewrite of the message in the background). Result: one commit per chat round, one branch per session. The CLI is called "experimental". The post does not discuss overlapping edits or testing in isolation — [Scott Chacon, GitButler blog](https://blog.gitbutler.com/parallel-claude-code)
- **Agents tab:** each Claude Code session is tied to a branch, and "changes are assigned to the respective branch as long as they are mutually exclusive". It runs in the same working directory, "making it unnecessary to run setup scripts or configure worktrees for each task". Only Claude Code is supported — [GitButler agents tab blog](https://blog.gitbutler.com/agents-tab) (via search snippet; the docs page docs.gitbutler.com/features/agents-tab returned 404 on 2026-10-16)
- **Trigger.dev production report (2026-04-16):** they dropped worktrees for their monorepo (Postgres, Redis, ClickHouse, Remix). Worktree pain points were port conflicts, shared DB migrations interfering, duplicated services, repeated dependency installs, and disk (they cite a forum report of ~9.82 GB for two worktrees of a ~2 GB codebase). They rejected the hooks model ("hardwires one session to one branch"). Instead they use a skill plus a CLAUDE.md line telling Claude to use `but` for git operations — [Trigger.dev](https://trigger.dev/blog/parallel-agents-gitbutler)
- **CLI commands agents use:** `but setup`, `but status --json`, `but commit <branch> -m … --changes <file-ids>`, `but stage <file> <branch>`, `but absorb`, `but amend`, `but squash`, `but reword`, `but push`, `but pr new`, `but undo`; flags `--json` and `--status-after` — [Trigger.dev](https://trigger.dev/blog/parallel-agents-gitbutler). A plain `but commit` takes ALL uncommitted changes, so agents must select specific files or hunks — [Wavect](https://wavect.io/blog/gitbutler-parallel-ai-agents-one-workspace/)
- **Limits Trigger.dev admits:** if several agents modify the same file for different branches, "conflicts and interference" occur (they cite GitButler maintainers on races). Use worktrees or sequential work for overlapping tasks. Tests with side effects on a shared DB need real isolation. Tasks needing different build flags or env vars don't work well in one directory — [Trigger.dev](https://trigger.dev/blog/parallel-agents-gitbutler)
- **Same file or lines across branches (hunk/dependency locks):** dependency tracking is per line range, "somewhat like git-blame". It keeps an ordered list of hunk ranges tracking which commit owns which lines. If a file's lines were committed on branch A, changes to them cannot be committed to branch B: they stay "unassigned". The suggested fix is to stack B on A (`but move <b> <a>`). GitButler cannot split a single hunk by ID when wanted and unwanted edits are in the same hunk. A known issue: `absorb` can pick the wrong commit when a hunk is locked by another branch — [community skill pages via search: skillselion `but` skill](https://skillselion.com/skills/gitbutlerapp/gitbutler/but); [issue #2513 line-based moves](https://upd.dev/gitbutlerapp/gitbutler/issues/2513) (community and issue-tracker sources, not official docs)
- **Failure modes in one shared directory (Wavect, 2026-10-08):** "a branch label is not a file lock". A second agent writing a stale full-file replacement erases the first agent's change. Hotspots (shared types, routing, lockfiles, migrations, generated code) need ownership rules. A green combined build does not prove each branch works alone, because a branch can silently depend on another branch's API change. Tests run while code changes may mix states. Hot reload is not guaranteed and file updates are not atomic. `but pull` rebases all applied branches, so it must be coordinated with active agents. Workspace-wide operations (apply/unapply) need one designated coordinator — [Wavect](https://wavect.io/blog/gitbutler-parallel-ai-agents-one-workspace/)
- **Running one branch alone:** there is no dedicated "test this branch in isolation" feature. `but unapply` removes a branch's (whole stack's) changes from the working directory, and `but apply` brings them back. Unapplying the others is the closest workaround — [GitButler docs: but unapply](https://docs.gitbutler.com/commands/but-unapply); [but apply](https://docs.gitbutler.com/commands/but-apply). A community skill warns that `but pull` before unapplying a merged branch errors with orphan-branch conflicts — [skillsmp gitbutler skill](https://skillsmp.com/creators/gaboe/opencode-gitbutler/skill-gitbutler)
- **License:** Functional Source License 1.1 ("Fair Source"). You may run, inspect and contribute, but not build a competing product. Each version converts to MIT after two years. It is not OSI-compatible because of the non-compete clause — [GitButler: now fair source](https://blog.gitbutler.com/gitbutler-is-now-fair-source); [repo mirror README](https://upd.dev/gitbutlerapp/gitbutler)
- **Maturity:** the `but` CLI runs on the same Rust backend engine as the desktop app (Tauri/Rust/Svelte) — [repo README mirror](https://upd.dev/gitbutlerapp/gitbutler). A third-party profile says a CLI technical preview launched April 2026, alongside a $17M Series A — [ai-market-watch (low authority)](https://www.ai-market-watch.com/company/gitbutler). Release 0.19.7 added `but tui` — [gittrend](https://gittrend.io/repo/gitbutlerapp/gitbutler)
- **Windows:** the desktop app ships as a Windows x64 MSI (WinGet id GitButler.GitButler) — [silentinstallhq listing](https://app.silentinstallhq.com/installer/110679ec8d4d8d3220769d6e4b644fa6050976bceebf38d02c8bcbcbb54897e9)
- **Community multi-agent skills** exist, e.g. `but rub` for assigning files to branches or moving work between agents after the fact — [outfitter-dev gitbutler-multi-agent skill](https://skills.cat/skills/outfitter-dev/agents/gitbutler-multi-agent)

### Inferences
- **Disk/setup:** close to zero per task. One checkout, one dependency install, one build output. Zero stack knowledge is needed to set it up, since GitButler works on any git repo.
- **Fit with Otaq's Review column:** poor. 10–15 branches waiting for review all sit merged together in the one directory. "Run ticket X's app" means unapplying 14 other branches. That rewrites files under any agent still working in Dev, and invalidates the incremental build each time. A validator reading branch X sees the union, not X.
- **License and dependency risk:** FSL bars a "competing product". Otaq, a kanban that drives agents with branch management, could arguably compete with GitButler's Agents tab. Embedding or redistributing GitButler needs a license reading, and depending on users installing it conflicts with zero configuration.
- **Ownership:** it moves the "who owns this hunk" problem into the VCS. Overlapping edits make branches implicitly stack. That collides with Otaq's decision that stacking is deferred (beads-style deps).

### Gaps
- No official doc found on exact behaviour when two *uncommitted* sessions edit the same hunk at the same moment (which branch wins the assignment). The hooks-mode post is silent on this.
- Whether the Windows MSI puts `but` on PATH, and how the hooks behave on Windows: not confirmed.
- No evidence of a GitButler MCP server specifically (I found the hooks, the CLI with `--json`, and skills). Not confirmed either way.
- The official Agents tab doc page returned 404 (it may have moved).

## Jujutsu (jj)

### Takeaway
jj workspaces are structurally the same as git worktrees: one shared repo store plus a full separate working directory per workspace. They do not share build output, so they do not reduce the node_modules/target/venv cost at all. What jj adds: the working copy is itself a commit, snapshotted automatically, so agent edits are never "lost uncommitted" and conflicts can be committed and resolved later. It also adds an operation log. Secondary workspaces have no `.git`, so plain git tooling inside them is awkward.

### Cited Findings
- **Workspaces:** one repo can back multiple working copies. `jj workspace add` creates one, and each workspace can check out a different commit. A workspace is a working-copy directory plus a `.jj/` that links back to the main repo. `jj workspace forget` leaves the files on disk. `jj workspace remove` deletes the directory, including ignored files. On Windows you cannot remove the directory you are running in, so use `jj -R` — [jj docs: working copy](http://docs.jj-vcs.dev/latest/working-copy/)
- **Working copy as a commit:** most jj commands first snapshot changed files into the working-copy commit. New files are auto-tracked, deletions auto-untracked, and each snapshot is an operation in the op log — [jj docs](http://docs.jj-vcs.dev/latest/working-copy/)
- **Stale working copies:** rewriting a commit checked out in *another* workspace makes that workspace "stale". The fix is `jj workspace update-stale`, which can create a recovery commit — [jj docs](http://docs.jj-vcs.dev/latest/working-copy/). This is relevant when one agent rebases something another agent's workspace sits on.
- **Conflicts are first-class:** jj writes conflict markers into files, tracks the sides, and allows partial or deferred resolution. Directory/file/symlink conflicts are poorly supported (issue #19) — [jj docs](http://docs.jj-vcs.dev/latest/working-copy/)
- `jj workspace add` puts the new working-copy commit on the same parents as the current one unless `-r` is given. Sparse patterns are inherited by default (`--sparse-patterns full|empty` to change) — [jj-workspace-add man page](https://man.archlinux.org/man/jj-workspace-add.1.en)
- Workspaces share commit history and the operation log; each has its own `@` — [using-jj-workspaces skill (community)](https://skills.cat/skills/edmundmiller/dotfiles/using-jj-workspaces)
- **Git interop:** secondary jj workspaces lack a `.git/` folder, and `git worktree add` directories are not visible to `jj workspace list` — [carryctx test source](https://docs.rs/crate/carryctx/0.4.4/source/tests/worktree_test.rs). A May 2026 jj branch fixes git commands run from a secondary colocated workspace updating git HEAD through the *default* workspace. It may not be released yet — [jj compare colocated-workspaces](https://upd.dev/jj-vcs/jj/compare/main...colocated-workspaces). `gh` can't find the repo in non-colocated setups without `GIT_DIR=.jj/repo/store/git` — [jj docs/github.md mirror](https://git.joshthomas.dev/mirrors/jj/src/branch/main/docs/github.md)
- Claude Code's `--worktree` flag reportedly fails silently on jj colocated repos (detached HEAD) — [claudeissues #27466](https://claudeissues.com/issue/27466-worktree-flag-silently-fails-on-jj-jujutsu-colocated-repos-detached-head)
- **Agent tooling around jj:** `wsg` manages a pool of jj workspaces as reusable worker slots (`wsg pool <N>`) — [jarvvski/wsg](https://github.com/jarvvski/wsg). One author describes numbered jj workspaces as "durable worker slots", where a finished workspace is moved elsewhere in the tree rather than deleted — [Medium: "Git Wasn't Designed for Agents"](https://medium.com/@shrmtv/git-wasnt-designed-for-agents-7a5aaa11698f). The "durable slot" idea could keep build output warm across tasks.
- A rules page recommends one workspace per separable agent task, because a shared working copy lets agents "overwrite each other, mix unrelated diffs, or make validation ambiguous" — [joshka.net rule (2026)](https://www.joshka.net/practice/rules/agent-workflow/agent-isolate-workspaces-by-task/)
- Gradle tried jj and did not adopt it, for a "petty" reason — [Gradle blog](https://blog.gradle.org/the-petty-reason-we-didnt-end-up-using-jj-at-gradle) (not fetched; reason unverified)

### Inferences
- **Disk cost:** the same as git worktrees, since jj shares only the repo store. Build output is per directory either way.
- **Setup cost:** it adds a dependency (the jj binary) and changes the user's VCS model. Colocated mode keeps `.git` only in the main workspace, which conflicts with tools (Claude Code `--worktree`, `gh`, IDEs) in secondary workspaces. For a zero-config product targeting arbitrary git users, jj is a heavy ask with no disk win.
- The useful ideas to borrow are auto-snapshot (never-lost agent edits) and "slot" reuse (keep N warm directories and re-point them to different changes instead of creating and deleting them). Both can be imitated with plain git (`git switch` in a pooled worktree, plus periodic WIP commits).

### Gaps
- No official jj document on agent parallelism.
- Windows support quality for jj was not verified in this session.
- No source quantified jj workspace disk/time versus git worktree.

## Shared single checkout with coordination (locks, patches, stash per agent, per-agent commits)

### Takeaway
Nearly every source says agents that write in parallel to one checkout fail silently: the later write wins, builds and tests see half-done edits, and the index lock contends. The workable coordination schemes are (a) advisory file reservations (MCP Agent Mail, with an optional pre-commit guard), (b) directory ownership assigned in the prompt, and (c) task-level lock files with a separate clone per agent (Anthropic's C compiler). I found no tool that gives true per-agent isolated *execution* inside one shared tree. The user's observation that "sub-agents in one checkout worked fine" matches the case where tasks touch disjoint files and nobody runs a branch in isolation.

### Cited Findings
- Two agents in one directory fail silently: neither git nor the agents error when one overwrites the other's uncommitted edits. Git conflict detection compares commits, so concurrent writes to one tree never reach it. Other failure modes are test port contention, `index.lock` failures, corrupted build states, and test failures unrelated to logical conflicts. Separate branch names protect only history, not filesystem state. Read-only agents can safely share — [search summary of fast.io, Medium (koriigami), mindstudio, openreplay](https://medium.com/@koriigami/running-parallel-claude-code-agents-on-a-single-codebase-927d70e7349d) (third-party blogs)
- When two agents edit the same file, "they overwrite each other's changes without noticing, causing file conflicts and syntax errors". Subagents isolate context, not files. Directory ownership in the orchestrator prompt is used as a third option (e.g. "Backend agent: owns src/api/billing/ and src/db/migrations/") — [fast.io agent teams guide](https://fast.io/resources/claude-code-agent-teams-setup/); [morphllm](https://www.morphllm.com/claude-code-git-worktree) (third-party)
- Claude Code agent teams (experimental, off by default) coordinate via a shared task list with locking to stop two agents claiming the same *task*. That is not the same as file locking — [fast.io](https://fast.io/resources/claude-code-agent-teams-setup/) (third-party; official docs not checked)
- Claude Code subagents can opt into `isolation: worktree` in frontmatter, and `--worktree` creates `.claude/worktrees/<name>` on branch `worktree-<name>` — [morphllm](https://www.morphllm.com/claude-code-git-worktree) (third-party)
- **MCP Agent Mail:** advisory file reservations (leases) on files or globs, keyed to repo-root-relative paths. All agents in one working directory share a project namespace. An optional pre-commit guard blocks commits that conflict with another agent's exclusive reservation — [MCP Agent Mail docs](https://docsearch.algolia.com/mcp/docs/repo/dicklesworthstone/mcp_agent_mail)
- **Anthropic C compiler (Nicholas Carlini):** 16 agents, ~2,000 sessions, ~$20k, ~100k-line Rust C compiler. Each agent ran in its own Docker container with its own clone of a shared bare repo, not a shared tree. Agents claimed tasks by writing a file to `current_tasks/`, with git push races deciding who wins. Work ended with pull, merge, push and lock removal. Merge conflicts were frequent — [Anthropic engineering](https://anthropic.com/engineering/building-c-compiler)
- GitButler's model is itself "per-agent commits on one tree", with the failure modes listed in the GitButler section (stale full-file writes, combined build ≠ each branch builds, tests mixing states) — [Wavect](https://wavect.io/blog/gitbutler-parallel-ai-agents-one-workspace/)

### Inferences
- **Patches applied later** (agent works on a scratch copy, emits a diff, the diff is applied to the shared tree) is just a worktree with extra steps. The agent still needs somewhere to build and test its version.
- **Stash per agent** doesn't work concurrently: stash is repo-global and operates on the single working tree and index.
- **The core blocker for Otaq:** the Review column needs each ticket's version *runnable* (app, shell, validator) for days. A shared tree can hold only one version of each file at a time. So any shared-tree scheme still has to materialize a per-ticket tree on demand. That turns into "cheap tree + shared build cache" rather than "no tree".
- One agent's broken build or syntax error blocks every other agent's build and test in a shared tree. With 10+ agents this happens often.

### Gaps
- No academic paper found on concurrent multi-agent editing of one working tree (the searches turned up only blogs and tools).
- No measured data on how often parallel subagents in one checkout collide.

## Sparse checkout, partial clone, git alternates

### Takeaway
These cut the size of *source* and *history*, which is not Otaq's problem (the source is 31 MB). Git worktrees already share the object store. None of these touch node_modules, target/ or venv, so they give almost no relief.

### Cited Findings
- Linked worktrees share everything except per-worktree files. All `refs/` are shared (except bisect/worktree/rewritten), pseudo-refs like HEAD and the index are per worktree, and config is shared unless `extensions.worktreeConfig` is enabled — [git-worktree docs](https://git-scm.com/docs/git-worktree)
- `git worktree add --no-checkout` exists for customization "such as configuring sparse-checkout". `core.sparseCheckout` can be set per worktree via `extensions.worktreeConfig` — [git-worktree docs](https://git-scm.com/docs/git-worktree)
- jj workspaces inherit sparse patterns by default — [jj-workspace-add man page](https://man.archlinux.org/man/jj-workspace-add.1.en)

### Inferences
- Because worktrees already share objects, alternates and partial clone add nothing over worktrees on one machine. Sparse checkout could shrink a worktree's source footprint, but that needs stack and repo knowledge (which paths a ticket needs), which conflicts with zero configuration. Its saving on a 31 MB source tree is negligible.
- The real cost lever lies outside version control: sharing or caching build output and dependencies across trees (e.g. a shared Cargo target dir, pnpm's content-addressed store, copy-on-write clones of a warm tree). That is likely covered by other researchers. This VCS-level research suggests the VCS layer is not where the 36 GB comes from.

### Gaps
- No disk or time measurements were gathered for sparse worktrees.
- Not investigated here: copy-on-write filesystem clones (APFS clonefile, Btrfs reflink, ReFS block cloning on Windows Dev Drive) of a warm worktree including build output. This is the most promising adjacent lever, but it falls outside the VCS scope.

## Summary comparison (inferred from the findings above)

| Approach | Disk per task | Setup per task | Branch runnable alone? | Windows | Stack knowledge | Main failure mode | Maturity |
|---|---|---|---|---|---|---|---|
| git worktree (baseline) | full build output + deps | full install + cold build | yes | yes | none for checkout; needed for install | disk and time | mature |
| GitButler parallel branches | ~0 | ~0 | only by unapplying the others (disturbs everyone) | MSI exists; CLI on Windows unverified | none | silent overwrites, hunk locks force stacking, combined build hides per-branch breakage | CLI tech preview ~Apr 2026; FSL license |
| jj workspaces | same as worktree | same as worktree + jj install | yes | yes (with caveats; unverified) | none | stale workspaces, no `.git` in secondary workspaces | jj pre-1.0, active |
| Shared checkout + locks/ownership | ~0 | ~0 | no | yes | none | silent overwrites, one broken build blocks all, tests see others' edits | ad hoc tools (Agent Mail), experimental |
| Sparse / partial / alternates | saves source only | small | yes | yes | sparse needs repo knowledge | doesn't address build output | mature git features |
