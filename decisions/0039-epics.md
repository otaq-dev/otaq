# 0039. Epics (beads-style): a container in Backlog, child tickets with hierarchical keys

- **Status:** Accepted. Partly superseded by 0059: epics get a local integration branch, go through Review, and are delivered as one PR.
- **Date:** 2026-10-09
- **Related:** 0004, 0005, 0019, 0029 (refined), 0032 (refined), 0033, 0036

## Context
The user wants beads-style epics: a big piece of work split into tickets that appear on the board. 0032 had explicitly
not adopted issue types. This decision adds exactly one extra type, the **epic**.

## Decision
- **Two types only:** ticket (default) and **epic**. ~~There are still no priorities.~~ Priorities were adopted in 0065.
- **An epic lives only in Backlog.** It can't be dragged to Dev and has **no agent, branch or worktree**. It is a container.
  Its card shows its title, progress (for example "3 / 5 done") and its child tickets.
- **Hierarchical keys, like beads:** epic **SHOP-40**, children **SHOP-40.1, SHOP-40.2, …** (refines 0029). Child numbers are
  never reused within the epic. There is **one level only** (no SHOP-40.1.1). A child **can't move to another epic**, because
  its key is fixed. Tickets outside an epic keep plain keys (SHOP-18).
  - Child branch and worktree names use the full key (`otaq/SHOP-40.1-wishlist-api`, `wt/shop-web/SHOP-40.1`).
- **Child tickets are normal tickets.** They have dependencies (also between siblings), stacking, Dev, review, closing and
  so on. They show a small **epic chip** ("Wishlist"), and the board can be filtered by epic.
- **Splitting an epic:**
  - **By hand:** "Add ticket to epic" on the epic.
  - **With Claude:** `otaq create --parent SHOP-40` (the beads flag), for example from the Project terminal, where
    the user's own skill decides how to split.
- **Completion:** when **every child is in Done** (merged or closed), the epic **moves to Done automatically**. This is
  bookkeeping (no tokens, nothing deleted, 0018). Reopening any child brings the epic back to Backlog.
- **CLI additions (refines 0032):**
  - `otaq create --type epic "Title"`
  - `otaq create --parent <EPIC> "Title"`
  - `otaq show <EPIC> --children`
  - `otaq list --parent <EPIC>`

## Refined by 0046
- A ticket can depend on an epic (ready when the epic is Done with at least one merged child; not stacked).

## Refined by 0044
- Closing an epic closes all unfinished children after one confirmation.
- An epic with nothing merged shows "Closed · not merged".
- Never-started tickets can be moved into an epic later (new child key, old key kept as an alias).

## Changelog
- 2026-10-09: created (user decision)
- 2026-10-09: epics have a base branch that their children always use (0051).
- 2026-10-09: epics become delivery units with a local integration branch and a review step; "Backlog only / no branch / auto-Done" superseded (0059).
- 2026-10-11: priorities adopted; epic children inherit the epic's priority (0065).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
