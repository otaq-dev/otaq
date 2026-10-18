---
name: address-review
description: Address the review feedback on the Otaq ticket named by $OTAQ_TICKET after the user sent it back - get the context with `otaq prime`, read the feedback with `otaq review feedback`, fix it, commit everything, then submit it again with `otaq review submit`.
disable-model-invocation: true
---

# Address the review feedback on your Otaq ticket

The user reviewed your ticket (its key is in `$OTAQ_TICKET`, for example `SHOP-27`) and **sent it back** with line
comments and maybe a general note. You are in the same worktree and on the same branch as before.

## Steps

1. **Get the context first.** Run `otaq prime` and read all of it: the ticket and its description, Otaq's
   rules and the user's **Agent instructions**.
2. **Follow the Agent instructions** that `prime` prints. They are the user's own rules and they apply to every step
   below. They add to these steps; they never remove getting the context (step 1) or submitting for review (step 6).
3. **Read the feedback:** run `otaq review feedback`. It prints the comments of the latest round, by file and
   line (a "removed line" comment is about a line your change deleted), and the user's note. For a ticket with an
   open PR it then lists the PR's unresolved review comments; address them too.
4. **Fix every comment.** If a comment is unclear, or you disagree with it, ask the user (see below) instead of
   guessing. Keep to the feedback and the ticket; file anything else as a new ticket.
5. **Commit everything** with the Commit skill that `prime` names. Nothing may stay
   uncommitted: `review submit` is refused while files are uncommitted. Delete scratch files instead of committing
   them.
6. **Submit for review again** with a short summary of what you changed for each comment, on stdin:

   ```sh
   otaq review submit --stdin <<'SUMMARY'
   lib/search.ts: the page size now comes from settings; restored the removed bounds check.
   app/page.tsx: shows the total number of results.
   SUMMARY
   ```

   Then **end your turn** and wait. Otaq may validate your work first: the result arrives in this session as a
   message. If validation failed, the message lists the findings: fix them, commit everything and submit again. When
   it passes, the ticket moves to Ready for Review and the user reviews the diff again.

## When you need the user

If a comment is unclear, or a decision belongs to the user, ask, then end your turn:

```sh
otaq ask human --stdin <<'QUESTION'
The comment on lib/search.ts line 12 asks for the page size from settings. There is no such setting yet: add one?
QUESTION
```

The ticket shows "Asks you" on the board. The answer arrives in this session as the user's next message.

### Offer options when there are clear alternatives

When the question has a few clear alternatives, offer them as **2–4 options** so the user can answer with one key, and
give each option a **note with its pros and cons**, and recommend one when there is a sensible default:

```sh
otaq ask human \
  --option "Add a setting" --note "Users can pick their page size; a new Settings row and a migration." \
  --option "Keep 20" --note "No new setting to maintain, but the review comment stays unaddressed." \
  --recommended 1 --stdin <<'QUESTION'
The review asks for the page size from settings, but there is no such setting yet. Add one (default 20), or keep the
fixed 20?
QUESTION
```

The answer then arrives as, for example, "Answer to your question: Add a setting (option 1)." The user can always type
something else instead.

An **open question** stays free text, with no options:

```sh
otaq ask human --stdin <<'QUESTION'
What should the empty results page say?
QUESTION
```

- 2–4 options, each a short label (at most 40 characters). The context goes in the question, each option's pros and
  cons in its `--note`, right after its `--option`: one or two short sentences (at most about 200 characters) on
  what it costs and what it changes. Give every option one.
- Recommend one with `--recommended N` (1-based) when there is a sensible default; leave it out when there isn't.
- Never invent options for an open question.
- The question text always goes through stdin; only the short labels and notes are arguments.
- The user's Agent instructions take precedence over this guidance.

## Rules

- **Never push, never open a PR, never merge.** The user does that from the board.
- **Long text always goes through stdin** (`--stdin`), never quoted arguments: summaries, questions, ticket bodies.
- If `otaq review submit` is refused, read why (usually uncommitted files), fix it and submit again.
