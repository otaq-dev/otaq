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

1. **Get the context first.** Run `crewboard prime` and read all of it: the ticket and its description, the tickets
   it depends on (with what their agents did), Crewboard's rules and the user's **Agent instructions**.
2. **Follow the Agent instructions** that `prime` prints. They are the user's own rules (tests to run, skills to use,
   how to plan) and they apply to every step below. They add to these steps; they never remove getting the context
   (step 1) or submitting for review (step 5).
3. **Plan, then implement** until the ticket is done, as its description describes. Keep to the ticket; file anything
   else as a new ticket (see below) instead of doing it here.
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

   Then **end your turn** and wait. Crewboard may validate your work first: the result arrives in this session as a
   message. If validation failed, the message lists the findings: fix them, commit everything and submit again. When
   it passes, the ticket moves to Ready for Review and the user reviews the diff.

## When you need the user

If you are stuck, or a decision belongs to the user, ask, then end your turn:

```sh
crewboard ask human --stdin <<'QUESTION'
Should `fullName` be split into first and last name everywhere, or only in the profile form?
QUESTION
```

The ticket shows "Asks you" on the board. The answer arrives in this session as the user's next message.

### Offer options when there are clear alternatives

When the question has a few clear alternatives, offer them as **2–4 options** so the user can answer with one key, and
recommend one when there is a sensible default:

```sh
crewboard ask human --option "Postmark" --option "SES" --option "Resend" --recommended 1 --stdin <<'QUESTION'
Which email provider should send the order confirmation? Postmark is already used for password resets.
QUESTION
```

The answer then arrives as, for example, "Answer to your question: Postmark (option 1)." The user can always type
something else instead.

An **open question** stays free text, with no options:

```sh
crewboard ask human --stdin <<'QUESTION'
What should the empty wishlist page say?
QUESTION
```

- 2–4 options, each a short label (at most 40 characters). Put the explanation in the question, not in the labels.
- Recommend one with `--recommended N` (1-based) when there is a sensible default; leave it out when there isn't.
- Never invent options for an open question.
- The question text always goes through stdin; only the short labels are arguments.
- The user's Agent instructions take precedence over this guidance.

## Rules

- **Never push, never open a PR, never merge.** The user does that from the board.
- **Long text always goes through stdin** (`--stdin`), never quoted arguments: summaries, questions, ticket bodies.
- Useful commands: `crewboard show <KEY>` (any ticket), `crewboard dep tree` (your ticket's chain),
  `crewboard create "Title" --stdin` (a follow-up in Backlog).
- If `crewboard review submit` is refused, read why (usually uncommitted files), fix it and submit again.
