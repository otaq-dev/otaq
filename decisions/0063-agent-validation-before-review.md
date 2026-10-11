# 0063. Validation before Review: a separate read-only validator checks the agent's work

- **Status:** Accepted
- **Date:** 2026-10-11
- **Related:** 0004 (refined), 0016, 0018 (partly superseded), 0020, 0022, 0030, 0034, 0040, 0042, 0045, 0048, 0050 (refined),
  0052 (refined), 0064, 0067
- **Supersedes:** from 0018, "No factory test gate, no retry limit… When the agent calls `ready_for_review`, the ticket goes
  straight to Ready for Review"

## Context
Grilling session, 2026-10-11. The user doesn't want to be the first and only check on an agent's work. When the agent
says it is finished, its work should be checked by an independent judge. Failures should go straight back to the agent,
and the user should only see work that passed. 0018 had left verification to the user's own skills, which the agent runs
on itself.

## Decision
**Flow.**
1. The agent commits and runs `crewboard review submit --stdin` as before (the command name stays). It is still refused while
   the worktree has uncommitted files (0050). When validation is on, the reply tells the agent: "Submitted. Validation is
   running; the result will come back to you here. End your turn."
2. The ticket **stays in Dev** with a neutral **"Validating…"** status: not orange, no inbox item, not Idle. The Stop hook
   doesn't mark the agent Idle while its ticket validates. It takes **one slot** under the optional agent limit (0016): the limit
   counts tickets, not sessions.
3. Crewboard starts a **fresh, separate `claude` session** in the ticket's worktree: the **validator**. The agent's submit is
   the go-ahead for these tokens (a new exception to 0018's click rule, like 0030).
4. The validator runs the **Validate** workflow skill (default `/crewboard:validate`, replaceable globally or per project,
   0050). It ends with `crewboard validate pass --stdin` or `crewboard validate fail --stdin` and its report.
5. **Fail:** the findings are delivered to the agent's session as an automatic message (0048 rules), and the ticket goes back
   to Agent Working with **"Validation failed · round N of M"**. The agent fixes, commits and submits again.
6. **Pass:** the ticket goes to **Ready for Review**, or is merged automatically when Auto-merge allows it (0064).

**The validator is a read-only judge.** It reads the ticket and the diff and runs checks (tests, lint). It never edits files,
never commits and never fixes anything. If tracked files changed or a new commit appeared during its run, its result is
discarded and the user is told why. Files ignored by `.gitignore` (test caches, coverage output) don't count.

**What it works from:** `crewboard prime --validate` prints the ticket's title and description, the agent's submit summary,
the diff against `base_sha`, the user's review comments when the ticket was sent back, earlier rounds' findings, the
user's Agent instructions (0040), and the **linked tickets** (parents with their summaries, children, epic siblings and the
epic's description). There is no separate "validator instructions" field; validator rules go in Agent instructions, or the
user replaces the Validate skill.

**The built-in `/crewboard:validate`** checks that (1) the change does what the description asks, including any acceptance
criteria written there, inferring reasonable ones when none are written; (2) the checks the Agent instructions name pass
(otherwise the project's obvious checks, when it can find them); (3) nothing unrelated or left over (debug code, scratch
files, edits outside the ticket's scope). Findings are a short list, one line per problem with file and line.
- A missing piece that **another linked ticket covers** is a **note**, not a failure ("UI for adding items: covered by
  SHOP-40.2").
- A missing piece that **no ticket covers** and that this ticket's description doesn't ask for is also a **note** ("Not
  covered by any ticket: removing items").
- Only gaps against **this ticket's** description fail it.

**Rounds.** Setting **Validation rounds**: 1–5, default **3** (global default, per-project override). After the last failed
round the findings are **not** sent to the agent. The card turns orange, **"Validation failed 3 times"**, with an inbox item
and the last report, and three actions:
- **Move to Review anyway**: the report is shown in the review. Auto-merge never applies to such a ticket.
- **Give it another round**: the findings go to the agent and one extra round is allowed.
- **Answer the agent**: the user's note is delivered with the findings, and the round counter restarts at 0.

**When the validator ends without a verdict.**
- It ends its turn without `validate pass|fail`: orange **"Validation ended without a result"**, with **Retry validation**
  (a fresh validator) and **Move to Review anyway**. Not counted as a round. Crewboard doesn't nudge it (0018).
- Crash, reboot or service restart: the ticket becomes **Interrupted** (0018, 0045). **Resume** starts a fresh validation;
  the old validator conversation isn't resumed. Not counted as a round.
- Usage limit: **"Waiting for limit reset"** like agents (0020, 0046); Resume all restarts the validation.

**Changes during validation.** Typing to the agent is allowed. If **tracked files change or a new commit appears** in the
worktree, or the user **edits the ticket's description**, the validation is **cancelled**: the validator stops, the round
doesn't count, the ticket returns to Agent Working, and the agent is told "Validation was cancelled because the code
changed. Submit again when you're done." (a description edit also sends the usual change note, 0048).

**The user's actions.**
- **Submit for review** (0052) **skips validation**: straight to Ready for Review, "Submitted by you", never auto-merged.
- New panel action **Validate and submit**: the same flow as the agent's submit, rounds and Auto-merge included.
- While validating, the "…" menu offers **Stop validation** (validator stopped, round not counted, back to Agent Working)
  and **Skip to Review** ("Validation skipped by you", never auto-merged). Both submit items are hidden while validating.
- **Park** and **Close** stop the agent and the validator; the validation is discarded. Dragging a parked ticket back to
  Dev resumes only the agent (0052).
- **After Send back** (0034), the next `review submit` validates again: the round counter restarts at 0, the validator also
  checks that each review comment was addressed, and Auto-merge applies when it passes.

**Seeing the validator.**
- The ticket's **Terminal tab** has an **Agent / Validator** switch (Validator exists once a validation has run). The user
  can type into the validator; it is the real CLI (0022).
- A validator waiting on a **permission prompt** makes the card orange: **"Needs you · validator"**, inbox group 1, OS
  notification, and ⌘J opens the Terminal tab on the Validator side.
- The last validator terminal stays readable until the next round starts. **Every round's report is kept in History.**
- **In Review**, the card says how the ticket got there: "Validated · round N", "Validation skipped by you", "Submitted by
  you", or "Failed 3 times · moved by you". The final report (what was checked, the notes) is visible next to the diff.
  Each "Not covered by any ticket" note has a **Create ticket** action (prefilled Backlog ticket in the same epic; the
  user's click, so the validator stays read-only).

**Notifications and inbox.** Passed → inbox group 3 (Ready for Review) as before. Failed rounds that go back to the agent:
nothing. Validator permission prompt, "failed N times", "ended without a result": inbox group 1 with an OS notification.

**Tokens and limits.** The validator's tokens count on the ticket and in today's total, split in the panel ("agent 61k ·
validation 23k (2 rounds)"). The plan-limit hold applies to validators: no new validation starts while it's active.

**Setting Validation on/off:** on by default, can be turned off per project (and globally). Off → `review submit` moves the
ticket straight to Ready for Review, as before this decision, and Auto-merge does nothing (0064).

**New CLI:** `crewboard validate pass --stdin`, `crewboard validate fail --stdin` (validator sessions only), and
`crewboard prime --validate`.

## Alternatives considered
- **The agent validates itself** in its own session: rejected, it grades its own work.
- **The validator may fix small things:** rejected, a judge that fixes is no longer independent, and two sessions would edit
  one worktree.
- **A new "Validating" column**, or moving to Review with a badge: rejected, the ticket isn't ready for the user yet, and the
  board removed short-lived columns before (0036, 0037).
- **Unlimited rounds:** rejected, an agent and a strict validator can loop and burn tokens.

## Consequences
- Every submit costs a validator session's tokens unless validation is off for the project.
- New card states, a Terminal-tab switch, a validation report in Review and History, new panel actions, new settings and new
  inbox items need designs (design update K).
- The Stop-hook mapping, reconcile table and inbox grouping gain validator cases.

## Changelog
- 2026-10-11: created (grilling session, user decision)
