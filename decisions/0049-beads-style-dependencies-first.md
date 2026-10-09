# 0049. v0.1 dependencies work like beads: a ticket waits until every dependency is merged (stacking later)

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0005 (stacking part deferred), 0006, 0013, 0014, 0015, 0030, 0038, 0039, 0041, 0044, 0046, 0048
- **Supersedes (for v0.1):** the stacked mode of 0005 and the v0.1 scope line that included it

## Context
Grilling session, question 6. Tickets with several parents raised hard problems under stacking: the Diff against a single
`base_sha` showed the other parent's code, restacks couldn't express "swap the old parent for the new one", and a pull
request can only target one branch. Looking at how beads handles this: an issue can have any number of blockers, and it
is ready only when **no blocker is still open** (`bd ready`: "open issues with no active blockers"). Beads never builds on
unfinished work. 0005 had kept this "conservative mode" as the alternative. v0.1 is a personal build with no launch date
(0025), so the stacking headline from 0014 isn't needed yet.

## Decision
- **A dependency is ready only when the parent is Done · Merged.** Closed never counts (0044). Epic dependencies already
  work this way (0046).
- **Any number of dependencies** per ticket (like beads). The DAG and cycle check from 0005 stay.
- **Every ticket branches from the base branch** (main) when it starts, which already contains all of its parents' work.
  `base_sha` is simply where the ticket left the base branch. The Diff is against it (0017).
- **After Mark merged** on a PR ticket, Crewboard fetches the base branch before starting the children that just became
  ready, so they start from the merged code. This is triggered by the user's click; there's still no polling (0017).
- Queued cards say what they wait for: **"waits for SHOP-18 to merge (in Review)"**.
- `crewboard prime` still gives the child its parents' summaries (0032). A merged parent has no live session, so a
  question to it is answered by resuming its conversation answer-only, or goes to the inbox when the project setting is off (0030).

**Not in v0.1 (deferred, to be revisited after daily use):** stacked branches, Paused children, `needs_restack`, the
stacking uses of the `restack` skill and "Resolving restack conflict", combined starting points for several parents, stacked PRs and PR
retargeting, "PR out of date · Push" (0041), the restack note (0048), and the bottom-up approve rule (0015), which is
unnecessary because a child can't reach Review before its parent is merged. If stacking returns, it comes back as a
**per-project setting**, and the refinements decided for it (checkpoint commit, automatic restack-conflict handling)
still apply.

**Still in v0.1:** rebasing onto the latest base branch at Local merge (0038) and on reopen (0044), including the
checkpoint commit for an agent's unsaved work (0005), and the **`restack` skill** for those base conflicts ("Send to agent to
fix", 0038).

## Alternatives considered
- **Full stacking with multiple parents (0005 as written):** rejected for v0.1 because of the Diff, restack and PR problems above.
- **One parent per ticket, stacked:** simpler, but it still keeps most of the restack machinery.
- **Hybrid (one dependency stacks, several wait until merged):** the best stacked option, kept as the starting point if
  stacking returns.

## Consequences
- Chains are slower: SHOP-20 starts only after the user has reviewed **and merged** SHOP-18. With Local merge, the wait is
  only the user's review time. With PRs, it lasts until Mark merged. Independent tickets still run in parallel.
- Roughly a fifth of v0.1, and most of its hardest git work, goes away.
- **0014's pitch** ("1. Dependencies and stacking") assumes stacking. Revisit it before any public launch.

## Changelog
- 2026-10-09: created (grilling session, user decision)
- 2026-10-09: "main" means the ticket's base branch; linked tickets must share a base (0051).
- 2026-10-09: for epic children, "merged" means merged into the epic's branch (0059).
- 2026-10-09: wording fixed: the `restack` skill stays for base conflicts (0038, 0044); questions to a merged parent follow 0030 (ticket-writing review).
