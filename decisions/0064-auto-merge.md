# 0064. Auto-merge: validated tickets can be Local-merged without a click

- **Status:** Accepted
- **Date:** 2026-10-11
- **Related:** 0015, 0018 (refined), 0038 (refined), 0040, 0049, 0050, 0059, 0063

## Context
Grilling session, 2026-10-11. With validation in place (0063), the user wants to be able to let validated work land
without reviewing every ticket, for example the small steps of an epic, while keeping their own review for what ships.

## Decision
**Setting "Auto-merge"**, a global default with a per-project override (the pattern of 0040 and 0050), with three values:
- **Off** (the default). A validated ticket waits in Ready for Review for the user, as before.
- **Epic children only.** A validated epic child is Local-merged into its epic branch (local, never pushed, 0059). The user
  still reviews the whole epic before it ships.
- **All tickets.** Standalone tickets are also Local-merged into their base branch, locally and never pushed. For solo
  projects without team PRs.

Rules:
- Auto-merge only ever does a **Local merge** (0038). Open PR always needs the user's confirmation dialog (0050).
- **Epics never auto-merge.** They have no agent and no validator, and their Review stays the user's (0059).
- It applies only to tickets that **passed validation** (0063). Never to "Submitted by you", "Validation skipped by you" or
  "Failed N times · moved by you". With validation off for the project, Auto-merge does nothing, and the settings say so.
- An auto-merged ticket lands in Done with **"Merged automatically"** and a History line. No inbox item, no notification.
  Waiting tickets that became ready start as after any merge.
- **Merge conflict (refines 0038):** the free rebase onto the latest target conflicts → nothing is merged, and the ticket
  goes back to its agent **automatically** with the `restack` skill (no "Send to agent to fix" click), card **"Merge
  conflict · fixing"**. The agent rebases onto the base, resolves the conflicts and calls `review submit`; validation runs
  again with the round counter at 0. After **3 conflicts in a row** the ticket stops and turns turquoise: **"Keeps conflicting
  with `<target>`"** (inbox group 1). Manual Local merge keeps the click.
- **Base diverged** from its remote (0059): Auto-merge doesn't try to fix it. The card turns turquoise: "`develop` and
  `origin/develop` have diverged. Push or pull, then retry" (inbox group 1).
- **The user's checkout is dirty** on the target branch (0038): the merge is refused as for a manual merge, the card turns
  turquoise with the same message and a Retry.
- **Cleanup (refines 0018):** an auto-merged ticket's worktree is removed as for any Done ticket (branch kept, never with
  uncommitted changes, Run and Shell stop, the agent's session ends). The project's Auto-merge setting is the user's standing
  go-ahead.
- First-run onboarding mentions Auto-merge in one line. No per-ticket toggle for now.

## Alternatives considered
- **On/off only:** rejected, it can't keep the user's review on what reaches the team's remote.
- **Auto-merge opens a PR:** impossible without the confirmation dialog (0050).
- **Default on:** rejected, nothing should land in a base branch without the user choosing that.

## Changelog
- 2026-10-11: created (grilling session, user decision)
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
