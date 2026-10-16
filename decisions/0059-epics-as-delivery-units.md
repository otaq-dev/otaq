# 0059. Epics are delivery units: agent tickets stay local, one branch and one PR per epic

- **Status:** Accepted (step 5 partly superseded by 0080: while children work, the epic is a track in the "Epics in flight" strip, not a Backlog card; "the epic has no agent" partly superseded by 0083: it has a read-only lead, while Open PR / Push still run in a short-lived delivery session)
- **Date:** 2026-10-09
- **Related:** 0015, 0023, 0039 (partly superseded), 0044, 0046, 0049, 0050, 0051 (partly superseded)
- **Supersedes:** from 0039, "an epic lives only in Backlog… has no branch or worktree… moves to Done automatically";
  from 0051, "children always use the epic's base"

## Context
SDD preparation, question 7. The team's remote tracker and Otaq work at different sizes: one human ticket in the
remote (for example **PAY-123 "Checkout"** in Jira or Azure Boards) can become several agent tickets on the board (SHOP-18,
SHOP-19, SHOP-20). The team should see **one branch and one PR for PAY-123**. The agents' small steps should stay on the
user's machine and never be synchronized with the remote one by one.

## Decision
**Two levels: agent tickets (local) and delivery units (pushed).**
- A **delivery unit** is what reaches the remote: an **epic**, or a **standalone ticket** (one not in an epic).
- **External ID:** epics and standalone tickets get an optional **External ID** field (for example `PAY-123`). The workflow
  skills (0050) use it in branch names and PR titles (`feature/PAY-123-checkout`, "PAY-123 Checkout").

**Epics:**
1. An epic gets a **local integration branch** when its first child starts (temporary name `otaq/SHOP-40`, 0050). It is
   created from the project's base branch (see "Base branch" below). It has no agent. Merges into it happen in a
   Otaq-owned scratch worktree (0038).
2. **Children branch from the epic's branch, and Local merge squashes them back into it.** This is local and never pushed.
   Children don't offer Open PR. Dependencies between children follow 0049, measured against the epic's branch: SHOP-20
   starts once SHOP-18 and SHOP-19 are merged into the epic branch.
3. **When every child is Done** (merged into the epic branch, or closed, with at least one merged), the epic moves to **Ready
   for Review**. The user reviews the **whole feature**: the epic branch against its base.
4. **Approve on an epic:**
   - **Open PR:** the confirmation dialog (0050) proposes the delivery branch name, **one commit per merged child**, and the PR
     title and description. One push and one PR. Then Approved · PR open → Mark merged → Done.
   - **Local merge** into the base branch, for projects with no team or no remote.
   - Open PR and Push to PR on an epic run the skill in a **short-lived `claude` session in the epic's scratch worktree**,
     started by the user's click and ended after `otaq pr opened` (the epic has no agent session).
   - Send back on an epic isn't available (it has no agent). To fix something, the user reopens or adds a child ticket; the
     epic returns to Backlog until the new child is merged.
5. The epic's card moves through the columns like a ticket from step 3 on (Review → Approved → Done). While children work, it
   stays in Backlog with its progress ("2 / 3 merged").

**Standalone tickets** are their own delivery unit, as before: Open PR, or Local merge into the base branch.

**Base branch (refines 0051):**
- Project setting **"Base branch"**, **prefilled with the remote's default branch** (where `origin/HEAD` points, for example
  `develop`), editable. A project without a remote uses the current branch.
- Standalone tickets and epics use the project's base branch, or a per-ticket/per-epic override (0051). **Epic children always
  use their epic's branch.**
- **Keeping the base current:** before a delivery unit starts, and before a Local merge into the base branch, Otaq runs
  `git fetch`. If the local base branch is only behind its remote, Otaq fast-forwards it (in the user's checkout only if
  it's clean, as in 0038). If local and remote have **diverged**, Otaq doesn't merge or rebase. It shows "`develop` and
  `origin/develop` have diverged. Push or pull, then retry" and holds the ticket in Queued.
- **Local merge never pushes.** Pushing the base branch stays the user's call.

## Alternatives considered
- Pushing every agent ticket as its own branch and PR: rejected. It floods the team's remote with agent-sized work.
- Starting tickets from the remote copy and pushing at every Local merge: rejected for the same reason.

## Consequences
- An epic needs a branch, a scratch worktree for merges, an External ID and a review step. Epic children need no PR flow.
- Dependencies on an epic (0046) are ready when the epic is Done · Merged, as before.
- Move to epic (0044): a never-started ticket moved into an epic now uses the epic's branch.

## Changelog
- 2026-10-09: created (SDD preparation, user decision)
- 2026-10-09: Open PR / Push on an epic run in a short-lived delivery session started by the click (SDD review, user decision).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
- 2026-10-15: step 5 partly superseded: while children work, the epic is a track in the "Epics in flight" strip; it becomes a card again in Ready for Review (0080).
- 2026-10-16: "it has no agent" partly superseded: the epic has a read-only lead agent (0083); delivery (Open PR, Push) still runs in its own short-lived session, and Send back on an epic still isn't available.
