# 0084. Recorded decisions: an agent decides unless the question blocks it, and the user sees every choice in review

- **Status:** Accepted
- **Date:** 2026-10-16
- **Related:** 0016, 0030, 0034, 0040, 0050, 0059, 0061, 0062, 0063, 0064, 0079, 0083

## Context
While building v0.1 and M15/M16 outside Otaq, implementing agents followed the runbook rule "stop only for a blocker;
otherwise put it in a file and proceed" (0062). They wrote about 40 "Deferred: built as option N" entries into
`tickets/questions.md`: real product choices, made without asking the user. The user later resolved 10 of them and
changed 3; 31 were never reviewed (`reports/Lead and child agents - evidence from past runs.md`). Only 2 real product
blockers came up in all runs. Otaq's agents today either ask the user (`otaq ask human`, and wait) or make such a
choice with no record at all.

The user wants agents not to be blocked, and not to make silent assumptions either.

## Decision
**Every ticket's agent** (standalone tickets and epic children) follows one rule when it meets a product question
that the code and the documents don't settle:
- **If it can't go on without the answer**, it asks: `otaq ask human` as today (0061 options). An epic child's
  question goes to its lead first (0083).
- **Otherwise it decides and records it.** It builds one option and records the choice on its ticket with a new
  `otaq` command: the question, the options it saw, what it built and why. For example, SHOP-23: "Split `fullName`
  everywhere or only in the profile form? Built: only in the profile form. Other option: everywhere. Why: the
  description only mentions the profile form." Nothing waits.
- **Recorded decisions are first-class.** Otaq stores them on the ticket (not in a file in the user's repo, 0023) and
  **lists them in the review**: in the ticket's own review, and in its epic's review together with every child's, so
  the user sees them even when a ticket was merged automatically (0064). They also show in the ticket's History.
  While the ticket works they are read-only in its panel; in Ready for Review its card shows a neutral count ("1
  decision"), and nothing after approval.
- **In review, Approve keeps them all; each item has Change….** Approving marks every listed decision "kept by you"
  in the History; no extra click is needed to keep one.
  - On a ticket still in review, **Change…** sends it back to its agent with the other option (or the user's own
    words), like Send back (0034). For example "Split `fullName` everywhere".
  - In an epic's review the child is already merged, so **Change…** creates a new child for the fix ("Change: split
    `fullName` everywhere", added by you); the epic becomes a track again, as for any added child (0080, 0083).
- The built-in `work-ticket` skill teaches the rule; the user's Agent instructions (0040) can add to it ("always ask
  about copy text").

## Alternatives considered
- **Always ask and wait:** rejected. Past runs show product questions are frequent and real blockers rare; the user
  doesn't want work blocked.
- **Ask, and build around the open point (no decision until the user answers):** not chosen; the ticket would wait at
  the end for each open point.
- **Ticking Keep or Change on every decision before Approve:** rejected; it adds clicks to every review and invites
  ticking without reading.
- **A list for information only:** rejected; it repeats the past runs, where choices were seen late or never acted on.
- **A setting choosing between asking and deciding** (enforced by Otaq): not added for now; the review list makes every
  choice visible. Can be added later.
- **Epic children only:** rejected; most past choices came from tickets outside an epic, and every agent runs the same
  skill.

## Consequences
- A new `otaq` command for recording a decision (name settled in the SDD), a new record on the ticket, and a
  "Decisions" list in the review (brief 07) and in the epic's review.
- An epic's lead reads each merged child's recorded decisions, passes them on to never-started siblings and marks
  any that a written source contradicts; it never overrules one (0083).

## Changelog
- 2026-10-16: created (user decision, after the evidence report on past runs)
- 2026-10-16: Approve keeps every listed decision; Change… sends the ticket back, or adds a fix child in an epic's
  review (user decision)
- 2026-10-16: the epic lead passes children's decisions on and marks contradicted ones (0083)
- 2026-10-16: a ticket in Ready for Review shows a neutral "N decisions" count on its card (design audit T, user decision)
