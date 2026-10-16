# 0081. Ticket size check: a cheap agent flags Backlog tickets that look too big for one agent run

- **Status:** Proposed
- **Date:** 2026-10-16
- **Related:** 0016, 0018 (exception), 0020, 0040, 0066, 0068

## Context
A ticket that is one task for a person can be several for an agent: as a session nears its context limit, the agent
loses track and makes mistakes. Splitting tickets well is up to the user today, and nothing checks it. The user wants
Otaq to warn when a ticket looks too big, without ever blocking them.

## Decision (agreed so far)
- **A "Check size" box in the ticket form** (and `otaq create … --check-size` / `otaq update … --check-size`), under
  the description: *"A cheap agent reads the code and tells you if this is too big for one agent run."* Ticking it is
  the user's go-ahead for that ticket's check (0018's click rule, no exception needed). Ticking it when editing a ticket
  checks it again.
- **A per-project setting is the box's default**, off by default (nothing spends tokens unless the user says so, 0018).
  Onboarding shows it so the user knows the option exists; how it shows it (its own step, a checkbox, a line on the
  Done screen) is left to the design agent (brief 14).
- **Tickets only, not epics.** Standalone tickets and epic children have the box; the epic form doesn't (an epic never
  runs, 0039). An epic is where a ticket flagged as too big goes when it is split.
- **Saving never waits for the check.** The ticket lands in Backlog at once; the check runs in the background and its
  result appears on the card when it is done.
- **Tickets a Project agent creates are not checked** (they don't go through the form); its skill sizes them (open
  question: the size rule).
- **The manual form stays** (0019). Creating tickets only through an agent was considered and rejected (see below).
- **It reads the code.** A read-only `claude` session in the project folder with the cheapest model (Haiku) and only the
  read and search tools, so it can see how much of the code a ticket touches (for example "renaming `User.id` touches 140
  files in 6 modules"), not just the ticket text.
- **The card** shows a quiet "Looks big" hint with the reason.
- **On drag to Ready for Dev or Dev**, if the stored result flags the ticket, a dialog appears at once:
  **Start anyway** · **Split with a Project agent** (0068) · **Cancel**. The user can always start.
- **The drag never waits.** If the check hasn't finished, the ticket starts as it does today, with no dialog.
- Check tokens count on the ticket and in today's total (0020).

## Alternatives considered
- **Checking at the moment of the drag:** rejected. Reading the code takes 30 s to 2 min, and a ticket the user just told
  to start would then stop to ask for confirmation.
- **Checking the ticket text only:** rejected. It can't see how much of the code a change touches.
- **Checking every Backlog ticket automatically (behind a project setting):** replaced by the per-ticket box, which is a
  real click, spends tokens only where the user wants them and leaves Project agent tickets out by itself.
- **Creating tickets only through an agent, so every ticket is sized:** rejected. Quick tickets would take minutes and
  tokens, no ticket could be written while the plan limit holds Project agents (0068), and `otaq create` from
  scripts would need the same exception. The form stays, with the box.
- **Leaving splitting entirely to the user:** rejected. Tickets that are too big are a main cause of failed agent runs.

## Changelog
- 2026-10-16: created as Proposed (discussion with the user): setting as go-ahead, background check that reads code,
  dialog on drag, the drag never waits.
- 2026-10-16: the setting is off by default and onboarding shows it (user decision).
- 2026-10-16: how onboarding shows it is left to the design agent (user decision).
- 2026-10-16: per-ticket "Check size" box in the form, with the project setting as its default, replaces the automatic
  check of every Backlog ticket; saving never waits; Project agent tickets aren't checked; the manual form stays (user
  decisions).
- 2026-10-16: tickets and epic children only; epics have no box (user decision).
