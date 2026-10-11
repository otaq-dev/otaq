# 0036. Remove the Investigate and Ready for Dev columns: Backlog goes straight to Dev

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0004, 0009, 0016, 0018, 0019 (investigation part superseded), 0031, 0032, 0033
- **Supersedes:** the Investigate column and investigation step from 0019

## Context
0019 added an optional Investigate column (study the ticket, post findings and questions, build nothing), with Ready
for Dev as the landing place afterwards. The user decided that investigating is simply part of Dev work, and that
how an agent investigates belongs in the user's own work-ticket skill (the same principle as tests in 0018). Without
Investigate, Ready for Dev would only be a second Backlog.

## Decision
- **Board columns:** Backlog → Dev (Queued and Agent Working) → Ready for Review → Approved → Done (5 columns after
  0037 removed Needs Input).
- **Removed:**
  - The Investigate column and the Ready for Dev column
  - The Investigation tab
  - The `investigate-ticket` skill
  - `crewboard investigation post`
  - The "investigated ✓" marker
- **The agent investigates inside Dev**, in the same session, before building. How it does that (for example "explore, then
  ask before coding") is up to the user's own work-ticket skill.
- **Kept:**
  - `crewboard ask human` for questions during Dev (shown as "Asks you" in Dev, 0037)
  - `crewboard update` for agents and the Project terminal to improve a ticket's description ~~and "Done when"~~ (removed, 0067)
  - "Updated by agent" markers (0019)
- **"Look but don't build"** is covered by the Project terminal, or by writing it in the ticket ("investigate only, ask
  me before coding").
- The ticket form, Claude's ticket tools and the Project terminal from 0019 are unchanged.

## Changelog
- 2026-10-09: created (user decision)
- 2026-10-11: "Done when" removed (0067). A Ready for Dev column returns as the Queued state shown on its own (0066), not as a second Backlog.
