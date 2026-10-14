# 0066. "Ready for Dev": Queued gets its own column

- **Status:** Accepted
- **Date:** 2026-10-11
- **Related:** 0004 (refined), 0007, 0016, 0036 (refined), 0047 (refined), 0049, 0065

## Context
Grilling session, 2026-10-11. The user wants a clear place agents take work from, with Backlog staying "not yet". Queued
already is that place (dragging to Dev means "start when possible"), but it sits at the bottom of the Dev column, under
the working cards, and is easy to miss. 0036 removed an earlier Ready for Dev column because it would have been "a second
Backlog"; this one is not, because it is the existing Queued state.

## Decision
- **Six columns:** Backlog · **Ready for Dev** · Dev · Ready for Review · Approved · Done.
- **Ready for Dev is the Queued state shown as its own column.** No new rules: the ticket waits for its dependencies to be
  merged (0049), a free slot (0016) or the plan-limit reset, and says what it waits for. The scheduler starts tickets from it
  in the start order of 0065.
- **Dev** holds tickets an agent is on (working, validating, turquoise, background, Interrupted…).
- Dragging a ticket to **Ready for Dev** is the go-ahead. Dragging it straight to **Dev** also works: it lands in Ready for
  Dev when it can't start yet. Dragging Ready for Dev → Backlog takes it out of the queue (nothing was started).
- Queued-only states (Start interrupted, Parent closed, held by base divergence or the plan limit) show in Ready for Dev.
  Cards that need the user still sort to the top of their column.
- **Backlog never starts by itself** (0016: "The factory never adds tickets to the queue by itself"). An unblocked Backlog
  ticket shows a quiet **"Ready to start"** hint.
- The scheduler never queues tickets itself (unchanged).

## Changelog
- 2026-10-11: created (grilling session, user decision)
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
