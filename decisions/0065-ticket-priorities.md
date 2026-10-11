# 0065. Ticket priorities (1–4) and the start order

- **Status:** Accepted
- **Date:** 2026-10-11
- **Related:** 0007 (refined), 0016, 0031, 0032 (refined), 0039 (refined), 0047 (refined), 0066
- **Supersedes:** "no priorities" in 0007, 0032 and 0039

## Context
Grilling session, 2026-10-11. With a limit on running agents, the user needs to say which ready ticket starts next.
Oldest-first (0007) can't express "this bug before that feature".

## Decision
- **Priority field** on tickets and epics: **1 Urgent · 2 High · 3 Normal · 4 Low**. New tickets default to **3**.
- **Epic children inherit the epic's priority** unless it is set on the child.
- **Start order** from Ready for Dev (0066), used whenever not every ready ticket can start at once:
  1. **pinned** tickets (most recently pinned first)
  2. then **priority** 1 → 4
  3. then the ticket that has been **ready the longest**
  The Ready for Dev column is sorted in this order. Other columns keep their order (0047).
- **Pinning doesn't change the priority field.** Pinned tickets go first anyway; unpinning restores the ticket's place.
- No "tickets unblocked by the one that just finished go first" rule: pinning covers "do this next".
- **CLI (beads-style):** `crewboard create -p 1`, `crewboard update -p 2`, `crewboard list --priority 1`. Agents and Project
  agents can set it on tickets they create; agents can't change other tickets' priority (0031).
- Priority is shown on cards and in the ticket form; the design decides how (design update K).

## Refines 0047
Pins now also decide start order (above), not only board order.

## Changelog
- 2026-10-11: created (grilling session, user decision)
