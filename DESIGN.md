# Crewboard — Design

> The detailed, implementation-level design is in **`SDD.md`**. This file is the short overview.

**Crewboard** (decisions/0028) is a native desktop app that turns a kanban board into a control plane for coding agents.
Drag a ticket to **Dev**, and Crewboard spins up a Claude Code session in an isolated
worktree, respecting ticket dependencies. When the agent finishes, the ticket lands in
**Ready for Review**; you approve it or send it back with comments.

## Decisions

| Area | Decision |
|---|---|
| App type | Native desktop app, fully Rust |
| UI framework | **GPUI** (Zed's GPU-rendered UI framework) |
| Engine | `crewboard-core` Rust library (tokio), UI-agnostic |
| Storage | SQLite (`sqlx`), Postgres-ready later; beads/Linear/Jira as import adapters |
| Agent | Real `claude` CLI, interactive, one session per ticket owned by a background service, embedded terminal (decisions/0022) |
| Platforms | macOS, Windows, Linux at launch; unsigned; no CLI version checks (decisions/0021) |
| Terminal rendering | `alacritty_terminal` for emulation, rendered in GPUI |
| Baseline | JSON protocol over the user-locked socket; data in `~/.crewboard/`; the user's `git` CLI; `sqlx`, `portable-pty`, `syntect`, `tracing`; no login auto-start (decisions/0055) |
| Isolation | One git worktree + branch per ticket |
| Dependencies | DAG, beads-style: a ticket starts when every parent is merged (decisions/0049); stacking later |
| Run & Shell | Two ports per ticket (agent; Run and Shell), each exposed as `$PORT`; click-only **Run** of the project's run command with a log; optional Open URL; a plain **Shell** in the worktree (decisions/0035) |

## Projects

See decisions/0024. Multiple projects (one git repo each), and you switch between them in the dashboard,
which shows one board per project. The Needs-you inbox, the concurrency limit and usage tracking are shared
across all projects. Dependencies stay within a project.

## Board / state machine

**Epics are delivery units** (decisions/0039, 0059): one human ticket from the team's tracker (External ID, e.g. PAY-123)
split into agent tickets SHOP-40.1, 40.2… Children branch from the epic's local integration branch and Local-merge back into
it (never pushed). When all children are Done, the epic goes to Ready for Review as one feature, and is delivered as **one
PR** (or one Local merge). Standalone tickets are their own delivery unit.

```
Backlog ──(drag to Dev)──────────────► Queued ──(deps ready + free slot)──► Agent Working
                                        ▲                                       │
                                        │                         ┌─────────────┼───────────────┐
                                        │                         ▼             ▼               ▼
                                        │              (Asks you: stays in Dev)        Ready for Review
                                        │                                                       │
                                        └───────────── Send back (action, you) ◄────────────────┤
                                                                                                ▼
                                                                                            Approved ──► Done/Merged
```

- **No Investigate or Ready for Dev columns** — investigating is part of Dev, and it is shaped by the user's own
  work-ticket skill (decisions/0036).
- **Closed** (inside Done, "Closed · not merged") — closed without merging, via Mark closed or Close ticket
  from any state. The branch is kept, and the ticket can be reopened by dragging it out (decisions/0033).
- **Delete** — only for tickets (and epics with no started children) that never went to Dev, after a confirmation that
  names the dependency links it removes. Deleting an epic deletes its never-started children too. Numbers are never reused;
  anything that has been to Dev can only be Closed (decisions/0060).
- **Waiting for limit reset** — agents stopped by the plan limit. No inbox items; Resume all after the reset (decisions/0046).
- **Pinned** tickets always sit at the top of their column, above orange ones. The pin follows the ticket across columns (decisions/0047).
- **Dev order:** pinned → everything that needs you (orange, then Interrupted / Start interrupted / Parent closed, from
  either section) → working → Queued (decisions/0047).
- **Queued** shows what each ticket is waiting on (`waits for SHOP-12, SHOP-15 to merge`).
- **Asks you / Needs you / Idle** (one orange family in Dev, not a column; it clears on your input, and never fires for the ticket whose terminal you have focused, decisions/0042) — the agent ran `crewboard ask human` or waits on a permission
  prompt. The card is highlighted and sorted to the top of Dev (decisions/0037).
- **Submit for review** (yours) and **Park** (drag Dev → Backlog: the agent stops, the work and conversation are kept, and
  dragging back to Dev resumes it) (decisions/0052).
- **Working in background** (neutral) — the agent ended its turn but its own background work (test runs, helper agents,
  scheduled wakeups) is still pending. Not orange, no inbox item (decisions/0042).
- **Interrupted** (a status on the card) — the session died (reboot or crash). The ticket waits for Resume (decisions/0018).
- Cards in a session that is blocked on a permission prompt get a "needs you" badge.

## Dependencies

See decisions/0049 (beads-style for v0.1; stacking deferred).

- Tickets form a DAG (`blocked_by`), with any number of dependencies per ticket. Cycles are rejected on insert.
- **Ready** means every dependency is **Done · Merged** (Closed never counts). Like beads, a ticket never builds on
  unfinished work.
- **Base branch** (decisions/0051, 0059): a project setting prefilled with the remote's default branch, overridable per ticket
  or epic, fixed once started. Linked tickets must share a base. Epic children use the epic's branch. Before a start or a
  Local merge into the base, Crewboard fetches and fast-forwards the local base; if it has diverged from the remote, it holds
  and asks the user to push or pull. Local merge never pushes.
- Every ticket branches from its base branch when it starts, so it already contains its parents' merged work.
  `base_sha` is where the ticket left the base branch; the Diff is against it.
- After **Mark merged** on a PR, Crewboard fetches the base branch before starting the children that just became ready.
- **Rebases onto the latest base** (Local merge, decisions/0038; reopen, decisions/0044) keep an agent's unsaved work with a
  checkpoint commit (`wip`, rebase, `git reset --soft`). Never stashes.
- **Stacking later** (decisions/0005, deferred): children building on unmerged parents, Paused children, restacks and stacked
  PRs may return as a per-project setting after daily use.
- Visualization:
  - Board: dependency badges on cards. Hover or select a card to highlight its chain
    and dim the rest; arrows are drawn only for the highlighted chain.
  - Stack list in the ticket side panel: indented parents and children with statuses (v0.1).
  - Graph view: maybe later (decisions/0006).

## Scheduler

A loop that runs on every state change, plus a slow tick:

1. Select tickets in `Queued` whose dependencies are ready (see above).
2. Start every ready ticket. The optional `max_concurrency` and back-pressure limits are off by default
   (decisions/0016). The plan-limit hold always applies (decisions/0020).
3. Starting a ticket: create the worktree and branch, run the repo's `setup` command,
   start a session in the background service, launch `claude` with the ticket prompt, then set the
   ticket to `Agent Working`.

## Agent sessions

```
GPUI app (client) ◄──local socket──► crewboard daemon (core) ◄──PTY──► session for ticket N
                                                  └─ CREWBOARD_TICKET=SHOP-12 claude --plugin-dir <crewboard-plugin> --allowedTools "Bash(crewboard:*)" "/crewboard:work-ticket"
                                                     (cwd = worktree)
```

- **Background service**: the core runs as `crewboard daemon` and owns every PTY. Closing the app window
  doesn't stop agents. `crewboard attach SHOP-12` opens a session from any terminal. Works on macOS,
  Windows and Linux.
- **Updating the service** (decisions/0022): after a rebuild, the app sees an older service and shows "Service update
  ready · Restart now (3 agents working, 2 waiting)". You pick the moment; a restart interrupts Dev tickets like a reboot.
- **You can type into the terminal at any time.** It is the real interactive CLI.
- **Crewboard never mixes its text with yours** (decisions/0048). Automatic messages (agent questions,
  continue after the limit reset) wait until the agent is idle and your input line is empty ("1 message waiting · Deliver now").
  Your own clicks (Send back, inbox answers) ask first if you have unsent text: Keep it and cancel / Clear it and send.
- **Editing a working ticket** sends its agent a short change note ("Added to Done when: …"), delivered the same way.
- **The agent's instructions live in a Claude Code plugin, not in Rust strings.** The app
  only sets `CREWBOARD_TICKET` and starts the skill. The agent then fetches everything
  else through `crewboard` CLI commands taught by the skills, so the data is always current. Long text goes
  through stdin, never through quoted arguments (decisions/0031).

### The `crewboard` Claude Code plugin

```
plugin/
  .claude-plugin/plugin.json
  skills/
    work-ticket/SKILL.md      # main workflow: crewboard prime → plan → implement → Commit skill → crewboard review submit
    commit/ push/ open-pr/    # built-in workflow skills; each replaceable globally or per project (decisions/0050)
    address-review/SKILL.md   # pull review feedback, fix, re-submit
    restack/SKILL.md          # resolve rebase conflicts (onto main at Local merge or reopen)
    answer-question/SKILL.md  # resumed finished ticket: answer a linked agent's question, change nothing
  hooks/hooks.json            # Notification + Stop + StopFailure + UserPromptSubmit → `crewboard hook <event>`
```

- **Agent commands** (`crewboard` CLI, no MCP server, modeled on beads `bd`, decisions/0031 and 0032).
  A missing KEY means `$CREWBOARD_TICKET`. Every command supports `--json`. Long text uses `--stdin`.
  **Agents may read anything, change only their own ticket, and create Backlog tickets**; the service enforces this
  (decisions/0031). The Project terminal (a plain terminal in the project folder, where you can also run
  `claude` yourself, decisions/0019) and your own terminals have full access.
  - `crewboard prime` — context for this ticket: details, parents and summaries, feedback, pending questions, rules, and
    the user's **Agent instructions** (global default + per project, decisions/0040).
  - `crewboard show [KEY]`, `list`, `ready`, `blocked`, `create` (`-f` batch), `update`.
  - `crewboard review submit --stdin` — moves the ticket to Ready for Review with the summary. Crewboard runs no
    checks; verification and retries live in the user's own skills (decisions/0018).
  - `crewboard review feedback` — in-app review comments plus unresolved PR comments.
  - `crewboard ask human --stdin` — the ticket shows "Asks you" at the top of Dev. Optional 2–4 answer options
    (`--option "<label>" … --recommended N`): the inbox, card and panel number them, preselect the recommended one, and
    always offer "Other…" for free text. Agent-to-agent questions stay free text (decisions/0061).
  - `crewboard ask <KEY> --stdin` / `crewboard answer <ID> --stdin` — a finished ticket is resumed automatically,
    answer-only, to reply (an exception to the click rule); asking a never-started ticket is rejected (decisions/0030).
    Agents on directly linked tickets ask each
    other through Crewboard. Logged on both tickets, delivered only when the target is idle; the inbox is used only when
    the project setting "Let finished tickets answer questions" is off (decisions/0030).
  - `crewboard dep add | remove | tree | cycles`.
- **Hooks** (`crewboard hook <event>`; they do nothing when `CREWBOARD_TICKET` is unset, so
  ordinary `claude` sessions aren't affected):
  - `Notification` → "Needs you" only for real requests (`permission_prompt`, `elicitation_dialog`,
    `elicitation_url_dialog`, `agent_needs_input`); other types are ignored.
  - `Stop` → if `background_tasks` or `session_crons` is non-empty: "Working in background" (neutral). Otherwise, if the agent
    stopped without running `crewboard review submit` or `crewboard ask human`: Idle (orange).
  - `StopFailure` → usage limit: "Waiting for limit reset" (decisions/0020). `UserPromptSubmit` → back to working.
- **Send back**: the app types `/crewboard:address-review` into the same session (or into
  `claude --resume <session-id>` if the session is gone).
- **Bonus**: the same plugin also works outside the app. You can run
  `CREWBOARD_TICKET=SHOP-12 claude` by hand in a worktree.

## Worktrees & previews

Per-repo settings live **in the app**. Crewboard never adds files to user repos (decisions/0023). Zero
config works: worktree plus `claude`. Example settings:

```
setup    = pnpm install
dev      = pnpm dev --port $PORT
env_from = .env.local
```

- Worktrees live outside the repo folder, in `<app-data>/wt/<repo-name>/<KEY-N>` (for example `~/.crewboard/wt/shop-web/SHOP-18`).

- Each ticket gets **two** unique ports (decisions/0026): one as `$PORT` in the agent's session, and one as `$PORT` in Run
  and Shell (Open URL uses it). Parallel agents never collide, and an agent's leftover dev server never blocks your Run.
- **Run** (formerly Preview) runs the project's run command **only on click** (Run / Stop, plus Open URL only if the
  project sets one) and also stops at Done. The **Shell** tab is a plain terminal in the worktree (decisions/0035).
  There is no proxy and no auto-start (decisions/0026).
- The run command is asked on first Run (with an Ask Claude suggestion) and saved per project.
  Per-ticket services are left out.

## Failure & recovery

See decisions/0018. Crewboard **detects and reports**. Anything that launches `claude` or deletes something
needs the user's click.
- After a reboot (decisions/0045): only Dev tickets become Interrupted. Orange tickets keep their question, and an inbox
  answer resumes and delivers it. Review and Approved tickets are left alone. Run and Shell show "stopped by restart".
  Interrupted tickets show a "Before the restart" view read from Claude's own saved conversation (by session ID).
- Startup reconcile: reattach to sessions that are still alive. Dead sessions become **Interrupted**, with
  Resume / Resume all in the inbox.
- A partial start shows "Start interrupted, Retry".
- A Cleanup view lists orphaned sessions, worktrees, branches and ports. Removal needs the user's confirmation.
- Done tickets have their worktree removed automatically (the branch is kept), unless it has uncommitted changes.
- No test gate or retry limit. These belong in the user's own skills.

## Usage & limits

See decisions/0020. Crewboard reads Claude Code's own session logs (read-only) and shows tokens per card plus
today's total. A dollar estimate is optional and off by default. When the plan limit is hit, one banner appears,
no new tickets start, and the user clicks Resume all after the reset.
- **Detection:** the plugin's `StopFailure` hook (`rate_limit`) marks agents "Waiting for limit reset". Reset times and the
  header **plan meter** ("5h · 82% · week · 40%", Pro/Max only) come from a transparent status-line command that records
  `rate_limits` and then shows the user's own status line unchanged. Safety net: "This is the usage limit" on Idle inbox items.

## Review

The card side panel has these tabs: **Terminal · Diff · Run · Shell · Ticket · History**.

- **Approve** has two actions (decisions/0015):
  - **Local merge**: squash into the base branch, in a Crewboard-owned scratch worktree. It first tries a free rebase onto
    the latest base. On conflict it offers "Send to agent to fix". Review cards get a read-only "conflicts with main"
    flag after every merge. It never touches the user's own uncommitted edits (decisions/0038).
  - **Open PR**: targets the base branch (no stacked PRs in v0.1, decisions/0049). Done by the user-replaceable **Open PR
    skill** (decisions/0050): it proposes the branch name, commit messages, PR title and description; a **confirmation dialog**
    lets you edit all of them; Crewboard applies the edits, then the skill pushes and opens the PR (GitHub, Azure DevOps…).
- **Commits** (decisions/0050): the agent commits through the **Commit skill**. `review submit`, Local merge and pushes refuse
  while there are uncommitted files. Crewboard never commits by itself.
- **Branch names** (decisions/0050): temporary `crewboard/SHOP-18` while working; the real name (`bugfix/…`, `feature/…`) is
  chosen at the first push and then locked.
- **Diff tab**: diff against the ticket's `base_sha`, in unified and side-by-side views, with line comments. No partial approval (decisions/0017).
- **Auth**: reuse the `gh` and `az` CLI logins; no tokens are stored.
- **Changed since review** (decisions/0043): if new commits appear after a ticket entered Review, Approve warns and shows
  only the new commits.
- **PR rework** (decisions/0041): drag an Approved ticket back to Ready for Review, then Send back. Pushes happen only through your Push to PR click
  and the confirmation dialog (decisions/0050).
  Approve offers "Push to PR #N".
- **PR state is manual**: Crewboard never polls. After Open PR, the user clicks **Mark merged** (Done; children
  that were waiting for it can start) or **Mark closed**. PR comments are pulled on demand when the ticket is sent back.
- **Send back** (an action, not a column) → your comments go to the agent in the same session, and the ticket
  returns to Dev with a "Sent back · N comments" marker (decisions/0034).

## Architecture

Cargo workspace:

```
crates/
  crewboard-core/   # domain, SQLite store, scheduler, git/worktrees, PTY sessions, ports, Run/Shell processes, socket server; runs as `crewboard daemon`
  crewboard-app/    # GPUI app: board, Stack list, terminal view, diff view (graph view maybe later)
  crewboard-cli/    # `crewboard` binary: launches app; `daemon`, `attach`, `hook`, and the agent commands (decisions/0032)
plugin/           # Claude Code plugin: skills and hooks (agent behaviour lives here)
```

Data model: see `SDD.md` §8 (the detailed design for implementation).

## Milestones

See decisions/0053. v0.1, macOS first: **M0** foundations · **M1** background service · **M2** board · **M3** sessions ·
**M4** agent signals · **M5** scheduler · **M6** review and Local merge · **M7** attention and recovery · **M8** usage ·
**M9** Run, Shell, Project terminal · **M10** PR flow · **M11** Windows and Linux.

All of v0.1 is built outside Crewboard, one milestone at a time; only blockers stop the work, other questions are deferred and
the user reviews everything after M11 (decisions/0062). Crewboard is used
from the work that comes after v0.1.

**Later:** stacking (decisions/0049), graph view (maybe), importers, the planner agent, the public launch. License: MIT (decisions/0054).

## Open questions

Tracked in decisions/README.md (the decision log is the source of truth).
