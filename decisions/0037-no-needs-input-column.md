# 0037. No Needs Input column: tickets that need the user stay in Dev, highlighted at the top

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0004, 0016, 0032, 0036

## Context
0004 had a separate **Needs Input** column for tickets whose agent asked the user a question. The cards already carry an
orange "Asks you" / "Needs you" highlight, and the inbox lists them across projects, so a separate column is redundant.

## Decision
- **The Needs Input column is removed.** When an agent asks a question (`crewboard ask human`) or waits on a
  permission prompt, the ticket **stays in Dev** with its orange **"Asks you" / "Needs you"** highlight.
- **Sorting:** inside Dev's Agent Working section, tickets that need the user float **to the top**, in the same order as
  the inbox (0016).
- **Board columns:** Backlog → Dev (Queued and Agent Working) → Ready for Review → Approved → Done. That's **5 columns**.
- The **Needs-you inbox is unchanged**. It still lists everything that needs the user across all projects.
- "The ticket goes to Needs Input" in older decisions now means "the ticket shows *Asks you* at the top of Dev".

## Refined by
- 0047: pinned tickets sort above orange ones.
- 0042: Idle joins the orange family. The highlight clears on the user's input.

## Changelog
- 2026-10-09: created (user decision)
