# 0033. Closed tickets: closed without merging, shown inside Done, can be reopened

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0004, 0005, 0015, 0017, 0018

## Context
The design review asked what "Mark closed" does (a PR closed without merging). 0017 said Otaq does
nothing else. The user decided that Mark closed should **close the ticket**, with a way to reopen it later. The
user also wants any ticket to be closable (for example "won't do").

## Decision
- **Closed** is an end state meaning "finished without merging". It is shown **inside the Done column** with a
  distinct label, "Closed · not merged", next to merged tickets, which are labeled "Merged". There is no separate column.
- **Ways to close:**
  - **Mark closed** on a ticket whose PR was closed without merging (replaces the 0017 behavior).
  - **Close ticket**, available on **any ticket in any state** (for example "won't do" from Backlog). If an agent
    is running, a confirmation says it will be stopped first.
- **On close:** the branch is **kept**. The worktree is removed unless it has uncommitted changes, which is the
  same rule as Done (0018). Any preview stops.
- **Reopen:** drag the ticket out of Done (or click "Reopen") to Backlog (0036). Its old
  branch is reused. An agent starts only when the user drags it to Dev (0018).
- **Children of a closed ticket** were built on work that never merged, so they cannot be approved. Their cards
  show **"parent SHOP-26 closed"**, and an inbox item asks the user to reopen the parent or remove the dependency.
  Otaq does not decide.

## Refined by
- 0044: Closed never counts as ready, and children of a closed parent pause. Only Closed (not Merged) tickets can be reopened,
  with a free rebase on the way to Dev.

## Changelog
- 2026-10-09: created (design review, user decision)
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
