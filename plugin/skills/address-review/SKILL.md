---
name: address-review
description: Address the review feedback on the Crewboard ticket named by $CREWBOARD_TICKET after the user sent it back - get the context with `crewboard prime`, read the feedback with `crewboard review feedback`, fix it, commit everything, then submit it again with `crewboard review submit`.
disable-model-invocation: true
---

# Address the review feedback on your Crewboard ticket

The user reviewed your ticket (its key is in `$CREWBOARD_TICKET`, for example `SHOP-27`) and **sent it back** with line
comments and maybe a general note. You are in the same worktree and on the same branch as before.

## Steps

1. **Get the context first.** Run `crewboard prime` and read all of it: the ticket, its "Done when" list, Crewboard's
   rules and the user's **Agent instructions**.
2. **Follow the Agent instructions** that `prime` prints. They are the user's own rules and they apply to every step
   below. They add to these steps; they never remove getting the context (step 1) or submitting for review (step 6).
3. **Read the feedback:** run `crewboard review feedback`. It prints the comments of the latest round, by file and
   line (a "removed line" comment is about a line your change deleted), and the user's note.
4. **Fix every comment.** If a comment is unclear, or you disagree with it, ask the user (see below) instead of
   guessing. Keep to the feedback and the ticket; file anything else as a new ticket.
5. **Commit everything** with the Commit skill that `prime` names (by default `/crewboard:commit`). Nothing may stay
   uncommitted: `review submit` is refused while files are uncommitted. Delete scratch files instead of committing
   them.
6. **Submit for review again** with a short summary of what you changed for each comment, on stdin:

   ```sh
   crewboard review submit --stdin <<'SUMMARY'
   lib/search.ts: the page size now comes from settings; restored the removed bounds check.
   app/page.tsx: shows the total number of results.
   SUMMARY
   ```

   Then end your turn. The ticket moves to Ready for Review and the user reviews the diff again.

## When you need the user

If a comment is unclear, or a decision belongs to the user, ask, then end your turn:

```sh
crewboard ask human --stdin <<'QUESTION'
The comment on lib/search.ts line 12 asks for the page size from settings. There is no such setting yet: add one?
QUESTION
```

The ticket shows "Asks you" on the board. The answer arrives in this session as the user's next message.

### Offer options when there are clear alternatives

When the question has a few clear alternatives, offer them as **2–4 options** so the user can answer with one key, and
recommend one when there is a sensible default:

```sh
crewboard ask human --option "Add a setting" --option "Keep 20" --recommended 1 --stdin <<'QUESTION'
The review asks for the page size from settings, but there is no such setting yet. Add one (default 20), or keep the
fixed 20?
QUESTION
```

The answer then arrives as, for example, "Answer to your question: Add a setting (option 1)." The user can always type
something else instead.

An **open question** stays free text, with no options:

```sh
crewboard ask human --stdin <<'QUESTION'
What should the empty results page say?
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
- If `crewboard review submit` is refused, read why (usually uncommitted files), fix it and submit again.
