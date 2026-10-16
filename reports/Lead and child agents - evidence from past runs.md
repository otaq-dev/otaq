# Lead and child agents: evidence from past orchestrator runs (2026-10-16)

Input for designing the epic lead ↔ child relationship (decision 0083). Otaq itself has no usage data yet (`~/.otaq`
doesn't exist), so this uses the runs where the user drove an orchestrator ("lead") and teammates ("children") by hand
in Claude Code.

## Sources
| Run | Session | Children | Shape |
|---|---|---|---|
| M3–M11 | e50d1263 | 19 | 9 milestone teammates one after another, then 8 parallel fix teammates, 1 integrator, 2 VM testers |
| M14 (Tauri UI) | 5fd4893e | 35 | parallel lanes, 41 tickets |
| M15 (Markdown editor) | aaa00027 | 17 | up to 3 in parallel, own worktrees, lead runs checks and fast-forwards |
| M16 (Epics on the board) | 9657d25b | 14 | same as M15; transcript ends before the last ticket |
| Questions record | `tickets/questions.md`, `review-M*.md` | | Q16–Q60 from implementation and planning |

Caveat: the children were Claude subagents told to "build an option, log it, carry on; stop only for a blocker", and the
lead could read their full reports. Otaq children are live sessions; the numbers guide, they don't transfer one to one.

## What the numbers say
**1. Children almost never ask the lead anything mid-work.**
- M15 and M16: 0 questions in 31 children. M14: 10 in 35 (4 about a sibling's work, 2 plan, 5 environment, 0 about
  code, 0 product). M3–M11: 5 (mostly environment, 1 sibling).
- Product questions were **deferred instead**: written down as "built as option N" and carried on. About 40 across the
  runs (Q17–Q34, Q37–Q47, Q49–Q54, Q56–Q60).

**2. Real blockers were the environment, which only the user can fix.** VM sign-in, a locked screen, refused
permissions, a full disk. Only 2 product blockers in all runs (Q48, a planned gate, settled in 13 minutes; Q36, a
decision change). One environment blocker was made worse by the lead's own rule ("snapshot first").

**3. The user's view on deferred product questions.**
- Of the 10 resolved with a built default, the user kept 7 and overrode 3 (Q49, Q51, Q52, all in M15: design fidelity
  and strictness). In Q52 the user picked an option nobody had offered.
- 31 deferred questions were never reviewed.
- About half of the deferred questions were **documents disagreeing** (decision vs ticket vs design frame), not real
  gaps. A lead holding a "which document wins" rule and the siblings' work could settle about 22 of 45 alone.
- After one per-question escalation the user said: "stop only if it's a blocker. Otherwise, put it in a file and
  proceed" (M3–M11, 10-09 22:37).

**4. Where the lead changed outcomes.**
- **Before work:** briefs with notes from earlier children ("carry-over"); M16's last brief carried notes from 9 tickets.
- **Routing between siblings:** passing one child's finding to another fixed real problems (M14 T-0236 → T-0237; lane C's
  banner doubt → lane B fixed the frame; an ownership call on `root.rs` in 1 minute).
- **Its own checks:** sending work back after reading the child's notes against the decisions fixed 2 shipped defects
  (M7, M9); suspecting a "flaky" test found a real bug (M14).
- **Order:** landing order and parallel waves changed while running (M15, M16), based on which files overlap.
- **Checking the user's answers against decisions:** Q50, where a quick "fix it" would have changed the SDD.

**5. Most messages were bookkeeping a program could do.**
- "Main moved, rebase now": 17 (M15/M16) + 20 (M14).
- "Your report was cut off, resend": 10 + 3 + 2.
- Question-number collisions: 4. Duplicate acknowledgements: 21.

**6. Timing hurt.** In M14, 15 of 49 lead notes reached the child more than 30 minutes late, because a message waits for
the child's turn to end. A "change of plan" arrived after the child had built a duplicate ticket. Otaq's 0048 rule
(deliver when idle) would repeat this.

**7. Stalls were caught by the user, not the lead.** A child waited for a background check that had already finished
(M14, and M15's T-0248); the user noticed. The lead also trusted words over state twice ("check is running"; a
fast-forward without checking the exit code).

**8. Siblings clashed outside their ticket's file list.** Shared registration files (icons, `progress.md`, question
numbers), a file two tickets edited (`editor.rs`), and one runtime effect git couldn't see (a stack overflow across two
tickets, M16).

**9. No data on nudging stuck children.** No child ran out of context, failed for good or was replaced.

## What this means for 0083
- Child questions to the lead will be rare. The main channel is **deferred decisions** ("built as option N"), which
  needs to be first-class: logged, visible, reviewed in a batch.
- Passing product questions on one by one goes against how the user worked; a batch with a recommendation and an
  example fits better, except for true blockers.
- The wake-on-merge turn is where the value is; it should read the child's notes and deferred decisions, route findings
  to siblings and adjust order.
- Notes to running children need a path that doesn't wait for the end of a long turn, or the lead should prefer
  editing children that haven't started.
- Otaq, not the lead, should do the bookkeeping (rebase on merge, numbering, whole reports) and give the lead facts
  (validator results, whether a process runs, which files each child touched).
- Environment and permission blockers go straight to the user.

Analysis scripts: the session scratchpad (`analysis-m12/`, `analysis-m14/`, `analysis-m15/`).
