---
name: work-ticket
description: Work on the Crewboard ticket named by $CREWBOARD_TICKET from start to review - get its context with `crewboard prime`, follow the user's Agent instructions, implement, commit everything, then submit it with `crewboard review submit`.
disable-model-invocation: true
---

# Work on your Crewboard ticket

You are the agent for one Crewboard ticket. Its key is in `$CREWBOARD_TICKET` (for example `SHOP-19`). You work in
the ticket's own git worktree, on its own branch. The user watches the board and can type into this session at any
time.

## Steps

1. **Get the context first.** Run `crewboard prime` and read all of it: the ticket, its "Done when" list, the tickets
   it depends on (with what their agents did), Crewboard's rules and the user's **Agent instructions**.
2. **Follow the Agent instructions** that `prime` prints. They are the user's own rules (tests to run, skills to use,
   how to plan) and they apply to every step below. They add to these steps; they never remove getting the context
   (step 1) or submitting for review (step 5).
3. **Plan, then implement** until every "Done when" item is met. Keep to the ticket; file anything else as a new ticket
   (see below) instead of doing it here.
4. **Commit everything** with the Commit skill that `prime` names (by default `/crewboard:commit`). Nothing may stay
   uncommitted: `review submit` is refused while files are uncommitted. Delete scratch files instead of committing
   them.
5. **Submit for review** with a short summary of what you changed, on stdin:

   ```sh
   crewboard review submit --stdin <<'SUMMARY'
   Added the price table with sorting by price and name.
   Prices align right; the table scrolls on mobile widths.
   SUMMARY
   ```

   Then end your turn. The ticket moves to Ready for Review and the user reviews the diff.

## When you need the user

If you are stuck, or a decision belongs to the user, ask, then end your turn:

```sh
crewboard ask human --stdin <<'QUESTION'
Should `fullName` be split into first and last name everywhere, or only in the profile form?
QUESTION
```

The ticket shows "Asks you" on the board. The answer arrives in this session as the user's next message.

## Rules

- **Never push, never open a PR, never merge.** The user does that from the board.
- **Long text always goes through stdin** (`--stdin`), never quoted arguments: summaries, questions, ticket bodies.
- Useful commands: `crewboard show <KEY>` (any ticket), `crewboard dep tree` (your ticket's chain),
  `crewboard create "Title" --stdin` (a follow-up in Backlog), `crewboard update --add-done-when "…"`.
- If `crewboard review submit` is refused, read why (usually uncommitted files), fix it and submit again.
