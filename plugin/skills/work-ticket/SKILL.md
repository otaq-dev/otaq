---
name: work-ticket
description: Work on the Otaq ticket named by $OTAQ_TICKET from start to review - get its context with `otaq prime`, follow the user's Agent instructions, implement, commit everything, then submit it with `otaq review submit`.
disable-model-invocation: true
---

# Work on your Otaq ticket

You are the agent for one Otaq ticket. Its key is in `$OTAQ_TICKET` (for example `SHOP-19`). You work in
the ticket's own git worktree, on its own branch. The user watches the board and can type into this session at any
time.

## Steps

1. **Get the context first.** Run `otaq prime` and read all of it: the ticket and its description, the tickets
   it depends on (with what their agents did), Otaq's rules and the user's **Agent instructions**.
2. **Follow the Agent instructions** that `prime` prints. They are the user's own rules (tests to run, skills to use,
   how to plan) and they apply to every step below. They add to these steps; they never remove getting the context
   (step 1) or submitting for review (step 5).
3. **Plan, then implement** until the ticket is done, as its description describes. Keep to the ticket; file anything
   else as a new ticket (see below) instead of doing it here. **Run `otaq notes` each time you start a new step**
   and act on what it says: in an epic, the epic's lead leaves notes there for you (for example "the API returns
   `items[]`, not `list`").
4. **Commit everything** with the Commit skill that `prime` names. Nothing may stay
   uncommitted: `review submit` is refused while files are uncommitted. Delete scratch files instead of committing
   them.
5. **Submit for review** with a short summary of what you changed, on stdin:

   ```sh
   otaq review submit --stdin <<'SUMMARY'
   Added the price table with sorting by price and name.
   Prices align right; the table scrolls on mobile widths.
   SUMMARY
   ```

   If you learned something a later ticket should know (an API shape, a trap, a file others will touch), end the
   summary with a paragraph that starts `Notes for later tickets:`. In an epic, the lead reads it and passes it on.

   Then **end your turn** and wait. Otaq may validate your work first: the result arrives in this session as a
   message. If validation failed, the message lists the findings: fix them, commit everything and submit again. When
   it passes, the ticket moves to Ready for Review and the user reviews the diff.

## Product questions: record, or ask only if you can't go on

When you meet a product question that the code and the documents don't settle (what to build, not how):

- **If you can't go on without the answer**, ask the user (below).
- **Otherwise decide and record it.** Build one option, record the choice on your ticket, and carry on; nothing
  waits. The user sees every recorded decision in the review and can change it there.

Give each field on its own line, `Other option:` once per option:

```sh
otaq decision record --stdin <<'DECISION'
Question: Split `fullName` everywhere, or only in the profile form?
Built: only in the profile form
Other option: everywhere
Why: the description only mentions the profile form
DECISION
```

The user's Agent instructions may add to this rule (for example "always ask about copy text"); follow them.

## When you need the user

If you are stuck, or you can't go on without the user's decision, ask, then end your turn:

```sh
otaq ask human --stdin <<'QUESTION'
Should `fullName` be split into first and last name everywhere, or only in the profile form?
QUESTION
```

The ticket shows "Asks you" on the board. The answer arrives in this session as the user's next message.

### Offer options when there are clear alternatives

When the question has a few clear alternatives, offer them as **2–4 options** so the user can answer with one key, and
recommend one when there is a sensible default:

```sh
otaq ask human --option "Postmark" --option "SES" --option "Resend" --recommended 1 --stdin <<'QUESTION'
Which email provider should send the order confirmation? Postmark is already used for password resets.
QUESTION
```

The answer then arrives as, for example, "Answer to your question: Postmark (option 1)." The user can always type
something else instead.

An **open question** stays free text, with no options:

```sh
otaq ask human --stdin <<'QUESTION'
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
- Useful commands: `otaq show <KEY>` (any ticket), `otaq dep tree` (your ticket's chain),
  `otaq create "Title" --stdin` (a follow-up in Backlog).
- If `otaq review submit` is refused, read why (usually uncommitted files), fix it and submit again.
