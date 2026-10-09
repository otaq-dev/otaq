# 0016. "Needs you" inbox, optional concurrency limit and back-pressure

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0004, 0005, 0007, 0009, 0011, 0012, 0014

## Context
With 5–10 interactive agents (0014), the user's attention is the bottleneck. Badges on cards don't scale.
Under a prompting permission mode (0011), Paused children (0005), restack conflicts and the review queue
all compete for the user.

## Decision
**"Needs you" inbox, in v0.1:**
- One ordered queue:
  1. Orange items: Asks you, Needs you, **Idle** (0042). That is, permission prompts, `ask_human` questions, and agent-to-agent questions to tickets with no live session (0030).
  2. Interrupted sessions (0018), Start interrupted and **Resume failed** tickets, and children whose parent was closed (0033). (Restack conflicts are not inbox items:
     the agent resolves them automatically, 0005.)
  3. Ready for Review.

  Within each group, tickets that block more children come first.
- A next-item hotkey (⌘J) opens the ticket with its terminal focused. **It does not advance on its own:** after
  the user answers, the ticket stays open so they can watch the agent react. The user presses the hotkey again
  when ready for the next item.
- OS notifications only for items that **block an agent**, and never for the ticket whose terminal has focus (0042).
- Fed by the plugin hooks and the `crewboard` agent commands (0009, 0031).
- **Shared across all projects.** Items are labeled with their project (0024).

**Concurrency (refines 0007):**
- **Both limits are optional settings, OFF by default.** By default, every ticket the user drags starts as
  soon as its dependencies allow. The factory never adds tickets to the queue by itself, so the user
  controls the load. When enabled:
- `max_concurrency` (suggested value 5) counts tickets in **Agent Working, including waiting (orange) ones** (0042, user decision). Paused and Review tickets don't count.
  (Previously only actively working agents counted. The Investigate column was removed in 0036.) Tickets in Needs Input,
  Review or Paused don't count.
- The limit only controls **starting new tickets**. An agent the user unblocks always resumes, even if
  that briefly exceeds the limit.
- **Back-pressure** (suggested value 3): no new tickets start while that many items wait in the inbox.
- Both are settings (0012) and can be adjusted live from the board header.

**Permissions (refines 0011):**
- On first run, if the user's Claude Code mode is a prompting one, warn that 5–10 agents will stall
  often, and link to the docs on allowlists and auto mode. **Warning only.** The mode is never changed.

## Alternatives considered
- Badges and OS notifications only: rejected, because they don't work at 5–10 agents.
- The factory answering permission prompts itself: rejected, because it breaks 0011 and the user's trust.
- A per-repo permission override (as an in-app setting, 0023): deferred.

## Changelog
- 2026-10-09: created
- 2026-10-09: waiting (orange) tickets now count toward the optional limit; Idle joins inbox group 1 (0042).
- 2026-10-09: no auto-advance; the user moves to the next item with the hotkey (design review, user decision).
- 2026-10-09: the concurrency limit and back-pressure are now optional and off by default (user decision, Topic 10).
- 2026-10-09: the inbox and limits are global across projects (0024).
- 2026-10-09: the Investigate column was removed (0036), so only Agent Working counts.
- 2026-10-09: Investigate also counts toward max_concurrency (0019).
- 2026-10-09: inbox group 2 changed from Failed to Interrupted, because the Failed column was removed (0018).
- 2026-10-09: restack conflicts removed from inbox group 2; the agent resolves them automatically (0005).
- 2026-10-09: Paused children no longer occur in v0.1 (0049).
- 2026-10-09: no notification for the ticket whose terminal has focus (0042).
- 2026-10-09: Resume failed sits in inbox group 2 (SDD review, user decision).
