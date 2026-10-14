---
name: push
description: Otaq's default Push workflow skill - get the context with `crewboard prime`, propose messages for the commits not yet on the PR's branch with `crewboard pr propose --stdin`, and only after the user confirms push them to the existing PR, then report it with `crewboard pr opened <url>`.
disable-model-invocation: true
---

# Push new commits to your ticket's pull request

Your ticket (its key is in `$CREWBOARD_TICKET`, for example `SHOP-26`) already has an open pull request. The user
reviewed the rework and clicked **Push to PR**. You propose the messages of the new commits; **the user confirms before
anything is pushed**. This happens in two turns, like opening the PR did.

## Turn 1: propose

1. **Get the context first.** Run `crewboard prime` and read all of it. Its "Pull request" section names the PR, its
   branch (locked: it can't be renamed any more), the target branch, and the **new** commits that aren't on the remote
   branch yet, oldest first, with their current messages. Commits that were already pushed are not yours to change.
2. **Follow the Agent instructions** that `prime` prints. They are the user's own rules and they apply to every step
   below. They add to these steps; they never remove getting the context (step 1), proposing before pushing (step 3)
   or reporting back (step 7).
3. **Propose** with `crewboard pr propose --stdin`: one message per new commit, in the listed order. Keep the branch as
   `prime` names it. The title and description stay those of the open PR, so leave them out:

   ```sh
   crewboard pr propose --stdin <<'PROPOSAL'
   {
     "branch": "bugfix/SHOP-26-checkout-totals",
     "commits": [
       {"sha": "9c8d7e6", "message": "SHOP-26: round totals per line, as the review asked"}
     ]
   }
   PROPOSAL
   ```

   If the command refuses the proposal, fix what it names and propose again. Never reword or rebase commits yourself.
4. **End your turn.** Don't push. The user reviews the messages in Otaq.

## Turn 2: push

Otaq types a message such as "The user confirmed the push" into this session once the user clicks **Push**. If the
user cancels instead, nothing happens and nothing is pushed.

5. Run `crewboard prime` again. Otaq has already reworded the new commits as confirmed; don't change them.
6. **Push the branch** to `origin`, without `--force`:

   ```sh
   git push origin bugfix/SHOP-26-checkout-totals
   ```

   Use the user's own git credentials as they are; never store, print or ask for a token. If the push is rejected
   (for example because someone else pushed to the PR), don't force it and don't rebase: tell the user what git said
   with `crewboard ask human --stdin` and end your turn.
7. **Report back** with the PR's URL as `prime` shows it: `crewboard pr opened <url>`, for example
   `crewboard pr opened https://github.com/acme/shop-web/pull/481`. The ticket returns to "Approved · PR open". End
   your turn.

## Rules

- **Never push before Otaq tells you the user confirmed**, and never with `--force`.
- Don't change files or commits. If something must change, ask the user with `crewboard ask human --stdin`.
- **Long text always goes through stdin** (`--stdin`), never quoted arguments.
