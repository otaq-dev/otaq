# 0045. What the reboot check covers, and "Before the restart" from the stored session ID

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0005, 0018 (refined), 0020, 0030, 0034, 0035, 0042

## Context
0018 turns every "live ticket" whose session died into Interrupted, offering Resume and Resume all, but it didn't define
which tickets count. If every ticket counted, Resume all would wake agents with nothing to do. If too few counted, questions
the user hadn't answered yet would be lost. (Raised by the product owner, review 2.)

## Decision
After a reboot or service crash, by ticket state:
- **Dev · working:** **Interrupted** (0018).
- **Dev · turquoise (Asks you, Needs you, Idle; 0042):** **Interrupted, but the label and question text are kept.** Answering
  from the inbox **resumes the session and types the answer in**, as one click (the user's go-ahead). Permission prompts
  simply ask again after Resume.
- **Ready for Review and Approved:** **not** Interrupted, no inbox item, and **not part of Resume all**. Their session resumes only
  when the user Sends back (0034). Questions from other agents follow 0030 (resumed answer-only, or the inbox when the project setting is off). History notes "session ended (restart)".
- **Paused:** stays Paused. After its restack it becomes **Interrupted** and waits for the user's Resume. It doesn't restart itself.
- **Run and Shell processes:** show **"stopped by restart"**. They are never restarted automatically (0035).
- **Resume all** covers only the Dev tickets above.

**Seeing what an agent was doing (the user's alternative to saving terminal output):**
- Otaq stores **every Claude session ID** per ticket (already needed for `claude --resume`). History lists them, so a
  ticket's sessions can be traced later.
- For an Interrupted ticket, the Terminal tab shows a **"Before the restart"** view built by **reading Claude Code's own
  saved conversation** for that session ID (read-only, like usage in 0020). It shows the last messages and actions.
- Otaq does **not** copy terminal output to disk. If Claude has already deleted the conversation (its own cleanup
  setting), the view says so.

## Changelog
- 2026-10-09: created (product owner review 2, user decision)
- 2026-10-09: wording fixed: questions to Review/Approved tickets follow 0030 (ticket-writing review).
- 2026-10-11: validator sessions: a crash or restart during validation makes the ticket Interrupted, and Resume starts a fresh validation (0063). Project agents show "Ended by restart · Resume" (0068).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
