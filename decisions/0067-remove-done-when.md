# 0067. No "Done when" field: acceptance criteria live in the description

- **Status:** Accepted
- **Date:** 2026-10-11
- **Related:** 0019 (refined), 0032 (refined), 0036 (refined), 0048 (refined), 0063
- **Supersedes:** the "Done when" checklist and the empty-"Done when" warning of 0019

## Context
Grilling session, 2026-10-11. The user doesn't want Crewboard to judge how well a ticket is written, or to change how people
write tickets for their agents. The agent, and now the validator (0063), are responsible for working out what "done" means
from whatever the ticket says.

## Decision
- The **"Done when" checklist is removed** everywhere: the ticket form (including its add row), the card's "missing Done
  when" hint, the warning dialog when a ticket goes to Dev, the `crewboard prime` section, the CLI flags `--done-when` and
  `--add-done-when`, and the "Added to 'Done when'" change notes.
- A ticket has a **title and a description** (plus its other fields). People may still write acceptance criteria in the
  description, for example under a "Done when" heading; that is ordinary text with no special handling.
- **Existing items are moved into the description** by a migration, appended as a Markdown list under a "Done when" heading,
  so nothing is lost.
- Crewboard never warns about a missing or thin description.
- The agent's skills say "until the ticket is done, as its description describes", and the validator checks against the
  description (0063).
- Not affected: the "Done when" lists in this repository's own `tickets/` files and decision 0056, which are about building
  Crewboard, not the product.

## Changelog
- 2026-10-11: created (grilling session, user decision)
