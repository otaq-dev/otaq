# 0047. Pinned tickets: always at the top of their column

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0037, 0042 (ordering refined)

## Decision
- Any ticket (and any epic in Backlog) can be **pinned** and unpinned. A pin icon appears on the card, with "Pin" / "Unpin"
  in the card's right-click menu and the panel's "…" menu.
- **A pinned ticket is always at the top of its column**, above everything else, **including needs-you tickets that wait for
  the user** (this refines the ordering in 0037 and 0042).
- **The pin follows the ticket:** pinned SHOP-19 is first in Dev, and stays first when it moves to Ready for Review,
  Approved and so on, until the user unpins it.
- **Several pins in one column:** the **most recently pinned is first**. Pinning a ticket again bumps it to the top.
- Pinning only affects ordering on the board. It doesn't change the inbox order, notifications, or any agent behavior.
  *Refined by 0065: pinned tickets also start first from Ready for Dev.*
- Column order is now: **pinned** → turquoise (Asks you / Needs you / Idle, inbox order) → **other needs-you states
  (Interrupted, Start interrupted, Parent closed; inbox group 2 order)** → working → Paused. Queued has its
  own section, and pinned tickets also sit at the top of Queued. *(Superseded by the changelog entry below: Agent Working sits above Queued, and needs-you cards float to the top of Dev from either section.)*

## Changelog
- 2026-10-09: created (user decision)
- 2026-10-09: non-turquoise needs-you states sort right after the needs-you ones (design audit 2, user decision).
- 2026-10-09: Dev layout: Agent Working above Queued. **Any card that needs the user floats to the top of Dev, whatever
  its section** (for example a Queued "Start interrupted" ticket). Order: pinned → needs-you (turquoise, then Interrupted / Start
  interrupted / Restack conflict / Parent closed) → working → Paused → Queued (user decision).
- 2026-10-09: Restack conflict is no longer a needs-you state; it sorts with working tickets (0005).
- 2026-10-09: Paused no longer occurs in v0.1 (0049).
- 2026-10-11: pins also decide start order (0065); Queued is its own Ready for Dev column, so the Dev layout no longer has a Queued section (0066).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
- 2026-10-15: pinning an epic pins it as a whole: its track is first in the strip and its children start first; the sleeve shows the pin (0080).
