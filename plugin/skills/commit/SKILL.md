---
name: commit
description: Otaq's default Commit workflow skill - commit every change in this worktree with "<KEY>: <what changed>" messages, before `crewboard review submit`.
---

# Commit everything

Commit all of your work in this worktree so nothing is left uncommitted. `crewboard review submit` is refused while
any file is uncommitted, and the user reviews and merges only what is committed. Otaq never commits for you.

1. Run `git status` and look at every changed, deleted and untracked file.
2. Delete scratch files you made (logs, notes, temporary scripts) instead of committing them. Never commit secrets or
   env files (`.env`, keys, tokens). If one shows up as untracked, don't commit it and don't delete it: ask the user
   with `crewboard ask human --stdin` (it may belong in `.gitignore`).
3. Stage the rest and commit. Use one commit, or a few if the work splits into clear steps. Each message starts with
   the ticket key from `$CREWBOARD_TICKET`:

   ```sh
   git add -A
   git commit -m "SHOP-19: add the price table with sorting"
   ```

4. The user's git hooks and settings apply. If a hook rejects the commit, fix the problem and commit again; never use
   `--no-verify`.
5. Check that `git status` is clean. **Never push.**
