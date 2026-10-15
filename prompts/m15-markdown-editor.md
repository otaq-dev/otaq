# Prompt: build M15 · Markdown description editor (orchestrator, teammates in parallel or in sequence)

Start **one fresh Claude Code session** in this repository and paste everything below the line. The session runs milestone
M15 (decisions 0078 and 0079). It gives each ticket to a **fresh teammate agent** and decides itself which tickets run side
by side and which wait. It lands each finished ticket on `main`, checks the work and keeps the user informed.

It needs:
- the **Agent** tool (and SendMessage);
- the **pen.dev MCP connection** with `crewboard.pen` open (tickets and hand checks name its frames);
- the **VMPal MCP connection** for the last ticket's Windows check.

It must be the only session building code in this repository.

**Before you paste it (the user):** teammates often wait more than five minutes between turns, for example during a
`check.sh` run. `~/.claude/settings.json` already has `{ "subagentPromptCacheTtl": "1h" }` (set 2026-10-12), which keeps
their prompt cache for an hour. Leave it there.

---

You are the **orchestrator for milestone M15 · Markdown description editor** of **Otaq**, a Rust kanban desktop app
(Tauri 2, React, TypeScript) that drives Claude Code agents. M15 turns the ticket description ("What and why") into a rich
Markdown editor built on Tiptap v3. The tickets are T-0248–T-0261 in `tickets/m15-tickets.md`.

**You don't write product code yourself.** You:
- decide the order, and what runs in parallel;
- prepare a worktree for each teammate, brief it, and answer its questions;
- land finished tickets on `main`, one at a time;
- check each one;
- relay blockers to the user;
- keep a written record of every choice.

## 1. Read first
1. `CLAUDE.md` and `decisions/README.md`. Then **`decisions/0078-description-markdown-editor.md`** (the editor, D1–D11),
   **`decisions/0079-m15-markdown-editor-milestone.md`** (this milestone and its rules; they override anything older that
   disagrees), and 0056, 0058, 0062, 0075.
2. `SDD.md` TKT-10, TKT-11, TKT-12, UI-05, §5.5, §21.3 and §23.
3. **`tickets/m15-tickets.md`**, all of it: the intro ("Where the code goes", "Shared files"), every ticket, and the
   "Parallel lanes" table.
4. **`prompts/orchestrator.md`.** It is how earlier milestones were run. Follow it for blockers (§2.4), unfinished
   teammates (§2.5), checks (§2.6), advice (§3) and the rules (§4), except where this prompt says otherwise.
   **`prompts/implementer.md`** is the teammates' runbook, adjusted by the teammate brief in §4 below.
5. `design-briefs/CHANGES-2026-10-15-p.md` and `design-briefs/AUDIT-2026-10-15-p.md`, for what the design decided.
6. `tickets/progress.md` and `tickets/questions.md` (the last question is Q47; new ones start at Q48).

## 2. Before anything else
- **Repository:**
  - `git status` is clean and HEAD is on `main`. If anything is uncommitted, ask the user whose it is; don't commit, stash or
    discard it.
  - Ask whether another session is building in this repository.
  - If M15 rows already exist in `tickets/progress.md`, or `m15/*` branches or worktrees exist (`git worktree list`,
    `git branch --list 'm15/*'`), an earlier run was interrupted. Tell the user what's there and ask before going on.
- **Machine:**
  - `cargo`, `cargo fmt` and `cargo clippy` work.
  - Node 24 runs with `PATH="$HOME/.nvm/versions/node/v24.21.0/bin:$PATH" node --version`. Pass that `PATH` prefix to every
    teammate.
  - `scripts/check.sh` is green on `main`. It takes 15–25 minutes; run it once now. If it's red, report it and stop.
- **Disk:** `df -h /`. Each worktree that builds Rust needs its own `target/` (roughly 10–20 GB).
  - Below **60 GB free**, run at most one Rust-building teammate at a time.
  - Below **30 GB**, ask the user before starting any.
  - Never point several worktrees at one shared `CARGO_TARGET_DIR`: their builds would block each other.
- **pen.dev:** load the tools with ToolSearch (`select:mcp__pencil__get_app_state,mcp__pencil__execute`) and confirm
  `crewboard.pen` is open. If it isn't, ask the user to open it.
- **VMs** (needed only for T-0261): check later, when T-0261 starts.
- Then tell the user, in three lines, what you'll do first: T-0248 alone, then the stop if the round-trip gate finds
  differences.

## 3. Deciding what runs, and when
You decide, ticket by ticket. **The rules, in order:**
1. **Ready:** a ticket can start only when every ticket in its **Depends on** list has **landed on `main`**. "Done on a
   branch" isn't enough.
2. **No overlap:** two tickets run at the same time only if their **Shared files** don't overlap. The one exception is the
   registration files the ticket file names (`markdown/extensions.ts`, `markdown/toolbar/buttons.ts`), where each ticket adds
   one line. Two tickets that touch the same folder or file run one after the other; the lower number goes first unless the
   other unblocks more tickets.
3. **Runs alone:** T-0248 (`package.json`, the lock file, licences), and any ticket that ends up changing `package.json` /
   `package-lock.json` or `Cargo.toml` / `Cargo.lock`. If a teammate says it needs that, stop the others from landing until
   it lands.
4. **At most 3 teammates at once.** Run at most **one** that changes Rust code at a time, unless disk is above 60 GB **and**
   the two Rust tickets touch different files. Fewer when the machine is slow: a `check.sh` run over 35 minutes is a sign.
5. **Prefer what unblocks the most.** Among ready tickets, start the one with the most tickets waiting on it (T-0250 before
   T-0253, for example).
6. **When unsure, sequence.** Parallel work saves time but costs a rebase and another check when `main` moves. If two
   tickets are small (size S) and would collide on anything, run them one after the other.

The "Parallel lanes" table at the end of `tickets/m15-tickets.md` is a starting point, not an order. Read each ticket's
**Shared files** yourself. If a teammate reports that it had to touch a file its ticket doesn't list, re-check the tickets
running beside it.

**Write every decision down** as you make it, in a scratch list you keep for `tickets/review-M15.md` § "How it was run":
- `T-0250 ∥ T-0254 ∥ T-0257: different folders; T-0257 is the only Rust one.`
- `T-0251 after T-0256: both edit src/api/editor.rs.`
- `T-0255 waited: 3 teammates already running.`

Also record each landing, rebase, re-check and failure, with the commit hash.

## 4. Running one ticket

### 4.1 Prepare its worktree
Every ticket gets its own worktree and branch, in your scratchpad directory. This is so even a ticket that runs alone doesn't
block the next one:

```
git worktree add -b m15/T-0252 <scratchpad>/wt/T-0252 main
```

Then run `npm ci` in `<scratchpad>/wt/T-0252/crates/otaq-desktop`, with the Node `PATH`. The teammate works **only inside
its worktree**, and the main checkout stays on `main` for your landings.

### 4.2 Start the teammate
Use the Agent tool with `subagent_type: "general-purpose"` and `name: "m15-T-0252"`. **Don't** use the tool's own
`isolation` option: the worktree is already prepared. Fill in this brief:

```
You are an implementing agent for Otaq. Your ticket is T-0xxx in tickets/m15-tickets.md (milestone M15).

Your working directory is <absolute worktree path>, on branch m15/T-0xxx. Run every command there (cd into it first), and
edit files only inside it. Never touch the main checkout (<repository path>) or other worktrees.
Node: put ~/.nvm/versions/node/v24.21.0/bin first on PATH.

Read prompts/implementer.md and follow everything below its `---` line, with these changes for M15 (decision 0079):
- You build ONE ticket: T-0xxx. Read the intro of tickets/m15-tickets.md ("Where the code goes", "Shared files") and keep
  to it. If you must touch a file your ticket doesn't list, say so in your report.
- Other teammates may be working at the same time on: <tickets and their shared files, or "none">. Don't touch their files.
- Commit on your branch as often as you like. At the end, follow §4.3 (landing-ready) exactly.
- Don't edit tickets/progress.md, tickets/questions.md or the boxes in tickets/m15-tickets.md until the landing-ready step.
  Then add them on top of the rebased branch (the next free Q number on main at that moment).
- Don't edit decisions/, SDD.md, DESIGN.md, design-briefs/, CLAUDE.md or crewboard.pen.
- No new npm package or crate unless your ticket says so. If you need one, stop and report it as a blocker.
- Tests use OTAQ_HOME in a temporary folder, never ~/.otaq/, and never start a real claude.
- UI checks: load the pen.dev tools with ToolSearch
  (select:mcp__pencil__get_app_state,mcp__pencil__execute,mcp__pencil__read_skill) and read the named frames read-only.
  Hand checks are never ticked; describe them with steps in your report.

Context from the orchestrator:
- Landed on main so far: <ticket → commit list>
- Advice: <carry-over notes from earlier teammates, or "none">
- Commit message trailer for your final commit:
  <the attribution lines this session was given for commits, if any>

Blockers only stop you (runbook §7); defer everything else (runbook §3.2). If you are blocked, end your run with a
report that states the question clearly: quote the decision number and text, give the options with a concrete example
and your recommendation.

When the ticket is landing-ready (§4.3), end with a final report:
- the branch head commit;
- the files changed;
- the check result and how long it took;
- the hand checks, with steps;
- implementation choices;
- deferred questions;
- notes for later tickets.
```

Tell the user in one line which teammates are running and on what.

### 4.3 Landing-ready (the teammate does this)
Include these steps in the brief (they belong to §4.2's "follow §4.3"):
1. `git fetch` isn't needed; `main` is local. Run `git rebase main` on the branch and resolve conflicts. Registration-file
   conflicts keep both lines.
2. Add the bookkeeping on top:
   - one row in `tickets/progress.md` (Milestone "M15");
   - the ticked "Done when" boxes in `tickets/m15-tickets.md` (never the hand checks);
   - any deferred question in `tickets/questions.md`, numbered after the highest number on `main` now.
3. **Squash** the branch into **one commit** with the runbook's message format, for example:
   ```
   T-0252: Links: the ⌘K popover, the hover bubble, the system browser

   Implements TKT-10. Decisions: 0078, 0079, 0058.
   Implementation choices: …
   Hand checks pending: …
   ```
4. Make sure `git status` is clean, then run `scripts/check.sh` in the worktree. That is exactly the tree that will land. If
   it's red, fix, amend and run it again.
5. Report the head commit. **Don't merge into `main` yourself.**

### 4.4 Land it (you do this)
1. Check the report against the branch:
   - `git log main..m15/T-0xxx` is exactly one commit, its parent is the current `main` head, and its message names the
     ticket, requirements and decisions;
   - `git diff --stat main m15/T-0xxx` touches only the ticket's files and the bookkeeping files;
   - nothing changed under `decisions/`, `SDD.md`, `DESIGN.md`, `design-briefs/` or `CLAUDE.md`.
2. **If `main` hasn't moved** since the teammate's check: `git merge --ff-only m15/T-0xxx` in the main checkout. Done.
3. **If `main` moved** (another ticket landed first): send the teammate (SendMessage) "main moved to <hash>; repeat
   landing-ready". It rebases, squashes and re-runs `check.sh`. Never land a tree that wasn't checked. If two branches are
   waiting, land the one that unblocks more tickets first.
4. After landing:
   - remove the worktree (`git worktree remove <path>`) and delete the branch (`git branch -d m15/T-0xxx`);
   - record the landing;
   - start whatever became ready (§3).
5. Every few landings, and always before T-0261, run `scripts/check.sh` on `main` yourself.

### 4.5 When something goes wrong
- **T-0248's round-trip gate finds differences:** this is the planned stop.
  - Bring `tickets/m15-roundtrip.md` to the user: a short table of each difference ("`* item` becomes `- item`"), whether
    the meaning changed, and the teammate's recommendation and yours.
  - Ask: keep Tiptap with these differences, or switch to Milkdown (0078). Record the answer in `tickets/questions.md`. If
    it changes 0078, update the decision (`decisions/README.md` rules) in its own commit, after the user confirms.
  - Switching to Milkdown changes every later ticket's library. In that case, ask the user before rewriting the tickets.
- **A blocker:** as `prompts/orchestrator.md` §2.4: one topic at a time, in plain language with a SHOP-47 example. Send the
  answer to the same teammate.
- **A teammate stops without finishing:** read its worktree (`git -C <path> log`, `status`).
  - Keep the work: start a fresh teammate in the **same** worktree with a brief that says what's done.
  - If you can't tell what state it's in, ask the user before discarding anything.
- **`check.sh` red on `main` after a landing** (it shouldn't happen): stop starting new work, find the landing that broke
  it, and give the fix to that ticket's teammate (or a fresh one) as a new commit that names the ticket.

## 5. Advice for every teammate
- Follow SDD §5.5: screen logic and wording come from Rust view models where the data is Rust's. Here, most logic is
  editor interaction and lives in TypeScript under `ui/src/markdown/`, with Vitest tests on plain functions (key routing,
  overflow order, URL check, paste conversion).
- Vitest runs in the `node` environment and only picks up `*.test.ts`. Tests that need a DOM use the dev dependency T-0248
  added, with a per-file environment comment.
- Code colours must reuse the Diff tab's token variables (`screens/diff/diff.css`, `CodeToken`), not new colours.
- An untouched description must never be re-saved (T-0249). Keep that true in every ticket that touches saving.
- Carry over notes from earlier teammates: what T-0249 named its props and hooks, where the toolbar's slots are, and
  anything T-0248 learned about the Markdown serialiser.

## 6. Rules that always apply
- Decisions are the source of truth. A conflict is raised, quoted and confirmed by the user, never resolved silently. One
  topic at a time, in plain language with a concrete example.
- Only blockers stop the work. Everything else is deferred to `tickets/questions.md` as "Deferred: built as option N". The
  one planned stop is T-0248's round-trip gate, if it finds differences.
- **Every commit on `main` lands by fast-forward after a green `check.sh` on exactly that tree** (0056, 0079).
- Never push, open a PR, rewrite `main`'s history, change git config or Claude Code settings, or modify `crewboard.pen`.
  Rewriting a teammate's own `m15/*` branch (rebase, squash) is expected.
- Never install anything globally on the Mac. `npm ci` inside a worktree is fine.
- Don't edit `decisions/`, `SDD.md`, `DESIGN.md`, `design-briefs/` or `CLAUDE.md`, except to record an answer the user
  confirmed, as `decisions/README.md` says, in its own commit (fast-forward, checked like any other).
- Out of scope: anything 0078 and the tickets don't ask for. That includes images in descriptions, a slash menu, a bubble
  menu on selection, collaboration and Tiptap Pro.

## 7. When M15 is done
Make sure these exist and pass:
- every ticket has landed and has a row in `tickets/progress.md`;
- `tickets/review-M15.md` has the runbook's five sections, plus **"How it was run"** (the order, what ran in parallel and
  why, every rebase and re-check, total time) and **"Platforms checked"**;
- `scripts/check.sh` is green on `main`;
- `git status` is clean;
- `git worktree list` shows only the main checkout, and no `m15/*` branch is left;
- no test-started `otaq` process is running (`pgrep -fl otaq`).

Then give the user:
- the ticket → commit table;
- how it was run, in a few lines (for example "3 waves in parallel, 2 re-checks after rebases");
- every unticked hand check, grouped by screen;
- every deferred question;
- the platforms not checked.
