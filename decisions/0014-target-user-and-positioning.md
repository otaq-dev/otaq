# 0014. Target user and positioning

- **Status:** Accepted
- **Note (0049):** Stacking is deferred for v0.1 (0049). Pitch item 1 must be revisited before any public launch.
- **Date:** 2026-10-09
- **Related:** 0001, 0005, 0008, 0011, 0013

## Context
The space is crowded: Vibe Kanban (open source, a board with Claude Code and worktrees) is the closest,
and Conductor, Claude Squad and Crystal overlap. An open-source launch needs a clear user and a clear
reason to switch.

## Decision
- **Launch user:** a solo developer supervising **5–10 agents in parallel**.
- **Why switch (in priority order of the pitch):**
  1. **Dependencies and stacking:** plan a feature as connected tickets, and agents build them in order,
     stacked on each other.
  2. **Native and fast:** a real native Rust app, not a web or Electron UI.
  3. **Live terminal control:** watch and talk to every agent in a real Claude Code terminal.
- Not in scope for launch: teams and shared boards. A team version is a possible later direction and
  would revisit 0003.

## Consequences
- With 5–10 agents, **attention management** becomes a core feature: a "needs you" inbox, compact
  cards, and alert priority (Topic 3).
- With 5–10 interactive sessions and inherited permissions (0011), many agents may wait on prompts at
  once. Topic 3 has to design for this.
- Usage-limit and cost visibility matter more with that many parallel sessions (Topic 7).
- Stacking is the headline feature, which weighs on whether it ships in v0.1 or later (0013, Topic 2).

## Changelog
- 2026-10-09: created
- 2026-10-09: stacking deferred for v0.1 (0049); pitch to revisit before launch.
