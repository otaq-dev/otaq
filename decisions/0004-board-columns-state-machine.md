# 0004. Board columns and ticket state machine

- **Status:** Accepted
- **Note (0049):** Paused (and restacking) is not in v0.1: dependencies wait until the parent is merged (0049).
- **Date:** 2026-10-08
- **Related:** 0005, 0007, 0009

## Decision
Columns/states:

`Backlog → Queued (Ready for Dev column, 0066) → Agent Working (→ Validating, 0063) → Ready for Review` (questions and permission prompts stay in Dev, highlighted at the top; 0037) (Queued is shown as its own **Ready for Dev** column since 0066; Agent Working and Validating are the Dev column)
`Ready for Review → {Send back → Dev (Agent Working, or Queued if the optional limit is full)} | Approved → Done/Merged`
`any state → Closed` (shown inside Done as "Closed · not merged"; reopen by dragging out; 0033)

- Dragging a ticket to **Dev** means "start when possible": the ticket enters **Queued**.
- **Epics** (0039, 0059) never go to Dev and have no agent. While their children work they stay in Backlog; when all
  children are Done they move to **Ready for Review** and are delivered as one PR or Local merge (0059).
- **Queued** cards show what they are waiting on (`waiting on SHOP-12, SHOP-15`).
- **Asks you / Needs you** (a status in Dev, sorted to the top): the agent ran `otaq ask human` or is waiting on a
  permission prompt, and the card shows the question. There is no Needs Input column (0037).
- **Paused**: the ticket's parent was sent back for changes. Its agent is interrupted and waits until it is
  restacked onto the reworked parent (0005). Shown as a status on the card, not as a separate column.
- **Interrupted**: the session died (reboot or crash). The ticket waits for the user's Resume (0018). Shown as a status on the card.
- A **"needs you" badge** appears when the session is blocked on a permission prompt or is idle.
- The agent submits with `otaq review submit` (0032). A separate read-only **validator** checks the work first; the
  ticket reaches **Ready for Review** when it passes, or is merged automatically when Auto-merge allows it (0063, 0064).
  With validation off for the project, the submit moves the ticket straight to Ready for Review.
- The user approves tickets (**Approved**) or **sends them back**, which returns them to Dev. There is no Changes Requested column (0034).

## Alternatives considered
- Plain To-do, Doing and Done columns. Rejected because they cannot show blocked, waiting or failed agent states.

## Changelog
- 2026-10-08: created
- 2026-10-09: added the Paused status (Topic 2).
- 2026-10-09: added the Interrupted status (0018).
- 2026-10-09: added the Investigate column (0019).
- 2026-10-09: removed the Investigate and Ready for Dev columns (0036).
- 2026-10-09: removed the Needs Input column (0037).
- 2026-10-09: removed the Changes Requested column; Send back returns the ticket to Dev (0034).
- 2026-10-09: added the Closed end state inside Done (0033).
- 2026-10-09: removed the factory test gate, retry limit and Failed column. Checks and retries belong in the user's own skills (0018).
- 2026-10-09: Paused status deferred with stacking (0049).
- 2026-10-09: the user can also Submit for review, and can Park a ticket by dragging Dev → Backlog (0052).
- 2026-10-09: epics follow 0059: Review and delivery instead of auto-Done (ticket-writing review).
- 2026-10-11: Ready for Dev column for Queued (0066); validation before Review (0063); Auto-merge (0064).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
