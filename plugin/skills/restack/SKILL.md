---
name: restack
description: Bring the Otaq ticket named by $OTAQ_TICKET up to date with the branch it merges into - get the context with `otaq prime`, rebase onto the target it names, resolve the conflicts, commit, then end your stage with `otaq stage next`.
disable-model-invocation: true
---

# Rebase your Otaq ticket and resolve its conflicts

Your ticket's branch (its key is in `$OTAQ_TICKET`, for example `SHOP-19`) **conflicts with the branch it merges
into**: Otaq tried to rebase it for a Local merge, or after reopening the ticket, and git stopped on conflicts.
Otaq put everything back as it was; resolving the conflicts is your job now.

## Steps

1. **Get the context first.** Run `otaq prime` and read all of it. Its "Resolve first" section names the
   **target** to rebase onto (for example `main`, `develop`, or the epic's branch) and the conflicting files.
2. **Follow the Agent instructions** that `prime` prints. They are the user's own rules and they apply to every step
   below. They add to these steps; they never remove getting the context (step 1) or ending your stage (step 6).
3. **Rebase onto the target** that `prime` names, in this worktree:

   ```sh
   git rebase main
   ```

   Use the local target branch as `prime` names it; never push, never fetch-and-reset, never stash.
4. **Resolve every conflict.** Keep both sides' intent: the target's changes are other merged work, yours is this
   ticket's. Continue the rebase (`git add <file>`, then `git rebase --continue`) until it finishes. If a conflict needs
   a decision that belongs to the user, ask (see below) instead of guessing; `git rebase --abort` puts the branch back.
5. **Commit everything** with the commit skill `prime` names (`otaq-commit`, or your agent's own
   commit skill; when it names none, `git add -A` and `git commit`) if anything is left uncommitted after the rebase,
   then run the checks the Agent instructions ask for. Nothing may stay uncommitted.
6. **End your stage again** with a short summary of how you resolved the conflicts, on stdin:

   ```sh
   otaq stage next --stdin <<'SUMMARY'
   Rebased onto main. lib/pricing.ts: kept main's rounding and moved the new currency column on top of it.
   SUMMARY
   ```

   Then end your turn. The ticket moves to Ready for Review and the user can merge it.

## When you need the user

```sh
otaq ask human --stdin <<'QUESTION'
main renamed `total()` to `grandTotal()` while this ticket changed what `total()` returns. Should the new behaviour go
into `grandTotal()`?
QUESTION
```

When the question has a few clear alternatives, offer **2–4 options** (short labels, `--option "…"`, and
`--recommended N` when there is a sensible default); keep the question text on stdin. The user's Agent instructions
take precedence over this guidance.

## Rules

- **Never push, never open a PR, never merge, never stash.** The user does the merge from the board.
- **Long text always goes through stdin** (`--stdin`), never quoted arguments.
