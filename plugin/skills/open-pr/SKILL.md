---
name: open-pr
description: Crewboard's default Open PR workflow skill - get the context with `crewboard prime`, propose the branch name, commit messages, PR title and description with `crewboard pr propose --stdin`, and only after the user confirms push the branch and open the PR with `gh` or `az`, then report it with `crewboard pr opened <url>`.
disable-model-invocation: true
---

# Open a pull request for your Crewboard ticket

The user approved your ticket (its key is in `$CREWBOARD_TICKET`, for example `SHOP-18`) and clicked **Open PR**. You
prepare the pull request; **the user confirms it before anything is pushed**. This happens in two turns:

- **Turn 1 — propose.** You look at the work and propose a branch name, a message for each commit, a PR title and a
  description. Then you stop. Crewboard shows your proposal to the user, who can edit every part of it.
- **Turn 2 — push.** Once the user confirms, Crewboard renames the branch and rewords the commits itself, then types a
  message telling you to push. Only then do you push and open the PR.

## Turn 1: propose

1. **Get the context first.** Run `crewboard prime` and read all of it. Its "Pull request" section names the branch
   the PR targets, the remote, the PR provider, the ticket's External ID (if any) and every commit that goes into the
   PR, oldest first, with its current message.
2. **Follow the Agent instructions** that `prime` prints. They are the user's own rules (branch names, commit message
   style, PR templates) and they apply to every step below. They add to these steps; they never remove getting the
   context (step 1), proposing before pushing (step 5) or reporting the PR (step 9).
3. **Check the provider.** `prime` names it from the `origin` remote URL: `github.com` is **GitHub** (you will use
   `gh`); `dev.azure.com` or `*.visualstudio.com` is **Azure DevOps** (you will use `az repos`). For any other remote,
   **stop**: propose nothing, push nothing, and reply "This remote isn't GitHub or Azure DevOps, so the built-in Open PR
   skill can't open the PR. Use your own Open PR skill (Settings → Workflow skills)." Then end your turn.
4. **Write the proposal.**
   - **Branch name:** follow the user's rules if the Agent instructions give any (for example
     `bugfix/SHOP-18-login-redirect`, or `feature/PAY-123-wishlist` from the External ID). Otherwise use
     `crewboard/SHOP-18-<slug>`, where the slug is a few lowercase words of the title joined by `-`.
   - **One message per commit**, for every commit `prime` lists, in the same order. Keep each message about what that
     commit changed. You only propose: never run `git commit --amend`, `git rebase` or `git branch -m` yourself.
   - **PR title:** the key (or the External ID) and the title, for example `SHOP-18 Login redirect`.
   - **PR description:** what changed and why, and how it was tested, in Markdown.
5. **Propose it** with `crewboard pr propose --stdin`. The proposal is one JSON object on stdin:

   ```sh
   crewboard pr propose --stdin <<'PROPOSAL'
   {
     "branch": "crewboard/SHOP-18-login-redirect",
     "commits": [
       {"sha": "a1b2c3d", "message": "SHOP-18: return to the page you came from after login"},
       {"sha": "e4f5a6b", "message": "SHOP-18: test the redirect for deep links"}
     ],
     "title": "SHOP-18 Login redirect",
     "description": "After login the app now returns to the page you came from.\n\nTested with the new e2e test."
   }
   PROPOSAL
   ```

   `sha` is the commit's ID as `prime` lists it (short or full). The list must hold every listed commit, in order. If
   the command refuses the proposal, fix what it names and propose again.
6. **End your turn.** Don't push and don't wait in a loop. The user reviews the proposal in Crewboard.

## Turn 2: push and open the PR

Crewboard types a message such as "The user confirmed the pull request" into this session once the user clicks **Push &
open PR**. If the user cancels instead, nothing happens and nothing is pushed.

7. **Read what was confirmed.** Run `crewboard prime` again. Its "Pull request" section now shows the **confirmed**
   branch name, title and description. The user may have edited them: use them exactly as shown. Crewboard has already
   renamed the branch and reworded the commits; don't change them.
8. **Push and open the PR** against the target branch `prime` names, with the confirmed title and description. Put the
   description in a file, never in a quoted argument:
   - **GitHub:**

     ```sh
     git push -u origin crewboard/SHOP-18-login-redirect
     gh pr create --base main --head crewboard/SHOP-18-login-redirect \
       --title "SHOP-18 Login redirect" --body-file /tmp/SHOP-18-pr.md
     ```

     `gh pr create` prints the PR's URL.
   - **Azure DevOps:**

     ```sh
     git push -u origin crewboard/SHOP-18-login-redirect
     az repos pr create --source-branch crewboard/SHOP-18-login-redirect --target-branch main \
       --title "SHOP-18 Login redirect" --description "$(cat /tmp/SHOP-18-pr.md)" --output json
     ```

     The PR's URL is the repository's `webUrl` from that JSON, then `/pullrequest/` and its `pullRequestId`.

   Use the user's own logins and git credentials as they are. Never store, print or ask for a token. If `gh` or `az`
   says you are not logged in, or the push is rejected, don't retry with other options: tell the user what failed with
   `crewboard ask human --stdin` (for example "gh isn't logged in. Run `gh auth login`, then tell me to retry.") and end
   your turn.
9. **Report the PR:** `crewboard pr opened <url>`, for example
   `crewboard pr opened https://github.com/acme/shop-web/pull/481`. The ticket then shows "PR #481". End your turn.

## Rules

- **Never push before Crewboard tells you the user confirmed.** Turn 1 only proposes.
- Push only the confirmed branch, to `origin`, and never with `--force`. Never merge the PR yourself.
- Don't change files or commits in either turn. If something must change, ask the user with
  `crewboard ask human --stdin` instead.
- **Long text always goes through stdin** (`--stdin`) or a file, never quoted arguments.
