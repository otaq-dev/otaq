# Prompt: implement the Crewboard v0.1 tickets (runbook)

Start **one fresh Claude Code session per milestone** in this repository and paste everything below the line, followed by
the milestone to work on, for example `Milestone: M3`. The session needs the **pen.dev MCP connection** (tools
`mcp__pencil__*`) for UI tickets. If no milestone is given, the agent continues the first milestone in `tickets/progress.md`
that isn't finished.

---

You are the implementing agent for **Crewboard**, a native Rust (GPUI) kanban app that drives Claude Code agents. You build
the v0.1 tickets in `tickets/v0.1-tickets.md`, **one milestone per session, one ticket at a time, in file order**. There is no
review stop between milestones: the user reviews everything after the last milestone, M12 (decision 0062). **Only blockers stop the work**
(§7); every other question is deferred (§3.2).

## 1. Before you write any code (every session)
1. Read `CLAUDE.md`, then `decisions/README.md` (the decision index; decisions are the source of truth).
2. Read `SDD.md` §0 (how to use it), §2 (principles), §3.2 (out of scope), §23 (quality bar and definition of done) and §26
   (implementation choices).
3. Read the intro of `tickets/v0.1-tickets.md` and every ticket of your milestone. Read `tickets/progress.md` to see what is
   already done. If `tickets/progress.md` doesn't exist, create it with the header in §6.
4. Check the machine:
   - `cargo --version`, `cargo fmt --version` and `cargo clippy --version` work
   - `git --version` works
   - for any ticket that builds `crewboard-app` (GPUI): `xcrun -f metal` finds the Metal compiler. GPUI needs the full Xcode app
     (not only the command-line tools) and its Metal toolchain. If it's missing, **stop and tell the user**; don't work around it.
5. For UI tickets: call `mcp__pencil__get_app_state` and confirm `crewboard.pen` is the open document. If it isn't, ask the user to
   open it.

## 2. Pick the next ticket
- Take the **first ticket of your milestone, in file order, that is not marked done** in `tickets/progress.md`.
- Every ticket in its **Depends on** list must be done. If one isn't (for example it belongs to a later milestone or was skipped),
  **stop and ask the user**. Don't build the missing piece inside another ticket.
- Work on **one ticket at a time**. Finish it, check it and commit it before starting the next one.

## 3. Build one ticket
1. **Understand it.** Read the ticket, the SDD requirements it cites (search `SDD.md` for each ID, for example `GIT-03`) and
   **every decision it cites** (open the files in `decisions/`). For UI tickets, look at the named frame in `crewboard.pen` with
   read-only calls (`Get` with a visitor to read texts, `TakeScreenshot` on the named sub-frame). **Never modify the design.**
2. **Look for conflicts.** If the ticket, the SDD, the design and a decision disagree, the **decision is right**. If you find a
   real conflict or a gap that the documents don't answer, **defer it** (decision 0062):
   - add it to `tickets/questions.md` as the next question number (Q17, Q18…), in the format already used there: quote the
     decision number and its text, say what the ticket or design says instead, give the options with a concrete example and
     your recommendation
   - build the option that follows the decisions (where no decision answers it, your recommendation), in a way that is easy
     to change later, and end the question with the line `- **Deferred: built as option N.**`
   - name the question in the commit message and in the review file, and carry on
   - never resolve it silently: every deferred choice must be in `tickets/questions.md`
   - only if every option is risky or hard to undo later, it is a **blocker**: stop and ask (§7)
3. **Build it** in the crate the ticket names. Follow the ticket's "What and why" and "Done when" exactly; don't add features
   that no ticket or requirement asks for, and don't build later tickets early.
4. **Test it** the way the ticket says (SDD §23.1):
   - core logic in `crewboard-core` without any UI
   - git behaviour against **temporary repositories** (the harness from T-0020 once it exists)
   - **no fake `claude`**: hooks are tested by feeding `crewboard hook <event>` real-shaped JSON; PTY and session plumbing by
     running an ordinary command (`cat`, `sh -c …`) in place of `claude`
   - UI logic in view models with unit tests
   - every test that touches the data folder sets `CREWBOARD_HOME` to a temporary directory. **Tests must never read or write
     the real `~/.crewboard/`**, and must never start a real `claude`.
5. **Run the checks:** `scripts/check.sh` must be green (`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`). This is
   the definition of done (decision 0056). There are no GitHub Actions. For T-0001, which comes before the script exists, run the
   three commands by hand. Never commit with failing checks.
6. **Tick the boxes** in `tickets/v0.1-tickets.md`: change `- [ ]` to `- [x]` only for "Done when" items you actually verified.
   Leave the **hand checks** (see §4) unticked.
7. **Commit**, one commit per ticket, on `main`. There is no remote: **never push** and never open a PR. Never use
   `--no-verify`. The message:

   ```
   T-0046: Port pool: two ports per ticket

   Implements SES-01. Decisions: 0026.
   Implementation choices: port range 4000–4999, allocation skips ports in use (bind check).
   Hand checks pending: none.
   ```

   Wherever a ticket says "name it in the PR" or "note it in the PR", put it in the commit message body instead.
8. **Log it:** add one line to `tickets/progress.md` (format in §6).

Size L tickets may take several commits, but they stay one ticket: the last commit carries the full message above. If a
ticket is clearly too big for one session, don't split it into new tickets: build it across several commits, and if you
run low on context stop after a committed step with `tickets/progress.md` saying where the ticket stands, so the next agent
continues it.

## 4. Hand checks
Some "Done when" items need the user, for example:
- "checked by hand against the named frames" (comparing the running app with the `crewboard.pen` frame)
- "verified by hand once with the real `claude`" or "real `claude` behaviour checked by hand"
- "checked once, by hand", and anything to be checked on Windows or Linux

You **don't tick these**. You may prepare them: take a `TakeScreenshot` of the frame, describe how to reach the same state in the
app, and note any difference you can already see. Each hand check goes into the milestone review file (§5) with exact steps.

## 5. End of the milestone: stop for review
When every ticket of the milestone is committed, write `tickets/review-M<N>.md` (for example `tickets/review-M3.md`) with:
1. **What was built:** one line per ticket, with its commit hash.
2. **How to try it:** the exact commands to build and run (for example `cargo run -p crewboard-app`, `crewboard daemon start`),
   and the sample data or temporary repositories to use.
3. **Hand checks:** a checklist grouped by ticket. Each item names the frame (`crewboard.pen → 04 · Ticket card variants · Light →
   Variant grid`), the steps to reach the state, and what to compare.
4. **Implementation choices** made in this milestone (SDD §26), and why.
5. **Open questions** added to `tickets/questions.md`, if any.

Then **stop** and report that the milestone is done. Don't start the next milestone in the same session. There is no review
stop: the next milestone starts after the orchestrator's checks. The user does the hand checks after M12; anything that
fails is fixed then, in a new commit that names the ticket.

All of v0.1 is built with this runbook, outside Crewboard (decision 0062). Never switch to building with Crewboard part-way.

## 6. `tickets/progress.md`
Create it like this if it doesn't exist, and add one row per finished ticket:

```markdown
# v0.1 progress

| Ticket | Title | Milestone | Commit | Hand checks pending | Notes |
|---|---|---|---|---|---|
| T-0001 | Initialize the repository and Cargo workspace | M0 | a1b2c3d | none | gpui not added yet |
```

A milestone is finished when all its tickets have a row and its review file exists. If the session ends halfway, the next
session continues from this file.

## 7. Rules that always apply
- **Don't edit the design documents:** `decisions/`, `SDD.md`, `DESIGN.md`, `design-briefs/`, `crewboard.pen` and `CLAUDE.md` are
  changed only by the user (or when the user asks you to). In `tickets/` you only tick boxes, add questions, and write
  `progress.md` and the review files.
- **Respect the product principles in SDD §2** in the code you write, for example: nothing launches `claude` or deletes anything
  without a user click (except the listed exceptions); never write into the user's repo; never stash; never touch a dirty
  checkout; never type into a session over the user's unsent text.
- **Out of scope** (SDD §3.2): no stacking, graph view, importers, a planner agent (the M12 Project agent is in scope), teams, signing or packaging.
- **Dependencies:** use the crates of decision 0055 (`gpui` pinned to one Zed commit, `alacritty_terminal`, `portable-pty`, `sqlx`,
  `tokio`, `syntect`, `tracing`). Small, common helper crates are fine; name every new crate and why in the commit body.
- **Your own machine:** don't change the user's Claude Code settings, shell profile or global git config, and don't install
  anything. If a tool is missing, stop and ask.
- **Stop and ask only for blockers** (decision 0062): a dependency ticket isn't done; a tool is missing; the checks can't be
  made green without changing what the ticket asks for; a step would delete or overwrite something outside the repository;
  or a question whose every option is risky or hard to undo later. Everything else (decision conflicts, gaps, design
  disagreements) is deferred as in §3.2.
