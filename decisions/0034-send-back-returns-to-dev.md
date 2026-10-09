# 0034. No Changes Requested column: Send back returns the ticket to Dev

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0004, 0005, 0016, 0017

## Context
0004 had a separate **Changes Requested** column between Review and Queued. The design review showed it only
held tickets for a moment ("Re-queued"). The user wants Send back to be an **action** that hands the ticket back
to the agent, not a place.

## Decision
- **The Changes Requested column is removed.**
- **Send back** is an action on a ticket in Ready for Review (for an Approved ticket with an open PR, drag it back to
  Ready for Review first, 0041) (in the Diff tab or the panel action bar). It bundles
  the line comments and the optional note (0017), and **moves the ticket straight back to Dev**:
  - If the session is alive, the agent gets the feedback in the **same session** (`/crewboard:address-review`)
    and continues right away. The Send back click counts as the go-ahead (0018).
  - If the session is gone, it is resumed (`claude --resume`) as part of the same click.
  - If the optional agent limit (0016) is on and full, the ticket waits in **Queued** until a slot is free.
- The card shows a **"Sent back · N comments"** marker (and the round, for example "round 2") while the agent
  works on the feedback, so rework is easy to tell apart from a first attempt.
- **Children still pause** until the parent is back in Review (0005).
- PR feedback works the same way: the user presses Send back, and the agent fetches PR comments with
  `crewboard review feedback` (0017).

## Changelog
- 2026-10-09: created (design review, user decision)
