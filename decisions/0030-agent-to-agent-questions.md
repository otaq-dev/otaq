# 0030. Agents ask each other through Otaq, and every message is logged

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0005, 0009, 0016, 0018, 0021, 0029

## Context
Agents on linked tickets sometimes need each other's knowledge. For example, SHOP-20's agent needs the
response format of SHOP-18's checkout API. Direct Claude-to-Claude messaging would be invisible to the user,
could wake agents without a click, and would depend on a Claude Code feature that may change (0021).

## Decision
- A new tool, **`ask_ticket(key, question)`**, routed through Otaq. It is now `otaq ask <KEY>` (0031).
- **Only between tickets linked by a direct dependency** (parent ↔ child).
- **Every question and answer is logged on both tickets** and visible to the user (a Messages section in the
  side panel).
- Delivery rules (refined in the grilling session; an explicit exception to 0018):
  - If the target ticket has a **live session**, the question is delivered **when its agent is idle between
    turns** (and the input line is empty, 0048). It never interrupts a running turn. The target answers with
    `otaq answer`.
  - If the target has **no live session but a stored Claude session** (for example a Done parent, the normal case
    under 0049), Otaq **resumes it automatically to answer**:
    1. It recreates the worktree **at the same path** on the target's kept branch, so `claude --resume <session-id>` finds
       the conversation.
    2. It types the question with the **`answer-question`** skill: answer only, don't change code, reply with `otaq answer`.
    3. After the answer, it ends that session and removes the temporary worktree. If files were changed, the worktree is
       kept and listed in the Cleanup view (0018); nothing is deleted silently.
    4. The target's card shows **"answering SHOP-20"** meanwhile. The plan-limit hold applies (0020).
  - If the target **has never had a session** (it never started, so it made no progress), `otaq ask` is
    **rejected** with an error: "SHOP-18 hasn't started yet, so there is no progress to ask about. Ask the human
    instead (`otaq ask human`)."
  - If the stored conversation can't be resumed (Claude deleted it), the ask is rejected the same way, with "SHOP-18's
    conversation is no longer available".
- **Per-project setting "Let finished tickets answer questions"**, on by default. When it's off, questions to a ticket
  without a live session go to the user's Needs-you inbox ("SHOP-20 asks SHOP-18: …") instead.
- The **asker's card** shows **"asked SHOP-40.1 · waiting"** until the answer is delivered (display only, low in the flag
  order; it never outranks a needs-you status). The target shows "answering SHOP-40.2" while it is resumed to answer.
- Asking is **non-blocking**. The asking agent keeps working. The answer is typed into its session when it is
  idle.

## Alternatives considered
- Direct Claude-to-Claude session messaging: rejected (invisible, can spend tokens unprompted, depends on CLI internals).
- Allowing any ticket in the project to ask any other: rejected in favor of dependency links only.

## Related safeguard
- 0043: if an answer (or anything else) adds commits after a ticket entered Review, Approve warns and shows only the new commits.

## Changelog
- 2026-10-09: created
- 2026-10-09: delivery also waits until the target's input line has no unsent text from the user (0048).
- 2026-10-09: finished tickets are resumed automatically to answer (answer-only, exception to 0018); asking a never-started ticket is rejected; inbox only when the setting is off (grilling session, user decision).
- 2026-10-09: the asking ticket's card shows "asked <KEY> · waiting" (ticket-writing review Q9, user decision).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
