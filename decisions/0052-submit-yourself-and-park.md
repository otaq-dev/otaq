# 0052. Taking a ticket out of the agent's hands: "Submit for review" and "Park"

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0004 (refined), 0016, 0018, 0026, 0033, 0045, 0050

## Context
Grilling session, question 14. Only the agent could move a ticket out of Dev (`otaq review submit`, 0004), apart from
closing it (0033). Two everyday cases weren't covered: the user finishes the work themselves (for example fixes the last
detail of SHOP-23 in the Shell tab), or the user wants to set a ticket aside without throwing its work away (SHOP-25 turned out
bigger than expected). Close means "won't do" and puts the card in Done.

## Decision
- **"Submit for review"** in the panel's "…" menu, available when the agent isn't mid-turn (idle, turquoise or Interrupted).
  - It runs the same checks as the agent's submit (refuses uncommitted files, 0050).
  - The summary is "Submitted by you", plus an optional note.
  - It **skips validation** and is never auto-merged (0063, 0064). A second item, **Validate and submit**, runs the agent's
    flow (validation, rounds, Auto-merge).
  - The session stays alive, so Send back works as usual.
- **Park: drag Dev → Backlog**, after a confirmation that the agent will be stopped.
  - Otaq keeps the branch, the worktree (including uncommitted files) and the Claude session ID.
  - The card shows **"Parked · has work"**. It doesn't count toward the optional agent limit, and its ports are freed (0026).
  - **Dragging it back to Dev resumes the same conversation** (`claude --resume`, new ports). The drag is the go-ahead (0018).
- Parking is different from closing: a parked ticket stays in Backlog and keeps its work, while Close means "won't do" (0033).

## Changelog
- 2026-10-09: created (grilling session, user decision)
- 2026-10-11: Submit for review skips validation; Validate and submit added (0063). Park also stops a running validator (0063).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
- 2026-10-15: dragging an epic's track to Backlog when every child has started parks all running children after one confirmation (0080).
