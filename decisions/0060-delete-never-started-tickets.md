# 0060. Delete is allowed only for tickets that never went to Dev

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0018, 0019, 0029, 0033, 0044

## Context
Ticket-writing review, Q6. No decision allowed deleting a ticket; the only way out was Close ("won't do"), which leaves the
ticket in Done. The Project terminal design shows deleting a ticket that Claude created by mistake (SHOP-21 in a four-ticket
split), and closing it would clutter Done and an epic's progress.

## Decision
- **Delete** is available only for tickets (and epics with no started children) that **never went to Dev**: no branch,
  worktree or conversation exists.
- Entry points: the card's right-click menu, the panel's "…" menu, and a Project agent's "Created and changed" summary (0068).
- A confirmation is required (a deletion, 0018). If other tickets depend on it, the confirmation names them and those links
  are removed.
- **Deleting an epic** deletes its never-started children with it, after one confirmation that lists them ("Delete SHOP-40
  and its 2 tickets (SHOP-40.1, SHOP-40.2)? None of them started."), like Close epic (0044).
- The ticket's number is **never reused** (0029). Its old key stops resolving.
- Anything that has been to Dev can only be **Closed** (0033), so its history is kept.

## Changelog
- 2026-10-09: created (ticket-writing review Q6, user decision)
- 2026-10-09: deleting an epic deletes its never-started children in one confirmation (ticket-writing review Q13, user decision).
- 2026-10-11: the created-tickets summary belongs to Project agents (0068).
