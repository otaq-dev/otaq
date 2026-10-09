# 0052. Taking a ticket out of the agent's hands: "Submit for review" and "Park"

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0004 (refined), 0016, 0018, 0026, 0033, 0045, 0050

## Context
Grilling session, question 14. Only the agent could move a ticket out of Dev (`crewboard review submit`, 0004), apart from
closing it (0033). Two everyday cases weren't covered: the user finishes the work themselves (for example fixes the last
detail of SHOP-23 in the Shell tab), or the user wants to set a ticket aside without throwing its work away (SHOP-25 turned out
bigger than expected). Close means "won't do" and puts the card in Done.

## Decision
- **"Submit for review"** in the panel's "…" menu, available when the agent isn't mid-turn (idle, orange or Interrupted).
  - It runs the same checks as the agent's submit (refuses uncommitted files, 0050).
  - The summary is "Submitted by you", plus an optional note.
  - The session stays alive, so Send back works as usual.
- **Park: drag Dev → Backlog**, after a confirmation that the agent will be stopped.
  - Crewboard keeps the branch, the worktree (including uncommitted files) and the Claude session ID.
  - The card shows **"Parked · has work"**. It doesn't count toward the optional agent limit, and its ports are freed (0026).
  - **Dragging it back to Dev resumes the same conversation** (`claude --resume`, new ports). The drag is the go-ahead (0018).
- Parking is different from closing: a parked ticket stays in Backlog and keeps its work, while Close means "won't do" (0033).

## Changelog
- 2026-10-09: created (grilling session, user decision)
