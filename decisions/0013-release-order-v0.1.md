# 0013. Release order: v0.1 is the usable core loop, including dependencies

- **Status:** Accepted
- **Note (0053):** the milestone list below is superseded by 0053 (M0–M11).
- **Note (0049):** Step 6 (full stacking) is deferred: v0.1 dependencies wait until the parent is merged (0049).
- **Date:** 2026-10-09
- **Related:** 0004, 0005, 0007, 0010, 0006

## Context
The original milestones put Review (old M7) after stacking (M5) and previews (M6), so the basic
"ticket in, approved change out" loop could not be used until most of the plan was built. The product
owner suggested reordering so the user can try it on real work early. The user accepted, on the
condition that **dependencies are part of v0.1**.

## Decision
**v0.1, the version to use on real work:**
1. Skeleton: the workspace, a GPUI window, SQLite, the board.
2. Tickets: create and edit, drag and drop, **dependencies (DAG, cycle check, badges, Stack list in the side panel; 0006)**.
3. Sessions: worktree, background service + PTY + `claude`, embedded terminal, `factory attach` (0022),
   a unique `$PORT` per ticket (0026).
4. Agent signals: the `factory` plugin, `otaq` agent commands (0031), hooks, auto-moving cards.
5. **Scheduler: Queued state, dependency-aware start, concurrency limit.**
6. **Full stacking** (0005): multiple parents, `base_sha` rebases, Paused children, automatic restack.
7. Review: Diff tab, **two approve actions: Local merge and PR (GitHub / Azure DevOps)** (0015), Send back.
8. **"Needs you" inbox** with keyboard triage (0016).

**Build order within v0.1 (0027):** macOS first and Local merge first. Windows, Linux and PR providers follow.

**Also in v0.1:** Run and Shell (0035, 0046), after Local merge on macOS.

**Later releases:** the graph view
(maybe later, 0006), importers and the planner agent.

## Open
- None. Topic 1 was decided in 0014 and Topic 2 in 0005 and 0015.

## Changelog
- 2026-10-09: created
- 2026-10-09: tmux replaced by the background service (0022).
- 2026-10-09: removed the factory test gate, retry limit and Failed column. Checks and retries belong in the user's own skills (0018).
- 2026-10-09: added the Needs-you inbox to v0.1 (Topic 3).
- 2026-10-09: full stacking and both approve actions moved into v0.1 (Topic 2). v0.1 is now larger.
- 2026-10-09: step 6 (full stacking) deferred (0049).
- 2026-10-09: the milestone list is superseded by 0053 (M0–M11).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
