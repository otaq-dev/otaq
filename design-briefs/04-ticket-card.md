# 04: Ticket card (every state)

Read `00-README.md` and `01-design-system.md` first.

## Context
The card is the most repeated element, and the user scans 30+ of them. It must answer, in under a second:
**What is it? Is it OK? Does it need me? What is it waiting on?**

## Card anatomy (propose the layout)
- Ticket number and title (allow two lines, then truncate)
- **Status indicator**: icon and color plus label, from the design system
- **Dependency badges**: `⛓ SHOP-18 SHOP-19`, or "waiting on SHOP-20" in Queued
- **Last signal** on working cards: "active 40 s ago". It turns into a warning when stale (for example more than
  5 min), so silent breakage is visible (0027).
- **Tokens used** by this ticket (small, secondary) (0020)
- **Needs-you badge** when the agent is blocked on the user
- Optional: a small "running" indicator with the port (Run, 0035), PR number, or "local merge"
- Hover: quick actions (open terminal, open panel), and the full title as a tooltip

## Design every variant (use the sample tickets)
| Variant | Sample | Notes |
|---|---|---|
| Backlog, complete | – | Calm |
| Backlog, missing "Done when" | SHOP-24 | Subtle hint, not an error |
| Queued, waiting on dependencies | SHOP-21 | "waiting on SHOP-20" |
| Queued, ready to start | – | About to start |
| Queued, **Start interrupted** | SHOP-28 | Retry button |
| Agent Working, normal | SHOP-19 | Last signal, tokens |
| Agent Working, **stale signal** | BILL-4 | "active 4 min ago" warning |
| Agent Working, **Needs you** (permission prompt) | SHOP-22 | Shows the prompt text, shortened |
| Queued, **waits for merge** | SHOP-20 | "waits for SHOP-18, SHOP-19 to merge" (0049) |
| Agent Working, **Working in background** | SHOP-19 | "background: test run, 1 helper agent · 6 min" (neutral, not orange, 0042) |
| Agent Working, **Interrupted** | SHOP-25 | Resume button |
| Dev · Idle (orange, 0042) | SHOP-19 variant | "Idle · ended its turn: 'Should I also update the tests?'" |
| Dev · Asks you | SHOP-23 | Shows the question, shortened; sorted to the top of Dev (0037) |
| Ready for Review | SHOP-18 | +412 −38 · 9 files |
| Agent Working · **Sent back** | SHOP-27 | "Sent back · 3 comments · round 2" (0034) |
| Approved, **PR open** | SHOP-26 | "PR #481 · GitHub", and Mark merged / Mark closed |
| Done · Merged | SHOP-17 | Faded |
| Done · Closed, not merged | SHOP-26 after Mark closed | Faded, plus a "Reopen" affordance (0033) |
| Parent closed | SHOP-20 variant | "parent SHOP-18 closed" (0033) |
| Asked a linked ticket, waiting for the answer | SHOP-20 | "asked SHOP-18 · waiting" (0030) |
| Running (Run, on any card) | SHOP-19 | `▶ :4019` |
| Pinned (0047) | SHOP-19 | Pin icon; first in its column, above orange cards |
| Selected / focused / hovered / dragging | – | Interaction states |
| Chain-highlighted / dimmed | – | Used by board hover (brief 03) |

Show what happens when a card combines several flags, for example Needs you + Running + 2 dependencies.
Define a priority order for which flag wins the primary status slot.

## Deliverables
A full variant sheet, light and dark, plus one "compact density" option if you think it's needed for 10 columns.
