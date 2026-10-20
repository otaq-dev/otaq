---
name: grill
description: Settle the Otaq ticket named by $OTAQ_TICKET with the user before any code is written - get the context with `otaq prime`, ask one question at a time with `otaq ask human`, write what was agreed into the ticket with `otaq update`, then end your stage with `otaq stage next`. Never edits files.
disable-model-invocation: true
---

# Grill your Otaq ticket with the user

You run the **Grill** stage of an Otaq ticket (its key is in `$OTAQ_TICKET`, for example `SHOP-18`). No code has been
written yet. Your job is to settle the ticket with the user: find what is unclear or undecided, ask about it **one
question at a time**, and write what you agreed into the ticket, so the next stage's agent builds the right thing
without asking again.

**You never edit files and never commit.** You read the code to ask good questions; the next stage writes the code.

## Steps

1. **Get the context first.** Run `otaq prime` and read all of it: the ticket and its description, the user's
   **Agent instructions** and the project's notes, the tickets it depends on, and the stages after yours.
2. **Follow the Agent instructions** that `prime` prints. They are the user's own rules and they apply to every step
   below. They add to these steps; they never remove getting the context (step 1), asking one question at a time
   (step 4) or ending your stage (step 6).
3. **Find the open points.** Read the description and the code it touches, then list for yourself what is unclear
   or undecided: what is asked, for whom, which screens and data, what happens at the edges (empty, too many, errors,
   signed out), what is out of scope, and how the user will know it works. Put the ones that change what gets built
   first. Drop anything the description, the code or the Agent instructions already answer.
4. **Ask one question at a time** with `otaq ask human` (see below), then **end your turn**. The ticket shows "Asks
   you" on the board; the answer arrives in this session as the user's next message. Read it, check your list again
   (an answer can settle or drop the next point), and ask the next question. Say where you are: "Question 2 of about
   4". Stop when nothing that changes what gets built is open, or when the user says "enough" or "go".
5. **Write what was agreed into the ticket** with `otaq update` (below): the description as the user wrote it, then
   an **Agreed** section with one line per decision.
6. **End your stage** with a short summary on stdin, then end your turn. The ticket moves to the next stage, whose
   agent reads your summary in `prime`:

   ```sh
   otaq stage next --stdin <<'SUMMARY'
   Settled with the user in 4 questions: CSV only, current filters applied, at most 10,000 rows, admins only.
   The ticket's description now has an Agreed section.
   SUMMARY
   ```

   **Nothing to build?** Only when the user agrees the ticket isn't needed (for example "already fixed in SHOP-31"),
   end the ticket instead, with the reason on stdin. It goes to Done as "Closed · nothing to merge":

   ```sh
   otaq stage done --stdin <<'REASON'
   Already fixed in SHOP-31: the user confirmed the export there covers this.
   REASON
   ```

## Asking

The question text always goes through stdin. When the question has a few clear alternatives, offer **2–4 options**
(short labels, at most 40 characters), each with a `--note` of its pros and cons right after its `--option`, and
`--recommended N` when one is the sensible default:

```sh
otaq ask human \
  --option "CSV only" --note "Opens in any spreadsheet; no formatting." \
  --option "CSV and Excel" --note "Keeps column widths and dates; one more library to add." \
  --recommended 1 --stdin <<'QUESTION'
Question 1 of about 4. Which formats should the order export offer?
QUESTION
```

An **open question** stays free text, with no options. Never invent options for it:

```sh
otaq ask human --stdin <<'QUESTION'
Question 2 of about 4. Who needs this export: shop admins only, or every staff account?
QUESTION
```

- **One question per turn.** A ticket holds one open question; a second one would replace the first.
- The user may answer in the agent's terminal instead of the inbox; treat it the same.
- The user's Agent instructions take precedence over this guidance.

## Writing the agreement

`otaq update` replaces the whole description, so send the old text with the agreement added. Keep what the user
wrote; add the **Agreed** section after it:

```sh
otaq update SHOP-18 --stdin <<'BODY'
Let shop admins export the order list.

## Agreed
- Format: CSV only (Excel later, if asked).
- The export uses the filters shown on the order list.
- At most 10,000 rows; above that the button says to narrow the filters.
- Admins only; other staff don't see the button.
- Out of scope: scheduled exports.
BODY
```

Write each line as a decision, not as the question. If the user changed the title's meaning, update the title too
(`otaq update SHOP-18 --title "Export orders as CSV"`).

## Rules

- **Never edit, create or delete files, never commit, never push.** Read and run code only to understand it.
- Don't plan the code or start building: the next stage does that.
- Ask before you decide anything that changes what gets built; don't guess an answer the user hasn't given.
- **Long text always goes through stdin** (`--stdin`), never quoted arguments.
- Always finish with `otaq stage next --stdin` (or `otaq stage done --stdin` when there is nothing to build).
