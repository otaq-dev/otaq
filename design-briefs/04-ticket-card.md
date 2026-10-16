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
| ~~Backlog, missing "Done when"~~ | – | **Removed** (0067) |
| Queued, waiting on dependencies | SHOP-21 | "waiting on SHOP-20" |
| Queued, ready to start | – | About to start |
| Queued, **Start interrupted** | SHOP-28 | Retry button |
| Agent Working, normal | SHOP-19 | Last signal, tokens |
| Agent Working, **stale signal** | BILL-4 | "active 4 min ago" warning |
| Agent Working, **Needs you** (permission prompt) | SHOP-22 | Shows the prompt text, shortened |
| Queued, **waits for merge** | SHOP-20 | "waits for SHOP-18, SHOP-19 to merge" (0049) |
| Agent Working, **Working in background** | SHOP-19 | "background: test run, 1 helper agent · 6 min" (neutral, not turquoise, 0042) |
| Agent Working, **Interrupted** | SHOP-25 | Resume button |
| Dev · Idle (turquoise, 0042) | SHOP-19 variant | "Idle · ended its turn: 'Should I also update the tests?'" |
| Dev · Asks you | SHOP-23 | Shows the question, shortened; sorted to the top of Dev (0037) |
| Ready for Review | SHOP-18 | +412 −38 · 9 files |
| Agent Working · **Sent back** | SHOP-27 | "Sent back · 3 comments · round 2" (0034) |
| Approved, **PR open** | SHOP-26 | "PR #481 · GitHub", and Mark merged / Mark closed |
| Done · Merged | SHOP-17 | Faded |
| Done · Closed, not merged | SHOP-26 after Mark closed | Faded, plus a "Reopen" affordance (0033) |
| Parent closed | SHOP-20 variant | "parent SHOP-18 closed" (0033) |
| Asked a linked ticket, waiting for the answer | SHOP-20 | "asked SHOP-18 · waiting" (0030) |
| Running (Run, on any card) | SHOP-19 | `▶ :4019` |
| Pinned (0047) | SHOP-19 | Pin icon; first in its column, above turquoise cards |
| Selected / focused / hovered / dragging | – | Interaction states |
| Chain-highlighted / dimmed | – | Used by board hover (brief 03) |

Show what happens when a card combines several flags, for example Needs you + Running + 2 dependencies.
Define a priority order for which flag wins the primary status slot.

## Deliverables
A full variant sheet, light and dark, plus one "compact density" option if you think it's needed for 10 columns.

## M12 additions (design update K; decisions 0063–0068)
- **Priority** on every card (1 Urgent · 2 High · 3 Normal · 4 Low). Normal (3) may be quiet or hidden; Urgent must stand out
  without looking like a needs-you state. Samples: SHOP-31 (1), SHOP-35 (2), SHOP-24 (4).
- **"Ready to start"** hint on an unblocked Backlog card (SHOP-24).
- The **"missing Done when"** variant is removed.
- New variants:

  | Variant | Sample | Notes |
  |---|---|---|
  | Dev · **Validating…** | SHOP-29 | Neutral (calm, like working), "round 1 of 3 · 2 min"; not turquoise |
  | Agent Working · **Validation failed · round 2 of 3** | SHOP-30 | Like "Sent back": a marker, the agent is working |
  | Dev · **Needs you · validator** (turquoise) | SHOP-33 | Shows the validator's prompt, shortened: "validator: Allow `Bash(pnpm test)`?" |
  | Dev · **Validation failed 3 times** (turquoise) | SHOP-32 | The last finding, shortened; actions in the panel |
  | Dev · **Validation ended without a result** (turquoise) | SHOP-34 | Retry validation |
  | Dev · **Merge conflict · fixing** | SHOP-40.4 | Working, with the conflict marker (Auto-merge, 0064) |
  | Dev · **Keeps conflicting with `otaq/SHOP-40`** (turquoise) | SHOP-40.4 variant | After 3 conflicts in a row |
  | Ready for Review · **Validated · round 2** | SHOP-18 | Plus the existing +412 −38 · 9 files |
  | Ready for Review · **Validation skipped by you** / **Submitted by you** / **Failed 3 times · moved by you** | SHOP-18 variants | The last one must read as "did not pass" |
  | Done · **Merged automatically** | SHOP-40.1 | Faded like Merged |
  | Ready for Dev · **waiting for a free slot (5 / 5 agents)** | SHOP-35 | Limit sample |
  | Backlog · **created by Project agent "Checkout slowness"** | SHOP-36 | Origin label naming the agent (0068) |
- Add the new states to the flag priority order (which wins the primary status slot).

## Update Q (0080): the epic sleeve
Epic children wear a violet sleeve ("Wishlist · 2 of 3") instead of the epic chip; standalone tickets have none. The epic
card's child rows can be dragged one by one. See `CHANGES-2026-10-15-q.md`.

## Update S (0083): children and the lead
New epic-child states: "asked lead · waiting" (display only), "Asks you · via Wishlist lead" (a passed-on question), and
"Updated by Wishlist lead". See `CHANGES-2026-10-16-s.md`.

## Update T (0083): unread notes and the same-file flag
Quiet markers on working epic children: "1 unread note from Wishlist lead" and "same file as SHOP-40.4" (file on hover).
Neither is turquoise. See `CHANGES-2026-10-16-t.md`.
