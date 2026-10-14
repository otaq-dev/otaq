---
name: answer-question
description: Answer a linked Otaq ticket's question from this finished ticket's conversation - get the context with `crewboard prime`, answer only (change nothing), and reply with `crewboard answer <ID> --stdin`.
disable-model-invocation: true
---

# Answer a linked ticket's question

This ticket (its key is in `$CREWBOARD_TICKET`, for example `SHOP-17`) is finished. Otaq reopened its conversation
for one reason: **the agent of a linked ticket asked it a question** (for example SHOP-21, which depends on this
ticket's work). You answer from what you did here. **You change nothing.**

## Steps

1. **Get the context first.** Run `crewboard prime` and read all of it. Its "Pending questions" section lists the
   question, who asked it and its ID.
2. **Follow the Agent instructions** that `prime` prints, as far as they apply to answering. They never allow changing
   anything in this session.
3. **Find the answer** in this conversation and in the code of this worktree. You may read files, search, and read
   `git log` / `git show`. Keep it short and concrete: the asking agent is in the middle of its own work.
4. **Reply** with the question's ID, the answer on stdin:

   ```sh
   crewboard answer 0199d2a4-6f1e-7c3b-9a52-1f0e8d7c6b5a --stdin <<'ANSWER'
   POST /checkout returns 402 with {"error": "card_declined", "decline_code": "..."} for a declined card.
   See lib/payments/errors.ts.
   ANSWER
   ```

   If you don't know, say so in the answer and name who might (for example "ask the user"). Never guess.
5. **End your turn.** Otaq delivers the answer and closes this session.

## Rules

- **Change nothing:** don't edit, create or delete files, don't commit, don't push, don't run commands that change
  the repository, the database or anything else (no installs, no migrations, no builds that write files).
- Don't start new work, don't file tickets, don't run `crewboard review submit`.
- **Long text always goes through stdin** (`--stdin`), never quoted arguments.
