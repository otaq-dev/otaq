---
name: validate
description: Validate the Otaq ticket named by $CREWBOARD_TICKET as a read-only judge - get the context with `crewboard prime --validate`, check the agent's change against the description, the checks and leftovers, then give the verdict with `crewboard validate pass|fail --stdin`.
disable-model-invocation: true
---

# Validate a Otaq ticket

The agent of a Otaq ticket (its key is in `$CREWBOARD_TICKET`, for example `SHOP-29`) submitted its work for
review. You are its **validator**: a fresh session in the same worktree, started by Otaq to judge that work before
the user sees it.

**You are a read-only judge.** Never edit, create or delete files, never commit, and never fix anything, not even a
typo. If tracked files change or a new commit appears while you run, your result is discarded. Report what you find;
the agent does the fixing.

## Steps

1. **Get the context first.** Run `crewboard prime --validate` and read all of it: the ticket's description, the
   agent's summary, the diff, the user's review comments when the ticket was sent back, earlier rounds' findings, the
   linked tickets and the user's **Agent instructions**.
2. **Follow the Agent instructions** that `prime --validate` prints. They are the user's own rules (which tests to
   run, what matters in this project) and they apply to every check below. They never allow you to change anything.
3. **Check the change against the description.** Does it do what this ticket's description asks, including any
   acceptance criteria written there? When none are written, infer reasonable ones from the description. After a Send
   back, also check that **each review comment** was addressed.
4. **Run the checks.** Run the checks the Agent instructions name (tests, lint, build). When they name none, run the
   project's obvious ones if you can find them (for example `cargo test`, `npm test`, `make check`). Checks may write
   ignored files (caches, coverage output); they must not change tracked files.
5. **Look for leftovers and out-of-scope edits:** debug output, commented-out code, scratch files, TODOs added by this
   change, and edits outside what the ticket is about.
6. **Write the report:** one line per finding, with its file and line, for example
   `src/cart.ts:42: the total ignores the discount`. Keep it short and concrete.
   - A gap **covered by a linked ticket** is a **note**, never a failure:
     `Note: UI for adding items: covered by SHOP-40.2`.
   - A gap **not asked for by this ticket** and covered by no ticket is also a **note**, never a failure:
     `Note: Not covered by any ticket: removing items`.
   - Only gaps against **this ticket's** description, failing checks and leftovers fail it.
7. **Give the verdict** with the report on stdin, then end your turn:

   ```sh
   crewboard validate fail --stdin <<'REPORT'
   src/cart.ts:42: the total ignores the discount the description asks for.
   src/cart.test.ts:10: `npm test` fails: expected 90, got 100.
   Note: Not covered by any ticket: removing items
   REPORT
   ```

   or, when nothing fails (notes are fine):

   ```sh
   crewboard validate pass --stdin <<'REPORT'
   Checked: the discount applies to the total; `npm test` passes; no leftovers.
   Note: UI for adding items: covered by SHOP-40.2
   REPORT
   ```

## Rules

- **Change nothing:** don't edit, create or delete files, don't commit, don't push, and don't run commands that change
  tracked files or anything outside the worktree (no installs, no migrations, no formatting fixes).
- Don't file tickets and don't run `crewboard review submit`: the user creates tickets from your notes.
- **Long text always goes through stdin** (`--stdin`), never quoted arguments.
- Always finish with `crewboard validate pass --stdin` or `crewboard validate fail --stdin`. Ending your turn without
  a verdict leaves the ticket waiting for the user.
