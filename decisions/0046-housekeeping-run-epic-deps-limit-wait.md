# 0046. Housekeeping: Run and Shell in v0.1, depending on an epic, "Waiting for limit reset"

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0005, 0013, 0020, 0027, 0035, 0039, 0042, 0044

## Decision
1. **Run and Shell are in v0.1** (0035). They are built after Local merge on macOS (0027 build order). This replaces
   "previews later" in 0013.
2. **A ticket can depend on an epic** (user decision), for example `otaq dep add SHOP-50 SHOP-40`.
   - The dependency is **ready when the epic is Done with at least one merged child**, which means all children are
     finished (0039). An epic that ends "Closed · not merged" never counts as ready (0044).
   - Because an epic has no branch, the dependent ticket **is not stacked**. It branches from the base branch (main),
     which already contains the epic's merged work.
   - If a child of the epic is reopened later, the epic returns to Backlog (0039). Dependent tickets that already started
     are unaffected, because they built from main and aren't stacked.
3. **Plan usage limit vs. Idle (fixes a 0020/0042 conflict):** while the usage-limit banner is showing, agents that stop
   because of the limit show **"Waiting for limit reset"**, not Idle. They **don't create inbox items** (one banner, 0020).
   After the reset, **Resume all** (the user's click) tells those sessions to continue.

## Changelog
- 2026-10-09: created (product owner review 2, user decision)
- 2026-10-09: the limit stop is detected by the plugin's StopFailure hook; Stop doesn't fire for it (0020).
- 2026-10-09: a ticket can depend on an epic only if it has the epic's base branch (0051).
- 2026-10-09: unchanged under 0059: a dependency on an epic is ready when the epic is Done · Merged.
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
