# 0044. Closing parents and epics, grouping into epics later, and reopening

- **Status:** Accepted
- **Note (0049):** Item 1: with 0049 a child never starts before its parent is merged, so there are no working children to pause; the inbox item for a closed parent still applies to Queued children.
- **Date:** 2026-10-09
- **Related:** 0005, 0029, 0033 (refined), 0038, 0039 (refined)

## Context
0033 (Closed tickets) and 0039 (epics) left gaps around closed parents, closing epics, grouping existing tickets, and
reopening merged work. (Raised by the product owner, review 2.)

## Decision
1. **Closed parents:** a Closed ticket **never counts as ready** for dependencies (0005), even though it is shown inside Done.
   When a parent is closed, its **working children pause** ("parent SHOP-18 closed"), so no tokens are spent on work that
   can't merge. Pausing starts nothing. The inbox item from 0033 asks the user to reopen the parent or remove the dependency.
2. **Closing an epic with unfinished tickets:** a **single confirmation** lists the unfinished child tickets and any agents
   that will be stopped, then **closes them all** (bulk Close ticket, 0033). Merged children stay merged.
3. **An epic with nothing merged:** if every child ends Closed, the epic shows **"Closed · not merged"** in Done, not "Merged".
4. **Grouping later (refines 0039):** a ticket that has **never been to Dev** (no branch or worktree yet) can be moved into
   an epic. It gets the epic's next child key (for example SHOP-51 → SHOP-50.2). The **old key keeps resolving** as an
   alias (`crewboard show SHOP-51` still works). Tickets that have been to Dev keep their key.
5. **Reopening (refines 0033):** **only Closed tickets can be reopened.** Merged tickets are locked, and fixing merged work
   means a new ticket. When a reopened ticket is dragged to Dev, its old branch is first rebased onto the current base with
   the same no-token attempt as 0038. On a conflict, resolving it is the agent's first task.
   The agent starts in a **new session** (`/crewboard:work-ticket` on the old branch), not by resuming the old conversation:
   Close means "stop this", unlike Park (0052), which resumes. If the rebase conflicted, `crewboard prime` tells the agent to
   resolve it first (the `restack` skill).

## Changelog
- 2026-10-09: created (product owner review 2, user decision)
- 2026-10-09: the free rebase on reopen keeps uncommitted worktree changes with a checkpoint commit (0005).
- 2026-10-09: item 1 mostly moot in v0.1: children start only after the parent is merged (0049).
- 2026-10-09: Move to epic: the ticket adopts the epic's base branch (0051).
- 2026-10-09: a ticket moved into an epic uses the epic's branch (0059).
- 2026-10-09: a reopened ticket starts a new session, not a resume (ticket-writing review Q5, user decision).
