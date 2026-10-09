# 0038. Local merge when main has moved: free rebase on click, early conflict flag, never touch the user's edits

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0005, 0015, 0018, 0027, 0034

## Context
Raised by the product owner (second review). Two tickets branched from the same main can conflict once one of them
is merged, for example SHOP-18 and SHOP-19 both editing `lib/pricing.ts`. Local merge is built first (0027), so this
happens from day one. It was also undefined where the merge runs and what happens to the user's own checkout.

## Decision
- **On the Local merge click**, Crewboard first tries a **rebase onto the latest base branch** (the same rebase it uses for
  restacks, 0005). It costs no tokens. If the rebase is clean, it squash-merges.
- **If the rebase conflicts:** nothing is merged, and the ticket stays in **Ready for Review** with one button,
  **"Send to agent to fix"**. That button returns the ticket to Dev with the `restack` skill (0034). It is the click that
  spends tokens.
- **Early warning:** after every merge into a base branch, Crewboard runs a **read-only test merge** for each ticket in
  Ready for Review on that base (no files touched, no tokens). Tickets that would conflict get a **"conflicts with
  main"** flag, so the user sees it before clicking Approve.
- **Where the merge happens:** in a **scratch worktree owned by Crewboard**, never in the user's own folder.
  - If the base branch is checked out in the user's folder and that checkout is **clean**, it is fast-forwarded to the
    new commit.
  - If the user's checkout has **uncommitted changes**, Crewboard **refuses** with a message ("shop-web has uncommitted
    changes on main"). It **never stashes** or otherwise touches the user's edits. The user cleans up and retries.

## Changelog
- 2026-10-09: created (product owner review 2, user decision)
- 2026-10-09: "main" means the ticket's base branch (0051).
- 2026-10-09: before Local merge into the base branch, Crewboard fetches and fast-forwards the local base, refusing when it has diverged (0059).
