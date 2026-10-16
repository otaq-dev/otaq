# 0018. Failure and recovery: detect and report, never self-heal

- **Status:** Accepted. The "no test gate" part is superseded by 0063 (validation before Review).
- **Date:** 2026-10-09
- **Related:** 0004, 0007, 0008, 0010, 0016, 0017

## Context
With 5–10 agents, the database and reality will drift apart: reboots kill the background service (0022), `claude` crashes, starts
fail halfway through. The user does not want the factory acting on its own (see 0017, 4d).

## Decision
**Core rule:** the factory **detects and reports**. Anything that **launches `claude` (spends tokens)** or
**deletes something** needs a user click. Pure bookkeeping fixes to the factory's own records happen
automatically.

- **Startup reconcile:** each live ticket is compared with its session in the background service and its worktree.
  - Session alive: reattach silently.
  - Session gone (reboot) or `claude` exited: the ticket becomes **Interrupted** (a status on the card, see
    0004), with an inbox item offering **Resume** (`claude --resume <id>` in a new session) and
    **Resume all**. Nothing ever resumes automatically.
- **Partial starts:** a start that stopped halfway shows **"Start interrupted"** with one action, **Retry start**,
  which re-runs the **whole** start from the beginning. There is no per-step status, so the start must be safe to
  re-run (idempotent: reuse an existing worktree or branch). It is never re-run silently.
- **Resume failed:** if `claude --resume` can't find the saved conversation (for example the transcript was deleted
  or the path changed), the user chooses **Retry** or **Start from scratch**. Start from scratch is a new `claude`
  session in the same worktree, after a confirmation. It keeps the branch, commits and uncommitted files, stores the
  new session id, and logs a Runs entry. Otaq never falls back on its own.
- **Leftovers:** a **Cleanup view** lists orphaned sessions, worktrees and branches that no ticket
  owns, plus leaked ports. The user selects them and confirms. Nothing is deleted automatically.
- **Done tickets:** the worktree is **removed automatically** and the branch is kept, because the user's own
  Approve or Mark merged click, **or the project's Auto-merge setting (0064)**, is what led there. Exception: a worktree with **uncommitted changes is
  never removed**. It is flagged in the Cleanup view instead.

- ~~**No factory test gate, no retry limit, no Failed column.**~~ *Superseded by 0063: a separate read-only validator checks
  every submit (when validation is on), with a round limit. Still no Failed column.* Original text: `claude` works as the plain CLI. When the
  agent calls `ready_for_review`, the ticket goes straight to Ready for Review. Verification and retry
  behavior is left to **skills the user defines later**. The factory does not change how the Claude CLI
  works. A stuck agent escalates through `otaq ask human` ("Asks you" in Dev, 0037).

## Exceptions
- **Answering another agent's question (0030):** a ticket without a live session is resumed automatically, answer-only,
  when a linked agent asks it something. The asking agent's question is the go-ahead. Per-project setting, on by default.

## Refined by
- 0063: the agent's `review submit` is the go-ahead for a validator session (an exception to the click rule, like 0030).
- 0064: Auto-merge Local-merges validated tickets and sends merge conflicts back to the agent without a click.
- 0068: opening a Project agent is the go-ahead for its session; resuming one after a restart needs a click.
- 0083: opening an epic lead, or dragging the epic, is the go-ahead for the lead's turns after a child merges or is
  closed, when a child asks, and its one note to a stuck (also Idle) child.
- 0045: which ticket states become Interrupted after a reboot, Resume all scope, and the "Before the restart" view.

## Alternatives considered
- Auto-heal (auto-resume, auto-retry, auto-cleanup): rejected. It acts on its own and could start 10
  sessions at once.
- No reconcile at all: rejected as too fragile for 5–10 agents.
- A factory test gate with a retry limit and a Failed column (3 attempts, then Continue / Restart fresh /
  Reset / Backlog): rejected by the user in favor of their own skills.

## Changelog
- 2026-10-09: created
- 2026-10-09: simplified Start interrupted (Retry start re-runs everything); added Retry / Start from scratch when Resume fails (design review, user decision).
- 2026-10-09: tmux wording replaced by background-service sessions (0022).
- 2026-10-09: decided that there is no test gate, no retry limit and no Failed column.
- 2026-10-09: exception: finished tickets are resumed automatically to answer another agent's question (0030).
- 2026-10-11: test gate reintroduced as a separate validator (0063); Auto-merge as a go-ahead for merge, conflict handoff and Done cleanup (0064).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
- 2026-10-16: new exception: an epic lead's automatic turns (0083).
