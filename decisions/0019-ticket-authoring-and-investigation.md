# 0019. Ticket authoring: form, Claude ticket tools, investigation step

- **Status:** Accepted. The investigation step and Investigate column are superseded by 0036. The form, the ticket
  tools and the planning terminal still apply.
- **Date:** 2026-10-09
- **Related:** 0004, 0009, 0016, 0018

## Context
An agent only does as well as its ticket. With 5–10 agents, vague tickets turn into inbox questions or
wasted work. Following 0018, the factory provides structure and tools, and the planning behavior lives in
the user's skills.

## Decision
- **Ticket form:** Title, What and why, **Done when** (a checklist), Depends on. An empty "Done when" shows
  a **warning** when the ticket is dragged to Dev. It never blocks.
- **Claude can create tickets (v0.1):** agents get `create_ticket` and `link_tickets` (now `crewboard ticket create` / `crewboard ticket link`, 0031). New
  tickets land in **Backlog**. The app has a **planning terminal**, a Claude session not tied to any
  ticket, for running the user's own skills (for example, "split this feature into tickets"). The tools
  also work from a normal terminal. The factory ships the tools only. Planning skills are the user's.
  - **Refined (grilling session):** the planning terminal, now called the **Project terminal**, is a **plain terminal in the project's own folder** (the user's
    repo checkout), not a Claude session. The user creates and splits tickets and epics there with `crewboard` commands (full
    access, 0031), runs any project command, or starts `claude` themselves to make changes without a ticket. Changes made
    there are the user's own (0038 is about Crewboard's processes). It's owned by the background service, so it survives
    closing the window (0022).
- **Investigation step:** before implementing, an agent can **investigate** a ticket and produce an
  **investigation result** that may include **questions for the user**. The agent can ask questions at any
  time and **update the ticket** with what it learns.
  - New tools: `update_ticket(fields)` and `post_investigation(result)` (now `crewboard ticket update` /
    `crewboard investigation post`, 0031). Questions go through the
    existing `ask_human`.
  - The investigation result is shown on the ticket, next to Ticket in the side panel.

- **Investigate is a separate column**, sitting between Backlog and Ready for Dev:
  - The user drags a ticket to **Investigate**. An agent session starts (plugin skill
    `/factory:investigate-ticket`), reads the code, posts findings, asks questions (the ticket moves to Needs
    Input while it waits), and updates the ticket.
  - When the investigation is finished, the ticket moves to **Ready for Dev**. **Nothing is built until the user
    drags it to Dev.**
  - Dragging it to Dev **continues the same session**, so the agent keeps what it learned.
  - Investigating doesn't wait for dependencies. A child can be investigated before its parent is built.
    The investigation runs in a worktree on the base branch. When the ticket moves to Dev, its branch is
    re-pointed at the parent's head (0005) before any code is written.
  - When the optional `max_concurrency` limit is enabled (0016), an investigating agent counts toward it.

## Changelog
- 2026-10-09: created
- 2026-10-09: chose a separate Investigate column (option A).
- 2026-10-09: the investigation step was removed; it is part of Dev now (0036).
- 2026-10-09: edits to a working ticket are passed to its agent as a change note (0048).
- 2026-10-09: the planning terminal is a plain terminal in the project folder; the user may run `claude` there themselves (grilling session, user decision).
- 2026-10-09: renamed "planning terminal" to **Project terminal** (grilling session, user decision).
