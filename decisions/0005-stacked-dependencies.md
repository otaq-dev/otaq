# 0005. Dependency DAG with stacked branches

- **Status:** Accepted. **Stacked mode deferred for v0.1 by 0049** (a dependency is ready only when the parent is merged).
  The DAG, the cycle check and the checkpoint-commit rule still apply.
- **Date:** 2026-10-08
- **Related:** 0004, 0006, 0007

## Decision
- Tickets form a DAG (`blocked_by`). Any dependency that would create a cycle is rejected. Dependencies stay
  within one project (0024).
- **Stacked mode:** a dependency counts as ready once the parent is in *Ready for Review*, *Approved*
  or *Done*. The child's branch is created from the parent's branch head.
- **Multiple parents:** branch from the first parent and merge in the others. If that merge conflicts,
  the agent resolves it as its first task.
- **Base commit recorded:** when a child branch is created, store the parent commit it was branched from
  (`base_sha`). Every rebase uses `--onto`, so squash merges don't show the parent's commits again in the child.
- **Parent changed after review feedback:** its children are flagged `needs_restack` and **paused** (see
  0004): any running agent turn is interrupted, the session is kept alive, and the card shows Paused.
  Once the parent is back in Review, the factory tries a mechanical `git rebase --onto <parent-new> <parent-old> <child>`.
  If that conflicts, the child's agent **resolves it automatically** in its live session with the `restack` skill (0009).
  The user's Send back click on the parent already covers this, so there's no inbox item. The card shows
  **"Resolving restack conflict"** as a normal working status; the agent escalates with `otaq ask human` if stuck.
  If the child's session is gone (Interrupted), it waits for the user's Resume (0045). A child sitting in **Ready for
  Review or Approved** whose rebase conflicts stays where it is with a conflict flag and **"Send to agent to fix"**, like 0038.
  After a successful restack, the child resumes. Its agent is told that the parent changed and the
  branch was restacked.
- **Unsaved work in the child's worktree:** before any rebase, Otaq makes a checkpoint commit of the
  agent's uncommitted changes (`wip: paused by Otaq`), rebases, then undoes it with `git reset --soft HEAD~1`, so
  the agent's files come back exactly as they were, on top of the new parent. If the rebase or the undo conflicts, the
  ticket shows **Resolving restack conflict** and the agent resolves it with the `restack` skill. The user's git hooks run on the checkpoint commit too (0055); if a hook rejects it, the operation
  stops with a clear error ("Your pre-commit hook rejected Otaq's temporary checkpoint. Commit or discard the files in
  SHOP-26, then retry."). Never stashes. The same rule
  applies to every Otaq rebase in an agent's worktree (merge onto main, reopen, 0038, 0044).
- **Parent closed (0044):** Closed never counts as ready. Working children pause with "parent closed".
- **Parent merged into main:** its children are rebased onto main
  (`git rebase --onto main <base_sha> <child>`) and their base is updated.
- **What triggers the restack on merge:** for Local merge, the user's Approve click. For PRs, the user's
  **Mark merged** click (0017). The factory never detects remote merges by itself.
- **Scope:** the full model (multiple parents, automatic restack cascade, re-queueing on conflict) ships in
  **v0.1**. The user chose this over a minimal single-parent version (Topic 2).

## Alternatives considered
- **Conservative mode** (a child waits until the parent is merged): simpler and always correct, but
  slower. It could later be added as a per-repo setting.

## Consequences
- Throughput is faster, but review changes cascade down the stack, so restack handling is a core
  feature rather than an edge case.

## Changelog
- 2026-10-08: created
- 2026-10-09: confirmed full stacking in v0.1. Added `base_sha` and `--onto` rebases. Children **pause**
  while their parent is being reworked, instead of continuing (user decision, Topic 2).
- 2026-10-09: the merge-triggered restack for PRs now waits for the user's Mark merged (0017, 4d).
- 2026-10-09: unsaved work in the child's worktree is kept through a checkpoint commit around the rebase (grilling session, user decision).
- 2026-10-09: restack conflicts are resolved by the agent automatically with no inbox item; children in Review or Approved get "Send to agent to fix" (grilling session, user decision).
- 2026-10-09: stacked mode deferred for v0.1 by 0049; dependencies wait until the parent is merged. The DAG, cycle check and checkpoint-commit rule still apply.
- 2026-10-09: the checkpoint commit respects the user's git hooks; a rejection stops the operation with a clear error (ticket-writing review Q12, user decision).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
