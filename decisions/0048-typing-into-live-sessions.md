# 0048. Typing into live sessions: never mix with the user's unsent text

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0005, 0022, 0030, 0034, 0042, 0045, 0046

## Context
Otaq types text into a live `claude` terminal in several places: Send back (`/otaq:address-review`, 0034),
agent-to-agent questions and answers (0030), an inbox answer after a reboot (0045), "continue" after the usage-limit
reset (0046), and the "parent changed, branch restacked" note (0005). 0030 said "deliver when the agent is idle", but
nothing covered the user's own half-typed text. Example: the user has typed `also rename the` in SHOP-20's terminal
without pressing Enter, and SHOP-18's answer arrives. Typing it now would merge the two texts and send the mix to Claude.
(Grilling session, question 1.)

## Decision
- The background service already relays every keystroke (0022), so it tracks whether a session's **input line has
  unsent text**. This covers the in-app terminal and `otaq attach`.
- **Automatic deliveries** (agent-to-agent messages, the restack note, continue after the limit reset) are sent only
  when the agent is **idle between turns and the input line is empty**. Until then the card shows a
  **"1 message waiting"** chip with **Deliver now**.
- **The user's own clicks** (Send back, answering from the inbox) are never held. If the input line has unsent text, a
  short prompt asks: *"SHOP-20 has unsent text in its terminal: Keep it and cancel / Clear it and send."*
- Otaq **never clears the user's text silently**.
- **Ticket edits while the agent works** (grilling session, question 15): when the user saves changes to a ticket whose
  agent is running, Otaq sends a short automatic note under the rule above, with only what changed, for example
  *"The ticket was updated by the user. The description changed: added 'works on mobile widths'. Run `otaq prime`
  for the full ticket."* ("Done when" removed by 0067.) Edits made by the agent itself send nothing. Queued, Parked and Backlog tickets get nothing, because the agent
  reads the latest version when it starts or resumes.

## Alternatives considered
- Always deliver immediately and accept the occasional mix-up: rejected, because it sends garbage to the agent.
- Hold every delivery, including the user's clicks: rejected, because a click is already the go-ahead.

## Consequences
- The service has to keep a small per-session "unsent input" flag, reset by Enter, and cleared when Otaq clears the line.
- A new card chip ("N messages waiting · Deliver now") and the unsent-text prompt need designs (brief 06, update F).

## Changelog
- 2026-10-09: created (grilling session, user decision)
- 2026-10-09: the restack note is deferred with stacking (0049).
- 2026-10-09: user edits to a working ticket are sent to the agent as a short change note (grilling session, user decision).
- 2026-10-11: validation results (failed findings, cancellation) are automatic deliveries under these rules (0063); change-note example no longer mentions "Done when" (0067).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
- 2026-10-16: an epic lead's notes to a working child also wait as unread notes read at commit and `otaq prime`; `review submit` is refused while one is unread; typing it in when idle still applies (0083).
